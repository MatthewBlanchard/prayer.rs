#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

cargo test -p prayer-mcp-client
cargo test -p prayer-mcp-client --test local_mcp_e2e -- --ignored
