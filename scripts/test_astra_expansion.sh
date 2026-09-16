#!/usr/bin/env bash
# End-to-end Astra/X11 regression: polling detector -> matcher -> deletion -> injector.
set -Eeuo pipefail

CORE="$(realpath "$1")"
TEST_ROOT="$(mktemp -d)"
CONFIG="$TEST_ROOT/config-root"
TARGET_PID=""
WORKER_PID=""

cleanup() {
  if [[ -n "$WORKER_PID" ]]; then kill "$WORKER_PID" 2>/dev/null || true; fi
  if [[ -n "$TARGET_PID" ]]; then kill "$TARGET_PID" 2>/dev/null || true; fi
  rm -rf "$TEST_ROOT"
}
trap cleanup EXIT

mkdir -p "$CONFIG/config" "$CONFIG/match" "$CONFIG/packages" "$CONFIG/runtime"
cat >"$CONFIG/config/default.yml" <<'YAML'
show_icon: false
YAML
cat >"$CONFIG/match/base.yml" <<'YAML'
matches:
  - trigger: ":x11test"
    replace: "RESPANSO_EXPANSION_OK"
YAML

cat >"$TEST_ROOT/target.cpp" <<'CPP'
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/keysym.h>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    Display *display = XOpenDisplay(nullptr);
    if (!display) return 3;
    int screen = DefaultScreen(display);
    Window root = RootWindow(display, screen);
    Window window = XCreateSimpleWindow(display, root, 10, 10, 640, 120, 1,
                                        BlackPixel(display, screen),
                                        WhitePixel(display, screen));
    XStoreName(display, window, "rEspanso expansion regression target");
    XSelectInput(display, window, KeyPressMask | StructureNotifyMask);
    XMapWindow(display, window);
    XFlush(display);

    for (;;) {
        XEvent event;
        XNextEvent(display, &event);
        if (event.type == MapNotify) break;
    }
    XSetInputFocus(display, window, RevertToParent, CurrentTime);
    XFlush(display);

    FILE *ready = std::fopen(argv[2], "w");
    if (!ready) return 4;
    std::fprintf(ready, "%lu\n", static_cast<unsigned long>(window));
    std::fclose(ready);

    std::string value;
    for (;;) {
        XEvent event;
        XNextEvent(display, &event);
        if (event.type != KeyPress) continue;

        KeySym sym = NoSymbol;
        char buffer[64] = {0};
        int count = XLookupString(&event.xkey, buffer, sizeof(buffer) - 1,
                                  &sym, nullptr);
        if (sym == XK_BackSpace) {
            if (!value.empty()) value.pop_back();
            continue;
        }
        if (sym == XK_Return || sym == XK_KP_Enter) {
            FILE *result = std::fopen(argv[1], "w");
            if (!result) return 5;
            std::fwrite(value.data(), 1, value.size(), result);
            std::fclose(result);
            break;
        }
        if (count > 0) value.append(buffer, static_cast<size_t>(count));
    }

    XDestroyWindow(display, window);
    XCloseDisplay(display);
    return 0;
}
CPP

g++ -O2 -std=c++11 "$TEST_ROOT/target.cpp" -lX11 -o "$TEST_ROOT/target"

RESULT="$TEST_ROOT/result.txt"
READY="$TEST_ROOT/ready.txt"
"$TEST_ROOT/target" "$RESULT" "$READY" >"$TEST_ROOT/target.log" 2>&1 &
TARGET_PID=$!

for _ in $(seq 1 100); do
  [[ -s "$READY" ]] && break
  if ! kill -0 "$TARGET_PID" 2>/dev/null; then
    cat "$TEST_ROOT/target.log" >&2 || true
    echo 'Expansion target exited before becoming ready' >&2
    exit 1
  fi
  sleep 0.05
done
[[ -s "$READY" ]] || { echo 'Expansion target did not become ready' >&2; exit 1; }

export RESPANSO_X11_INJECTOR=xdotool
"$CORE" --config_dir "$CONFIG" \
  --package_dir "$CONFIG/packages" \
  --runtime_dir "$CONFIG/runtime" \
  worker >"$TEST_ROOT/worker.log" 2>&1 &
WORKER_PID=$!

for _ in $(seq 1 160); do
  if ! kill -0 "$WORKER_PID" 2>/dev/null; then
    cat "$TEST_ROOT/worker.log" >&2 || true
    cat "$CONFIG/runtime/espanso.log" >&2 2>/dev/null || true
    cat "$CONFIG/runtime/x11-native.log" >&2 2>/dev/null || true
    echo 'Worker exited before expansion test became ready' >&2
    exit 1
  fi
  if grep -q 'X11XDOToolInjector init success' "$TEST_ROOT/worker.log" 2>/dev/null || \
     grep -q 'X11XDOToolInjector init success' "$CONFIG/runtime/espanso.log" 2>/dev/null; then
    break
  fi
  sleep 0.05
done

if ! grep -q 'X11XDOToolInjector init success' "$TEST_ROOT/worker.log" 2>/dev/null && \
   ! grep -q 'X11XDOToolInjector init success' "$CONFIG/runtime/espanso.log" 2>/dev/null; then
  cat "$TEST_ROOT/worker.log" >&2 || true
  cat "$CONFIG/runtime/espanso.log" >&2 2>/dev/null || true
  echo 'xdotool-safe injector did not initialize' >&2
  exit 1
fi

# The Astra build must explicitly use the same keyboard backend proven by the
# real workstation logs. A green XI2-only Xvfb test is not sufficient because
# some Astra/KDE sessions advertise XI2 RawKey support without delivering it.
for _ in $(seq 1 80); do
  if grep -q 'keyboard=xquerykeymap-poll' "$CONFIG/runtime/x11-native.log" 2>/dev/null; then
    break
  fi
  sleep 0.05
done
if ! grep -q 'keyboard=xquerykeymap-poll' "$CONFIG/runtime/x11-native.log" 2>/dev/null; then
  cat "$TEST_ROOT/worker.log" >&2 || true
  cat "$CONFIG/runtime/x11-native.log" >&2 2>/dev/null || true
  echo 'Astra keyboard backend is not XQueryKeymap polling' >&2
  exit 1
fi

# The target set itself as the X11 input focus. XTEST-generated keystrokes are
# visible to XQueryKeymap just like physical key state changes.
xdotool type --delay 55 ':x11test'
sleep 1
xdotool key Return

for _ in $(seq 1 100); do
  [[ -f "$RESULT" ]] && break
  sleep 0.05
done

cat "$TEST_ROOT/worker.log" || true
cat "$CONFIG/runtime/espanso.log" 2>/dev/null || true
cat "$CONFIG/runtime/x11-native.log" 2>/dev/null || true

[[ -f "$RESULT" ]] || { echo 'No result from X11 target' >&2; exit 1; }
ACTUAL="$(cat "$RESULT")"
if [[ "$ACTUAL" != 'RESPANSO_EXPANSION_OK' ]]; then
  echo "Expansion mismatch: got '$ACTUAL'" >&2
  exit 1
fi

grep -q 'XInitThreads=ok' "$CONFIG/runtime/x11-native.log"
grep -q 'keyboard=xquerykeymap-poll' "$CONFIG/runtime/x11-native.log"
echo 'Astra X11: XQueryKeymap detector -> matcher -> erase -> xdotool injector expansion PASS'
