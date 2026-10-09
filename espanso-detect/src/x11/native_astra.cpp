/*
 * Astra Linux / X11 detector for rEspanso pol_run.
 *
 * Hardened Astra X servers may disable X11 RECORD and, on some KDE/Astra
 * sessions, XInput2 RawKey events are advertised but never delivered to the
 * client.  The proven portable path therefore polls the core keyboard bitmap
 * with XQueryKeymap, while keeping XInput2 only for raw mouse buttons.
 *
 * Hotkeys still try the normal XGrabKey path.  If KDE (or another client)
 * already owns a shortcut and XGrabKey returns BadAccess asynchronously, the
 * shortcut is transparently recognized from the same XQueryKeymap polling
 * stream instead of being disabled.
 */

#include "native.h"

#include <errno.h>
#include <locale.h>
#include <memory>
#include <mutex>
#include <poll.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <utility>
#include <vector>

#include <X11/XKBlib.h>
#include <X11/Xlib.h>
#include <X11/Xproto.h>
#include <X11/Xutil.h>
#include <X11/extensions/XInput2.h>
#include <X11/keysym.h>
#include <X11/keysymdef.h>

extern "C" void detect_write_x11_log(const char *line);

struct RegisteredHotKey {
    int key_code;
    unsigned int state;
    unsigned int valid_modifiers;
    bool poll_fallback;
};

typedef struct {
    Display *display;
    int xi_opcode;
    bool xi2_mouse;
    void *rust_instance;
    EventCallback event_callback;

    char previous_keymap[32];
    bool keymap_initialized;

    std::vector<RegisteredHotKey> hotkeys;
    std::vector<int> active_hotkey_keys;

    unsigned long long polls;
    unsigned long long presses;
    unsigned long long releases;
    unsigned long long translated;
    unsigned long long empty;
    unsigned long long shift_recovered;
    unsigned long long hotkeys_polled;
} DetectContext;

static void write_diag(const char *line) {
    if (!line) {
        return;
    }
    detect_write_x11_log(line);
}

static unsigned int current_xkb_state(Display *display) {
    XkbStateRec xkb_state = {};
    if (XkbGetState(display, XkbUseCoreKbd, &xkb_state) != Success) {
        return 0;
    }

    // XKeyEvent.state keeps modifier masks in the low bits and the active
    // keyboard group in bits 13-14.  Preserving the group is required for
    // Russian/English layout switching when XLookupString translates text.
    return static_cast<unsigned int>(xkb_state.mods) |
           ((static_cast<unsigned int>(xkb_state.group) & 0x3U) << 13);
}

static int emit_input_event(DetectContext *context, int event_type,
                            int key_code, unsigned int state) {
    if (!context || !context->event_callback) {
        return 0;
    }

    InputEvent event = {};

    if (event_type == KeyPress || event_type == KeyRelease) {
        XKeyEvent raw_event = {};
        raw_event.display = context->display;
        raw_event.window = DefaultRootWindow(context->display);
        raw_event.root = DefaultRootWindow(context->display);
        raw_event.subwindow = None;
        raw_event.time = CurrentTime;
        raw_event.x = 1;
        raw_event.y = 1;
        raw_event.x_root = 1;
        raw_event.y_root = 1;
        raw_event.same_screen = True;
        raw_event.keycode = key_code;
        raw_event.state = state;
        raw_event.type = event_type;

        int res = XLookupString(&raw_event, event.buffer,
                                sizeof(event.buffer) - 1, NULL, NULL);
        if (res > 0) {
            event.buffer_len = res;
        } else {
            memset(event.buffer, 0, sizeof(event.buffer));
            event.buffer_len = 0;
        }

        event.event_type = INPUT_EVENT_TYPE_KEYBOARD;
        event.key_code = key_code;
        event.key_sym = XLookupKeysym(&raw_event, 0);
        event.status = event_type == KeyPress ? INPUT_STATUS_PRESSED
                                              : INPUT_STATUS_RELEASED;
    } else if (event_type == ButtonPress || event_type == ButtonRelease) {
        event.event_type = INPUT_EVENT_TYPE_MOUSE;
        event.key_code = key_code;
        event.status = event_type == ButtonPress ? INPUT_STATUS_PRESSED
                                                 : INPUT_STATUS_RELEASED;
    }

    if (event.event_type != 0) {
        context->event_callback(context->rust_instance, event);
    }
    return event.buffer_len;
}

static void emit_hotkey_event(DetectContext *context, int key_code,
                              unsigned int state) {
    if (!context || !context->event_callback) {
        return;
    }

    InputEvent event = {};
    event.event_type = INPUT_EVENT_TYPE_HOTKEY;
    event.key_code = key_code;
    event.state = state;
    event.status = INPUT_STATUS_PRESSED;
    context->event_callback(context->rust_instance, event);
}

static bool initialize_xinput2_mouse(DetectContext *context) {
    int event = 0;
    int error = 0;
    if (!XQueryExtension(context->display, "XInputExtension",
                         &context->xi_opcode, &event, &error)) {
        return false;
    }

    int major = 2;
    int minor = 0;
    if (XIQueryVersion(context->display, &major, &minor) != Success ||
        major < 2) {
        return false;
    }

    unsigned char mask[(XI_LASTEVENT + 7) / 8];
    memset(mask, 0, sizeof(mask));
    XISetMask(mask, XI_RawButtonPress);
    XISetMask(mask, XI_RawButtonRelease);

    XIEventMask event_mask = {};
    event_mask.deviceid = XIAllMasterDevices;
    event_mask.mask_len = sizeof(mask);
    event_mask.mask = mask;

    Window root = DefaultRootWindow(context->display);
    if (XISelectEvents(context->display, root, &event_mask, 1) != Success) {
        return false;
    }

    XFlush(context->display);
    return true;
}

static bool keymap_key_down(const char keymap[32], int key_code) {
    if (key_code < 0 || key_code > 255) {
        return false;
    }
    const unsigned char byte =
        static_cast<unsigned char>(keymap[key_code >> 3]);
    return (byte & (1U << (key_code & 7))) != 0;
}

static RegisteredHotKey *find_matching_hotkey(DetectContext *context,
                                               int key_code,
                                               unsigned int state) {
    if (!context) {
        return nullptr;
    }

    for (auto &hotkey : context->hotkeys) {
        if (hotkey.key_code == key_code &&
            (state & hotkey.valid_modifiers) == hotkey.state) {
            return &hotkey;
        }
    }
    return nullptr;
}

static bool hotkey_key_is_active(DetectContext *context, int key_code) {
    if (!context) {
        return false;
    }
    for (int active : context->active_hotkey_keys) {
        if (active == key_code) {
            return true;
        }
    }
    return false;
}

static bool release_active_hotkey_key(DetectContext *context, int key_code) {
    if (!context) {
        return false;
    }
    for (auto it = context->active_hotkey_keys.begin();
         it != context->active_hotkey_keys.end(); ++it) {
        if (*it == key_code) {
            context->active_hotkey_keys.erase(it);
            return true;
        }
    }
    return false;
}

static void log_input_stats(DetectContext *context) {
    if (!context || context->polls == 0 || (context->polls % 2000ULL) != 0) {
        return;
    }

    unsigned long long fallbacks = 0;
    for (const auto &hotkey : context->hotkeys) {
        if (hotkey.poll_fallback) {
            ++fallbacks;
        }
    }

    char line[512] = {0};
    snprintf(line, sizeof(line),
             "[rESP-INPUT] pid=%ld backend=xquerykeymap polls=%llu "
             "presses=%llu releases=%llu translated=%llu empty=%llu "
             "shift_recovered=%llu hotkeys_polled=%llu hotkey_fallbacks=%llu "
             "xi2_mouse=%s\n",
             static_cast<long>(getpid()), context->polls, context->presses,
             context->releases, context->translated, context->empty,
             context->shift_recovered, context->hotkeys_polled, fallbacks,
             context->xi2_mouse ? "on" : "off");
    write_diag(line);
}

static void poll_keyboard(DetectContext *context) {
    if (!context || !context->display) {
        return;
    }

    char current[32] = {0};
    XQueryKeymap(context->display, current);
    ++context->polls;

    if (!context->keymap_initialized) {
        memcpy(context->previous_keymap, current, sizeof(current));
        context->keymap_initialized = true;
        log_input_stats(context);
        return;
    }

    for (int key_code = 8; key_code <= 255; ++key_code) {
        const bool was_down = keymap_key_down(context->previous_keymap, key_code);
        const bool is_down = keymap_key_down(current, key_code);
        if (was_down == is_down) {
            continue;
        }

        const unsigned int state = current_xkb_state(context->display);

        if (is_down) {
            ++context->presses;

            RegisteredHotKey *hotkey =
                find_matching_hotkey(context, key_code, state);
            if (hotkey) {
                if (!hotkey_key_is_active(context, key_code)) {
                    context->active_hotkey_keys.push_back(key_code);
                    if (hotkey->poll_fallback) {
                        emit_hotkey_event(context, key_code, hotkey->state);
                        ++context->hotkeys_polled;
                    }
                }
                // Do not feed a configured shortcut into the text matcher.
                // Grabbed hotkeys arrive through the normal KeyPress event;
                // fallback hotkeys are emitted above from polling.
                continue;
            }

            const int len = emit_input_event(context, KeyPress, key_code, state);
            if (len > 0) {
                ++context->translated;
            } else {
                ++context->empty;
            }
        } else {
            ++context->releases;
            if (release_active_hotkey_key(context, key_code)) {
                continue;
            }
            emit_input_event(context, KeyRelease, key_code, state);
        }
    }

    memcpy(context->previous_keymap, current, sizeof(current));
    log_input_stats(context);
}

// XGrabKey errors are asynchronous.  Trap only the registration round-trip,
// preserve the runtime-wide hardened handler for every other X11 error, then
// restore it immediately after registration.
struct GrabErrorTrap {
    Display *display;
    int error_code;
    int request_code;
    int minor_code;
};

static thread_local GrabErrorTrap *active_grab_trap = nullptr;
static std::mutex grab_handler_mutex;
static XErrorHandler delegated_error_handler = nullptr;

static int hotkey_grab_error_handler(Display *display, XErrorEvent *error) {
    if (active_grab_trap && active_grab_trap->display == display &&
        error->request_code == X_GrabKey) {
        active_grab_trap->error_code = error->error_code;
        active_grab_trap->request_code = error->request_code;
        active_grab_trap->minor_code = error->minor_code;
        return 0;
    }

    return delegated_error_handler ? delegated_error_handler(display, error) : 0;
}

int32_t detect_check_x11() {
    Display *display = XOpenDisplay(NULL);
    if (!display) {
        return 0;
    }
    XCloseDisplay(display);
    return 1;
}

void *detect_initialize(void *_rust_instance, int32_t *error_code) {
    setlocale(LC_ALL, "");

    std::unique_ptr<DetectContext> context(new DetectContext());
    context->display = XOpenDisplay(NULL);
    context->xi_opcode = 0;
    context->xi2_mouse = false;
    context->rust_instance = _rust_instance;
    context->event_callback = nullptr;
    memset(context->previous_keymap, 0, sizeof(context->previous_keymap));
    context->keymap_initialized = false;
    context->polls = 0;
    context->presses = 0;
    context->releases = 0;
    context->translated = 0;
    context->empty = 0;
    context->shift_recovered = 0;
    context->hotkeys_polled = 0;

    if (!context->display) {
        *error_code = -1;
        return nullptr;
    }

    int dummy = 0;
    if (!XkbQueryExtension(context->display, &dummy, &dummy, &dummy, &dummy,
                           &dummy)) {
        *error_code = -3;
        XCloseDisplay(context->display);
        return nullptr;
    }

    // Keyboard input deliberately does not depend on XI2.  Some Astra/KDE
    // sessions advertise XI2 successfully but never deliver RawKey events.
    // XI2 remains useful for non-exclusive global mouse button observation.
    context->xi2_mouse = initialize_xinput2_mouse(context.get());

    XQueryKeymap(context->display, context->previous_keymap);
    context->keymap_initialized = true;
    XKeysymToKeycode(context->display, XK_F1);

    char line[256] = {0};
    snprintf(line, sizeof(line),
             "[rESP-INPUT] pid=%ld keyboard=xquerykeymap-poll interval_ms=2 "
             "xi2_mouse=%s\n",
             static_cast<long>(getpid()),
             context->xi2_mouse ? "on" : "off");
    write_diag(line);

    return context.release();
}

ModifierIndexes detect_get_modifier_indexes(void *_context) {
    DetectContext *context = static_cast<DetectContext *>(_context);
    ModifierIndexes indexes = {};

    XModifierKeymap *map = XGetModifierMapping(context->display);
    if (!map) {
        return indexes;
    }

    for (int i = 0; i < 8; ++i) {
        for (int j = 0; j < map->max_keypermod; ++j) {
            const int code = map->modifiermap[i * map->max_keypermod + j];
            if (code == 0) {
                continue;
            }

            const KeySym sym =
                XkbKeycodeToKeysym(context->display, code, 0, 0);
            if (sym == XK_Control_L || sym == XK_Control_R) {
                indexes.ctrl = i;
            } else if (sym == XK_Super_L || sym == XK_Super_R) {
                indexes.meta = i;
            } else if (sym == XK_Shift_L || sym == XK_Shift_R) {
                indexes.shift = i;
            } else if (sym == XK_Alt_L || sym == XK_Alt_R) {
                indexes.alt = i;
            }
        }
    }

    XFreeModifiermap(map);
    return indexes;
}

HotKeyResult detect_register_hotkey(void *_context, HotKeyRequest request,
                                    ModifierIndexes mod_indexes) {
    DetectContext *context = static_cast<DetectContext *>(_context);
    HotKeyResult result = {};

    const KeyCode key_code =
        XKeysymToKeycode(context->display, request.key_sym);
    if (key_code == 0) {
        return result;
    }

    uint32_t valid_modifiers = 0;
    valid_modifiers |= 1U << mod_indexes.alt;
    valid_modifiers |= 1U << mod_indexes.ctrl;
    valid_modifiers |= 1U << mod_indexes.shift;
    valid_modifiers |= 1U << mod_indexes.meta;

    uint32_t target_modifiers = 0;
    if (request.ctrl) target_modifiers |= 1U << mod_indexes.ctrl;
    if (request.alt) target_modifiers |= 1U << mod_indexes.alt;
    if (request.shift) target_modifiers |= 1U << mod_indexes.shift;
    if (request.meta) target_modifiers |= 1U << mod_indexes.meta;

    result.state = target_modifiers;
    result.key_code = key_code;

    bool denied = false;
    int denied_code = 0;
    int denied_request = X_GrabKey;
    int denied_minor = 0;
    std::vector<unsigned int> grabbed;
    Window root = DefaultRootWindow(context->display);

    {
        std::lock_guard<std::mutex> guard(grab_handler_mutex);
        XSync(context->display, False);
        delegated_error_handler = XSetErrorHandler(hotkey_grab_error_handler);

        for (uint32_t state = 0; state < 256; ++state) {
            if ((state == 0 || (state & ~valid_modifiers) != 0) &&
                (state & valid_modifiers) == 0) {
                const uint32_t final_modifiers = state | target_modifiers;
                GrabErrorTrap trap = {context->display, 0, 0, 0};
                active_grab_trap = &trap;
                XGrabKey(context->display, key_code, final_modifiers, root,
                         False, GrabModeAsync, GrabModeAsync);
                XSync(context->display, False);
                active_grab_trap = nullptr;

                if (trap.error_code != 0) {
                    denied = true;
                    denied_code = trap.error_code;
                    denied_request = trap.request_code;
                    denied_minor = trap.minor_code;
                    break;
                }
                grabbed.push_back(final_modifiers);
            }
        }

        if (denied) {
            for (unsigned int modifiers : grabbed) {
                XUngrabKey(context->display, key_code, modifiers, root);
            }
            XSync(context->display, False);
        }

        XSetErrorHandler(delegated_error_handler);
        delegated_error_handler = nullptr;
    }

    RegisteredHotKey registration = {
        static_cast<int>(key_code), target_modifiers, valid_modifiers, denied};
    bool exists = false;
    for (const auto &hotkey : context->hotkeys) {
        if (hotkey.key_code == registration.key_code &&
            hotkey.state == registration.state) {
            exists = true;
            break;
        }
    }
    if (!exists) {
        context->hotkeys.push_back(registration);
    }

    // Even when XGrabKey is denied, Rust must keep the (key_code,state)->ID
    // mapping because polling emits the same native HotKey event shape.
    result.success = 1;

    char line[512] = {0};
    if (denied) {
        char error_text[160] = {0};
        XGetErrorText(context->display, denied_code, error_text,
                      sizeof(error_text));
        snprintf(line, sizeof(line),
                 "[rESP-HOTKEY-GRAB] pid=%ld keycode=%u modifiers=0x%x "
                 "error=%d(%s) request=%d minor=%d\n",
                 static_cast<long>(getpid()), key_code, target_modifiers,
                 denied_code,
                 error_text[0] ? error_text : "unknown",
                 denied_request, denied_minor);
        write_diag(line);

        snprintf(line, sizeof(line),
                 "[rESP-HOTKEY] pid=%ld backend=xquerykeymap-fallback "
                 "keycode=%u modifiers=0x%x\n",
                 static_cast<long>(getpid()), key_code, target_modifiers);
        write_diag(line);
    } else {
        snprintf(line, sizeof(line),
                 "[rESP-HOTKEY] pid=%ld backend=xgrabkey keycode=%u "
                 "modifiers=0x%x\n",
                 static_cast<long>(getpid()), key_code, target_modifiers);
        write_diag(line);
    }

    return result;
}

static void process_x11_event(DetectContext *context, XEvent *event) {
    if (!context || !event) {
        return;
    }

    if (event->type == MappingNotify) {
        XMappingEvent *mapping = reinterpret_cast<XMappingEvent *>(event);
        if (mapping->request == MappingKeyboard) {
            XRefreshKeyboardMapping(mapping);
        }
        return;
    }

    // Successfully grabbed hotkeys are delivered as ordinary KeyPress events.
    if (event->type == KeyPress) {
        emit_hotkey_event(context, event->xkey.keycode, event->xkey.state);
        return;
    }

    if (!context->xi2_mouse || event->type != GenericEvent ||
        event->xcookie.extension != context->xi_opcode) {
        return;
    }

    if (!XGetEventData(context->display, &event->xcookie)) {
        return;
    }

    const int evtype = event->xcookie.evtype;
    XIRawEvent *raw = static_cast<XIRawEvent *>(event->xcookie.data);
    if (raw) {
        const unsigned int state = current_xkb_state(context->display);
        if (evtype == XI_RawButtonPress) {
            emit_input_event(context, ButtonPress, raw->detail, state);
        } else if (evtype == XI_RawButtonRelease) {
            emit_input_event(context, ButtonRelease, raw->detail, state);
        }
    }

    XFreeEventData(context->display, &event->xcookie);
}

int32_t detect_eventloop(void *_context, EventCallback callback) {
    DetectContext *context = static_cast<DetectContext *>(_context);
    if (!context) {
        return -1;
    }
    context->event_callback = callback;

    const int fd = XConnectionNumber(context->display);

    while (true) {
        while (XPending(context->display) > 0) {
            XEvent event;
            XNextEvent(context->display, &event);
            process_x11_event(context, &event);
        }

        poll_keyboard(context);

        // Two milliseconds matches the already proven Astra polling build:
        // low enough latency for expansions/hotkeys without a busy spin.
        pollfd descriptor = {fd, POLLIN, 0};
        const int result = poll(&descriptor, 1, 2);
        if (result > 0 &&
            (descriptor.revents & (POLLERR | POLLHUP | POLLNVAL))) {
            return -2;
        }
        if (result < 0) {
            if (errno == EINTR) {
                continue;
            }
            return -2;
        }
    }

    return 1;
}

int32_t detect_destroy(void *_context) {
    DetectContext *context = static_cast<DetectContext *>(_context);
    if (!context) {
        return -1;
    }

    if (context->display) {
        // Releases only grabs owned by this client.
        XUngrabKey(context->display, AnyKey, AnyModifier,
                   DefaultRootWindow(context->display));
        XSync(context->display, False);
        XCloseDisplay(context->display);
    }
    delete context;
    return 1;
}
