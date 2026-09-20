# Astra 1.7 X11 stability fixes

The portable build selects `espanso-detect/src/x11/native_astra.cpp`, not
`native.cpp`. Both files previously existed, but a fix to the latter alone does
not change the shipped Astra detector.

## Fixed failure paths

- `XGrabKey` / `BadAccess`: wait for the asynchronous response, intercept only
  that request's expected conflict, roll back successful lock-mask variants,
  and report the unavailable shortcut through the existing Rust logger. Other
  shortcuts and raw input remain available. A shortcut owned by Fly or another
  program is not taken over; choose a free combination in the application that
  owns it, or use Studio's AI panel directly.
- Initialize Xlib threading before GTK and the worker's detector/injector threads.
- Use `poll` for the input connection; retry interrupted waits and report broken
  connections without overflowing `fd_set` for high descriptor numbers.
- Long Linux runtime paths: bind/connect using an open directory descriptor
  through `/proc/self/fd`. The socket stays inside the chosen runtime folder;
  no shared world-writable runtime directory or shortened configuration path is
  introduced. Requires the normal Linux proc filesystem.
- Unmanaged startup writes native stderr to `runtime/startup.log` (created with
  mode 0600), rotates it on the next start above 1 MiB, and diagnostics show both
  that file and `runtime/espanso.log`. Worker signals are logged explicitly.
- Five consecutive failures before 60 seconds of stable uptime stop the daemon
  instead of restarting forever. `stop.sh` can contact the daemon even when the
  worker has already crashed; it does not kill unrelated Espanso installations.
- Session detection uses XDG/display variables without a potentially blocking
  shell/logind query.

## Regression gates

`scripts/test_astra_x11.sh` compiles the actual Astra detector and runs against
Xvfb: occupied Alt+L with Caps Lock, rollback, successful subsequent registration,
exactly one hotkey event, continued raw keyboard events, and preservation of the
previous handler for unrelated errors.

`scripts/test_astra_worker.sh` starts the complete core under Xvfb with a long
Unicode runtime path and requires it to remain alive for ten seconds. The Rust
IPC regression separately tests an actual connection over a long Unicode path.
The Debian 10 portable build runs these gates plus the workspace tests.

Xvfb verifies regression behavior, not Astra's particular Fly theme, clipboard
clients, X server policy or real display driver. Final workstation acceptance:
start `run.sh`, expand a test trigger in a text editor, switch RU/EN layouts,
open Studio and AI preview with Alt+L (if free), then stop/start the same folder.
If there is a failure, run `diagnose.sh`; do not send API key files or clinical
text. AI provider requests still require configured credentials and review.


## AstraSafeInjector

The current Astra build uses a configurable X11 safety policy rather than one
hard-coded injection behaviour. The user-facing profiles are `Safe`,
`Balanced`, `Fast` and `Legacy`; the shipped Astra portable config defaults
to `Safe`.

The main invariants are:

- Safe/Balanced wait for physically held Ctrl/Alt/Shift/Meta keys to be released
  instead of synthesizing key-up events for every pressed key.
- The rendered expansion target is pinned before UI rendering and consumed once.
  With focus guard enabled, injection aborts if that target cannot be restored
  and verified after bounded retries.
- The fast libxdo path has a circuit breaker. Repeated backend errors move later
  operations to the XTest path for the current worker session.
- Optional libxdo self-healing recreates the backend context after an error.
  The failed operation is not blindly replayed, preventing duplicate partial
  clinical text.
- `Legacy` is a compatibility escape hatch that preserves the old
  release-all-pressed-keys behaviour. It is not the recommended Astra profile.

These values are defined once in `espanso-config` and reused by runtime and
Match Studio, so the UI cannot silently drift from the actual injector defaults.

Useful log markers:

- `[rESP-HOTKEY]` — registration/detection backend.
- `[rESP-FOCUS]` — one-shot target pin/restore.
- `[rESP-INJECT]` — injection operation and selected backend.
- `[rESP-INJECT-BREAKER]` — fast-path circuit breaker activation.
- `[rESP-INJECT-HEAL]` — libxdo context recreation.

`scripts/test_astra_injector_stress.sh` repeats the complete X11 expansion path
under Xvfb. It is a regression gate, not a substitute for a real workstation
soak test with the target MИС, RU/EN layout switching, forms and session
restart/suspend behaviour.
