/*
 * Astra Linux / X11 detector for rEspanso pol_run.
 *
 * Hardened Astra X servers may disable the X11 RECORD extension and can also
 * suppress XInput2 raw keyboard delivery. Text expansion therefore uses
 * XQueryKeymap polling for global keyboard transitions. XInput2 is retained
 * only for global mouse button events. This requires no root privileges and
 * no access to /dev/input.
 */

#include "native.h"

#include <chrono>
#include <errno.h>
#include <locale.h>
#include <memory>
#include <stdio.h>
#include <string.h>
#include <sys/select.h>
#include <unistd.h>
#include <vector>

#include <X11/XKBlib.h>
#include <X11/Xlib.h>
#include <X11/Xproto.h>
#include <X11/Xutil.h>
#include <X11/extensions/XInput2.h>
#include <X11/keysym.h>
#include <X11/keysymdef.h>

extern "C" void detect_write_x11_log(const char *line);

typedef struct {
    Display *display;
    int xi_opcode;
    bool xi2_mouse_enabled;
    void *rust_instance;
    EventCallback event_callback;

    char previous_keymap[32];
    bool keymap_initialized;

    unsigned long long poll_count;
    unsigned long long press_count;
    unsigned long long release_count;
    unsigned long long translated_count;
    unsigned long long empty_translation_count;
    unsigned long long recovered_shift_count;
    std::chrono::steady_clock::time_point last_diag;
} DetectContext;

// XGrabKey reports ownership conflicts asynchronously. Without an explicit
// error trap, BadAccess can reach whichever process-wide Xlib handler GTK/KDE
// installed and terminate the worker with exit code 1. Keep the trap scoped to
// the synchronous registration round-trip and restore the previous handler.
static thread_local int hotkey_grab_error_code = Success;
static thread_local unsigned int hotkey_grab_key_code = 0;
static thread_local unsigned int hotkey_grab_modifiers = 0;
static XErrorHandler previous_hotkey_error_handler = nullptr;

static int hotkey_grab_error_handler(Display *display, XErrorEvent *error) {
    if (error && error->request_code == X_GrabKey) {
        hotkey_grab_error_code = error->error_code;

        char error_text[256] = {0};
        if (display) {
            XGetErrorText(display, error->error_code, error_text,
                          sizeof(error_text));
        }

        char line[768] = {0};
        snprintf(line, sizeof(line),
                 "[rESP-HOTKEY-GRAB] pid=%ld keycode=%u modifiers=0x%x "
                 "error=%u(%s) request=%u minor=%u\n",
                 static_cast<long>(getpid()), hotkey_grab_key_code,
                 hotkey_grab_modifiers,
                 static_cast<unsigned int>(error->error_code),
                 error_text[0] ? error_text : "unknown",
                 static_cast<unsigned int>(error->request_code),
                 static_cast<unsigned int>(error->minor_code));
        detect_write_x11_log(line);
        return 0;
    }

    if (previous_hotkey_error_handler) {
        return previous_hotkey_error_handler(display, error);
    }

    return 0;
}

static unsigned int current_xkb_state(Display *display) {
    XkbStateRec xkb_state = {};
    if (XkbGetState(display, XkbUseCoreKbd, &xkb_state) != Success) {
        return 0;
    }

    // XKeyEvent.state keeps modifier masks in the low bits and the active
    // keyboard group in bits 13-14. XkbStateRec.mods is the effective modifier
    // state (base + latched + locked), which is exactly what XLookupString
    // expects for a synthetic XKeyEvent.
    return static_cast<unsigned int>(xkb_state.mods) |
           ((static_cast<unsigned int>(xkb_state.group) & 0x3U) << 13);
}

static bool is_modifier_keycode(Display *display, int key_code) {
    const KeySym sym = XkbKeycodeToKeysym(display, key_code, 0, 0);
    return sym == XK_Shift_L || sym == XK_Shift_R ||
           sym == XK_Control_L || sym == XK_Control_R ||
           sym == XK_Alt_L || sym == XK_Alt_R ||
           sym == XK_Meta_L || sym == XK_Meta_R ||
           sym == XK_Super_L || sym == XK_Super_R ||
           sym == XK_Caps_Lock || sym == XK_Num_Lock;
}

static void emit_input_event(DetectContext *context, int event_type,
                             int key_code, unsigned int state) {
    if (!context || !context->event_callback) {
        return;
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

        // Ask Xlib for both the printable bytes and the resolved KeySym. Using
        // the KeySym returned by XLookupString (rather than XLookupKeysym index
        // 0) preserves Shift and the active XKB group for RU/EN layouts.
        KeySym resolved_sym = NoSymbol;
        XComposeStatus compose = {};
        const int res = XLookupString(&raw_event, event.buffer,
                                      sizeof(event.buffer) - 1,
                                      &resolved_sym, &compose);
        if (res > 0) {
            event.buffer_len = res;
            event.buffer[res] = '\0';
            if (event_type == KeyPress) {
                context->translated_count++;
            }
        } else {
            memset(event.buffer, 0, sizeof(event.buffer));
            event.buffer_len = 0;
            if (event_type == KeyPress) {
                context->empty_translation_count++;
            }
        }

        if (resolved_sym == NoSymbol) {
            resolved_sym = XkbKeycodeToKeysym(context->display, key_code, 0, 0);
        }

        event.event_type = INPUT_EVENT_TYPE_KEYBOARD;
        event.key_code = key_code;
        event.key_sym = static_cast<int32_t>(resolved_sym);
        event.status = event_type == KeyPress ? INPUT_STATUS_PRESSED
                                              : INPUT_STATUS_RELEASED;

        if (event_type == KeyPress) {
            context->press_count++;
        } else {
            context->release_count++;
        }
    } else if (event_type == ButtonPress || event_type == ButtonRelease) {
        event.event_type = INPUT_EVENT_TYPE_MOUSE;
        event.key_code = key_code;
        event.status = event_type == ButtonPress ? INPUT_STATUS_PRESSED
                                                 : INPUT_STATUS_RELEASED;
    }

    if (event.event_type != 0) {
        context->event_callback(context->rust_instance, event);
    }
}

static void maybe_log_input_stats(DetectContext *context) {
    const auto now = std::chrono::steady_clock::now();
    if (now - context->last_diag < std::chrono::seconds(5)) {
        return;
    }
    context->last_diag = now;

    char line[640] = {0};
    snprintf(line, sizeof(line),
             "[rESP-INPUT] pid=%ld backend=xquerykeymap polls=%llu "
             "presses=%llu releases=%llu translated=%llu empty=%llu "
             "shift_recovered=%llu xi2_mouse=%s\n",
             static_cast<long>(getpid()), context->poll_count,
             context->press_count, context->release_count,
             context->translated_count, context->empty_translation_count,
             context->recovered_shift_count,
             context->xi2_mouse_enabled ? "on" : "off");
    detect_write_x11_log(line);
}

static bool key_is_down(const char keymap[32], int key_code) {
    const unsigned int code = static_cast<unsigned int>(key_code);
    return (static_cast<unsigned char>(keymap[code >> 3]) &
            static_cast<unsigned char>(1U << (code & 7U))) != 0;
}

static bool shift_is_down_in_keymap(Display *display, const char keymap[32]) {
    const KeyCode left = XKeysymToKeycode(display, XK_Shift_L);
    const KeyCode right = XKeysymToKeycode(display, XK_Shift_R);
    return (left != 0 && key_is_down(keymap, left)) ||
           (right != 0 && key_is_down(keymap, right));
}

static bool key_has_distinct_shift_level(Display *display, int key_code,
                                         unsigned int state) {
    const int group = static_cast<int>((state >> 13) & 0x3U);
    const KeySym base = XkbKeycodeToKeysym(display, key_code, group, 0);
    const KeySym shifted = XkbKeycodeToKeysym(display, key_code, group, 1);
    return base != NoSymbol && shifted != NoSymbol && base != shifted;
}

static void poll_keyboard(DetectContext *context) {
    char current_keymap[32] = {0};
    if (!XQueryKeymap(context->display, current_keymap)) {
        return;
    }

    context->poll_count++;

    if (!context->keymap_initialized) {
        memcpy(context->previous_keymap, current_keymap,
               sizeof(context->previous_keymap));
        context->keymap_initialized = true;
        maybe_log_input_stats(context);
        return;
    }

    const unsigned int state = current_xkb_state(context->display);
    const bool previous_shift_down =
        shift_is_down_in_keymap(context->display, context->previous_keymap);
    const bool current_shift_down =
        shift_is_down_in_keymap(context->display, current_keymap);

    // XQueryKeymap gives us snapshots, not per-key event timestamps. A common
    // fast sequence such as Shift+';' can therefore look like this:
    // previous poll: Shift down, ';' up
    // current poll:  Shift up,   ';' down
    // The application correctly received ':' because Shift was down at the
    // real key press, but translating from only the current XKB state would
    // incorrectly produce ';'. If Shift was released inside the same polling
    // interval, preserve it for newly-pressed shift-sensitive keys. This also
    // fixes other shifted trigger characters without recording typed content.
    const bool recover_released_shift = previous_shift_down && !current_shift_down;

    // Modifier presses must reach Rust before ordinary key presses from the
    // same poll. Releases use the opposite order. This preserves Espanso's
    // modifier-state middleware even when two transitions happen inside one
    // polling interval.
    for (int pass = 0; pass < 4; ++pass) {
        for (int key_code = 8; key_code < 256; ++key_code) {
            const bool was_down = key_is_down(context->previous_keymap, key_code);
            const bool is_down = key_is_down(current_keymap, key_code);
            if (was_down == is_down) {
                continue;
            }

            const bool modifier = is_modifier_keycode(context->display, key_code);
            if (pass == 0 && is_down && modifier) {
                emit_input_event(context, KeyPress, key_code, state);
            } else if (pass == 1 && is_down && !modifier) {
                unsigned int press_state = state;
                if (recover_released_shift &&
                    key_has_distinct_shift_level(context->display, key_code, state)) {
                    press_state |= ShiftMask;
                    context->recovered_shift_count++;
                }
                emit_input_event(context, KeyPress, key_code, press_state);
            } else if (pass == 2 && !is_down && !modifier) {
                emit_input_event(context, KeyRelease, key_code, state);
            } else if (pass == 3 && !is_down && modifier) {
                emit_input_event(context, KeyRelease, key_code, state);
            }
        }
    }

    memcpy(context->previous_keymap, current_keymap,
           sizeof(context->previous_keymap));
    maybe_log_input_stats(context);
}

static bool initialize_xinput2_mouse(DetectContext *context) {
    int event = 0;
    int error = 0;
    if (!XQueryExtension(context->display, "XInputExtension",
                         &context->xi_opcode, &event, &error)) {
        context->xi_opcode = 0;
        return false;
    }

    int major = 2;
    int minor = 0;
    if (XIQueryVersion(context->display, &major, &minor) != Success ||
        major < 2) {
        context->xi_opcode = 0;
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
        context->xi_opcode = 0;
        return false;
    }

    XFlush(context->display);
    return true;
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
    context->xi2_mouse_enabled = false;
    context->rust_instance = _rust_instance;
    context->event_callback = nullptr;
    memset(context->previous_keymap, 0, sizeof(context->previous_keymap));
    context->keymap_initialized = false;
    context->poll_count = 0;
    context->press_count = 0;
    context->release_count = 0;
    context->translated_count = 0;
    context->empty_translation_count = 0;
    context->recovered_shift_count = 0;
    context->last_diag = std::chrono::steady_clock::now();

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

    // Text input no longer depends on XInput2: Astra may allow XI2 setup while
    // withholding raw keyboard events. XI2 is optional and used only to keep
    // mouse clicks as matcher separators.
    context->xi2_mouse_enabled = initialize_xinput2_mouse(context.get());

    XKeysymToKeycode(context->display, XK_F1);

    // Baseline the current keyboard bitmap so keys already held during startup
    // are not emitted as synthetic presses.
    XQueryKeymap(context->display, context->previous_keymap);
    context->keymap_initialized = true;

    char line[512] = {0};
    snprintf(line, sizeof(line),
             "[rESP-INPUT] pid=%ld keyboard=xquerykeymap-poll interval_ms=2 "
             "xi2_mouse=%s\n",
             static_cast<long>(getpid()),
             context->xi2_mouse_enabled ? "on" : "off");
    detect_write_x11_log(line);

    fprintf(stderr,
            "rEspanso: using XQueryKeymap keyboard detector "
            "(Astra X11 backend, XI2 mouse=%s)\n",
            context->xi2_mouse_enabled ? "on" : "off");

    return context.release();
}

ModifierIndexes detect_get_modifier_indexes(void *_context) {
    DetectContext *context = static_cast<DetectContext *>(_context);
    ModifierIndexes indexes = {};

    XModifierKeymap *map = XGetModifierMapping(context->display);
    if (!map) {
        return indexes;
    }

    for (int i = 0; i < 8; i++) {
        for (int j = 0; j < map->max_keypermod; j++) {
            int code = map->modifiermap[i * map->max_keypermod + j];
            if (code == 0) {
                continue;
            }
            KeySym sym = XkbKeycodeToKeysym(context->display, code, 0, 0);
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

    KeyCode key_code = XKeysymToKeycode(context->display, request.key_sym);
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
    result.success = 1;

    Window root = DefaultRootWindow(context->display);
    std::vector<uint32_t> grabbed_modifiers;

    // Drain previous requests first, then make every grab synchronous so a
    // BadAccess conflict is attributed to this registration rather than a
    // later clipboard/injector Xlib call.
    XSync(context->display, False);
    previous_hotkey_error_handler = XSetErrorHandler(&hotkey_grab_error_handler);

    for (uint32_t state = 0; state < 256; state++) {
        if ((state == 0 || (state & ~valid_modifiers) != 0) &&
            (state & valid_modifiers) == 0) {
            const uint32_t final_modifiers = state | target_modifiers;

            hotkey_grab_error_code = Success;
            hotkey_grab_key_code = key_code;
            hotkey_grab_modifiers = final_modifiers;

            XGrabKey(context->display, key_code, final_modifiers, root, False,
                     GrabModeAsync, GrabModeAsync);
            XSync(context->display, False);

            if (hotkey_grab_error_code != Success) {
                result.success = 0;
                break;
            }

            grabbed_modifiers.push_back(final_modifiers);
        }
    }

    if (result.success == 0) {
        // Do not leave a partially registered hotkey behind if one of the
        // NumLock/CapsLock variants is already owned by KDE or another app.
        for (uint32_t modifiers : grabbed_modifiers) {
            XUngrabKey(context->display, key_code, modifiers, root);
        }
        XSync(context->display, False);
    }

    XSetErrorHandler(previous_hotkey_error_handler);
    previous_hotkey_error_handler = nullptr;
    hotkey_grab_key_code = 0;
    hotkey_grab_modifiers = 0;

    return result;
}

static void process_event(DetectContext *context, XEvent *event) {
    if (event->type == MappingNotify) {
        XMappingEvent *mapping = reinterpret_cast<XMappingEvent *>(event);
        if (mapping->request == MappingKeyboard) {
            XRefreshKeyboardMapping(mapping);
        }
        return;
    }

    // Events generated by XGrabKey are used only for Espanso hotkeys.
    if (event->type == KeyPress) {
        InputEvent input_event = {};
        input_event.event_type = INPUT_EVENT_TYPE_HOTKEY;
        input_event.key_code = event->xkey.keycode;
        input_event.state = event->xkey.state;
        if (context->event_callback) {
            context->event_callback(context->rust_instance, input_event);
        }
        return;
    }

    if (!context->xi2_mouse_enabled || context->xi_opcode == 0 ||
        event->type != GenericEvent ||
        event->xcookie.extension != context->xi_opcode) {
        return;
    }

    if (!XGetEventData(context->display, &event->xcookie)) {
        return;
    }

    const int evtype = event->xcookie.evtype;
    XIRawEvent *raw = static_cast<XIRawEvent *>(event->xcookie.data);
    if (raw) {
        switch (evtype) {
        case XI_RawButtonPress:
            emit_input_event(context, ButtonPress, raw->detail, 0);
            break;
        case XI_RawButtonRelease:
            emit_input_event(context, ButtonRelease, raw->detail, 0);
            break;
        default:
            break;
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
            process_event(context, &event);
        }

        // Poll at 500 Hz. The shorter window materially reduces ambiguity
        // between printable key presses and nearby modifier releases while
        // remaining cheap on a local X11 connection.
        poll_keyboard(context);

        fd_set fds;
        FD_ZERO(&fds);
        FD_SET(fd, &fds);
        timeval timeout = {0, 2000};
        const int result = select(fd + 1, &fds, NULL, NULL, &timeout);
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
        XCloseDisplay(context->display);
    }
    delete context;
    return 1;
}
