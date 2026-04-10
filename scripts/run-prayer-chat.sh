#!/usr/bin/env bash
set -euo pipefail

# Launcher: boots prayer-api + prayer-mcp, then starts TUI chat.
#
# Defaults:
#   Provider: google
#   Model:    gemma-4-31b-it
#   API key:  GEMINI_API_KEY (google) or OPENAI_API_KEY (openai)
#
# Usage:
#   scripts/run-prayer-chat.sh
#
# Optional flags:
#   --provider google|openai
#   --model gemma-4-31b-it
#   --llm-base-url https://api.openai.com/v1  (openai only)
#   --api-key <key>
#   --api-bind 127.0.0.1:7777
#   --mcp-bind 127.0.0.1:5000

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_NAME="$(basename "$0")"
LOG_DIR="$ROOT_DIR/logs"
API_LOG="$LOG_DIR/run-prayer-chat-api.log"
MCP_LOG="$LOG_DIR/run-prayer-chat-mcp.log"

PROVIDER="google"
MODEL="gemma-4-31b-it"
LLM_BASE_URL="https://api.openai.com/v1"
API_KEY=""
API_BIND="127.0.0.1:7777"
MCP_BIND="127.0.0.1:5000"

log() {
  printf '[%s] %s\n' "$SCRIPT_NAME" "$*"
}

die() {
  printf '[%s] %s\n' "$SCRIPT_NAME" "$*" >&2
  exit 1
}

require_arg_value() {
  local flag="$1"
  local value="${2:-}"
  if [[ -z "$value" || "$value" == --* ]]; then
    die "Missing value for ${flag}"
  fi
}

validate_provider() {
  case "$1" in
    google|openai) ;;
    *) die "Invalid provider '$1'. Expected 'google' or 'openai'." ;;
  esac
}

usage() {
  cat <<'USAGE'
Usage:
  scripts/run-prayer-chat.sh [options]

Options:
  --provider <name>       LLM provider: google or openai (default: google)
  --model <name>          LLM model (default: gemma-4-31b-it)
  --llm-base-url <url>    LLM base URL, openai provider only (default: https://api.openai.com/v1)
  --api-key <key>         API key (default: GEMINI_API_KEY for google, OPENAI_API_KEY for openai)
  --api-bind <host:port>  prayer-api bind (default: 127.0.0.1:7777)
  --mcp-bind <host:port>  prayer-mcp bind (default: 127.0.0.1:5000)
  -h, --help              Show help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --provider)
      require_arg_value "$1" "${2:-}"
      PROVIDER="$2"
      shift 2
      ;;
    --model)
      require_arg_value "$1" "${2:-}"
      MODEL="$2"
      shift 2
      ;;
    --llm-base-url)
      require_arg_value "$1" "${2:-}"
      LLM_BASE_URL="$2"
      shift 2
      ;;
    --api-key)
      require_arg_value "$1" "${2:-}"
      API_KEY="$2"
      shift 2
      ;;
    --api-bind)
      require_arg_value "$1" "${2:-}"
      API_BIND="$2"
      shift 2
      ;;
    --mcp-bind)
      require_arg_value "$1" "${2:-}"
      MCP_BIND="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown arg: $1" >&2
      usage
      exit 1
      ;;
  esac
done

validate_provider "$PROVIDER"

# Resolve API key from env if not set explicitly
if [[ -z "$API_KEY" ]]; then
  if [[ "$PROVIDER" == "google" ]]; then
    API_KEY="${GEMINI_API_KEY:-}"
  else
    API_KEY="${OPENAI_API_KEY:-}"
  fi
fi

if [[ -z "$API_KEY" ]]; then
  if [[ "$PROVIDER" == "google" ]]; then
    echo "Missing API key. Set GEMINI_API_KEY or pass --api-key." >&2
  else
    echo "Missing API key. Set OPENAI_API_KEY or pass --api-key." >&2
  fi
  exit 1
fi

API_PID=""
MCP_PID=""

cleanup() {
  if [[ -n "$MCP_PID" ]]; then
    kill "$MCP_PID" 2>/dev/null || true
    wait "$MCP_PID" 2>/dev/null || true
  fi
  if [[ -n "$API_PID" ]]; then
    kill "$API_PID" 2>/dev/null || true
    wait "$API_PID" 2>/dev/null || true
  fi
}

show_recent_log_lines() {
  local path="$1"
  if [[ -f "$path" ]]; then
    echo
    log "Last lines from ${path}:"
    tail -n 20 "$path" || true
  fi
}

wait_for_tcp() {
  local host="$1"
  local port="$2"
  local timeout_sec="$3"
  local start
  start="$(date +%s)"

  while true; do
    if (echo >"/dev/tcp/${host}/${port}") >/dev/null 2>&1; then
      return 0
    fi
    if (( "$(date +%s)" - start >= timeout_sec )); then
      echo "Timed out waiting for ${host}:${port}" >&2
      return 1
    fi
    sleep 0.1
  done
}

preflight_checks() {
  command -v cargo >/dev/null 2>&1 || die "cargo not found in PATH."
}

trap cleanup EXIT

preflight_checks
mkdir -p "$LOG_DIR"

cd "$ROOT_DIR"

log "Starting prayer-api on ${API_BIND}..."
PRAYER_RS_BIND="$API_BIND" cargo run -q -p prayer-api >"$API_LOG" 2>&1 &
API_PID="$!"
if ! wait_for_tcp "${API_BIND%:*}" "${API_BIND##*:}" 20; then
  show_recent_log_lines "$API_LOG"
  die "prayer-api failed to become ready."
fi

log "Starting prayer-mcp on ${MCP_BIND}..."
cargo run -q -p prayer-mcp -- \
  --prayer-url "http://${API_BIND}" \
  --transport sse \
  --bind "$MCP_BIND" >"$MCP_LOG" 2>&1 &
MCP_PID="$!"
if ! wait_for_tcp "${MCP_BIND%:*}" "${MCP_BIND##*:}" 20; then
  show_recent_log_lines "$MCP_LOG"
  die "prayer-mcp failed to become ready."
fi

log "Launching prayer-mcp-client chat..."
echo

EXTRA_ARGS=()
if [[ "$PROVIDER" == "openai" ]]; then
  EXTRA_ARGS+=(--llm-base-url "$LLM_BASE_URL")
fi

log "Provider=${PROVIDER} Model=${MODEL}"
log "API=${API_BIND} MCP=${MCP_BIND}"
log "Logs: $API_LOG | $MCP_LOG"

cargo run -q -p prayer-mcp-client -- chat \
  --provider "$PROVIDER" \
  --model "$MODEL" \
  --api-key "$API_KEY" \
  --mcp-url "http://${MCP_BIND}/mcp" \
  "${EXTRA_ARGS[@]}"
