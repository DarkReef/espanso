#!/usr/bin/env bash

# Creates an app bundle for MacOS
#
# Optionally accepts a path to an espanso executable as the first argument and
# a path to an espanso-editor executable as the second argument. If no paths
# are provided, universal binaries are created from the release target folders.

set -Eeuf -o pipefail

readonly TARGET_DIR=target/mac/Espanso.app

main() {
  # Pass binaries as "$1" and "$2"; default to universal binaries.
  local espanso_bin=${1:-universal}
  local editor_bin=${2:-universal}

  rm -rf -- "${TARGET_DIR}"

  local VERSION
  VERSION=$(awk -F '"' '/^version/ { print $2; exit }' espanso/Cargo.toml)

  mkdir -p "${TARGET_DIR}"/Contents
  mkdir -p "${TARGET_DIR}"/Contents/MacOS
  mkdir -p "${TARGET_DIR}"/Contents/Resources

  sed -e "s/VERSION/${VERSION}/" espanso/src/res/macos/Info.plist > "${TARGET_DIR}"/Contents/Info.plist

  /bin/echo "APPL????" > "${TARGET_DIR}"/Contents/PkgInfo

  cp -f espanso/src/res/macos/icon.icns "${TARGET_DIR}"/Contents/Resources/icon.icns

  if [[ "${espanso_bin}" != universal ]]; then
    cp "${espanso_bin}" "${TARGET_DIR}/Contents/MacOS/espanso"
  else
    lipo -create \
      -output "${TARGET_DIR}/Contents/MacOS/espanso" \
      target/aarch64-apple-darwin/release/espanso target/x86_64-apple-darwin/release/espanso
  fi

  if [[ "${editor_bin}" != universal ]]; then
    cp "${editor_bin}" "${TARGET_DIR}/Contents/MacOS/espanso-editor"
  else
    lipo -create \
      -output "${TARGET_DIR}/Contents/MacOS/espanso-editor" \
      target/aarch64-apple-darwin/release/espanso-editor target/x86_64-apple-darwin/release/espanso-editor
  fi

  chmod +x "${TARGET_DIR}/Contents/MacOS/espanso" "${TARGET_DIR}/Contents/MacOS/espanso-editor"
}
main "$@"
