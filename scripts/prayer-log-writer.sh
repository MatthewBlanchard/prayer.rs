#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: prayer-log-writer.sh <run-log-dir> <service-name> <latest-log-dir>" >&2
  exit 2
fi

RUN_LOG_DIR="$1"
SERVICE_NAME="$2"
LATEST_LOG_DIR="$3"
MAX_FILE_MB="${PRAYER_LOG_MAX_FILE_MB:-25}"
MAX_FILE_BYTES=$((MAX_FILE_MB * 1024 * 1024))

mkdir -p "$RUN_LOG_DIR" "$LATEST_LOG_DIR"

ACTIVE_LOG="$RUN_LOG_DIR/${SERVICE_NAME}.log"
LATEST_LINK="$LATEST_LOG_DIR/${SERVICE_NAME}.log"
ROTATE_INDEX=0

: >"$ACTIVE_LOG"
ln -sfn "$ACTIVE_LOG" "$LATEST_LINK"

log_size() {
  if [[ -f "$ACTIVE_LOG" ]]; then
    wc -c <"$ACTIVE_LOG"
  else
    echo 0
  fi
}

rotate_if_needed() {
  local size
  size="$(log_size)"
  if (( size < MAX_FILE_BYTES )); then
    return 0
  fi

  local stamp rotated
  stamp="$(date +%Y-%m-%d_%H-%M-%S)"
  ROTATE_INDEX=$((ROTATE_INDEX + 1))
  rotated="$RUN_LOG_DIR/${SERVICE_NAME}.${stamp}.${ROTATE_INDEX}.log"
  mv "$ACTIVE_LOG" "$rotated"
  : >"$ACTIVE_LOG"
  ln -sfn "$ACTIVE_LOG" "$LATEST_LINK"

  if command -v gzip >/dev/null 2>&1; then
    gzip -f "$rotated" &
  fi
}

while IFS= read -r line || [[ -n "$line" ]]; do
  printf '%s\n' "$line" >>"$ACTIVE_LOG"
  rotate_if_needed
done
