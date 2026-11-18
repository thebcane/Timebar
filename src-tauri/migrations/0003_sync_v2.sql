-- Sync v2: a per-row "needs pushing" flag replaces the timestamp push cursor,
-- so an edit is pushed even if this device's clock is behind other devices'.
-- 1 = changed locally since the last successful push. New rows default to 1;
-- every local UPDATE sets it to 1; sync clears it after pushing and when it
-- applies a pulled row.
-- Existing rows start at 1, so everything is re-pushed once after upgrading.
-- That's safe: the server ignores any row older than the one it has.
-- Append-only: never edit after shipping.

ALTER TABLE folders  ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;
ALTER TABLE tasks    ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;
ALTER TABLE sessions ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;

CREATE INDEX idx_folders_dirty  ON folders(dirty)  WHERE dirty = 1;
CREATE INDEX idx_tasks_dirty    ON tasks(dirty)    WHERE dirty = 1;
CREATE INDEX idx_sessions_dirty ON sessions(dirty) WHERE dirty = 1;

-- Keep updated_at moving forward on local edits. If this device's clock is
-- behind the device that last touched a row, a plain `updated_at = now` would
-- stamp the edit as OLDER than the version it replaces, and last-write-wins
-- would throw it away. These bump it to old + 1 ms instead. They only fire on
-- local edits (dirty = 1); applying a pulled row clears dirty and is always
-- newer anyway. SQLite triggers don't recurse by default, so the inner UPDATE
-- doesn't re-fire them.
CREATE TRIGGER folders_updated_at_forward AFTER UPDATE ON folders
WHEN NEW.dirty = 1 AND NEW.updated_at <= OLD.updated_at
BEGIN
  UPDATE folders SET updated_at = OLD.updated_at + 1 WHERE id = NEW.id;
END;

CREATE TRIGGER tasks_updated_at_forward AFTER UPDATE ON tasks
WHEN NEW.dirty = 1 AND NEW.updated_at <= OLD.updated_at
BEGIN
  UPDATE tasks SET updated_at = OLD.updated_at + 1 WHERE id = NEW.id;
END;

CREATE TRIGGER sessions_updated_at_forward AFTER UPDATE ON sessions
WHEN NEW.dirty = 1 AND NEW.updated_at <= OLD.updated_at
BEGIN
  UPDATE sessions SET updated_at = OLD.updated_at + 1 WHERE id = NEW.id;
END;
