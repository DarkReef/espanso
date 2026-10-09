#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_DIR="$(mktemp -d)"
trap 'rm -rf "$TEST_DIR"' EXIT
# native_astra.cpp now writes the same diagnostics as the production worker via
# detect_write_x11_log(), which is implemented by runtime.cpp. Link both so the
# regression test exercises the actual Astra detector/runtime pair.
g++ -std=c++11 -Wall -Wextra -Werror -pthread \
  "$ROOT/scripts/test_astra_x11.cpp" \
  "$ROOT/espanso-detect/src/x11/runtime.cpp" \
  -lX11 -lXi -lXtst \
  -o "$TEST_DIR/astra-x11-test"
timeout 20s xvfb-run -a "$TEST_DIR/astra-x11-test"
