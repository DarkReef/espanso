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

# The real Astra portable bundle always contains Match Studio in its config
# root. The launcher uses this marker to skip the interactive first-run wizard;
# without it this smoke test exercises a fresh installation instead of the
# portable service lifecycle and correctly times out waiting for user input.
: >"$CONFIG/rEspanso-Match-Studio"

core_args=(
  --config_dir "$CONFIG"
  --package_dir "$CONFIG/packages"
  --runtime_dir "$CONFIG/runtime"
)

dump_diagnostics() {
  echo '=== Astra restart smoke diagnostics ===' >&2
  echo "config: $CONFIG" >&2
  echo '--- runtime directory ---' >&2
  find "$CONFIG/runtime" -maxdepth 2 -printf '%M %u:%g %s %p\n' 2>&1 || true
  echo '--- startup.log ---' >&2
  if [[ -f "$CONFIG/runtime/startup.log" ]]; then
    cat "$CONFIG/runtime/startup.log" >&2 || true
  else
    echo '(missing)' >&2
  fi
  echo '--- espanso.log ---' >&2
  if [[ -f "$CONFIG/runtime/espanso.log" ]]; then
    cat "$CONFIG/runtime/espanso.log" >&2 || true
  else
    echo '(missing)' >&2
  fi
  echo '--- matching processes ---' >&2
  ps -ef | grep -E '[r]Espanso|[e]spanso' >&2 || true
  echo '=== end Astra restart smoke diagnostics ===' >&2
}

cleanup() {
  local rc=$?
  trap - EXIT
  set +e
  if (( rc != 0 )); then
    dump_diagnostics
  fi
  timeout --kill-after=2s 12s "$CORE" "${core_args[@]}" service stop >/dev/null 2>&1
  rm -rf "$TEST_ROOT"
  exit "$rc"
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
