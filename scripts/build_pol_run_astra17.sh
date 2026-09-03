#!/usr/bin/env bash
set -Eeuo pipefail

# Build rEspanso X11 for Astra Linux 1.7 without requiring sudo on the target.
# Run this script inside Debian 10 (buster), whose glibc is 2.28.

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
  libssl-dev \
  libwxgtk3.0-gtk3-dev \
  libx11-dev \
  libxkbcommon-dev \
  libxtst-dev \
  pkg-config

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --profile minimal --default-toolchain stable
# shellcheck disable=SC1091
. "$HOME/.cargo/env"

echo "Build host: $(ldd --version | head -n1)"
rustc --version
cargo --version

cargo build --locked --release \
  -p espanso --bin espanso \
  --no-default-features \
  --features modulo,vendored-tls

BIN="target/release/espanso"
OUT="target/pol-run-astra"
PACKAGE="rEspanso-pol_run-Astra17-X11-x86_64"
ROOT="$OUT/$PACKAGE"

rm -rf "$OUT"
mkdir -p "$ROOT/lib"
cp "$BIN" "$ROOT/rEspanso-core"
chmod +x "$ROOT/rEspanso-core"

ldd "$BIN" >"$ROOT/build-ldd.txt"

# Bundle the exact SONAMEs required by the executable. wxWidgets 3.0 and the
# GCC C++ runtime are safe to ship next to the program; glibc is deliberately
# left to the target Astra system.
while read -r name arrow path rest; do
  case "$name" in
    libwx_*.so*|libstdc++.so.6|libgcc_s.so.1)
      if [[ "$arrow" == '=>' && -f "$path" ]]; then
        cp -L "$path" "$ROOT/lib/$name"
      fi
      ;;
  esac
done <"$ROOT/build-ldd.txt"

cat >"$ROOT/run.sh" <<'RUN'
#!/usr/bin/env bash
set -e
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$ROOT/rEspanso-core" "$@"
RUN

cat >"$ROOT/diagnose.sh" <<'DIAG'
#!/usr/bin/env bash
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
echo '== system =='
uname -a
ldd --version | head -n1 || true
echo
echo '== bundled =='
ls -lh "$ROOT/lib" || true
echo
echo '== missing =='
ldd "$ROOT/rEspanso-core" | grep 'not found' || echo none
echo
echo '== version =='
"$ROOT/rEspanso-core" --version || true
DIAG

cat >"$ROOT/README.txt" <<'README'
rEspanso pol_run — Astra Linux 1.7 / X11 / x86_64

Portable build intended for a non-admin workstation. No sudo is required on
the Astra machine.

Run:
  ./run.sh

If startup fails:
  ./diagnose.sh

The core is built on Debian 10 (glibc 2.28). wxWidgets 3.0, libstdc++ and
libgcc_s are bundled in ./lib. glibc itself is NOT bundled or replaced.
README

chmod +x "$ROOT/run.sh" "$ROOT/diagnose.sh"

objdump -T "$BIN" 2>/dev/null \
  | grep -o 'GLIBC_[0-9.]*' \
  | sort -Vu >"$ROOT/glibc-required.txt" || true

MAX_GLIBC="$(sed 's/GLIBC_//' "$ROOT/glibc-required.txt" | sort -V | tail -n1)"
echo "Maximum required GLIBC: $MAX_GLIBC"
if [[ "$(printf '%s\n%s\n' "$MAX_GLIBC" '2.28' | sort -V | tail -n1)" != '2.28' ]]; then
  echo "ERROR: rEspanso-core requires GLIBC newer than 2.28"
  exit 1
fi

LD_LIBRARY_PATH="$ROOT/lib" ldd "$ROOT/rEspanso-core" >"$ROOT/packaged-ldd.txt"
if grep -q 'not found' "$ROOT/packaged-ldd.txt"; then
  cat "$ROOT/packaged-ldd.txt"
  echo 'ERROR: package has unresolved libraries in the Debian 10 build environment'
  exit 1
fi

tar -C "$OUT" -czf "$OUT/$PACKAGE.tar.gz" "$PACKAGE"
sha256sum "$OUT/$PACKAGE.tar.gz" >"$OUT/$PACKAGE.sha256"

ls -lh "$OUT/$PACKAGE.tar.gz" "$OUT/$PACKAGE.sha256"
