#!/usr/bin/env bash
# Smoke-test the real wxWidgets modulo search under X11: Enter must return the selected ID.
set -Eeuo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 /path/to/espanso-core" >&2
  exit 2
fi
if [[ -z "${DISPLAY:-}" ]]; then
  echo 'DISPLAY is not set; run this test under xvfb-run or an X11 session' >&2
  exit 2
fi

CORE="$(realpath "$1")"
TEST_DIR="$(mktemp -d)"
SEARCH_PID=''
cleanup() {
  set +e
  if [[ "$SEARCH_PID" =~ ^[0-9]+$ ]]; then
    kill "$SEARCH_PID" 2>/dev/null || true
  fi
  rm -rf "$TEST_DIR"
}
trap cleanup EXIT

cat >"$TEST_DIR/search.json" <<'JSON'
{
  "title": "rEspanso X11 Search Smoke",
  "hint": "Press Enter",
  "algorithm": "ikey",
  "items": [
    {
      "id": "astra-enter-ok",
      "label": "Astra Enter regression check",
      "trigger": ":astra-enter",
      "search_terms": ["astra", "enter"],
      "is_builtin": false
    }
  ]
}
JSON

"$CORE" modulo search -j -i "$TEST_DIR/search.json" \
  >"$TEST_DIR/output.json" 2>"$TEST_DIR/search.err" &
SEARCH_PID=$!

window=''
for _ in $(seq 1 80); do
  window="$(xdotool search --onlyvisible --name 'rEspanso X11 Search Smoke' 2>/dev/null | head -n1 || true)"
  if [[ -n "$window" ]]; then
    break
  fi
  if ! kill -0 "$SEARCH_PID" 2>/dev/null; then
    cat "$TEST_DIR/search.err" >&2 || true
    echo 'search process exited before its X11 window became available' >&2
    exit 1
  fi
  sleep 0.05
done

if [[ -z "$window" ]]; then
  cat "$TEST_DIR/search.err" >&2 || true
  echo 'could not find the modulo search X11 window' >&2
  exit 1
fi

# Use XSetInputFocus first, then a normal synthetic key. The search frame owns
# Enter and must consume it; selection is returned only after the deferred
# Submit callback has completed and wxEntry exits.
xdotool windowfocus --sync "$window"
xdotool key --clearmodifiers Return

for _ in $(seq 1 100); do
  if ! kill -0 "$SEARCH_PID" 2>/dev/null; then
    break
  fi
  sleep 0.05
done
if kill -0 "$SEARCH_PID" 2>/dev/null; then
  echo 'search process did not close after Enter' >&2
  exit 1
fi
wait "$SEARCH_PID"
SEARCH_PID=''

cat "$TEST_DIR/output.json"
grep -q '"selected":"astra-enter-ok"' "$TEST_DIR/output.json"
echo 'Astra modulo search: Enter selection PASS'
