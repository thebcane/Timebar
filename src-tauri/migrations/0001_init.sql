-- Initial schema (Section 5). Timestamps are integer UTC epoch milliseconds.
-- Append-only: never edit this file after it has shipped; add new numbered files.

CREATE TABLE folders (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  color       TEXT NOT NULL,
  parent_id   TEXT REFERENCES folders(id) ON DELETE SET NULL,
  sort_order  INTEGER NOT NULL DEFAULT 0,
  archived    INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE tasks (
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  color        TEXT NOT NULL,
  folder_id    TEXT REFERENCES folders(id) ON DELETE SET NULL,
  hourly_rate  REAL NOT NULL DEFAULT 0,
  currency     TEXT NOT NULL DEFAULT 'USD',
  billable     INTEGER NOT NULL DEFAULT 0,
  sort_order   INTEGER NOT NULL DEFAULT 0,
  archived     INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);

CREATE TABLE sessions (
  id            TEXT PRIMARY KEY,
  task_id       TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  started_at    INTEGER NOT NULL,
  ended_at      INTEGER,                      -- NULL = running
  note          TEXT,
  rate_snapshot REAL NOT NULL DEFAULT 0,
  billed        INTEGER NOT NULL DEFAULT 0,    -- marked paid/billed
  source        TEXT NOT NULL DEFAULT 'timer', -- 'timer' | 'manual'
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);

CREATE INDEX idx_sessions_task    ON sessions(task_id);
CREATE INDEX idx_sessions_started ON sessions(started_at);
CREATE INDEX idx_sessions_running ON sessions(ended_at) WHERE ended_at IS NULL;

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
