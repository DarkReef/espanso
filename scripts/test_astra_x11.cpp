// Regression tests against the actual native_astra.cpp selected by build.rs.
#include "../espanso-detect/src/x11/native_astra.cpp"
#include <X11/extensions/XTest.h>
#include <assert.h>
#include <unistd.h>

static int forwarded_errors = 0;
static int hotkeys = 0;
static int letters = 0;
static int inherited_handler(Display *, XErrorEvent *) {
    ++forwarded_errors;
    return 0;
}
static void input_callback(void *, InputEvent event) {
    if (event.event_type == INPUT_EVENT_TYPE_HOTKEY) ++hotkeys;
    if (event.event_type == INPUT_EVENT_TYPE_KEYBOARD &&
        event.status == INPUT_STATUS_PRESSED && event.key_sym == XK_a) ++letters;
}
static void pump(DetectContext *context) {
    for (int i = 0; i < 100; ++i) {
        while (XPending(context->display)) {
            XEvent event;
            XNextEvent(context->display, &event);
            process_event(context, &event);
        }
        usleep(1000);
    }
}
int main() {
    assert(XInitThreads());
    XSetErrorHandler(inherited_handler);
    Display *owner = XOpenDisplay(nullptr);
    assert(owner);
    int32_t error = 0;
    auto *context = static_cast<DetectContext *>(detect_initialize(nullptr, &error));
    assert(context);
    context->event_callback = input_callback;
    const auto indexes = detect_get_modifier_indexes(context);
    const unsigned int alt = 1U << indexes.alt;
    const KeyCode key = XKeysymToKeycode(owner, XK_l);
    // Conflict on a later lock variant exercises rollback of earlier grabs.
    XGrabKey(owner, key, alt | LockMask, DefaultRootWindow(owner), False,
             GrabModeAsync, GrabModeAsync);
    XSync(owner, False);
    assert(forwarded_errors == 0);
    HotKeyRequest request = {};
    request.key_sym = XK_l;
    request.alt = 1;
    assert(detect_register_hotkey(context, request, indexes).success == 0);
    assert(forwarded_errors == 0); // expected BadAccess never reaches GTK
    // Rollback released the plain Alt+L variant for another X11 client.
    XGrabKey(owner, key, alt, DefaultRootWindow(owner), False,
             GrabModeAsync, GrabModeAsync);
    XSync(owner, False);
    assert(forwarded_errors == 0);
    XUngrabKey(owner, key, AnyModifier, DefaultRootWindow(owner));
    XSync(owner, False);
    assert(detect_register_hotkey(context, request, indexes).success == 1);
    const KeyCode alt_key = XKeysymToKeycode(owner, XK_Alt_L);
    XTestFakeKeyEvent(owner, alt_key, True, 0);
    XTestFakeKeyEvent(owner, key, True, 0);
    XTestFakeKeyEvent(owner, key, False, 0);
    XTestFakeKeyEvent(owner, alt_key, False, 0);
    XSync(owner, False);
    pump(context);
    assert(hotkeys == 1);
    const KeyCode letter = XKeysymToKeycode(owner, XK_a);
    XTestFakeKeyEvent(owner, letter, True, 0);
    XTestFakeKeyEvent(owner, letter, False, 0);
    XSync(owner, False);
    pump(context);
    assert(letters == 1); // raw input still arrives after a grab conflict
    XDestroyWindow(owner, 0); // unrelated errors retain the original handler
    XSync(owner, False);
    assert(forwarded_errors == 1);
    detect_destroy(context);
    XCloseDisplay(owner);
    puts("Astra X11: conflict, rollback, hotkey, raw input, error forwarding PASS");
}
