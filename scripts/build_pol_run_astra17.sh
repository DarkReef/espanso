#!/usr/bin/env bash
set -Eeuo pipefail

# Full rEspanso portable build for Astra Linux 1.7 / KDE / X11.
# The script is intended to run inside Debian 10 (buster), whose glibc is 2.28.
# Nothing here is installed on the target Astra workstation: the resulting
# archive is unpacked in the user's home directory and started with run.sh.

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
  pkg-config

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --profile minimal --default-toolchain stable
# shellcheck disable=SC1091
. "$HOME/.cargo/env"

echo "Build host: $(ldd --version | head -n1)"
rustc --version
cargo --version

# The pol_run branch intentionally uses the X11/OpenGL (glow) renderer for
# Match Studio. Cargo.lock from the upstream unstable branch was generated for
# the WGPU build, therefore this branch allows Cargo to extend the lockfile in
# the ephemeral CI workspace with the glow-only optional dependencies.
cargo build --release \
  -p espanso --bin espanso \
  --no-default-features \
  --features modulo,vendored-tls

cargo build --release \
  -p espanso-editor --bin espanso-editor

CORE="target/release/espanso"
STUDIO="target/release/espanso-editor"
TRAY="target/release/respanso-tray"
OUT="target/pol-run-astra-full"
PACKAGE="rEspanso-pol_run-Astra17-X11-Full-x86_64"
ROOT="$OUT/$PACKAGE"

# Build a tiny wxWidgets taskbar helper. wxWidgets/GTK is already required by
# Match Studio, so this adds a KDE tray icon without installing anything on the
# target Astra workstation.
# shellcheck disable=SC2046
g++ -O2 -pipe -std=c++11 scripts/pol_run_tray.cpp \
  $(wx-config --cxxflags) $(wx-config --libs) \
  -o "$TRAY"

rm -rf "$OUT"
mkdir -p \
  "$ROOT/lib" \
  "$ROOT/config" \
  "$ROOT/match" \
  "$ROOT/packages" \
  "$ROOT/runtime" \
  "$ROOT/scripts" \
  "$ROOT/docs"

cp "$CORE" "$ROOT/rEspanso-core"
cp "$STUDIO" "$ROOT/rEspanso-Match-Studio"
cp "$TRAY" "$ROOT/rEspanso-Tray"
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

# Bundle the ABI-sensitive libraries that caused the Astra failure. We do NOT
# bundle glibc or the X11/GL stack: those must stay coupled to the target KDE/X11
# session. wxWidgets, GCC runtime and OpenSSL 1.1 can safely live next to the app.
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

cat >"$ROOT/start-tray.sh" <<'TRAY_RUN'
#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PIDFILE="$ROOT/runtime/tray.pid"
LOGFILE="$ROOT/runtime/tray.log"

export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
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

# Give GTK a moment to register with the KDE system tray. Failure of the tray
# must never prevent the text-expansion engine from starting.
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
CORE="$ROOT/rEspanso-core"
STUDIO="$ROOT/rEspanso-Match-Studio"

export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export ESPANSO_CONFIG_DIR="$ROOT"
export ESPANSO_PACKAGE_DIR="$ROOT/packages"
export ESPANSO_RUNTIME_DIR="$ROOT/runtime"

mkdir -p "$ROOT/config" "$ROOT/match" "$ROOT/packages" "$ROOT/runtime" "$ROOT/scripts"

core_args=(
  --config_dir "$ROOT"
  --package_dir "$ROOT/packages"
  --runtime_dir "$ROOT/runtime"
)

# Start the text-expansion engine in unmanaged user mode. No systemd service,
# package installation or sudo is required. If it is already running from this
# portable directory, keep it and ensure the KDE tray helper is present.
if ! "$CORE" "${core_args[@]}" service status >/dev/null 2>&1; then
  "$CORE" "${core_args[@]}" service start --unmanaged || {
    echo "Не удалось запустить движок rEspanso. Запускаю диагностику..." >&2
    "$ROOT/diagnose.sh" || true
    exit 1
  }
fi

"$ROOT/start-tray.sh" || true

# Open the complete Match Studio GUI against the same portable config root.
exec "$STUDIO" --config-dir "$ROOT"
RUN

cat >"$ROOT/studio.sh" <<'STUDIO_RUN'
#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$ROOT/rEspanso-Match-Studio" --config-dir "$ROOT"
STUDIO_RUN

cat >"$ROOT/start-engine.sh" <<'ENGINE'
#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

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

cat >"$ROOT/README-FIRST.txt" <<'README'
rEspanso pol_run — full portable build for Astra Linux 1.7 / KDE / X11 / x86_64

No sudo and no system installation are required on the workstation.

FIRST START
  chmod +x run.sh studio.sh start-engine.sh start-tray.sh stop.sh diagnose.sh \
    rEspanso-core rEspanso-Match-Studio rEspanso-Tray
  ./run.sh

run.sh does three things:
  1. starts the rEspanso text-expansion engine in unmanaged user mode;
  2. starts a persistent KDE/GTK system-tray icon;
  3. opens the full rEspanso Match Studio GUI.

TRAY ICON
  Left click              open Match Studio
  Right click             menu
  Menu > Match Studio     open Match Studio
  Menu > Stop rEspanso    stop the portable engine and remove the icon

The whole working configuration stays inside this directory:
  config/   global settings
  match/    YAML matches
  scripts/  Rhai scripts
  packages/ local packages
  runtime/  daemon/tray state and logs

Useful commands:
  ./studio.sh        open only Match Studio
  ./start-engine.sh  start engine + tray icon
  ./start-tray.sh    restore only the tray icon
  ./stop.sh          stop engine + tray icon
  ./diagnose.sh      print ABI/library/display/tray diagnostics

Compatibility design:
  - core, Match Studio and tray are built in Debian 10 / glibc 2.28;
  - Match Studio uses X11 + OpenGL (glow), not the newer WGPU path;
  - tray uses wxWidgets/GTK already required by Match Studio;
  - wxWidgets 3.0 and selected ABI-sensitive runtimes are bundled in ./lib;
  - glibc and the target X11/OpenGL/GTK stack are deliberately NOT replaced.
README

chmod +x \
  "$ROOT/run.sh" \
  "$ROOT/studio.sh" \
  "$ROOT/start-engine.sh" \
  "$ROOT/start-tray.sh" \
  "$ROOT/stop.sh" \
  "$ROOT/diagnose.sh"

# Record and enforce the maximum glibc symbol version required by all binaries.
for pair in "core:$CORE" "studio:$STUDIO" "tray:$TRAY"; do
  label="${pair%%:*}"
  binary="${pair#*:}"
  objdump -T "$binary" 2>/dev/null \
    | grep -o 'GLIBC_[0-9.]*' \
    | sort -Vu >"$ROOT/${label}-glibc-required.txt" || true

  max_glibc="$(sed 's/GLIBC_//' "$ROOT/${label}-glibc-required.txt" | sort -V | tail -n1)"
  echo "$label maximum required GLIBC: $max_glibc"
  if [[ -n "$max_glibc" && "$(printf '%s\n%s\n' "$max_glibc" '2.28' | sort -V | tail -n1)" != '2.28' ]]; then
    echo "ERROR: $label requires GLIBC newer than 2.28"
    exit 1
  fi
done

LD_LIBRARY_PATH="$ROOT/lib" ldd "$ROOT/rEspanso-core" >"$ROOT/core-packaged-ldd.txt"
LD_LIBRARY_PATH="$ROOT/lib" ldd "$ROOT/rEspanso-Match-Studio" >"$ROOT/studio-packaged-ldd.txt"
LD_LIBRARY_PATH="$ROOT/lib" ldd "$ROOT/rEspanso-Tray" >"$ROOT/tray-packaged-ldd.txt"

for packaged_ldd in \
  "$ROOT/core-packaged-ldd.txt" \
  "$ROOT/studio-packaged-ldd.txt" \
  "$ROOT/tray-packaged-ldd.txt"; do
  if grep -q 'not found' "$packaged_ldd"; then
    cat "$packaged_ldd"
    echo "ERROR: package has unresolved libraries: $packaged_ldd"
    exit 1
  fi
done

# Smoke tests that do not require an X server.
LD_LIBRARY_PATH="$ROOT/lib" "$ROOT/rEspanso-core" --version

tar -C "$OUT" -czf "$OUT/$PACKAGE.tar.gz" "$PACKAGE"
sha256sum "$OUT/$PACKAGE.tar.gz" >"$OUT/$PACKAGE.sha256"

ls -lh "$OUT/$PACKAGE.tar.gz" "$OUT/$PACKAGE.sha256"
