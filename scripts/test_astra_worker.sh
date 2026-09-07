#!/usr/bin/env bash
# Run within Xvfb, after building the full core. A worker must remain alive.
set -Eeuo pipefail
CORE="$(realpath "$1")"
TEST_ROOT="$(mktemp -d)"
trap 'rm -rf "$TEST_ROOT"' EXIT
CONFIG="$TEST_ROOT/Новая папка с длинным названием/portable-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
mkdir -p "$CONFIG/config" "$CONFIG/match" "$CONFIG/packages" "$CONFIG/runtime"
printf 'show_icon: false\n' > "$CONFIG/config/default.yml"
printf 'matches: []\n' > "$CONFIG/match/base.yml"
set +e
timeout --kill-after=2s 10s "$CORE" --config_dir "$CONFIG" \
  --package_dir "$CONFIG/packages" --runtime_dir "$CONFIG/runtime" worker > "$TEST_ROOT/worker.log" 2>&1
result=$?
set -e
cat "$TEST_ROOT/worker.log"
if [[ "$result" != 124 ]]; then
  echo "Worker did not survive the smoke test (status $result)" >&2
  exit 1
fi
grep -q 'binded to IPC unix socket' "$TEST_ROOT/worker.log"
echo 'Astra worker: X11 startup with long Unicode runtime path PASS'

# Exercise the portable launcher and daemon stop, using the same long path.
# The launcher recognizes this marker and skips its first-run desktop wizard.
touch "$CONFIG/rEspanso-Match-Studio"
core_args=(--config_dir "$CONFIG" --package_dir "$CONFIG/packages" --runtime_dir "$CONFIG/runtime")
"$CORE" "${core_args[@]}" service start --unmanaged || {
  cat "$CONFIG/runtime/startup.log" "$CONFIG/runtime/espanso.log"
  exit 1
}
sleep 1
"$CORE" "${core_args[@]}" service status
"$CORE" "${core_args[@]}" service stop
if "$CORE" "${core_args[@]}" service status; then
  echo 'Worker remained running after service stop' >&2
  exit 1
fi
echo 'Astra service: portable start, status and stop with long runtime path PASS'
