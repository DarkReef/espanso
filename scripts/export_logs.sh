#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STAMP="$(date '+%Y%m%d-%H%M%S')"
OUT_DIR="$ROOT/diagnostics"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/respanso-diag.XXXXXX")"
BUNDLE_DIR="$WORK_DIR/rEspanso-diagnostics-$STAMP"
ARCHIVE="$OUT_DIR/rEspanso-diagnostics-$STAMP.tar.gz"

cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

mkdir -p "$OUT_DIR" "$BUNDLE_DIR/runtime" "$BUNDLE_DIR/meta"

# Runtime logs are the primary evidence. They may contain paths/window titles,
# but the instrumented selection path never logs clipboard or selected-text
# contents. Limit individual files so a runaway log cannot create a huge bundle.
if [[ -d "$ROOT/runtime" ]]; then
  while IFS= read -r -d '' file; do
    base="$(basename "$file")"
    size="$(stat -c '%s' "$file" 2>/dev/null || echo 0)"
    if [[ "$size" -le 52428800 ]]; then
      cp -a "$file" "$BUNDLE_DIR/runtime/$base" 2>/dev/null || true
    else
      tail -c 52428800 "$file" >"$BUNDLE_DIR/runtime/$base.tail-50MiB" 2>/dev/null || true
    fi
  done < <(find "$ROOT/runtime" -maxdepth 1 -type f \( -name '*.log' -o -name '*.txt' -o -name '*.pid' \) -print0 2>/dev/null)
fi

{
  echo '===== rEspanso diagnostics metadata ====='
  echo "created_at=$(date --iso-8601=seconds 2>/dev/null || date)"
  echo "root=$ROOT"
  echo "user=$(id -un 2>/dev/null || true)"
  echo "uid=$(id -u 2>/dev/null || true)"
  echo "kernel=$(uname -srmo 2>/dev/null || uname -a)"
  echo "display=${DISPLAY:-<unset>}"
  echo "xdg_session_type=${XDG_SESSION_TYPE:-<unset>}"
  echo "xdg_current_desktop=${XDG_CURRENT_DESKTOP:-<unset>}"
  echo "desktop_session=${DESKTOP_SESSION:-<unset>}"
  echo "lang=${LANG:-<unset>}"
  echo "lc_all=${LC_ALL:-<unset>}"
  echo "xmodifiers=${XMODIFIERS:-<unset>}"
  echo "gtk_im_module=${GTK_IM_MODULE:-<unset>}"
  echo "qt_im_module=${QT_IM_MODULE:-<unset>}"
  if [[ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ]]; then echo 'dbus_session_bus=<set>'; else echo 'dbus_session_bus=<unset>'; fi
  echo
  if [[ -r /etc/os-release ]]; then
    echo '--- /etc/os-release ---'
    cat /etc/os-release
  fi
  echo
  echo '--- glibc ---'
  ldd --version 2>&1 | head -n 2 || true
} >"$BUNDLE_DIR/meta/system.txt" 2>&1

{
  echo '===== X11 state ====='
  command -v xdotool || true
  command -v xclip || true
  command -v xsel || true
  echo
  echo '--- focused window ---'
  xdotool getwindowfocus 2>&1 || true
  xdotool getwindowfocus getwindowname 2>&1 || true
  echo
  echo '--- active window property ---'
  xprop -root _NET_ACTIVE_WINDOW 2>&1 || true
  echo
  echo '--- X keyboard summary ---'
  setxkbmap -query 2>&1 || true
  echo
  echo '--- X server ---'
  xdpyinfo 2>&1 | sed -n '1,80p' || true
} >"$BUNDLE_DIR/meta/x11.txt" 2>&1

{
  echo '===== rEspanso processes ====='
  ps -eo pid,ppid,stat,lstart,cmd 2>/dev/null | grep -E '[r]Espanso|[e]spanso' || true
  echo
  echo '===== open process handles (summary) ====='
  for pid in $(pgrep -f 'rEspanso|espanso' 2>/dev/null || true); do
    echo "--- pid $pid ---"
    tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null || true
    echo
    grep -E '^(Name|State|Threads|VmRSS|voluntary_ctxt_switches|nonvoluntary_ctxt_switches):' "/proc/$pid/status" 2>/dev/null || true
  done
} >"$BUNDLE_DIR/meta/processes.txt" 2>&1

{
  echo '===== binary identity ====='
  for bin in rEspanso-core rEspanso-Match-Studio rEspanso-Tray bin/xdotool; do
    path="$ROOT/$bin"
    if [[ -f "$path" ]]; then
      printf '%s  ' "$bin"
      sha256sum "$path" 2>/dev/null | awk '{print $1}' || true
      file "$path" 2>/dev/null || true
    fi
  done
  echo
  "$ROOT/rEspanso-core" --version 2>&1 || true
} >"$BUNDLE_DIR/meta/binaries.txt" 2>&1

# Do not copy configuration or match bodies: they can contain API credentials,
# medical context or patient data. Only names, sizes and hashes are exported.
{
  echo '===== configuration inventory (contents intentionally excluded) ====='
  for dir in config match scripts packages; do
    if [[ -d "$ROOT/$dir" ]]; then
      echo "--- $dir ---"
      find "$ROOT/$dir" -maxdepth 3 -type f -printf '%P\t%s bytes\n' 2>/dev/null | sort || true
    fi
  done
  echo
  echo '===== configuration hashes ====='
  find "$ROOT/config" "$ROOT/match" -maxdepth 3 -type f -print0 2>/dev/null \
    | sort -z \
    | xargs -0 -r sha256sum 2>/dev/null \
    | sed "s#${ROOT}/##g" || true
} >"$BUNDLE_DIR/meta/config-inventory.txt" 2>&1

if [[ -x "$ROOT/diagnose.sh" ]]; then
  "$ROOT/diagnose.sh" >"$BUNDLE_DIR/meta/diagnose.txt" 2>&1 || true
fi

{
  echo 'This archive is intended for rEspanso X11 diagnostics.'
  echo 'Selected text and clipboard contents are NOT intentionally logged.'
  echo 'Configuration/match file contents are NOT included; only file metadata and hashes are exported.'
  echo 'Runtime logs can still contain local filesystem paths and application/window names.'
} >"$BUNDLE_DIR/PRIVACY.txt"

tar -C "$WORK_DIR" -czf "$ARCHIVE" "$(basename "$BUNDLE_DIR")"
sha256sum "$ARCHIVE" >"$ARCHIVE.sha256"
printf '%s\n' "$ARCHIVE" >"$ROOT/runtime/last-diagnostics-archive.txt"

printf 'Готово: %s\n' "$ARCHIVE"
printf 'SHA256: '
cat "$ARCHIVE.sha256"

if command -v notify-send >/dev/null 2>&1; then
  notify-send 'rEspanso' "Архив диагностики создан:\n$ARCHIVE" >/dev/null 2>&1 || true
fi
