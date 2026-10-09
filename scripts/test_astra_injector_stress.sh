#!/usr/bin/env bash
# Repeated end-to-end X11 expansion regression for AstraSafeInjector.
set -Eeuo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 /path/to/espanso-core" >&2
  exit 2
fi

CORE="$(realpath "$1")"
ITERATIONS="${RESPANSO_ASTRA_STRESS_ITERATIONS:-12}"

for i in $(seq 1 "$ITERATIONS"); do
  echo "=== AstraSafeInjector stress iteration $i/$ITERATIONS ==="
  timeout 20s bash scripts/test_astra_expansion.sh "$CORE"
done

echo "AstraSafeInjector stress: $ITERATIONS/$ITERATIONS expansion cycles PASS"
