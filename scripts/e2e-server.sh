#!/usr/bin/env bash
# Phase 5 模式 2 的本地等价路径：把 release 服务端当作真实 OS 进程启动，
# 让 tests/sync_e2e.rs 的 `mode2_external_server_roundtrip` 对接它。
#
# 用法：
#   scripts/e2e-server.sh            # 默认端口 18099
#   PORT=18099 scripts/e2e-server.sh
#
# 说明：本机未安装 Docker；对接容器时改用
#   cp .env.example .env && $EDITOR .env
#   docker compose up -d --build
#   COURSEPLAN_TEST_SERVER=http://127.0.0.1:8080 cargo test --test sync_e2e -- --nocapture
set -euo pipefail

PORT="${PORT:-18099}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DATA_DIR="$(mktemp -d "${TMPDIR:-/tmp}/courseplan-e2e-XXXXXX")"
JWT_SECRET="e2e-test-secret"
SERVER_LOG="$(mktemp "${TMPDIR:-/tmp}/courseplan-e2e-server-XXXXXX.log")"

cleanup() {
    if [[ -n "${SERVER_PID:-}" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    rm -rf "$DATA_DIR"
}
trap cleanup EXIT

echo "==> cargo build --release -p course-plan-server"
cargo build --release -p course-plan-server

echo "==> 启动 target/release/course-plan-server (PORT=$PORT, DATA_DIR=$DATA_DIR)"
DATA_DIR="$DATA_DIR" JWT_SECRET="$JWT_SECRET" PORT="$PORT" RUST_LOG=info \
    ./target/release/course-plan-server >"$SERVER_LOG" 2>&1 &
SERVER_PID=$!

# 等待 /api/health 就绪（最多 ~15s）。
for _ in $(seq 1 75); do
    if curl -fsS "http://127.0.0.1:$PORT/api/health" >/dev/null 2>&1; then
        break
    fi
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "服务端提前退出，日志：" >&2
        cat "$SERVER_LOG" >&2
        exit 1
    fi
    sleep 0.2
done

echo "==> 服务端日志（前 20 行）"
sed -n '1,20p' "$SERVER_LOG"

echo "==> 运行模式 2 测试"
COURSEPLAN_TEST_SERVER="http://127.0.0.1:$PORT" \
    cargo test --test sync_e2e mode2_external_server_roundtrip -- --nocapture
