#!/usr/bin/env bash
set -Eeuo pipefail

# Full rEspanso portable build for Astra Linux 1.7 / KDE / X11.
# Run this script inside an amd64 Debian 10 (buster) build environment whose
# glibc is 2.28. Nothing is installed on the target Astra workstation: the
# resulting archive is unpacked in the user's home directory and started with
# run.sh.

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  echo 'ERROR: build_pol_run_astra17.sh must run as root inside the Debian 10 build container.' >&2
  exit 1
fi

if [[ ! -r /etc/os-release ]]; then
  echo 'ERROR: cannot identify the build operating system.' >&2
  exit 1
fi
# shellcheck disable=SC1091
. /etc/os-release
if [[ "${ID:-}" != 'debian' || "${VERSION_ID:-}" != 10* ]]; then
  echo "ERROR: this portable ABI build must run on Debian 10/buster, got ${PRETTY_NAME:-unknown}." >&2
  exit 1
fi
if [[ "$(dpkg --print-architecture)" != 'amd64' ]]; then
  echo 'ERROR: this packaging script currently targets amd64/x86_64 only.' >&2
  exit 1
fi

cat >/etc/apt/sources.list <<'APT'
deb http://archive.debian.org/debian buster main
deb http://archive.debian.org/debian buster-updates main
deb http://archive.debian.org/debian-security buster/updates main
APT
printf 'Acquire::Check-Valid-Until false;\n' >/etc/apt/apt.conf.d/99archive

apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  build-essential \
  binutils \
  ca-certificates \
  curl \
  file \
  git \
  libdbus-1-dev \
  libgl1-mesa-dev \
  libssl-dev \
  libwxgtk3.0-gtk3-dev \
  libx11-dev \
  libxcursor-dev \
  libxi-dev \
  libxinerama-dev \
  libxkbcommon-dev \
  libxrandr-dev \
  libxtst-dev \
  pkg-config \
  xauth \
  xdotool \
  xvfb

RUST_TOOLCHAIN="${RESPANSO_RUST_TOOLCHAIN:-1.98.1}"

# CI mounts $HOME/.cargo and $HOME/.rustup from an Actions cache. A cold build
# installs the pinned toolchain; warm builds reuse it without contacting rustup.
if [[ ! -x "$HOME/.cargo/bin/rustup" ]]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain "$RUST_TOOLCHAIN"
fi
export PATH="$HOME/.cargo/bin:$PATH"
if ! rustup toolchain list | grep -q "^$RUST_TOOLCHAIN-"; then
  rustup toolchain install "$RUST_TOOLCHAIN" --profile minimal
fi
rustup default "$RUST_TOOLCHAIN" >/dev/null

bash -n \
  scripts/build_pol_run_astra17.sh \
  scripts/test_astra_x11.sh \
  scripts/test_astra_worker.sh \
  scripts/test_astra_restart.sh \
  scripts/test_astra_search_enter.sh \
  scripts/test_astra_injector_stress.sh
bash scripts/test_astra_x11.sh

echo "Build host: $(ldd --version | head -n1)"
rustc --version
cargo --version

echo '=== static Rust checks (before build) ==='
cargo check --locked -p espanso-inject -p espanso-detect --no-default-features

# X11 core: no Wayland feature. vendored-tls avoids target OpenSSL coupling.
cargo build --locked --release \
  -p espanso --bin espanso \
  --no-default-features \
  --features modulo,vendored-tls

# Exercise the X11 worker itself, the unmanaged daemon/service lifecycle and the
# real wxWidgets search window before running the slower Rust workspace suite.
timeout 30s xvfb-run -a bash scripts/test_astra_worker.sh target/release/espanso
timeout 70s xvfb-run -a bash scripts/test_astra_restart.sh target/release/espanso
timeout 30s xvfb-run -a bash scripts/test_astra_search_enter.sh target/release/espanso
timeout 180s xvfb-run -a env RESPANSO_ASTRA_STRESS_ITERATIONS=12 \
  bash scripts/test_astra_injector_stress.sh target/release/espanso

# Run the complete workspace test suite with the same X11 feature selection
# before packaging anything. This includes espanso-ai MCP/workspace tests and
# Match Studio library tests. Clear external MCP credentials so the tests are
# deterministic even if the invoking shell is configured for an agent.
env -u RESPANSO_MCP_AGENT_ID -u RESPANSO_MCP_TOKEN \
  cargo test --locked --workspace --no-default-features \
  --features espanso/modulo,espanso/vendored-tls

# Match Studio is explicitly eframe + glow + X11 in espanso-editor/Cargo.toml.
cargo build --locked --release \
  -p espanso-editor --bin espanso-editor

CORE="target/release/espanso"
STUDIO="target/release/espanso-editor"
TRAY="target/release/respanso-tray"
OUT="target/pol-run-astra-full"
PACKAGE="rEspanso-pol_run-Astra17-X11-Full-x86_64"
ROOT="$OUT/$PACKAGE"

# Build a small wxWidgets taskbar helper. wxWidgets/GTK is already part of the
# Astra compatibility stack, so this adds a KDE tray icon without installing
# anything on the workstation.
# shellcheck disable=SC2046
g++ -O2 -pipe -std=c++11 scripts/pol_run_tray.cpp \
  $(wx-config --cxxflags) $(wx-config --libs) \
  -o "$TRAY"

rm -rf "$OUT"
mkdir -p \
  "$ROOT/lib" \
  "$ROOT/bin" \
  "$ROOT/config" \
  "$ROOT/match" \
  "$ROOT/packages" \
  "$ROOT/runtime" \
  "$ROOT/scripts" \
  "$ROOT/docs"

cp "$CORE" "$ROOT/rEspanso-core"
cp "$STUDIO" "$ROOT/rEspanso-Match-Studio"
cp "$TRAY" "$ROOT/rEspanso-Tray"
cp /usr/bin/xdotool "$ROOT/bin/xdotool"
cp -L /usr/lib/x86_64-linux-gnu/libxdo.so.3 "$ROOT/lib/libxdo.so.3"
cp espanso/src/res/config/default.yml "$ROOT/config/default.yml"
cp espanso/src/res/config/base.yml "$ROOT/match/base.yml"
cp LICENSE "$ROOT/LICENSE.txt"
if [[ -d docs/respanso ]]; then
  cp -R docs/respanso/. "$ROOT/docs/"
fi

chmod +x "$ROOT/rEspanso-core" "$ROOT/rEspanso-Match-Studio" "$ROOT/rEspanso-Tray"

ldd "$CORE" >"$ROOT/core-build-ldd.txt"
ldd "$STUDIO" >"$ROOT/studio-build-ldd.txt"
ldd "$TRAY" >"$ROOT/tray-build-ldd.txt"

# Bundle ABI-sensitive libraries that are known to differ on Astra. Do not
# bundle glibc or the X11/OpenGL stack: those must remain coupled to the target
# KDE/X11 session.
for ldd_file in \
  "$ROOT/core-build-ldd.txt" \
  "$ROOT/studio-build-ldd.txt" \
  "$ROOT/tray-build-ldd.txt"; do
  while read -r name arrow path rest; do
    case "$name" in
      libwx_*.so*|libstdc++.so.6|libgcc_s.so.1|libssl.so.1.1|libcrypto.so.1.1)
        if [[ "$arrow" == '=>' && -f "$path" ]]; then
          cp -L "$path" "$ROOT/lib/$name"
        fi
        ;;
    esac
  done <"$ldd_file"
done

cat >"$ROOT/check-x11.sh" <<'CHECK_X11'
#!/usr/bin/env bash
set -Eeuo pipefail
if [[ -z "${DISPLAY:-}" ]]; then
  echo 'rEspanso X11: DISPLAY не задан. Запустите приложение из пользовательской X11-сессии.' >&2
  exit 1
fi
if [[ -n "${XDG_SESSION_TYPE:-}" && "${XDG_SESSION_TYPE,,}" != 'x11' ]]; then
  echo "rEspanso X11: текущая сессия '${XDG_SESSION_TYPE}' не является X11." >&2
  echo 'Войдите в KDE/X11-сессию; XWayland не считается поддерживаемым режимом для перехвата ввода.' >&2
  exit 1
fi
CHECK_X11

cat >"$ROOT/start-tray.sh" <<'TRAY_RUN'
#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$ROOT/check-x11.sh"
PIDFILE="$ROOT/runtime/tray.pid"
LOGFILE="$ROOT/runtime/tray.log"

export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PATH="$ROOT/bin:$PATH"
mkdir -p "$ROOT/runtime"

if [[ -s "$PIDFILE" ]]; then
  old_pid="$(cat "$PIDFILE" 2>/dev/null || true)"
  if [[ "$old_pid" =~ ^[0-9]+$ ]] && kill -0 "$old_pid" 2>/dev/null; then
    exit 0
  fi
  rm -f "$PIDFILE"
fi

nohup "$ROOT/rEspanso-Tray" "$ROOT" >>"$LOGFILE" 2>&1 </dev/null &
tray_pid=$!
printf '%s\n' "$tray_pid" >"$PIDFILE"

sleep 0.35
if ! kill -0 "$tray_pid" 2>/dev/null; then
  rm -f "$PIDFILE"
  echo "rEspanso: значок не запустился; подробности: $LOGFILE" >&2
  exit 1
fi
TRAY_RUN

cat >"$ROOT/run.sh" <<'RUN'
#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$ROOT/check-x11.sh"
CORE="$ROOT/rEspanso-core"
STUDIO="$ROOT/rEspanso-Match-Studio"

export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PATH="$ROOT/bin:$PATH"
export ESPANSO_CONFIG_DIR="$ROOT"
export ESPANSO_PACKAGE_DIR="$ROOT/packages"
export ESPANSO_RUNTIME_DIR="$ROOT/runtime"

mkdir -p "$ROOT/config" "$ROOT/match" "$ROOT/packages" "$ROOT/runtime" "$ROOT/scripts"

core_args=(
  --config_dir "$ROOT"
  --package_dir "$ROOT/packages"
  --runtime_dir "$ROOT/runtime"
)

if ! "$CORE" "${core_args[@]}" service status >/dev/null 2>&1; then
  "$CORE" "${core_args[@]}" service start --unmanaged || {
    echo "Не удалось запустить движок rEspanso. Запускаю диагностику..." >&2
    "$ROOT/diagnose.sh" || true
    exit 1
  }
fi

"$ROOT/start-tray.sh" || true
exec "$STUDIO" --config-dir "$ROOT"
RUN

cat >"$ROOT/studio.sh" <<'STUDIO_RUN'
#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$ROOT/check-x11.sh"
export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PATH="$ROOT/bin:$PATH"
exec "$ROOT/rEspanso-Match-Studio" --config-dir "$ROOT"
STUDIO_RUN

cat >"$ROOT/start-engine.sh" <<'ENGINE'
#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$ROOT/check-x11.sh"
export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PATH="$ROOT/bin:$PATH"

"$ROOT/rEspanso-core" \
  --config_dir "$ROOT" \
  --package_dir "$ROOT/packages" \
  --runtime_dir "$ROOT/runtime" \
  service start --unmanaged

"$ROOT/start-tray.sh" || true
ENGINE

cat >"$ROOT/stop.sh" <<'STOP'
#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PIDFILE="$ROOT/runtime/tray.pid"
export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PATH="$ROOT/bin:$PATH"

"$ROOT/rEspanso-core" \
  --config_dir "$ROOT" \
  --package_dir "$ROOT/packages" \
  --runtime_dir "$ROOT/runtime" \
  service stop || true

if [[ -s "$PIDFILE" ]]; then
  tray_pid="$(cat "$PIDFILE" 2>/dev/null || true)"
  if [[ "$tray_pid" =~ ^[0-9]+$ ]]; then
    kill "$tray_pid" 2>/dev/null || true
  fi
fi
rm -f "$PIDFILE"
STOP

cat >"$ROOT/diagnose.sh" <<'DIAG'
#!/usr/bin/env bash
set +e
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PATH="$ROOT/bin:$PATH"

echo '===== rEspanso pol_run Astra diagnostics ====='
echo
printf 'System: '
uname -a
printf 'glibc:  '
ldd --version | head -n1
printf 'DISPLAY: %s\n' "${DISPLAY:-<empty>}"
printf 'XDG_SESSION_TYPE: %s\n' "${XDG_SESSION_TYPE:-<empty>}"
printf 'DBUS_SESSION_BUS_ADDRESS: %s\n' "${DBUS_SESSION_BUS_ADDRESS:-<empty>}"
echo

echo '--- bundled libraries ---'
ls -lh "$ROOT/lib" || true
echo

echo '--- core unresolved libraries ---'
ldd "$ROOT/rEspanso-core" | grep 'not found' || echo none
echo

echo '--- Match Studio unresolved libraries ---'
ldd "$ROOT/rEspanso-Match-Studio" | grep 'not found' || echo none
echo

echo '--- tray unresolved libraries ---'
ldd "$ROOT/rEspanso-Tray" | grep 'not found' || echo none
echo

echo '--- tray state ---'
if [[ -s "$ROOT/runtime/tray.pid" ]]; then
  tray_pid="$(cat "$ROOT/runtime/tray.pid" 2>/dev/null || true)"
  if [[ "$tray_pid" =~ ^[0-9]+$ ]] && kill -0 "$tray_pid" 2>/dev/null; then
    echo "running (pid $tray_pid)"
  else
    echo "stale pid file ($tray_pid)"
  fi
else
  echo 'not started'
fi
if [[ -f "$ROOT/runtime/tray.log" ]]; then
  echo '--- tray log tail ---'
  tail -n 40 "$ROOT/runtime/tray.log" || true
fi
echo

echo '--- engine log tail ---'
tail -n 80 "$ROOT/runtime/espanso.log" 2>/dev/null || true
echo '--- startup / native X11 log tail ---'
tail -n 80 "$ROOT/runtime/startup.log" 2>/dev/null || true
echo
echo '--- core version ---'
"$ROOT/rEspanso-core" --version || true
echo

echo '--- portable engine status ---'
"$ROOT/rEspanso-core" \
  --config_dir "$ROOT" \
  --package_dir "$ROOT/packages" \
  --runtime_dir "$ROOT/runtime" \
  service status || true
DIAG

cat >"$ROOT/mcp.sh" <<'MCP_RUN'
#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$ROOT/rEspanso-Match-Studio" --mcp --config-dir "$ROOT"
MCP_RUN

# Diagnostics is part of the package itself. Keeping package assembly in this
# script means CI only verifies the archive instead of mutating and rebuilding it.
install -m 0755 scripts/export_logs.sh "$ROOT/export-logs.sh"

cat >"$ROOT/README-FIRST.txt" <<'README'
rEspanso pol_run — full portable build for Astra Linux 1.7 / KDE / X11 / x86_64

No sudo and no system installation are required on the workstation.

FIRST START
  chmod +x *.sh rEspanso-core rEspanso-Match-Studio rEspanso-Tray
  ./run.sh

run.sh:
  1. verifies that the current desktop session is X11;
  2. starts the rEspanso engine in unmanaged user mode;
  3. starts the persistent KDE/GTK tray icon;
  4. opens the full Match Studio GUI.

The working configuration stays inside this directory:
  config/   global settings
  match/    YAML matches
  scripts/  Rhai scripts
  packages/ local packages
  runtime/  daemon/tray state and logs

Hotkeys:
  Ctrl+Alt+M          execute match from selected text (primary)
  Ctrl+Alt+Shift+M    execute match from selected text (fallback)
  Alt+L               AI rewrite selected text

Useful commands:
  ./studio.sh        open only Match Studio
  ./start-engine.sh  start engine + tray icon
  ./start-tray.sh    restore only the tray icon
  ./stop.sh          stop engine + tray icon
  ./diagnose.sh      ABI/library/display/tray diagnostics
  ./export-logs.sh    create a privacy-aware diagnostics archive in diagnostics/
  ./mcp.sh           MCP stdio server (agent credentials come from environment)

Diagnostics archives intentionally exclude configuration and match contents.
Selected-text and clipboard contents are not intentionally logged.

MCP agent connection:
  export RESPANSO_MCP_AGENT_ID='<id from Studio>'
  export RESPANSO_MCP_TOKEN='<token shown by Studio>'
  ./mcp.sh

Compatibility design:
  - binaries are built in Debian 10 / glibc 2.28;
  - core and Studio use X11; Studio uses OpenGL/glow, not WGPU;
  - native X11 detection uses XInput2 instead of the legacy RECORD extension;
  - wxWidgets 3.0 and selected ABI-sensitive runtimes are bundled in ./lib;
  - glibc and the target X11/OpenGL/GTK stack are deliberately NOT replaced.
README

chmod +x \
  "$ROOT/check-x11.sh" \
  "$ROOT/run.sh" \
  "$ROOT/studio.sh" \
  "$ROOT/start-engine.sh" \
  "$ROOT/start-tray.sh" \
  "$ROOT/stop.sh" \
  "$ROOT/diagnose.sh" \
  "$ROOT/export-logs.sh" \
  "$ROOT/mcp.sh" \
  "$ROOT/bin/xdotool"

# Fail packaging immediately if any generated launcher has a shell syntax error.
bash -n \
  "$ROOT/check-x11.sh" \
  "$ROOT/run.sh" \
  "$ROOT/studio.sh" \
  "$ROOT/start-engine.sh" \
  "$ROOT/start-tray.sh" \
  "$ROOT/stop.sh" \
  "$ROOT/diagnose.sh" \
  "$ROOT/export-logs.sh" \
  "$ROOT/mcp.sh"

# Record and enforce the maximum glibc symbol version required by all binaries.
for pair in "core:$CORE" "studio:$STUDIO" "tray:$TRAY" "xdotool:$ROOT/bin/xdotool"; do
  label="${pair%%:*}"
  binary="${pair#*:}"
  objdump -T "$binary" 2>/dev/null \
    | grep -o 'GLIBC_[0-9.]*' \
    | sort -Vu >"$ROOT/${label}-glibc-required.txt" || true

  max_glibc="$(sed 's/GLIBC_//' "$ROOT/${label}-glibc-required.txt" | sort -V | tail -n1)"
  echo "$label maximum required GLIBC: ${max_glibc:-none}"
  if [[ -n "$max_glibc" && "$(printf '%s\n%s\n' "$max_glibc" '2.28' | sort -V | tail -n1)" != '2.28' ]]; then
    echo "ERROR: $label requires GLIBC newer than 2.28" >&2
    exit 1
  fi
done

LD_LIBRARY_PATH="$ROOT/lib" ldd "$ROOT/rEspanso-core" >"$ROOT/core-packaged-ldd.txt"
LD_LIBRARY_PATH="$ROOT/lib" ldd "$ROOT/rEspanso-Match-Studio" >"$ROOT/studio-packaged-ldd.txt"
LD_LIBRARY_PATH="$ROOT/lib" ldd "$ROOT/rEspanso-Tray" >"$ROOT/tray-packaged-ldd.txt"
LD_LIBRARY_PATH="$ROOT/lib" ldd "$ROOT/bin/xdotool" >"$ROOT/xdotool-packaged-ldd.txt"

for packaged_ldd in \
  "$ROOT/core-packaged-ldd.txt" \
  "$ROOT/studio-packaged-ldd.txt" \
  "$ROOT/tray-packaged-ldd.txt" \
  "$ROOT/xdotool-packaged-ldd.txt"; do
  if grep -q 'not found' "$packaged_ldd"; then
    cat "$packaged_ldd"
    echo "ERROR: package has unresolved libraries: $packaged_ldd" >&2
    exit 1
  fi
done

# Smoke tests that do not require a real X server.
LD_LIBRARY_PATH="$ROOT/lib" "$ROOT/rEspanso-core" --version

# Modern MCP 2026-07-28 has no ping method. Probe server/discover with the
# mandatory per-request _meta envelope. Discover advertises modern revisions;
# legacy compatibility is tested separately through initialize below.
modern_probe="$(
  printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{},"io.modelcontextprotocol/clientInfo":{"name":"astra-build-smoke","version":"1"}}}}' \
    | env -u RESPANSO_MCP_AGENT_ID -u RESPANSO_MCP_TOKEN "$ROOT/mcp.sh"
)"
printf '%s\n' "$modern_probe" | grep -q '"resultType":"complete"'
printf '%s\n' "$modern_probe" | grep -q '"supportedVersions":\["2026-07-28"\]'
printf '%s\n' "$modern_probe" | grep -q '"io.modelcontextprotocol/serverInfo"'

# Protect backward compatibility: legacy MCP must still initialize and its ping
# result is the legacy empty object, not the modern resultType envelope.
legacy_probe="$(
  printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"astra-build-smoke","version":"1"}}}' \
    '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
    '{"jsonrpc":"2.0","id":2,"method":"ping"}' \
    | env -u RESPANSO_MCP_AGENT_ID -u RESPANSO_MCP_TOKEN "$ROOT/mcp.sh"
)"
printf '%s\n' "$legacy_probe" | grep -q '"protocolVersion":"2025-06-18"'
printf '%s\n' "$legacy_probe" | grep -q '"result":{}'

# Modern ping was removed in the 2026 era and must be rejected.
modern_ping="$(
  printf '%s\n' \
    '{"jsonrpc":"2.0","id":3,"method":"ping","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}' \
    | env -u RESPANSO_MCP_AGENT_ID -u RESPANSO_MCP_TOKEN "$ROOT/mcp.sh"
)"
printf '%s\n' "$modern_ping" | grep -q '"code":-32601'

tar -C "$OUT" -czf "$OUT/$PACKAGE.tar.gz" "$PACKAGE"
sha256sum "$OUT/$PACKAGE.tar.gz" >"$OUT/$PACKAGE.sha256"

ls -lh "$OUT/$PACKAGE.tar.gz" "$OUT/$PACKAGE.sha256"
