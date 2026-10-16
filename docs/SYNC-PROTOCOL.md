# CoursePlan 同步协议 v1（冻结真源）

> 状态: 待 Oracle 评审 (Gate 0)。客户端与服务端共用 `crates/sync-proto` 中的 Rust 类型，本文件为语义真源。

## 1. 模型

- **账号（account）**：用户注册的家庭账号，拥有一个独立数据空间。所有实体按账号隔离。
- **设备（device）**：客户端启动时生成 `device_id`（随机字符串，持久在客户端 `sync.json`）。
- **实体（entity）**：`kind + id` 唯一定位的一条记录。kind ∈ `profile | course | session | event | reminder | checkin | memo`。`profile` 为单例，id 固定 `"main"`。
- **墓碑（tombstone）**：删除 = 置 `deleted=true` 并盖时间戳，不物理删除；实体保留 `payload`（可为 null）供其他设备收敛。

## 2. 时间戳与冲突规则（LWW）

- 每次本地修改：`updated_at = 本地 unix 毫秒`。
- **合并谓词**（`sync-proto::wins(incoming, existing)`）：`incoming.updated_at > existing.updated_at` 严格大于才获胜；**平局：已存在者（服务端）获胜**。
- 服务端写入前 **clamp**：`updated_at = min(incoming.updated_at, now_ms + 300_000)`，防坏时钟污染。
- 客户端应用服务端下发的变更时不再比对（服务端已是裁决者），直接覆盖本地——唯一例外见 §7 checkin history。
- 客户端本地时间戳单调性不做强制保证；LWW 的幂等性保证即使时钟回拨也只是重复推送，无正确性损失。

## 3. 认证

- `POST /api/auth/register` `{username, password}` → `200 {token, user:{id, username}}`
  - 校验：username 2–32 字符、无空白/控制字符，唯一性 `COLLATE NOCASE`；password ≥ 8 字符。
  - `409 {error:{code:"username_taken"}}`
- `POST /api/auth/login` `{username, password}` → 同上；失败统一 `401 {error:{code:"invalid_credentials"}}`。
- 密码存储：argon2id（`argon2` crate 默认参数，随机盐）。
- 会话：JWT HS256，`{sub:<user_id>, exp:+30d}`；密钥环境变量 `JWT_SECRET`（缺失则启动失败）。
- 客户端存储（`sync.json`，权限 0600，与 `data.json` 分离）：`server_url, username, token, device_id, device_name, last_seq, push_watermark_ms`。

## 4. 接口

所有 `/api/sync/*` 需要 `Authorization: Bearer <token>`。请求体上限 4 MiB；单次 push ≤ 500 条变更；单条 `payload` ≤ 256 KiB。

### GET /api/health → `{status:"ok", version}`（无鉴权，供容器 healthcheck）

### GET /api/sync/pull?since=`<u64>` → `{changes:[Change], latest_seq:<u64>, has_more:<bool>}`

- 返回账号内 `server_seq > since` 的记录，按 `server_seq` 升序，单批上限 1000 条。
- `latest_seq` = 账号当前最大 `server_seq`（无记录则为 0）。
- 客户端循环 pull 直到 `!has_more`（每次用 `changes.last().server_seq` 做新 since），完成后持久化 `last_seq = latest_seq`。

### POST /api/sync/push `{changes:[Change]}` → `{applied:[{kind,id}], overridden:[Change], latest_seq}`

- 服务端逐条按 §2 裁决：
  - 入者胜 → 写库、`server_seq = ++seq_counter`，计入 `applied`；`overridden` 中的 Change 为服务端当前权威版本（含 `server_seq`）——客户端必须原样覆盖本地（不再二次裁决）。
  - 平局或败 → 计入 `overridden`（返回权威版本），不写库。
- 首次写入某账号时 `seq_counter` 从 0 开始递增（事务内 `UPDATE users SET seq_counter = seq_counter + n`）。

### Change（`sync-proto::Change`）

```json
{ "kind":"course", "id":"abc", "updated_at":1750000000123, "deleted":false,
  "device_id":"dev-x1", "payload": { /* 实体 JSON，deleted 时可为 null */ },
  "server_seq": 42  // 仅服务端下发时存在（Option<u64>）
}
```

## 5. 客户端同步循环

- 引擎在**专用工作线程**运行（reqwest + 自带 tokio current-thread runtime；`smol::channel` 与 UI 互传消息；UI 侧 `cx.spawn` 消费事件并 `cx.notify()`）。
- 触发：启动时（已登录）→ 每 5 分钟轮询 → 顶栏/设置页「立即同步」→ 本地 commit 后 3 秒防抖 push。
- 一轮 = `push(候选集) → pull(last_seq) → 循环 pull`。“候选集” = 本地 `updated_at > push_watermark_ms` 的实体（含墓碑）；一轮成功后 `push_watermark_ms = 本轮开始时刻`。
- 失败：请求超时 15s；退避 5s → 15s → 60s → 300s 封顶；401 → 状态转为 `AuthFailed` 停止轮询直至重新登录；网络错误 → `Offline`，恢复后自动重试。
- 状态事件（UI 可观察）：`Idle{last_sync}` / `Syncing` / `Offline` / `Error{msg}` / `AuthFailed` / `LoggedOut`。
- 客户端 UI 永不阻塞：引擎只改本地 DB + 发事件。

## 6. 首次登录策略（本地已有数据，且服务端非空）

登录后首次 pull 结果与本地比较：

1. **服务端为空** → 全量 push 本地（watermark=0 天然全量）。
2. **本地为空（或纯新装）** → 全量 pull。
3. **双方皆非空** → 弹对话框三选一：
   - **合并（推荐）**：先 pull 全量应用 LWW，再 push 全量（平局本地败 → 保服务端版本）。
   - **云端覆盖本地**：清空本地（软删本地全部实体以不回流；见下）后全量 pull。
   - **本地覆盖云端**：忽略服务端差异，全量 push（服务端已存在且更新的条目会被 overridden 返回，UI 提示数量）。

> 「云端覆盖本地」产生本地软删墓碑但没有对应服务端墓碑：这些墓碑**不要推送**——实现为「pull 前把本地 DB 替换为空 + `push_watermark_ms = now`」，不做软删流水。语义：本地未同步过的数据视为放弃。

## 7. 特殊合并：`checkin.history`

`history` 是 `"YYYY-MM-DD" -> bool` 的 map（false 表示显式取消打卡）。整体 LWW 会丢跨设备打卡，故客户端在**应用服务端下发的 checkin** 时：

1. `merged = remote.history ∪ local.history`，同 key 取 remote 值（远端裁决）；
2. 若 `merged != remote.history`：写入 merged 并把该实体 `updated_at = max(local.updated_at, remote.updated_at) + 1`，随后一轮自动 push 使服务端收敛（多为一次收敛；文件内所有 value 均为布尔、集合单调，实际是有限步收敛）；
3. 其余字段按 §2 整体覆盖。

服务端保持 kind 无关的通用处理（payload 不透明），该策略仅存在于客户端。

## 8. 服务端存储（SQLite）

```sql
CREATE TABLE users (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  username      TEXT NOT NULL UNIQUE COLLATE NOCASE,
  password_hash TEXT NOT NULL,
  created_at    INTEGER NOT NULL,
  seq_counter   INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE entities (
  account_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  kind       TEXT    NOT NULL,
  entity_id  TEXT    NOT NULL,
  payload    TEXT,            -- 实体 JSON；墓碑时可保留最后版本
  updated_at INTEGER NOT NULL,
  device_id  TEXT    NOT NULL DEFAULT '',
  deleted    INTEGER NOT NULL DEFAULT 0,
  server_seq INTEGER NOT NULL,
  PRIMARY KEY (account_id, kind, entity_id)
);
CREATE INDEX idx_entities_seq ON entities(account_id, server_seq);
```

- sqlx + SQLite，`journal_mode=WAL`、`busy_timeout=5s`、`foreign_keys=ON`；迁移内嵌 `sqlx::migrate!`。
- 写路径均在事务内执行（裁决 + seq 递增原子）。
- 错误码（统一 `{error:{code, message}}`）：`invalid_credentials` `username_taken` `invalid_request` `unauthorized` `payload_too_large` `internal`。

## 9. 删除语义

- 所有删除（课程、安排、日程、提醒、打卡任务、备忘）→ 本地软删（`deleted_at` 盖时间戳）+ 推送墓碑；本地读取一律过滤。
- 「清空全部数据」→ 对全部本地实体盖墓碑并全量推送（服务端其他设备随之清空）。
- 「载入示例数据」→ 作为普通新增实体推送。
- 墓碑 GC（>90 天）与服务端空间回收：v1 不做，文档声明。

## 10. 兼容性

- 旧 `data.json` 无 `updated_at`/`deleted_at` → `serde(default)`：`updated_at=0`、`deleted_at=None`。
- `updated_at=0` 的实体会在首次登录后全量推送（watermark 默认 0），符合预期。
- 客户端不因未知 `kind` 崩溃：未知 kind 的 Change 跳过并记录日志。
