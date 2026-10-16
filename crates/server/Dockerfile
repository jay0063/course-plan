# syntax=docker/dockerfile:1
#
# CoursePlan 同步服务端镜像（多阶段构建）。
#
# 契约（docs/PLAN.md §5 / docs/DEPLOY.md）:
#   二进制名  course-plan-server
#   PORT      默认 8080
#   DATA_DIR  默认 /data（DB = /data/courseplan.db，声明为 VOLUME）
#   JWT_SECRET 必填（缺失则服务端启动失败）
#   RUST_LOG  默认 info
#   健康检查  GET /api/health → 200 JSON
#
# 构建（仓库根目录为构建上下文）:
#   docker build -f crates/server/Dockerfile -t courseplan-server:dev .

# ---------------------------------------------------------------------------
# 阶段 1/2：builder
# ---------------------------------------------------------------------------
FROM rust:1-slim-bookworm AS builder

WORKDIR /app

# 先复制全部清单（含 Cargo.lock）以缓存依赖编译层；源码在下一步再覆盖。
# 注意：workspace 的每个成员包都必须在清单解析时存在目标文件，因此这里先用
# 占位桩（stub）满足 cargo，编译完依赖后由真实源码覆盖并重新编译。
COPY Cargo.toml Cargo.lock ./
COPY crates/sync-proto/Cargo.toml crates/sync-proto/Cargo.toml
COPY crates/server/Cargo.toml crates/server/Cargo.toml

RUN mkdir -p src crates/sync-proto/src crates/server/src \
    && printf 'fn main() {}\n' > src/main.rs \
    && printf '\n' > crates/sync-proto/src/lib.rs \
    && printf 'fn main() {}\n' > crates/server/src/main.rs \
    && cargo build --release --locked -p course-plan-server

# 复制真实工作区源码。COPY 后统一 touch，避免构建缓存因 mtime 判定源码未变。
COPY . .

RUN find src crates -name '*.rs' -exec touch {} + \
    && cargo build --release --locked -p course-plan-server

# ---------------------------------------------------------------------------
# 阶段 2/2：runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# ca-certificates：出站 TLS（如后续接入外部依赖）
# curl：仅用于容器 HEALTHCHECK
# sqlite3：DEPLOY.md §5 的容器内在线热备 / VACUUM INTO 命令依赖该 CLI
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl sqlite3 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 app \
    && useradd --system --uid 10001 --gid 10001 --home-dir /home/app --create-home --shell /usr/sbin/nologin app

COPY --from=builder /app/target/release/course-plan-server /usr/local/bin/course-plan-server

# 数据卷目录归非 root 用户所有（命名卷首次挂载时继承该属主）
RUN mkdir -p /data && chown app:app /data

ENV DATA_DIR=/data \
    PORT=8080 \
    RUST_LOG=info

EXPOSE 8080
VOLUME ["/data"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/api/health || exit 1

USER app
WORKDIR /home/app

ENTRYPOINT ["/usr/local/bin/course-plan-server"]
