/*
 * X11 runtime hardening and diagnostics for rEspanso pol_run.
 *
 * XInitThreads must be called before any other Xlib entry point in the
 * process. The worker invokes detect_prepare_x11_runtime() before creating
 * the tray, detector, injector or clipboard backends.
 */

#include <X11/Xlib.h>

#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static FILE *g_native_log = nullptr;

static void write_native_log(const char *line) {
    if (!line) {
        return;
    }

    fputs(line, stderr);
    fflush(stderr);

    if (g_native_log) {
        fputs(line, g_native_log);
        fflush(g_native_log);
    }
}

extern "C" void detect_write_x11_log(const char *line) {
    write_native_log(line);
}

static int respanso_x11_error_handler(Display *display, XErrorEvent *error) {
    char error_text[256] = {0};
    if (display && error) {
        XGetErrorText(display, error->error_code, error_text,
                      sizeof(error_text));
    }

    char line[768] = {0};
    snprintf(line, sizeof(line),
             "[rESP-X11-ERROR] pid=%ld error=%u(%s) request=%u minor=%u "
             "resource=0x%lx serial=%lu\n",
             static_cast<long>(getpid()),
             error ? static_cast<unsigned int>(error->error_code) : 0U,
             error_text[0] ? error_text : "unknown",
             error ? static_cast<unsigned int>(error->request_code) : 0U,
             error ? static_cast<unsigned int>(error->minor_code) : 0U,
             error ? error->resourceid : 0UL,
             error ? error->serial : 0UL);
    write_native_log(line);

    // X protocol errors such as BadAccess from XGrabKey are non-fatal for
    // rEspanso. Returning lets Xlib continue while preserving diagnostics.
    return 0;
}

static int respanso_x11_io_error_handler(Display *display) {
    const int saved_errno = errno;
    const int fd = display ? ConnectionNumber(display) : -1;

    char line[768] = {0};
    snprintf(line, sizeof(line),
             "[rESP-XIO-FATAL] pid=%ld errno=%d(%s) fd=%d\n",
             static_cast<long>(getpid()), saved_errno,
             strerror(saved_errno), fd);
    write_native_log(line);

    // Xlib treats I/O errors as fatal and may terminate the process after the
    // callback returns. The purpose of this callback is to make that failure
    // visible in x11-native.log and stderr before termination.
    return 0;
}

extern "C" int32_t detect_prepare_x11_runtime(const char *log_path) {
    if (log_path && log_path[0] != '\0') {
        g_native_log = fopen(log_path, "a");
        if (!g_native_log) {
            char line[512] = {0};
            snprintf(line, sizeof(line),
                     "[rESP-X11] unable to open native log '%s': errno=%d(%s)\n",
                     log_path, errno, strerror(errno));
            write_native_log(line);
        }
    }

    // This MUST be the first Xlib call made by the worker process.
    const int thread_support = XInitThreads();

    XSetErrorHandler(&respanso_x11_error_handler);
    XSetIOErrorHandler(&respanso_x11_io_error_handler);

    char line[512] = {0};
    snprintf(line, sizeof(line),
             "[rESP-X11] pid=%ld XInitThreads=%s native_log=%s\n",
             static_cast<long>(getpid()),
             thread_support != 0 ? "ok" : "FAILED",
             log_path && log_path[0] != '\0' ? log_path : "stderr-only");
    write_native_log(line);

    return thread_support != 0 ? 1 : 0;
}
