#!/usr/bin/env bash
set -euo pipefail

# Launcher for prayer-mcp-client-ts (TypeScript/web version).
#
# Boots prayer-api + prayer-mcp (same as run-prayer-chat.sh), then starts
# the TS Express+React server and opens the browser.
#
# Defaults:
#   Provider: google
#   Model:    gemma-4-31b-it
#   API key:  GEMINI_API_KEY (google) or OPENAI_API_KEY (openai)
#
# Usage:
#   scripts/run-prayer-chat-ts.sh [options]
#
# Options:
#   --provider google|openai
#   --model <name>
#   --llm-base-url <url>       (openai only)
#   --api-key <key>
#   --api-bind 127.0.0.1:7777
#   --mcp-bind 127.0.0.1:5000
#   --port 3001                (TS server port)
#   --no-browser               skip opening the browser

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TS_DIR="$ROOT_DIR/prayer-mcp-client-ts"

PROVIDER="google"
MODEL="gemma-4-31b-it"
LLM_BASE_URL="https://api.openai.com/v1"
API_KEY=""
API_BIND="127.0.0.1:7777"
MCP_BIND="127.0.0.1:5000"
TS_PORT="3001"
OPEN_BROWSER=true

usage() {
  cat <<'USAGE'
Usage:
  scripts/run-prayer-chat-ts.sh [options]

Options:
  --provider <name>       LLM provider: google or openai (default: google)
  --model <name>          LLM model (default: gemma-4-31b-it)
  --llm-base-url <url>    LLM base URL, openai provider only
  --api-key <key>         API key (default: GEMINI_API_KEY / OPENAI_API_KEY)
  --api-bind <host:port>  prayer-api bind (default: 127.0.0.1:7777)
  --mcp-bind <host:port>  prayer-mcp bind (default: 127.0.0.1:5000)
  --port <n>              TS server port (default: 3001)
  --no-browser            Don't open the browser automatically
  -h, --help              Show help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --provider)    PROVIDER="$2";     shift 2 ;;
    --model)       MODEL="$2";        shift 2 ;;
    --llm-base-url) LLM_BASE_URL="$2"; shift 2 ;;
    --api-key)     API_KEY="$2";      shift 2 ;;
    --api-bind)    API_BIND="$2";     shift 2 ;;
    --mcp-bind)    MCP_BIND="$2";     shift 2 ;;
    --port)        TS_PORT="$2";      shift 2 ;;
    --no-browser)  OPEN_BROWSER=false; shift ;;
    -h|--help)     usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 1 ;;
  esac
done

# Resolve API key from env if not passed explicitly
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

# Check node / npm are available
if ! command -v node &>/dev/null; then
  echo "node not found. Install Node.js 20+." >&2
  exit 1
fi

# Install deps and build frontend if needed
if [[ ! -d "$TS_DIR/node_modules" ]]; then
  echo "Installing npm dependencies..."
  (cd "$TS_DIR" && npm install --silent)
fi

echo "Building..."
(cd "$TS_DIR" && npm run build)

API_PID=""
MCP_PID=""
TS_PID=""

cleanup() {
  [[ -n "$TS_PID"  ]] && { kill "$TS_PID"  2>/dev/null || true; wait "$TS_PID"  2>/dev/null || true; }
  [[ -n "$MCP_PID" ]] && { kill "$MCP_PID" 2>/dev/null || true; wait "$MCP_PID" 2>/dev/null || true; }
  [[ -n "$API_PID" ]] && { kill "$API_PID" 2>/dev/null || true; wait "$API_PID" 2>/dev/null || true; }
}

wait_for_tcp() {
  local host="$1" port="$2" timeout_sec="$3"
  local start; start="$(date +%s)"
  while true; do
    if (echo >"/dev/tcp/${host}/${port}") >/dev/null 2>&1; then return 0; fi
    if (( "$(date +%s)" - start >= timeout_sec )); then
      echo "Timed out waiting for ${host}:${port}" >&2; return 1
    fi
    sleep 0.1
  done
}

trap cleanup EXIT
cd "$ROOT_DIR"

echo "Starting prayer-api on ${API_BIND}..."
PRAYER_RS_BIND="$API_BIND" cargo run -q -p prayer-api &
API_PID="$!"
wait_for_tcp "${API_BIND%:*}" "${API_BIND##*:}" 20

echo "Starting prayer-mcp on ${MCP_BIND}..."
cargo run -q -p prayer-mcp -- \
  --prayer-url "http://${API_BIND}" \
  --transport sse \
  --bind "$MCP_BIND" &
MCP_PID="$!"
wait_for_tcp "${MCP_BIND%:*}" "${MCP_BIND##*:}" 20

EXTRA_ARGS=()
if [[ "$PROVIDER" == "openai" ]]; then
  EXTRA_ARGS+=(--llm-base-url "$LLM_BASE_URL")
fi

echo "Starting prayer-mcp-client-ts on port ${TS_PORT}..."
(cd "$TS_DIR" && node dist/server/index.js \
  --provider "$PROVIDER" \
  --model "$MODEL" \
  --api-key "$API_KEY" \
  --mcp-url "http://${MCP_BIND}/mcp" \
  --port "$TS_PORT" \
  "${EXTRA_ARGS[@]}" \
) &
TS_PID="$!"
wait_for_tcp "127.0.0.1" "$TS_PORT" 15

echo
echo "Prayer Chat ready at http://localhost:${TS_PORT}"
echo "Press Ctrl+C to stop."
echo

if $OPEN_BROWSER; then
  if command -v xdg-open &>/dev/null; then
    xdg-open "http://localhost:${TS_PORT}" &>/dev/null &
  elif command -v open &>/dev/null; then
    open "http://localhost:${TS_PORT}" &>/dev/null &
  fi
fi

wait "$TS_PID"
