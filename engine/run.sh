#!/usr/bin/env bash
# Build and run the D.A.D engine locally on http://127.0.0.1:${PORT:-8791}.
# NAME picks the container (default dad-engine), so a second copy can run beside one that is busy:
#   NAME=dad-engine-dev PORT=8792 engine/run.sh
# Question code is untrusted, so the container gets a read-only root,
# a small tmpfs, and CPU/memory/process caps.
set -euo pipefail
cd "$(dirname "$0")"
PORT="${PORT:-8791}"
NAME="${NAME:-dad-engine}"
docker build -q -t dad-engine . >/dev/null
docker rm -f "$NAME" >/dev/null 2>&1 || true
# Local development uses the default bridge: Docker does not publish ports from an
# --internal network. In production the engine joins an internal network that only the
# D.A.D API can reach, so it has no way out. PHP URL access is off in php.ini either way.
docker run -d --name "$NAME" \
  -p "127.0.0.1:${PORT}:8080" \
  --read-only --tmpfs /tmp:size=16m \
  --cpus 2 --memory 256m --pids-limit 64 \
  --cap-drop ALL --security-opt no-new-privileges \
  -e PHP_CLI_SERVER_WORKERS=8 \
  dad-engine >/dev/null
for _ in $(seq 1 30); do
  curl -fsS "http://127.0.0.1:${PORT}/health" >/dev/null 2>&1 && { echo "engine up on http://127.0.0.1:${PORT}"; exit 0; }
  sleep 0.3
done
echo "engine did not come up"; docker logs "$NAME" | tail -20; exit 1
