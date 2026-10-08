#!/usr/bin/env bash
# Restore rEspanso's Ctrl+Space search shortcut on a portable Astra/Linux install.
# Usage: bash scripts/restore-ctrl-space.sh [path/to/rEspanso-portable]
set -euo pipefail

ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
CFG="$ROOT/config/default.yml"

if [[ ! -f "$CFG" ]]; then
  printf 'ERROR: rEspanso configuration not found: %s\n' "$CFG" >&2
  printf 'Usage: bash %s /path/to/rEspanso-portable\n' "$0" >&2
  exit 1
fi

if grep -Eq '^[[:space:]]*search_shortcut[[:space:]]*:[[:space:]]*CTRL\+SPACE([[:space:]#]|$)' "$CFG"; then
  printf 'Already configured: CTRL+SPACE (%s)\n' "$CFG"
  exit 0
fi

BACKUP="$CFG.before-ctrlspace.$(date +%Y%m%d-%H%M%S).bak"
cp -p -- "$CFG" "$BACKUP"

if grep -Eq '^[[:space:]]*search_shortcut[[:space:]]*:' "$CFG"; then
  sed -i -E 's/^[[:space:]]*search_shortcut[[:space:]]*:.*/search_shortcut: CTRL+SPACE/' "$CFG"
else
  printf '\nsearch_shortcut: CTRL+SPACE\n' >> "$CFG"
fi

printf 'Updated: %s\nBackup:  %s\n' "$CFG" "$BACKUP"
printf 'Verify after auto-reload: Ctrl+Space opens the search window.\n'
printf 'If not, check desktop/input-method shortcut conflicts and the rEspanso startup log.\n'
