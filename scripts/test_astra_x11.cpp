// Regression tests against the actual native_astra.cpp selected by build.rs.
// The Astra keyboard path intentionally uses XQueryKeymap polling because some
// real Astra/KDE X11 sessions advertise XI2 RawKey support but do not deliver
// those raw keyboard events to rEspanso.
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

static void pump(DetectContext *context, int iterations = 20) {
    for (int i = 0; i < iterations; ++i) {
        while (XPending(context->display)) {
            XEvent event;
            XNextEvent(context->display, &event);
            process_x11_event(context, &event);
        }
        poll_keyboard(context);
        usleep(2000);
    }
}

static void key_state(Display *display, DetectContext *context,
                      KeyCode key, bool pressed) {
    XTestFakeKeyEvent(display, key, pressed ? True : False, 0);
    XSync(display, False);
    // Keep the state visible for several 2 ms polls. This models a physical
    // key press and proves that detection comes from XQueryKeymap, not XI2.
    pump(context, 8);
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

    // Simulate KDE already owning Alt+L. XGrabKey must report BadAccess to the
    // rEspanso registration, but registration itself must remain successful so
    // the shortcut can be recognized by the XQueryKeymap fallback.
    XGrabKey(owner, key, alt, DefaultRootWindow(owner), False,
             GrabModeAsync, GrabModeAsync);
    XGrabKey(owner, key, alt | LockMask, DefaultRootWindow(owner), False,
             GrabModeAsync, GrabModeAsync);
    XSync(owner, False);
    assert(forwarded_errors == 0);

    HotKeyRequest request = {};
    request.key_sym = XK_l;
    request.alt = 1;
    const HotKeyResult registration =
        detect_register_hotkey(context, request, indexes);
    assert(registration.success == 1);
    assert(registration.key_code == key);
    assert(registration.state == alt);
    assert(forwarded_errors == 0);

    bool fallback_registered = false;
    for (const auto &hotkey : context->hotkeys) {
        if (hotkey.key_code == key && hotkey.state == alt &&
            hotkey.poll_fallback) {
            fallback_registered = true;
            break;
        }
    }
    assert(fallback_registered);

    // Hold Alt, then L, so XQueryKeymap observes both transitions. The owner
    // client still owns the X11 grab, therefore the rEspanso hotkey can only be
    // produced by the polling fallback.
    const KeyCode alt_key = XKeysymToKeycode(owner, XK_Alt_L);
    key_state(owner, context, alt_key, true);
    key_state(owner, context, key, true);
    key_state(owner, context, key, false);
    key_state(owner, context, alt_key, false);
    assert(hotkeys == 1);
    assert(context->hotkeys_polled == 1);

    // Ordinary keyboard input must also be observed by polling and translated
    // into the same InputEvent shape consumed by the Rust matcher.
    const KeyCode letter = XKeysymToKeycode(owner, XK_a);
    key_state(owner, context, letter, true);
    key_state(owner, context, letter, false);
    assert(letters == 1);
    assert(context->presses > 0);
    assert(context->translated > 0);

    XUngrabKey(owner, key, AnyModifier, DefaultRootWindow(owner));
    XSync(owner, False);

    // Registration must restore the inherited process-wide X error handler.
    XDestroyWindow(owner, 0);
    XSync(owner, False);
    assert(forwarded_errors == 1);

    detect_destroy(context);
    XCloseDisplay(owner);
    puts("Astra X11: XQueryKeymap keyboard + hotkey fallback survives XGrabKey conflict PASS");
}
