-- Soft-delete tombstones so deletions propagate across devices via sync
-- (Section 12). NULL = live; a timestamp = deleted at that UTC epoch ms.
-- Append-only: never edit after shipping.

ALTER TABLE folders  ADD COLUMN deleted_at INTEGER;
ALTER TABLE tasks    ADD COLUMN deleted_at INTEGER;
ALTER TABLE sessions ADD COLUMN deleted_at INTEGER;

CREATE INDEX idx_folders_updated  ON folders(updated_at);
CREATE INDEX idx_tasks_updated    ON tasks(updated_at);
CREATE INDEX idx_sessions_updated ON sessions(updated_at);
