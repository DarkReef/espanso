#!/usr/bin/env bash
# Smoke-test the tabbed wxWidgets search palette under X11.
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
  "title": "rEspanso Search Tabs Smoke",
  "hint": "Ctrl+Tab switches tabs",
  "algorithm": "ikey",
  "tabs_enabled": true,
  "items": [
    {
      "id": "trigger-ok",
      "label": "Regular trigger item",
      "trigger": ":hello",
      "search_terms": ["trigger"],
      "is_builtin": false,
      "category": "triggers"
    },
    {
      "id": "icd:I10",
      "label": "Эссенциальная первичная гипертензия",
      "trigger": "I10",
      "search_terms": ["I10", "гипертензия"],
      "is_builtin": false,
      "category": "codes"
    }
  ]
}
JSON

"$CORE" modulo search -j -i "$TEST_DIR/search.json"   >"$TEST_DIR/output.json" 2>"$TEST_DIR/search.err" &
SEARCH_PID=$!

window=''
for _ in $(seq 1 80); do
  window="$(xdotool search --onlyvisible --name 'rEspanso Search Tabs Smoke' 2>/dev/null | head -n1 || true)"
  if [[ -n "$window" ]]; then
    break
  fi
  if ! kill -0 "$SEARCH_PID" 2>/dev/null; then
    cat "$TEST_DIR/search.err" >&2 || true
    echo 'tabbed search process exited before its X11 window became available' >&2
    exit 1
  fi
  sleep 0.05
done

if [[ -z "$window" ]]; then
  cat "$TEST_DIR/search.err" >&2 || true
  echo 'could not find the tabbed modulo search X11 window' >&2
  exit 1
fi

xdotool windowfocus --sync "$window"
# The default tab is Triggers. Switch to Codes, narrow by the ASCII ICD code,
# then submit the first result.
xdotool key --clearmodifiers ctrl+Tab
sleep 0.1
xdotool type --clearmodifiers --delay 5 'I10'
sleep 0.1
xdotool key --clearmodifiers Return

for _ in $(seq 1 100); do
  if ! kill -0 "$SEARCH_PID" 2>/dev/null; then
    break
  fi
  sleep 0.05
done
if kill -0 "$SEARCH_PID" 2>/dev/null; then
  echo 'tabbed search process did not close after Enter' >&2
  exit 1
fi
wait "$SEARCH_PID"
SEARCH_PID=''

cat "$TEST_DIR/output.json"
grep -q '"selected":"icd:I10"' "$TEST_DIR/output.json"
echo 'Astra modulo search: Ctrl+Tab -> Codes -> ICD selection PASS'
