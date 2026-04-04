#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION_NAME="${ATLAS2_TMUX_SESSION:-atlas2}"
WINDOW_INDEX="${ATLAS2_TMUX_WINDOW:-0}"
TARGET="${SESSION_NAME}:${WINDOW_INDEX}"
BINARY_PATTERN="(${ROOT_DIR}/)?target/debug/atlas2"

cd "$ROOT_DIR"

cargo build

if ! tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
  tmux new-session -d -s "$SESSION_NAME" -c "$ROOT_DIR"
fi

tmux send-keys -t "$TARGET" C-c
sleep 1
tmux send-keys -t "$TARGET" "cd '$ROOT_DIR' && cargo run" Enter

for _ in $(seq 1 30); do
  if pgrep -f "$BINARY_PATTERN" >/dev/null; then
    echo "atlas2 is running"
    exit 0
  fi
  sleep 1
done

echo "atlas2 did not come up after restart" >&2
tmux capture-pane -pt "$TARGET" >&2 || true
exit 1
