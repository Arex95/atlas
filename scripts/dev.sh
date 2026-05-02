#!/usr/bin/env bash

set -e
cd "$(dirname "$0")/.."

if [[ ! -f .env ]]; then
  echo "==> .env not found; copying .env.example"
  cp .env.example .env
fi
if [[ ! -f apps/web/.env ]]; then
  echo "==> apps/web/.env not found; copying apps/web/.env.example"
  cp apps/web/.env.example apps/web/.env
fi

trap 'echo "==> stopping"; kill 0' INT TERM EXIT

echo "==> starting atlas-server (Rust) on :4000"
( cargo run -p atlas-server 2>&1 | sed 's/^/[server] /' ) &
SERVER_PID=$!

echo "==> starting web (Vite) on :3000"
( pnpm --filter web dev 2>&1 | sed 's/^/[web]    /' ) &
WEB_PID=$!

wait -n "$SERVER_PID" "$WEB_PID"
