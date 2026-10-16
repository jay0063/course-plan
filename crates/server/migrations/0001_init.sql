-- CoursePlan 同步服务端初始 schema（协议 §8）。
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
