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
OUT="target/pol-run-astra-full"
PACKAGE="rEspanso-pol_run-Astra17-X11-Full-x86_64"
ROOT="$OUT/$PACKAGE"

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
cp espanso/src/res/config/default.yml "$ROOT/config/default.yml"
cp espanso/src/res/config/base.yml "$ROOT/match/base.yml"
cp LICENSE "$ROOT/LICENSE.txt"
if [[ -d docs/respanso ]]; then
  cp -R docs/respanso/. "$ROOT/docs/"
fi

chmod +x "$ROOT/rEspanso-core" "$ROOT/rEspanso-Match-Studio"

ldd "$CORE" >"$ROOT/core-build-ldd.txt"
ldd "$STUDIO" >"$ROOT/studio-build-ldd.txt"

# Bundle the ABI-sensitive libraries that caused the Astra failure. We do NOT
# bundle glibc or the X11/GL stack: those must stay coupled to the target KDE/X11
# session. wxWidgets, GCC runtime and OpenSSL 1.1 can safely live next to the app.
for ldd_file in "$ROOT/core-build-ldd.txt" "$ROOT/studio-build-ldd.txt"; do
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
# portable directory, just keep it and open the GUI.
if ! "$CORE" "${core_args[@]}" service status >/dev/null 2>&1; then
  "$CORE" "${core_args[@]}" service start --unmanaged || {
    echo "Не удалось запустить движок rEspanso. Запускаю диагностику..." >&2
    "$ROOT/diagnose.sh" || true
    exit 1
  }
fi

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
exec "$ROOT/rEspanso-core" \
  --config_dir "$ROOT" \
  --package_dir "$ROOT/packages" \
  --runtime_dir "$ROOT/runtime" \
  service start --unmanaged
ENGINE

cat >"$ROOT/stop.sh" <<'STOP'
#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$ROOT/rEspanso-core" \
  --config_dir "$ROOT" \
  --package_dir "$ROOT/packages" \
  --runtime_dir "$ROOT/runtime" \
  service stop
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
  chmod +x run.sh studio.sh start-engine.sh stop.sh diagnose.sh \
    rEspanso-core rEspanso-Match-Studio
  ./run.sh

run.sh does two things:
  1. starts the rEspanso text-expansion engine in unmanaged user mode;
  2. opens the full rEspanso Match Studio GUI.

The whole working configuration stays inside this directory:
  config/   global settings
  match/    YAML matches
  scripts/  Rhai scripts
  packages/ local packages
  runtime/  daemon state and logs

Useful commands:
  ./studio.sh        open only Match Studio
  ./start-engine.sh  start only the text-expansion engine
  ./stop.sh          stop the portable engine
  ./diagnose.sh      print ABI/library/display diagnostics

Compatibility design:
  - core and Match Studio are built in Debian 10 / glibc 2.28;
  - Match Studio uses X11 + OpenGL (glow), not the newer WGPU path;
  - wxWidgets 3.0 and selected ABI-sensitive runtimes are bundled in ./lib;
  - glibc and the target X11/OpenGL stack are deliberately NOT replaced.
README

chmod +x \
  "$ROOT/run.sh" \
  "$ROOT/studio.sh" \
  "$ROOT/start-engine.sh" \
  "$ROOT/stop.sh" \
  "$ROOT/diagnose.sh"

# Record and enforce the maximum glibc symbol version required by both binaries.
for pair in "core:$CORE" "studio:$STUDIO"; do
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

if grep -q 'not found' "$ROOT/core-packaged-ldd.txt"; then
  cat "$ROOT/core-packaged-ldd.txt"
  echo 'ERROR: core package has unresolved libraries in the Debian 10 build environment'
  exit 1
fi
if grep -q 'not found' "$ROOT/studio-packaged-ldd.txt"; then
  cat "$ROOT/studio-packaged-ldd.txt"
  echo 'ERROR: Match Studio package has unresolved libraries in the Debian 10 build environment'
  exit 1
fi

# Smoke tests that do not require an X server.
LD_LIBRARY_PATH="$ROOT/lib" "$ROOT/rEspanso-core" --version

tar -C "$OUT" -czf "$OUT/$PACKAGE.tar.gz" "$PACKAGE"
sha256sum "$OUT/$PACKAGE.tar.gz" >"$OUT/$PACKAGE.sha256"

ls -lh "$OUT/$PACKAGE.tar.gz" "$OUT/$PACKAGE.sha256"
