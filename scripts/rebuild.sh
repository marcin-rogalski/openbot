#!/usr/bin/env bash
# Verify + package + relaunch the app in one call — the loop to run after a
# milestone, before eyeballing behavior in the real .app. Keeps the caller's
# token usage low: full cargo/vite/npm output is logged to a temp dir, not
# streamed back; only a short pass/fail summary prints, with the last lines
# of the failing step's log shown inline so you don't need a second call.
#
# Usage: ./scripts/rebuild.sh [--skip-verify] [--no-launch]
set -uo pipefail
cd "$(dirname "$0")/.."

SKIP_VERIFY=0
NO_LAUNCH=0
for arg in "$@"; do
  case "$arg" in
    --skip-verify) SKIP_VERIFY=1 ;;
    --no-launch) NO_LAUNCH=1 ;;
    *)
      echo "unknown option: $arg" >&2
      exit 2
      ;;
  esac
done

LOG_DIR="$(mktemp -d /tmp/openbot-rebuild.XXXXXX)"

run_step() {
  local name="$1"
  shift
  local log="$LOG_DIR/${name}.log"
  echo "→ ${name}..."
  if "$@" >"$log" 2>&1; then
    echo "  ✅ ${name}"
  else
    echo "  ❌ ${name} failed — last 40 lines ($log):"
    tail -40 "$log"
    exit 1
  fi
}

if [ "$SKIP_VERIFY" -eq 0 ]; then
  run_step verify ./scripts/verify.sh
else
  echo "→ verify skipped (--skip-verify)"
fi

run_step build npm run tauri:build

APP="src-tauri/target/release/bundle/macos/openbot.app"

if [ "$NO_LAUNCH" -eq 0 ]; then
  if pkill -x openbot >/dev/null 2>&1; then
    sleep 1 # let launchd settle before relaunching, or `open` can race and no-op
  fi
  open "$APP" || open "$APP"
  echo "✅ rebuilt and launched: $APP"
else
  echo "✅ rebuilt (not launched): $APP"
fi
echo "logs: $LOG_DIR"
