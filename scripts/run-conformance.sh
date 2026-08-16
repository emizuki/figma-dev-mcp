#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ADAPTER_PID=""
cleanup() {
  if [[ -n "${ADAPTER_PID}" ]]; then
    kill "${ADAPTER_PID}" 2>/dev/null || true
    wait "${ADAPTER_PID}" 2>/dev/null || true
    ADAPTER_PID=""
  fi
}
trap cleanup EXIT INT TERM

cargo build -p figma-dev-mcp-tests --bin conformance-server
"$ROOT/target/debug/conformance-server" &
ADAPTER_PID=$!

ready=0
for _ in $(seq 1 100); do
  if curl -sf "http://127.0.0.1:3060/healthz" >/dev/null; then
    ready=1
    break
  fi
  if ! kill -0 "${ADAPTER_PID}" 2>/dev/null; then
    echo "conformance-server exited before becoming ready" >&2
    wait "${ADAPTER_PID}" || true
    exit 1
  fi
  sleep 0.1
done
if [[ "${ready}" -ne 1 ]]; then
  echo "conformance-server did not become ready on 127.0.0.1:3060" >&2
  exit 1
fi

(
  cd "$ROOT/conformance"
  bun run modern
  bun run legacy
)
