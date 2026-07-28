#!/usr/bin/env bash
# Full verification gate — run before committing. Mirrors CI.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== rust (fmt · clippy · test) =="
(
  cd src-tauri
  cargo fmt --all --check
  cargo clippy --all-targets -- -D warnings
  cargo test
)

echo "== frontend (typecheck · biome · vitest) =="
npm run typecheck
npx biome check .
npm run test

echo "✅ all green"
