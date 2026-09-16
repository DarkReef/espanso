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

    // Another X11 client may already own Alt+L (or a lock-key variant). The
    // Astra backend must still register and observe the shortcut because it
    // now recognizes hotkeys from XInput2 raw events instead of XGrabKey.
    XGrabKey(owner, key, alt, DefaultRootWindow(owner), False,
             GrabModeAsync, GrabModeAsync);
    XGrabKey(owner, key, alt | LockMask, DefaultRootWindow(owner), False,
             GrabModeAsync, GrabModeAsync);
    XSync(owner, False);
    assert(forwarded_errors == 0);

    HotKeyRequest request = {};
    request.key_sym = XK_l;
    request.alt = 1;
    const HotKeyResult registration = detect_register_hotkey(context, request, indexes);
    assert(registration.success == 1);
    assert(registration.key_code == key);
    assert(registration.state == alt);
    assert(forwarded_errors == 0);

    // The raw XI2 stream must deliver exactly one hotkey even while the normal
    // X11 grab belongs to another client.
    const KeyCode alt_key = XKeysymToKeycode(owner, XK_Alt_L);
    XTestFakeKeyEvent(owner, alt_key, True, 0);
    XTestFakeKeyEvent(owner, key, True, 0);
    XTestFakeKeyEvent(owner, key, False, 0);
    XTestFakeKeyEvent(owner, alt_key, False, 0);
    XSync(owner, False);
    pump(context);
    assert(hotkeys == 1);

    // Ordinary raw input must remain available after hotkey recognition.
    const KeyCode letter = XKeysymToKeycode(owner, XK_a);
    XTestFakeKeyEvent(owner, letter, True, 0);
    XTestFakeKeyEvent(owner, letter, False, 0);
    XSync(owner, False);
    pump(context);
    assert(letters == 1);

    XUngrabKey(owner, key, AnyModifier, DefaultRootWindow(owner));
    XSync(owner, False);

    // The detector no longer replaces the process-wide X error handler.
    XDestroyWindow(owner, 0);
    XSync(owner, False);
    assert(forwarded_errors == 1);

    detect_destroy(context);
    XCloseDisplay(owner);
    puts("Astra X11: raw hotkey survives XGrabKey conflict; input continues PASS");
}
