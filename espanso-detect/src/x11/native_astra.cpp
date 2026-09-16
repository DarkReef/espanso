/*
 * Astra Linux / X11 detector for rEspanso pol_run.
 *
 * Hardened Astra X servers may disable the X11 RECORD extension. This
 * implementation therefore listens for global keyboard/mouse input through
 * XInput2 raw events. Espanso hotkeys are recognized from the same raw stream
 * instead of using exclusive XGrabKey registrations, so KDE or another X11
 * client cannot prevent rEspanso from observing a configured shortcut.
 * It requires no root privileges and no access to /dev/input.
 */

#include "native.h"

#include <locale.h>
#include <errno.h>
#include <vector>
#include <utility>
#include <memory>
#include <stdio.h>
#include <string.h>
#include <poll.h>

#include <X11/XKBlib.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/extensions/XInput2.h>
#include <X11/keysym.h>
#include <X11/keysymdef.h>

typedef struct {
    Display *display;
    int xi_opcode;
    void *rust_instance;
    EventCallback event_callback;
    unsigned int hotkey_modifier_mask;
    std::vector<std::pair<int, unsigned int>> hotkeys;
    std::vector<int> active_hotkey_keys;
} DetectContext;

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

static unsigned int current_xkb_state(Display *display) {
    XkbStateRec xkb_state = {};
    if (XkbGetState(display, XkbUseCoreKbd, &xkb_state) != Success) {
        return 0;
    }

    // XKeyEvent.state keeps modifier masks in the low bits and the active
    // keyboard group in bits 13-14. Preserving the group is important on
    // Russian/English layouts because XLookupString uses it for translation.
    return static_cast<unsigned int>(xkb_state.mods) |
           ((static_cast<unsigned int>(xkb_state.group) & 0x3U) << 13);
}

static bool initialize_xinput2(DetectContext *context) {
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
    XISetMask(mask, XI_RawKeyPress);
    XISetMask(mask, XI_RawKeyRelease);
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

static bool hotkey_matches(DetectContext *context, int key_code,
                           unsigned int state) {
    if (!context) {
        return false;
    }
    const unsigned int normalized = state & context->hotkey_modifier_mask;
    for (const auto &hotkey : context->hotkeys) {
        if (hotkey.first == key_code && hotkey.second == normalized) {
            return true;
        }
    }
    return false;
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
    context->rust_instance = _rust_instance;
    context->event_callback = nullptr;
    context->hotkey_modifier_mask = 0;

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

    if (!initialize_xinput2(context.get())) {
        // Rust currently maps -2 to the historical XRecordMissing error. In
        // this Astra-specific backend the practical meaning is that no usable
        // global X11 input source is available.
        *error_code = -2;
        XCloseDisplay(context->display);
        return nullptr;
    }

    XKeysymToKeycode(context->display, XK_F1);
    fprintf(stderr,
            "rEspanso: using XInput2 raw-event detector (Astra X11 backend)\n");

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

    // XInput2 raw events are observer-only and do not require exclusive grabs.
    // Keep a lightweight local registration so the Rust layer can retain its
    // existing (key_code, state) -> hotkey ID mapping without XGrabKey.
    context->hotkey_modifier_mask |= valid_modifiers;
    bool already_registered = false;
    for (const auto &hotkey : context->hotkeys) {
        if (hotkey.first == result.key_code && hotkey.second == result.state) {
            already_registered = true;
            break;
        }
    }
    if (!already_registered) {
        context->hotkeys.push_back(
            std::make_pair(result.key_code, result.state));
    }
    result.success = 1;

    fprintf(stderr,
            "rEspanso: hotkey keycode=%u modifiers=0x%x uses XInput2 raw events\n",
            key_code, target_modifiers);
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

    if (event->type != GenericEvent ||
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
        switch (evtype) {
        case XI_RawKeyPress:
            if (hotkey_matches(context, raw->detail, state)) {
                // Suppress auto-repeat while the hotkey key remains down. This
                // matches the old grab path, where one physical activation was
                // delivered as one Espanso hotkey event.
                if (!hotkey_key_is_active(context, raw->detail)) {
                    context->active_hotkey_keys.push_back(raw->detail);
                    emit_hotkey_event(context, raw->detail, state);
                }
            } else {
                emit_input_event(context, KeyPress, raw->detail, state);
            }
            break;
        case XI_RawKeyRelease:
            // A hotkey press is not exposed as an ordinary Keyboard event, so
            // suppress its matching release as well to keep matcher state sane.
            if (!release_active_hotkey_key(context, raw->detail)) {
                emit_input_event(context, KeyRelease, raw->detail, state);
            }
            break;
        case XI_RawButtonPress:
            emit_input_event(context, ButtonPress, raw->detail, state);
            break;
        case XI_RawButtonRelease:
            emit_input_event(context, ButtonRelease, raw->detail, state);
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

        pollfd descriptor = {fd, POLLIN, 0};
        const int result = poll(&descriptor, 1, 2000);
        if (result > 0 && (descriptor.revents & (POLLERR | POLLHUP | POLLNVAL))) {
            return -2;
        }
        if (result < 0) {
            if (errno == EINTR) continue;
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
