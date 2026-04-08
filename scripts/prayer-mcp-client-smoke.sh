#!/usr/bin/env bash
set -euo pipefail

# One-command local smoke test for prayer-mcp-client.
#
# Usage:
#   scripts/prayer-mcp-client-smoke.sh --llm-base-url http://127.0.0.1:11434/v1 --model llama3.1
#
# Optional overrides:
#   --message "..."
#   --mcp-url http://127.0.0.1:5000/mcp
#   --api-bind 127.0.0.1:7777
#   --mcp-bind 127.0.0.1:5000
#   --api-key <key>

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

LLM_BASE_URL=""
MODEL=""
MESSAGE="List current sessions and summarize what you see."
MCP_URL=""
API_BIND="127.0.0.1:7777"
MCP_BIND="127.0.0.1:5000"
API_KEY=""

usage() {
  cat <<'USAGE'
Usage:
  scripts/prayer-mcp-client-smoke.sh --llm-base-url <url> --model <model> [options]

Options:
  --message <text>      User message for the chat turn
  --mcp-url <url>       MCP endpoint URL (default: http://<mcp-bind>/mcp)
  --api-bind <host:port>  prayer-api bind (default: 127.0.0.1:7777)
  --mcp-bind <host:port>  prayer-mcp bind (default: 127.0.0.1:5000)
  --api-key <key>       LLM API key (optional)
  -h, --help            Show this help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --llm-base-url)
      LLM_BASE_URL="$2"
      shift 2
      ;;
    --model)
      MODEL="$2"
      shift 2
      ;;
    --message)
      MESSAGE="$2"
      shift 2
      ;;
    --mcp-url)
      MCP_URL="$2"
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
    --api-key)
      API_KEY="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown arg: $1" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$LLM_BASE_URL" || -z "$MODEL" ]]; then
  echo "Missing required args: --llm-base-url and --model" >&2
  exit 1
fi

if [[ -z "$MCP_URL" ]]; then
  MCP_URL="http://${MCP_BIND}/mcp"
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

echo "Running prayer-mcp-client chat..."
CMD=(
  cargo run -q -p prayer-mcp-client -- chat
  --llm-base-url "$LLM_BASE_URL"
  --model "$MODEL"
  --mcp-url "$MCP_URL"
  --message "$MESSAGE"
)

if [[ -n "$API_KEY" ]]; then
  CMD+=(--api-key "$API_KEY")
fi

"${CMD[@]}"
