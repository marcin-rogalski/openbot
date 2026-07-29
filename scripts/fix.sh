#!/usr/bin/env bash
# Auto-cleanup pass — applies machine-fixable lints so we don't hand-edit them.
# Removes unused imports/variables, applies clippy autofixes, then formats.
# Run before ./scripts/verify.sh. Anything left over is a real issue for verify.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== rust: clippy --fix (unused imports, autofixable lints) + fmt =="
(
  cd src-tauri
  # --allow-dirty/--allow-staged: we run this on a working tree mid-change.
  # Not -D warnings here: we want it to compile and apply fixes, not abort.
  cargo clippy --fix --all-targets --allow-dirty --allow-staged 2>/dev/null || true
  cargo fmt --all
)

echo "== frontend: biome --write =="
npx --no-install biome check --write . 2>/dev/null || true

echo "✅ cleaned"
