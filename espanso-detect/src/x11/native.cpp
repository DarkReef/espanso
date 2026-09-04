/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2021 Federico Terzi
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

/*
 * X11 detector used by the Astra portable build.
 *
 * Upstream Espanso historically relies on the X11 RECORD extension to
 * observe global keyboard events. Hardened Astra/KDE deployments can start
 * Xorg with RECORD disabled because it can be used for global key logging.
 *
 * For those systems we transparently fall back to XInput2 RawKey/RawButton
 * events. XInput2 is a normal X11 client API and requires no root privileges
 * or access to /dev/input. Injection is still handled by the existing XTest
 * backend in espanso-inject.
 */

#include "native.h"

#include <algorithm>
#include <locale.h>
#include <memory>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <X11/XKBlib.h>
#include <X11/Xlib.h>
#include <X11/Xlibint.h>
#include <X11/Xutil.h>
#include <X11/extensions/XInput2.h>
#include <X11/extensions/XTest.h>
#include <X11/extensions/record.h>
#include <X11/keysym.h>
#include <X11/keysymdef.h>

typedef union {
    unsigned char type;
    xEvent event;
    xResourceReq req;
    xGenericReply reply;
    xError error;
    xConnSetupPrefix setup;
} XRecordDatum;

enum DetectBackend {
    DETECT_BACKEND_RECORD = 1,
    DETECT_BACKEND_XINPUT2 = 2,
};

typedef struct {
    Display *data_disp;
    Display *ctrl_disp;
    XRecordRange *record_range;
    XRecordContext x_context;

    DetectBackend backend;
    int xi_opcode;

    void *rust_instance;
    EventCallback event_callback;
} DetectContext;

void detect_event_callback(XPointer, XRecordInterceptData *);
int detect_error_callback(Display *display, XErrorEvent *error);

static void emit_raw_event(DetectContext *context, int event_type, int key_code,
                           unsigned int state) {
    if (!context || !context->event_callback) {
        return;
    }

    InputEvent event = {};

    if (event_type == KeyPress || event_type == KeyRelease) {
        XKeyEvent raw_event = {};
        raw_event.display = context->ctrl_disp;
        raw_event.window = DefaultRootWindow(context->ctrl_disp);
        raw_event.root = DefaultRootWindow(context->ctrl_disp);
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

static bool initialize_xinput2(DetectContext *context) {
    int event = 0;
    int error = 0;
    if (!XQueryExtension(context->ctrl_disp, "XInputExtension",
                         &context->xi_opcode, &event, &error)) {
        return false;
    }

    int major = 2;
    int minor = 0;
    if (XIQueryVersion(context->ctrl_disp, &major, &minor) != Success) {
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

    Window root = DefaultRootWindow(context->ctrl_disp);
    if (XISelectEvents(context->ctrl_disp, root, &event_mask, 1) != Success) {
        return false;
    }
    XFlush(context->ctrl_disp);

    context->backend = DETECT_BACKEND_XINPUT2;
    fprintf(stderr,
            "rEspanso: X11 RECORD unavailable, using XInput2 raw-event fallback\n");
    return true;
}

int32_t detect_check_x11() {
    Display *check_disp = XOpenDisplay(NULL);
    if (!check_disp) {
        return 0;
    }
    XCloseDisplay(check_disp);
    return 1;
}

void *detect_initialize(void *_rust_instance, int32_t *error_code) {
    setlocale(LC_ALL, "");

    std::unique_ptr<DetectContext> context(new DetectContext());
    context->rust_instance = _rust_instance;
    context->ctrl_disp = XOpenDisplay(NULL);
    context->data_disp = nullptr;
    context->record_range = nullptr;
    context->x_context = 0;
    context->backend = DETECT_BACKEND_XINPUT2;
    context->xi_opcode = 0;

    if (!context->ctrl_disp) {
        *error_code = -1;
        return nullptr;
    }

    int dummy;
    if (!XkbQueryExtension(context->ctrl_disp, &dummy, &dummy, &dummy, &dummy,
                           &dummy)) {
        *error_code = -3;
        XCloseDisplay(context->ctrl_disp);
        return nullptr;
    }

    // Prefer the historical RECORD backend where available, but do not call
    // XRecordQueryVersion when the server does not advertise the extension:
    // hardened Astra X servers otherwise print a noisy Xlib error.
    int record_opcode = 0;
    int record_event = 0;
    int record_error = 0;
    bool has_record = XQueryExtension(context->ctrl_disp, "RECORD",
                                      &record_opcode, &record_event,
                                      &record_error) != 0;

    if (has_record) {
        context->data_disp = XOpenDisplay(NULL);
        if (!context->data_disp) {
            *error_code = -1;
            XCloseDisplay(context->ctrl_disp);
            return nullptr;
        }

        XSynchronize(context->ctrl_disp, True);

        if (!XRecordQueryVersion(context->ctrl_disp, &dummy, &dummy)) {
            has_record = false;
        }
    }

    if (has_record) {
        context->record_range = XRecordAllocRange();
        if (!context->record_range) {
            *error_code = -4;
            XCloseDisplay(context->data_disp);
            XCloseDisplay(context->ctrl_disp);
            return nullptr;
        }
        context->record_range->device_events.first = KeyPress;
        context->record_range->device_events.last = ButtonRelease;

        XRecordClientSpec client_spec = XRecordAllClients;
        context->x_context = XRecordCreateContext(
            context->ctrl_disp, 0, &client_spec, 1, &context->record_range, 1);
        if (!context->x_context) {
            *error_code = -5;
            XFree(context->record_range);
            XCloseDisplay(context->data_disp);
            XCloseDisplay(context->ctrl_disp);
            return nullptr;
        }

        if (!XRecordEnableContextAsync(context->data_disp, context->x_context,
                                       detect_event_callback,
                                       (XPointer)context.get())) {
            *error_code = -6;
            XRecordFreeContext(context->ctrl_disp, context->x_context);
            XFree(context->record_range);
            XCloseDisplay(context->data_disp);
            XCloseDisplay(context->ctrl_disp);
            return nullptr;
        }
        context->backend = DETECT_BACKEND_RECORD;
    } else {
        if (context->data_disp) {
            XCloseDisplay(context->data_disp);
            context->data_disp = nullptr;
        }
        if (!initialize_xinput2(context.get())) {
            *error_code = -2;
            XCloseDisplay(context->ctrl_disp);
            return nullptr;
        }
    }

    XSetErrorHandler(&detect_error_callback);
    XKeysymToKeycode(context->ctrl_disp, XK_F1);

    return context.release();
}

ModifierIndexes detect_get_modifier_indexes(void *_context) {
    DetectContext *context = (DetectContext *)_context;
    XModifierKeymap *map = XGetModifierMapping(context->ctrl_disp);

    ModifierIndexes indexes = {};
    for (int i = 0; i < 8; i++) {
        if (map->max_keypermod > 0) {
            int code = map->modifiermap[i * map->max_keypermod];
            KeySym sym = XkbKeycodeToKeysym(context->ctrl_disp, code, 0, 0);
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
    DetectContext *context = (DetectContext *)_context;
    KeyCode key_code = XKeysymToKeycode(context->ctrl_disp, request.key_sym);

    HotKeyResult result = {};
    if (key_code == 0) {
        return result;
    }

    uint32_t valid_modifiers = 0;
    valid_modifiers |= 1 << mod_indexes.alt;
    valid_modifiers |= 1 << mod_indexes.ctrl;
    valid_modifiers |= 1 << mod_indexes.shift;
    valid_modifiers |= 1 << mod_indexes.meta;

    uint32_t target_modifiers = 0;
    if (request.ctrl) target_modifiers |= 1 << mod_indexes.ctrl;
    if (request.alt) target_modifiers |= 1 << mod_indexes.alt;
    if (request.shift) target_modifiers |= 1 << mod_indexes.shift;
    if (request.meta) target_modifiers |= 1 << mod_indexes.meta;

    result.state = target_modifiers;
    result.key_code = key_code;
    result.success = 1;

    Window root = DefaultRootWindow(context->ctrl_disp);
    for (uint state = 0; state < 256; state++) {
        if ((state == 0 || (state & ~valid_modifiers) != 0) &&
            (state & valid_modifiers) == 0) {
            uint final_modifiers = state | target_modifiers;
            int res = XGrabKey(context->ctrl_disp, key_code, final_modifiers,
                               root, False, GrabModeAsync, GrabModeAsync);
            if (res == BadAccess || res == BadValue) {
                result.success = 0;
            }
        }
    }

    return result;
}

static void process_control_event(DetectContext *context, XEvent *event) {
    if (event->type == MappingNotify) {
        XMappingEvent *e = (XMappingEvent *)event;
        if (e->request == MappingKeyboard) {
            XRefreshKeyboardMapping(e);
        }
        return;
    }

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

    if (context->backend == DETECT_BACKEND_XINPUT2 &&
        event->type == GenericEvent &&
        event->xcookie.extension == context->xi_opcode &&
        XGetEventData(context->ctrl_disp, &event->xcookie)) {
        int evtype = event->xcookie.evtype;
        XIRawEvent *raw = (XIRawEvent *)event->xcookie.data;
        if (raw) {
            // XKeyEvent.state stores the active XKB group in bits 13-14.
            // Preserve it so Russian/English layout switching translates keys
            // correctly through XLookupString.
            unsigned int state = raw->mods.effective |
                                 ((raw->group.effective & 0x3) << 13);
            switch (evtype) {
            case XI_RawKeyPress:
                emit_raw_event(context, KeyPress, raw->detail, state);
                break;
            case XI_RawKeyRelease:
                emit_raw_event(context, KeyRelease, raw->detail, state);
                break;
            case XI_RawButtonPress:
                emit_raw_event(context, ButtonPress, raw->detail, state);
                break;
            case XI_RawButtonRelease:
                emit_raw_event(context, ButtonRelease, raw->detail, state);
                break;
            default:
                break;
            }
        }
        XFreeEventData(context->ctrl_disp, &event->xcookie);
    }
}

int32_t detect_eventloop(void *_context, EventCallback _callback) {
    DetectContext *context = (DetectContext *)_context;
    if (!context) {
        return -1;
    }
    context->event_callback = _callback;

    int ctrl_fd = XConnectionNumber(context->ctrl_disp);

    while (true) {
        if (context->backend == DETECT_BACKEND_XINPUT2) {
            while (XPending(context->ctrl_disp) > 0) {
                XEvent event;
                XNextEvent(context->ctrl_disp, &event);
                process_control_event(context, &event);
            }

            fd_set fds;
            FD_ZERO(&fds);
            FD_SET(ctrl_fd, &fds);
            timeval timeout = {2, 0};
            int ret = select(ctrl_fd + 1, &fds, NULL, NULL, &timeout);
            if (ret < 0) {
                return -2;
            }
            continue;
        }

        int data_fd = XConnectionNumber(context->data_disp);
        fd_set fds;
        FD_ZERO(&fds);
        FD_SET(ctrl_fd, &fds);
        FD_SET(data_fd, &fds);
        timeval timeout = {2, 0};
        int ret = select(std::max(ctrl_fd, data_fd) + 1, &fds, NULL, NULL,
                         &timeout);
        if (ret < 0) {
            return -2;
        }

        if (FD_ISSET(data_fd, &fds)) {
            XRecordProcessReplies(context->data_disp);
            while (XEventsQueued(context->data_disp, QueuedAlready) > 0) {
                XEvent event;
                XNextEvent(context->data_disp, &event);
            }
        }
        if (FD_ISSET(ctrl_fd, &fds)) {
            while (XPending(context->ctrl_disp) > 0) {
                XEvent event;
                XNextEvent(context->ctrl_disp, &event);
                process_control_event(context, &event);
            }
        }
    }

    return 1;
}

int32_t detect_destroy(void *_context) {
    DetectContext *context = (DetectContext *)_context;
    if (!context) {
        return -1;
    }

    if (context->backend == DETECT_BACKEND_RECORD) {
        if (context->x_context) {
            XRecordDisableContext(context->ctrl_disp, context->x_context);
            XRecordFreeContext(context->ctrl_disp, context->x_context);
        }
        if (context->record_range) {
            XFree(context->record_range);
        }
        if (context->data_disp) {
            XCloseDisplay(context->data_disp);
        }
    }

    if (context->ctrl_disp) {
        XCloseDisplay(context->ctrl_disp);
    }
    delete context;
    return 1;
}

void detect_event_callback(XPointer p, XRecordInterceptData *hook) {
    DetectContext *context = (DetectContext *)p;
    if (!context) {
        return;
    }

    if (hook->category != XRecordFromServer) {
        XRecordFreeData(hook);
        return;
    }

    XRecordDatum *data = (XRecordDatum *)hook->data;
    int event_type = data->type;
    int key_code = data->event.u.u.detail;
    unsigned int state = data->event.u.keyButtonPointer.state;
    emit_raw_event(context, event_type, key_code, state);
    XRecordFreeData(hook);
}

int detect_error_callback(Display *, XErrorEvent *error) {
    fprintf(stderr,
            "X11 Reported an error, code: %d, request_code: %d, minor_code: %d\n",
            error->error_code, error->request_code, error->minor_code);
    return 0;
}
