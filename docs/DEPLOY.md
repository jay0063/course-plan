# CoursePlan 同步服务端部署指南

本文档说明如何使用 Docker / Docker Compose 部署 CoursePlan 的同步服务端
（`course-plan-server`：Rust + axum + SQLite，单容器）。

> 相关契约见 `docs/PLAN.md` §5 与 `docs/SYNC-PROTOCOL.md` §3/§4/§8。

## 1. 前置条件

- 一台可长期运行的 Linux 主机（VPS / NAS / 家用服务器均可）
- Docker Engine ≥ 24 与 Docker Compose v2（`docker compose version` 有输出即可）
- **容器运行时权限**：当前用户需能访问 Docker 守护进程。若不在 `docker` 组，请在命令前加 `sudo`，
  或执行 `sudo usermod -aG docker $USER` 后重新登录（本仓库文档中的命令均按不加 `sudo` 书写）
- 若启用 TLS：一个已解析到本机的域名，并放通 80/443 端口

## 2. 快速开始

```bash
git clone <仓库地址> course-plan
cd course-plan

# 1) 准备环境变量
cp .env.example .env
#    生成并填入 JWT_SECRET（必填）
openssl rand -hex 32

# 2) 构建并启动
docker compose up -d --build

# 3) 验证健康
curl -fsS http://127.0.0.1:8080/api/health
# {"status":"ok","version":"..."}
```

客户端在「设置 → 同步」中填写 `服务器地址`（如 `http://<主机>:8080`）即可注册/登录。

### 2.1 部署自检（建议首次上线逐项执行）

以下清单已在开发机（Docker 26.1.5 + Compose v2）上实机验证通过，可直接照抄：

```bash
# 1) 健康检查
curl -fsS http://127.0.0.1:8080/api/health

# 2) 注册 → 推送 → 拉取 往返（应分别返回 applied 与 changes）
TOKEN=$(curl -fsS -X POST http://127.0.0.1:8080/api/auth/register \
  -H 'content-type: application/json' \
  -d '{"username":"selfcheck","password":"selfcheck-pass-123"}' | python3 -c 'import json,sys;print(json.load(sys.stdin)["token"])')
NOW=$(date +%s%3N)
curl -fsS -X POST http://127.0.0.1:8080/api/sync/push \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d "{\"changes\":[{\"kind\":\"memo\",\"id\":\"selfcheck-1\",\"updated_at\":$NOW,\"deleted\":false,\"device_id\":\"cli\",\"payload\":{\"id\":\"selfcheck-1\"}}]}"
curl -fsS "http://127.0.0.1:8080/api/sync/pull?since=0" -H "authorization: Bearer $TOKEN"

# 3) 重启后数据仍在（卷持久化）
docker compose restart && sleep 3
curl -fsS "http://127.0.0.1:8080/api/sync/pull?since=0" -H "authorization: Bearer $TOKEN"

# 4) 备份演练（方法 A：容器内 .backup，WAL 安全）
docker compose exec server sh -c 'sqlite3 /data/courseplan.db ".backup /data/courseplan-selfcheck.db"'
docker compose exec server sh -c 'sqlite3 /data/courseplan-selfcheck.db "pragma integrity_check; select count(*) from entities;"'

# 5) 清理（会删除卷内数据，演练环境专用）
docker compose down -v
```

> 仓库内还带一条端到端测试：`COURSEPLAN_TEST_SERVER=http://127.0.0.1:8080 cargo test --test sync_e2e mode2_external_server_roundtrip`
> （用真实客户端引擎对已部署服务端做注册/推送/拉取断言；`scripts/e2e-server.sh` 可一键对 release 二进制跑同样的检查）。

## 3. 环境变量

| 变量 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `JWT_SECRET` | ✅ 是 | — | JWT（HS256）签名密钥。**缺失时服务端启动失败**。建议 `openssl rand -hex 32`。更换后所有已签发 Token 失效，客户端需重新登录。 |
| `PORT` | 否 | `8080` | 服务端监听端口（容器内）。 |
| `DATA_DIR` | 否 | `/data` | 数据目录（容器内），SQLite 库位于 `${DATA_DIR}/courseplan.db`。compose 已挂载命名卷 `courseplan-data` 到 `/data`。 |
| `RUST_LOG` | 否 | `info` | 日志级别：`error` / `warn` / `info` / `debug` / `trace`。 |
| `COURSEPLAN_BIND` | 否 | `0.0.0.0` | 宿主机绑定地址（`docker-compose.yml` 的 `ports` 映射）。默认监听所有网卡；启用 TLS 叠加时建议设为 `127.0.0.1`（见 §7）。 |
| `COURSEPLAN_DOMAIN` | TLS 时需要 | — | 域名（仅 `docker-compose.tls.yml` 使用），Caddy 据此申请证书。 |

> `.env` 中已包含 `JWT_SECRET`、`COURSEPLAN_BIND`、`COURSEPLAN_DOMAIN`、`RUST_LOG` 四项。
> Compose 对必填项使用了 `${JWT_SECRET:?...}` 语法——未设置时会直接报错并提示查看 `.env.example`。

## 4. 数据卷与持久化

- 数据保存在命名卷 **`courseplan-data`**（`docker-compose.yml` 用 `name:` 显式固定该名字，
  不受 compose 项目名影响），挂载到容器 `/data`。
- 数据库文件：`/data/courseplan.db`。SQLite 使用 **WAL 模式**（`journal_mode=WAL`），
  运行期间还存在 `courseplan.db-wal`、`courseplan.db-shm` 两个旁路文件。
- 服务端以非 root 用户 `app`（uid 10001）运行；数据目录属主为 `app`。

查看卷实际路径：

```bash
docker volume inspect courseplan-data
```

## 5. 备份与恢复 ⚠️

> **WAL 模式下绝对不要直接 `cp` / `rsync` 正在使用的 `courseplan.db`。**
> 未 checkpoint 的已提交数据仍在 `-wal` 文件中，裸拷会得到**不一致或缺失**的副本。
> 安全做法只有三种：① SQLite `.backup`；② `VACUUM INTO`；③ **停机后**整卷拷贝（体积快照）。

备份文件统一命名 `courseplan-YYYY-MM-DD.db`（数据库快照）或 `courseplan-YYYY-MM-DD.tar.gz`（整卷打包）。

### 方式 A：sqlite3 `.backup`（在线热备，推荐）

在容器内调用 SQLite 备份命令，得到一致性快照：

```bash
docker compose exec server \
  sqlite3 /data/courseplan.db ".backup '/data/courseplan-$(date +%F).db'"

# 把快照取出到宿主机
docker compose cp server:/data/courseplan-$(date +%F).db ./courseplan-$(date +%F).db
```

> 镜像已内置 `sqlite3` CLI（见 `crates/server/Dockerfile`）。若自定义镜像移除了它，
> 可停机后在宿主机对卷内文件执行：`sqlite3 <卷路径>/courseplan.db ".backup '<目标>.db'"`。

### 方式 B：`VACUUM INTO`（SQL 一致性快照）

```bash
docker compose exec server \
  sqlite3 /data/courseplan.db "VACUUM INTO '/data/courseplan-$(date +%F).db'"
```

同样用 `docker compose cp server:/data/courseplan-$(date +%F).db ./courseplan-$(date +%F).db` 取出。

### 方式 C：停机冷备（最稳妥）

先停服务端，再对**已停止**的卷做整卷打包；恢复时 `-wal`/`-shm` 会被下游一起还原：

```bash
docker compose stop server
docker run --rm -v courseplan-data:/data -v "$PWD":/backup debian:bookworm-slim \
  tar czf /backup/courseplan-$(date +%F).tar.gz -C /data .
docker compose start server
```

建议配合 cron / 定时任务定期执行，并把备份转移到异地。

### 恢复（务必先停服务端）

**恢复前先 `docker compose stop server`**——对正在运行的库覆盖文件会损坏数据。
然后删除旧库与其 WAL 旁路文件、写入备份、把数据目录交还 uid `10001` 属主，最后再启动：

```bash
# 1) 停服务端（绝不能在运行中覆盖 .db）
docker compose stop server

# 2) 清掉旧库 + 残留的 -wal / -shm，写入备份，修复属主
docker run --rm -v courseplan-data:/data -v "$PWD":/backup debian:bookworm-slim \
  sh -c "rm -f /data/courseplan.db /data/courseplan.db-wal /data/courseplan.db-shm \
         && cp /backup/courseplan-YYYY-MM-DD.db /data/courseplan.db \
         && chown -R 10001:10001 /data"

# 3) 重新启动（迁移幂等，健康后即恢复服务）
docker compose start server
```

若备份是方式 C 的 `courseplan-YYYY-MM-DD.tar.gz`，把第 2 步换成解包到 `/data` 再 `chown -R 10001:10001 /data` 即可。

> 恢复演练建议按人工 checklist 执行：备份 → 停服务 → 覆盖 → 启服务 → `curl /api/health`
> 且客户端能登录拉取数据，才算验证通过。

## 6. 升级

迁移（`sqlx::migrate!`）在服务端启动时**自动执行**，无需手工操作：

```bash
cd course-plan
git pull
docker compose build server
docker compose up -d
docker compose logs -f server   # 观察迁移与启动日志
```

> 升级前请先按上一节做一次备份。若从旧版本跨多个版本升级，同样只需拉取代码后重建；
> 迁移按顺序幂等应用。

## 7. 启用 HTTPS（强推荐）

> **明文 HTTP 会以明文传输账号密码与 JWT**——在公网环境下形同裸奔。
> 生产部署**务必**叠加 TLS。仓库提供 Caddy 自动 HTTPS 叠加。

> ⚠️ **叠加 TLS 后，务必在 `.env` 设 `COURSEPLAN_BIND=127.0.0.1`**（或用防火墙封禁 8080）。
> 否则服务端的 8080 明文端口仍会与 HTTPS 并存对外可达，攻击者可直接绕过 Caddy 用明文连接。

```bash
# 1) 准备 Caddyfile（仓库内提供示例）
cp deploy/Caddyfile.example deploy/Caddyfile
#    ⚠️ 必须先执行上面的复制步骤：compose 把 ./deploy/Caddyfile 绑定挂载到容器内。
#    若该文件不存在，Docker 会在宿主机创建一个同名**目录**，导致 Caddy 以
#    「is a directory / not a file」等令人困惑的错误启动失败。

# 2) 在 .env 中填写域名与绑定地址
#    COURSEPLAN_DOMAIN=course.example.com   （需已解析到本机，且放通 80/443）
#    COURSEPLAN_BIND=127.0.0.1               （避免明文 8080 端口直接暴露）

# 3) 启动（基础 + TLS 叠加）
docker compose -f docker-compose.yml -f docker-compose.tls.yml up -d --build
```

Caddy 会自动向 Let's Encrypt 申请并续期证书，将请求反代到 `server:8080`，
证书持久化在 `courseplan-caddy-data` / `courseplan-caddy-config` 卷（`docker-compose.tls.yml`
已用 `name:` 显式固定）。客户端服务器地址填 `https://<域名>`。

### 备选：使用 nginx 反向代理

若不使用 Caddy，可用 nginx（自行申请证书，如 certbot）：

```nginx
server {
    listen 443 ssl;
    server_name course.example.com;

    ssl_certificate     /etc/letsencrypt/live/course.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/course.example.com/privkey.pem;

    # 推送批次最大 4 MiB，留出余量
    client_max_body_size 8m;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host              $host;
        proxy_set_header X-Real-IP         $remote_addr;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

server {
    listen 80;
    server_name course.example.com;
    return 301 https://$host$request_uri;
}
```

## 8. 健康检查、日志与排障

**健康检查**：容器内置 `HEALTHCHECK`，每 30s 请求一次 `GET /api/health`。

```bash
docker compose ps                       # STATUS 列显示 healthy
curl -fsS http://127.0.0.1:8080/api/health
docker inspect --format '{{.State.Health.Status}}' $(docker compose ps -q server)
```

**日志**：

```bash
docker compose logs -f server
```

排查时可临时在 `.env` 设 `RUST_LOG=debug` 后 `docker compose up -d` 重启。

**常见问题**：

| 现象 | 原因与处理 |
| --- | --- |
| 启动即退出，日志提示缺少 `JWT_SECRET` | `.env` 未设置或未生效；确认 `.env` 与 compose 同目录，值非空。 |
| `docker compose` 报 `JWT_SECRET must be set` | 同上，compose 的必填校验。 |
| 端口 8080 被占用 | 修改 compose 的 `ports` 映射（如 `"18080:8080"`），或释放端口。 |
| 客户端连不上 / 超时 | 检查主机防火墙放通端口；TLS 场景确认 80/443 可达、域名解析正确。 |
| 登录返回 401 | Token 过期或 `JWT_SECRET` 变更；重新登录。 |
| Caddy 无法签发证书 | 域名未解析到本机，或 80/443 未放通；查看 `docker compose logs caddy`。 |
| 容器 `unhealthy` | 查看 `logs`；确认服务端已监听 8080 且 `/api/health` 返回 200。 |

## 9. 安全注意事项

- **务必启用 HTTPS**（§7）。明文 HTTP 下密码与 JWT 可被中间人截获。
- `JWT_SECRET` 视为机密：使用高熵随机值，不入库、不提交、不外泄；轮换后需重新登录。
- 服务端默认监听全网卡；如仅内网使用，可用防火墙限制来源 IP。
- 数据库卷含明文用户数据；备份文件同样需要访问控制。
- 账号密码以 argon2id（随机盐）存储，服务端不保存明文密码。
- 本版本不含限流/风控、账号找回、端到端加密（见 `docs/PLAN.md` 非目标），请勿直接暴露于不受信任网络。
- 定期按 §5 备份并验证可恢复。

## 10. 卸载

```bash
docker compose down            # 停止并移除容器与网络，保留数据卷
docker image rm courseplan-server:dev
```

> **数据卷 `courseplan-data`（及 TLS 的 `courseplan-caddy-data` / `courseplan-caddy-config`）会被保留**，
> 便于重装后继续使用。若要连同数据一起彻底删除：

```bash
docker compose down -v         # ⚠️ 删除命名卷 → 数据不可恢复，请先备份
```

镜像与 compose 文件均可从仓库重新获取；被删除的数据无法恢复，操作前请确认已备份。
