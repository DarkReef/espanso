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
