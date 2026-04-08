#!/usr/bin/env bash
set -euo pipefail

# Super-simple launcher: boots prayer-api + prayer-mcp, then starts TUI chat.
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

PROVIDER="google"
MODEL="gemma-4-31b-it"
LLM_BASE_URL="https://api.openai.com/v1"
API_KEY=""
API_BIND="127.0.0.1:7777"
MCP_BIND="127.0.0.1:5000"

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
      PROVIDER="$2"
      shift 2
      ;;
    --model)
      MODEL="$2"
      shift 2
      ;;
    --llm-base-url)
      LLM_BASE_URL="$2"
      shift 2
      ;;
    --api-key)
      API_KEY="$2"
      shift 2
      ;;
    --api-bind)
      API_BIND="$2"
      shift 2
      ;;
    --mcp-bind)
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

trap cleanup EXIT

cd "$ROOT_DIR"

echo "Starting prayer-api on ${API_BIND}..."
PRAYER_RS_BIND="$API_BIND" cargo run -q -p prayer-api >/dev/null 2>&1 &
API_PID="$!"
wait_for_tcp "${API_BIND%:*}" "${API_BIND##*:}" 20

echo "Starting prayer-mcp on ${MCP_BIND}..."
cargo run -q -p prayer-mcp -- \
  --prayer-url "http://${API_BIND}" \
  --transport sse \
  --bind "$MCP_BIND" >/dev/null 2>&1 &
MCP_PID="$!"
wait_for_tcp "${MCP_BIND%:*}" "${MCP_BIND##*:}" 20

echo "Launching prayer-mcp-client chat..."
echo

EXTRA_ARGS=()
if [[ "$PROVIDER" == "openai" ]]; then
  EXTRA_ARGS+=(--llm-base-url "$LLM_BASE_URL")
fi

cargo run -q -p prayer-mcp-client -- chat \
  --provider "$PROVIDER" \
  --model "$MODEL" \
  --api-key "$API_KEY" \
  --mcp-url "http://${MCP_BIND}/mcp" \
  "${EXTRA_ARGS[@]}"
