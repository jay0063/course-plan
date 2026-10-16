# CoursePlan 同步协议 v1.2（冻结真源）

> 状态: Gate 0 已闭环（Oracle attempt 1: accept-with-fixes → v1.1；attempt 2: accept-with-fixes → v1.2，R1-R3 均为机械应用评审指定修复，不再复评）。
> v1.2 变更（复评修复）: R1 账号切换本地数据归档+清空；R2 Ack 仅在持久化后回出、sync.json 由 UI 线程写；R3 水位推进取「轮内最小本地改动戳」下限；超前台设备出站戳收敛上限；§6-3c 重戳基准改为 `max(now, server_time+1)`。
> v1.1 变更: 时间戳单调规则；统一 apply；游标按账号隔离；推送批次上限；单写者契约；首登三策略细化；checkin 合并覆盖 overridden 路径；服务端读快照保证。

## 1. 模型

- **账号（account）**：用户注册的家庭账号，拥有一个独立数据空间。所有实体按账号隔离。
- **设备（device）**：客户端启动时生成 `device_id`（随机字符串，持久在客户端 `sync.json`）。
- **实体（entity）**：`kind + id` 唯一定位。kind ∈ `profile | course | session | event | reminder | checkin | memo`；`profile` 为单例，id 固定 `"main"`。
- **墓碑（tombstone）**：删除 = 置 `deleted=true` 并盖时间戳，不物理删除，供其他设备收敛。
- **设备本地数据（不同步）**：提醒的「已通知」记账（原 `Reminder.notified_for`）移出同步载荷，存本地 `runtime_state.json`（`reminder_id -> 已通知日期键`），避免每台设备的通知记账互相污染并产生推送 churn。同步的只有实体本身。

## 2. 时间戳与冲突规则（LWW）

### 2.1 本地修改的时间戳（单调规则）

```
本地修改实体 E 时：
  ts = max(now_ms, push_watermark_ms + 1, E.updated_at + 1)
新建实体：
  ts = max(now_ms, push_watermark_ms + 1)
```

三个上限分别保证：
1. `now_ms` — 正常时钟下的常规取值；
2. `watermark + 1` — 即使本地时钟回拨，新改动也**必然**满足推送候选条件 `ts >= watermark`；
3. `E.updated_at + 1` — 本地改动**永远严格压过本地基线**（防止基线是别台设备被 clamp 的未来戳时，本地编辑被判负并静默回滚）。

### 2.2 合并谓词

```
wins(incoming, existing) = incoming.updated_at > existing.updated_at   // 严格大于
平局（相等）→ existing 一方获胜（服务端持有者胜）
```

### 2.3 服务端 clamp

写入前 `updated_at = min(incoming.updated_at, server_now_ms + 300_000)`（`sync-proto::clamp_updated_at`）。clamp 只压低存储时间戳；被压低的本地实体保持候选（§5），后续轮次幂等重推直至服务端时钟越过边界。

### 2.4 统一 apply

**服务端下发的一切变更**（pull 批次 + push `overridden` 列表）一律经过同一个函数落到本地：

```
apply_remote(local_latest_version, incoming_change):
  profile/course/session/event/reminder/memo:
    if local 不存在 or wins(incoming, local):  落地 incoming（ts = incoming.updated_at，不重戳）
    else:                                      保持本地不动（本地仍是候选，下轮自动再推）
  checkin: 见 §7
```

- 走 `wins` 后：稳定的远端版本照常落地（服务端已裁决过它相对自己持版本的胜负），只有「比远端更新」的本地编辑受保护并自动补推。
- 本地胜利的实体不会丢：其 `ts >= watermark + 1`（§2.1 不变量）→ 下一轮必为候选。
- `incoming.deleted=true` 且获胜 → 应用为本地墓碑；失败则忽略。

### 2.5 回拨/前跳的行为（含出站上限，v1.2）

- **回拨**：不丢改（§2.1）；期间推送可能重复——幂等，无正确性损失。
- **前跳（本地时钟超前）**：本地戳超前 → 每次同步都会重推（churn）直到真实时间越过；服务端始终 clamp；内容收敛，仅带宽浪费。
- **出站收敛上限（v1.2）**：客户端推送前将单条 `ts_out = min(entity.ts, last_server_time_ms + 300_000)`（本地保留原戳；只影响出站 payload）。这限制坏时钟设备能让服务端存储的戳上限，防止其长期压过其他设备的正常编辑（被 clamp 的一方仍靠 §2.1 `prev+1` 在本地反超、下轮补推）。
- **接受的残留**：若某设备本地戳极端超前（数量级），在它被修正前，其内容在服务端持续占优；其他设备的新编辑本地不丢，但在该设备修正前可能无法传播给第三方。属已知接受项（不做时钟同步）。

## 3. 认证与客户端凭据

- `POST /api/auth/register` `{username, password}` → `200 {token, user:{id, username}}`；`409 {error:{code:"username_taken"}}`
  - 校验：username 2–32 字符、无空白/控制字符，唯一性 `COLLATE NOCASE`（ASCII 大小写不敏感）；password ≥ 8。
- `POST /api/auth/login` → 同上；失败统一 `401 {error:{code:"invalid_credentials"}}`。
- 密码存储：argon2id（默认参数 + 随机盐）。会话：JWT HS256 `{sub:<user_id>, exp:+30d}`；`JWT_SECRET` 缺失则启动失败。
- 客户端凭据 `sync.json`（0600，与 `data.json` 分离）：

```json
{ "server_url": "http://host:8080", "user_id": 3, "username": "xiaoman",
  "token": "…", "device_id": "…", "device_name": "客厅台式机",
  "last_seq": 182, "push_watermark_ms": 1750000000000 }
```

- **游标按账号隔离**：登录/切换账号时若 `(server_url, user_id)` 与已存不同 → `last_seq=0, push_watermark_ms=0` 全量重拉重推（幂等安全）。
- **账号切换（v1.2，R1）**：若 `(server_url, user_id)` 与已存不同 → **先把现有 `data.json` 归档为 `data-<old_user_id>.json`，本地 DB 清空为新账号的空白空间，绝不把切换前的实体推入新账号**；随后走 §6 首登流程。登出：仅停止同步、保留本地数据；同账号再登录 → 游标重置 → 全量重拉（幂等）。
- 旧版 `sync.json`（无 `user_id`）视为不同账号 → 走上述切换流程（数据归档）。

## 4. 接口

`/api/sync/*` 需要 `Authorization: Bearer <token>`。请求体上限 4 MiB。

### GET /api/health → `{status:"ok", version}`（无鉴权，供容器 healthcheck）

### GET /api/sync/pull?since=`<u64>` →

```json
{ "changes":[Change], "latest_seq":184, "has_more":false, "server_time_ms":1750000123456 }
```

- 返回 `server_seq > since` 的记录，按 `server_seq` 升序，单批 ≤1000 条。
- **读快照保证**：批次内容与 `latest_seq` 在**同一读事务**内查询，不存在「latest_seq 越过未下发记录」的窗口。
- 客户端循环 pull 直到 `!has_more`；完成后 `last_seq = 最终响应.latest_seq`。

### POST /api/sync/push → `{changes:[Change]}` →

```json
{ "applied":[{"kind":"course","id":"abc"}],
  "overridden":[Change], "latest_seq":186, "server_time_ms":1750000123456 }
```

- 服务端逐条裁决（§2.2/§2.3）：入者胜 → 写库、分配 `server_seq`、计入 `applied`；平局或败 → 计入 `overridden` 且返回**服务端权威版本**（含 `server_seq`）。
- **批次契约**：单请求 ≤500 条**且** ≤3 MiB 序列化负载；超出由客户端分片循环推送（首登全量推送必然多批）。
- `overridden` 中的条目与 pull 变更一样走 §2.4 统一 apply（checkin 走 §7 合并），随后由候选机制决定是否补推。

### Change

```json
{ "kind":"course", "id":"abc", "updated_at":1750000000123, "deleted":false,
  "device_id":"dev-x1", "payload": { }?, "server_seq": 42? }
```

`server_seq` 仅服务端下发时存在；`payload` 为实体 JSON，墓碑时可为 null。

## 5. 客户端同步引擎

### 5.1 单写者契约与 Ack 持久性（v1.2，R2）

- `Database` **只在 UI 线程读写**（与 GPUI `App` 实体同线程）。
- 同步 worker（专用 OS 线程 + 自带 tokio current-thread runtime + reqwest(rustls)）只做：**网络请求 + 轮次状态机**，不触碰 DB、不写文件。
- 服务端变更经 smol channel 送 UI 事件队列；UI 线程执行统一 apply（§2.4）并**持久化 `data.json`（commit）成功之后**才回 `Ack`；worker 收到 Ack 后才推进并持久化 `sync.json` 游标。`sync.json` 由 UI 线程写（单写者延伸）。
- 崩溃语义：Ack 前崩溃 → 游标保留上轮值 → 幂等重放；不可能出现「游标已越过、变更已丢」。

### 5.2 触发与单飞

触发源：启动（已登录）、每 5 分钟、手动「立即同步」、本地 commit 后 3 秒防抖。**单飞**：任一轮次运行中，新触发只置 pending 标记，轮次结束立即补跑一次。

### 5.3 轮次算法

```
C0 = now_ms(); W = 已持久化的 push_watermark_ms
候选 = 本地全部实体(含墓碑) where updated_at >= W，按 ts 升序
for 批次 in 候选分片(≤500条 且 ≤3MiB):
    POST /push （ts_out 按 §2.5 出站上限收敛）
    → 批次内 applied/overridden 经统一 apply 落地（UI 持久化后 ack）
loop: GET /pull?since=last_seq … 直到 !has_more，每批统一 apply（同上）
成功结束:
    last_seq   = 最终 latest_seq
    watermark  = min(C0, 最终 server_time_ms, 本轮最小本地改动戳 min_mut_ts)   // v1.2，R3
        // 轮内时钟回拨产生的新编辑戳为 W_old+1，可能 < C0；取下限保证其恒在候选(>=)
        // 本轮无本地改动时 min_mut_ts 可省略
失败:
    游标/水位不动（幂等）; 退避 5s → 15s → 60s → 300s；401 → AuthFailed 停自动轮询
```

- 不变量：任何本地改动 `ts >= watermark + 1`（§2.1）→ 永不被轮次边界漏掉。
- `min_mut_ts` 由 Mutation API 在轮内记录（worker 向 UI 声明本轮开始时刻即可收集）。

### 5.4 状态事件（worker → UI）

`Idle{last_sync}` / `Syncing{progress}` / `Offline` / `Error{msg}` / `AuthFailed` / `LoggedOut`；另有 `Probe{counts_by_kind}` 供首登对话框（§6）。

## 6. 首次登录策略

登录成功后先做 **探针 pull（since=0，只统计不应用）**，拿 `counts_by_kind + latest_seq + server_time_ms`（账号切换已按 §3 归档清空）：

1. **服务端为空** → 全量 push 本地（watermark=0 天然全量；分片）。
2. **本地为空（或全新安装）** → `last_seq` 起点 0 全量 pull；`watermark = server_time_ms`。
3. **双方皆非空** → 对话框三选一：
   - **合并（推荐）**：按游标 0 全量 pull 并统一 apply（LWW），再全量 push 全部本地候选（含仅在本地存在的实体与墓碑）。
   - **云端覆盖本地**：丢弃本地未同步数据——**不做软删、不推任何墓碑**；本地 DB 直接替换为探针/全量 pull 快照，`last_seq = latest_seq`，`watermark = server_time_ms`。（语义 = 放弃本地，不是删除云端。）
   - **本地覆盖云端**：先以探针结果建立服务端 id 集合：
     1. 本地全部实体**重戳**为递增的 `max(now_ms, server_time_ms + 1)` 序列（v1.2，R5：压过服务端当前已知戳）；
     2. 对「仅存在于服务端」的 id 推送墓碑（`deleted=true, ts=now`）；
     3. 全量分片 push。
     → 效果：同 id 实体本地版本获胜，服务端独有实体被删。对话框文案明确说明「将删除云端独有数据」。中途失败幂等可重试（重戳与重推均安全）。

任何选择完成后进入常规循环（§5）。

## 7. checkin.history 合并（统一 apply 内，覆盖 pull 与 overridden 两路径）

`history` 为 `"YYYY-MM-DD" -> bool` map（false = 显式取消打卡）。**所有** incoming checkin 走同一合并：

```
apply_remote_checkin(local L, incoming R):
  if R.deleted: 按 §2.4 常规墓碑处理（不合并）
  remote_wins = local 不存在 or wins(R.updated_at, L.updated_at)     // 平局 → 远端
  merged_history = L.history ∪ R.history，同 key 取 R 值
  out = remote_wins ? R : L
  if out.history != merged_history:
      out.history = merged_history
      out.updated_at = max(L.updated_at, R.updated_at) + 1    // 保持候选 → 下轮补推
  // 否则落 winner 原样（不重戳）
  return out
```

- 收敛性：每次 bump 严格高于双方；推送后服务端 clamp 可能压低 → 保持候选按 §5 重试，直至服务端时钟越过 clamp 边界；并发方为有限设备数，每次冲突至少固化一个 key；由随机化收敛测试 + 确定性时钟注入（PLAN §6）兜底。
- 服务端对此毫无感知（payload 不透明，kind 无关）。

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
  payload    TEXT,
  updated_at INTEGER NOT NULL,
  device_id  TEXT    NOT NULL DEFAULT '',
  deleted    INTEGER NOT NULL DEFAULT 0,
  server_seq INTEGER NOT NULL,
  PRIMARY KEY (account_id, kind, entity_id)
);
CREATE INDEX idx_entities_seq ON entities(account_id, server_seq);
```

- sqlx + SQLite：`journal_mode=WAL`、`busy_timeout=5s`、`foreign_keys=ON`；迁移内嵌 `sqlx::migrate!`。
- push 写路径在**单事务**内完成裁决 + `seq_counter` 递增 + 行写入；pull 批次与 `latest_seq` 同一读事务（§4）。
- 请求上限：body 4 MiB；单条 payload ≤256 KiB；单批 ≤500 条。
- 备份（写入 `docs/DEPLOY.md`）：WAL 模式下**禁止**裸拷 `courseplan.db`；用 `sqlite3 courseplan.db ".backup /backup/x.db"` / `VACUUM INTO` 或停机拷贝。
- 错误码：`invalid_credentials` `username_taken` `invalid_request` `unauthorized` `payload_too_large` `internal`（统一 `{error:{code,message}}`）。

## 9. 删除语义

- 一切删除 → 本地软删 + 推送墓碑；读取一律过滤墓碑。
- 「清空全部数据」→ 全量盖墓碑并推送（其他设备随之清空）。
- 「载入示例数据」→ 普通新增实体推送。
- 已知且接受的 LWW 特性：**离线编辑可以复活被别处删除的实体**（编辑的 ts 更大即获胜）。LWW 的正确行为，非 bug。
- 「云端覆盖本地」（§6-3b）不产生墓碑；「本地覆盖云端」产生针对服务端独有 id 的墓碑。
- 墓碑 GC 与空间回收：v1 不做（文档声明）。

## 10. 兼容性

- 旧 `data.json` 无 `updated_at`/`deleted_at` → `serde(default)`：`updated_at=0`、`deleted_at=None`；首登后自然全量推送。
- 旧 `Reminder.notified_for` 字段：读取时迁移进 `runtime_state.json`、写回时不再输出。
- 客户端遇未知 `kind`：跳过并记录日志，不崩溃。
- 旧 `sync.json`：见 §3（按账号切换流程处理，数据归档）。
