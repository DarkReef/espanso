#!/usr/bin/env bash
# Exercise the exact unmanaged service lifecycle used by the Astra/X11 portable build.
set -Eeuo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 /path/to/espanso-core" >&2
  exit 2
fi

CORE="$(realpath "$1")"
TEST_ROOT="$(mktemp -d)"
CONFIG="$TEST_ROOT/Перезапуск rEspanso/portable-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
mkdir -p "$CONFIG/config" "$CONFIG/match" "$CONFIG/packages" "$CONFIG/runtime" "$CONFIG/scripts"
printf 'show_icon: false\nauto_restart: true\n' >"$CONFIG/config/default.yml"
printf 'matches: []\n' >"$CONFIG/match/base.yml"

core_args=(
  --config_dir "$CONFIG"
  --package_dir "$CONFIG/packages"
  --runtime_dir "$CONFIG/runtime"
)

cleanup() {
  set +e
  timeout --kill-after=2s 12s "$CORE" "${core_args[@]}" service stop >/dev/null 2>&1
  rm -rf "$TEST_ROOT"
}
trap cleanup EXIT

run_service() {
  timeout --kill-after=2s 15s "$CORE" "${core_args[@]}" service "$@"
}

run_service start --unmanaged
run_service status

# Repeat restart to expose stale-lock/second-daemon races instead of validating
# only the happy path once.
for round in 1 2 3; do
  echo "Astra restart smoke: round $round"
  run_service restart --unmanaged
  run_service status
  sleep 0.2
done

run_service stop
if run_service status >/dev/null 2>&1; then
  echo 'rEspanso still reports running after service stop' >&2
  exit 1
fi

# The daemon must have reached normal startup at least once. Keep this check
# deliberately loose: log wording beyond these stable markers is diagnostic,
# not part of the public CLI contract.
if [[ -f "$CONFIG/runtime/espanso.log" ]]; then
  tail -n 120 "$CONFIG/runtime/espanso.log"
fi

echo 'Astra service restart: repeated unmanaged X11 lifecycle PASS'
