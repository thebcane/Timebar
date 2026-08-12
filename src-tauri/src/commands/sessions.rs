//! Session lifecycle: the live timer, manual entries, edits (Sections 4 & 8).

use super::{active_view, e, emit_active_changed, session_view, CmdResult};
use crate::db::{now_ms, Db};
use crate::models::{Session, SessionPatch, SessionView};
use tauri::{AppHandle, State};

const SESSION_COLS: &str = "id, task_id, started_at, ended_at, note, rate_snapshot, billed, source, created_at, updated_at, deleted_at";

/// Set the `active_session_id` settings pointer (fast lookup; the null-ended
/// row remains authoritative).
async fn set_active_pointer(
    pool: &sqlx::SqlitePool,
    id: Option<&str>,
) -> Result<(), sqlx::Error> {
    match id {
        Some(v) => {
            sqlx::query("INSERT INTO settings (key, value) VALUES ('active_session_id', ?)
                         ON CONFLICT(key) DO UPDATE SET value=excluded.value")
                .bind(v)
                .execute(pool)
                .await?;
        }
        None => {
            sqlx::query("DELETE FROM settings WHERE key='active_session_id'")
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_active_session(db: State<'_, Db>) -> CmdResult<Option<SessionView>> {
    active_view(&db.pool).await.map_err(e)
}

/// Load one session (unclamped, whole interval) for editing.
#[tauri::command]
pub async fn get_session(db: State<'_, Db>, id: String) -> CmdResult<Option<SessionView>> {
    session_view(&db.pool, &id).await.map_err(e)
}

/// Start a task. Stops any running session first, atomically (single active
/// session invariant, §4). Snapshots the task's current rate onto the session.
/// Core of `start_task`, decoupled from Tauri state so it can be exercised
/// directly (see the launch self-test). Stops any running session and starts a
/// fresh one atomically, then records the active pointer. Returns the new id.
pub async fn do_start(
    pool: &sqlx::SqlitePool,
    task_id: &str,
    now: i64,
) -> Result<String, sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Stop whatever is running.
    sqlx::query("UPDATE sessions SET ended_at=?, updated_at=? WHERE ended_at IS NULL AND deleted_at IS NULL")
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

    // Snapshot the task's current rate.
    let rate: f64 = sqlx::query_scalar("SELECT hourly_rate FROM tasks WHERE id=?")
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;

    let sid = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO sessions (id, task_id, started_at, ended_at, note, rate_snapshot, billed, source, created_at, updated_at)
         VALUES (?, ?, ?, NULL, NULL, ?, 0, 'timer', ?, ?)",
    )
    .bind(&sid)
    .bind(task_id)
    .bind(now)
    .bind(rate)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    // Commit BEFORE touching the pointer: set_active_pointer uses a separate
    // pool connection, and SQLite has a single writer — writing it while `tx`
    // still holds the write lock would self-deadlock. The null-ended row is
    // authoritative regardless, so the pointer can lag safely.
    tx.commit().await?;
    set_active_pointer(pool, Some(&sid)).await?;
    Ok(sid)
}

#[tauri::command]
pub async fn start_task(
    app: AppHandle,
    db: State<'_, Db>,
    task_id: String,
) -> CmdResult<SessionView> {
    let sid = do_start(&db.pool, &task_id, now_ms())
        .await
        .map_err(|err| match err {
            sqlx::Error::RowNotFound => format!("no such task: {task_id}"),
            other => other.to_string(),
        })?;

    let view = session_view(&db.pool, &sid)
        .await
        .map_err(e)?
        .ok_or_else(|| "failed to load started session".to_string())?;
    emit_active_changed(&app, &db.pool).await;
    Ok(view)
}

/// Stop the running session, if any.
#[tauri::command]
pub async fn stop_active(app: AppHandle, db: State<'_, Db>) -> CmdResult<Option<SessionView>> {
    let now = now_ms();
    let running = active_view(&db.pool).await.map_err(e)?;
    let Some(running) = running else {
        return Ok(None);
    };

    sqlx::query("UPDATE sessions SET ended_at=?, updated_at=? WHERE id=? AND ended_at IS NULL")
        .bind(now)
        .bind(now)
        .bind(&running.id)
        .execute(&db.pool)
        .await
        .map_err(e)?;
    set_active_pointer(&db.pool, None).await.map_err(e)?;

    // Clear the menu-bar title immediately (don't wait for the 1s loop).
    crate::tray::set_tray_title(&app, None);

    let view = session_view(&db.pool, &running.id).await.map_err(e)?;
    emit_active_changed(&app, &db.pool).await;
    Ok(view)
}

/// Create a manual entry with explicit start/end (§4). Rate snapshots from the
/// task at creation time.
///
/// NOTE: `billable` is currently derived from the task (the v1 schema has no
/// per-session billable column). The parameter is accepted for a forward-
/// compatible contract; honoring an override needs a schema column (a clean
/// later migration).
#[tauri::command]
pub async fn create_manual_session(
    app: AppHandle,
    db: State<'_, Db>,
    task_id: String,
    started_at: i64,
    ended_at: i64,
    note: Option<String>,
    #[allow(unused_variables)] billable: Option<bool>,
) -> CmdResult<SessionView> {
    if ended_at <= started_at {
        return Err("ended_at must be after started_at".into());
    }
    let now = now_ms();
    let rate: f64 = sqlx::query_scalar("SELECT hourly_rate FROM tasks WHERE id=?")
        .bind(&task_id)
        .fetch_optional(&db.pool)
        .await
        .map_err(e)?
        .ok_or_else(|| format!("no such task: {task_id}"))?;

    let sid = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO sessions (id, task_id, started_at, ended_at, note, rate_snapshot, billed, source, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, 0, 'manual', ?, ?)",
    )
    .bind(&sid)
    .bind(&task_id)
    .bind(started_at)
    .bind(ended_at)
    .bind(&note)
    .bind(rate)
    .bind(now)
    .bind(now)
    .execute(&db.pool)
    .await
    .map_err(e)?;

    let view = session_view(&db.pool, &sid)
        .await
        .map_err(e)?
        .ok_or_else(|| "failed to load manual session".to_string())?;
    // A manual entry never changes the running session, but tree/entry views
    // may want to refresh; the active pointer is untouched.
    let _ = &app;
    Ok(view)
}

#[tauri::command]
pub async fn update_session(
    app: AppHandle,
    db: State<'_, Db>,
    id: String,
    patch: SessionPatch,
) -> CmdResult<SessionView> {
    let sql = format!("SELECT {SESSION_COLS} FROM sessions WHERE id=?");
    let mut s = sqlx::query_as::<_, Session>(&sql)
        .bind(&id)
        .fetch_one(&db.pool)
        .await
        .map_err(e)?;

    let reassigned = patch
        .task_id
        .as_ref()
        .is_some_and(|new| new != &s.task_id);

    if let Some(v) = patch.task_id {
        s.task_id = v;
    }
    if let Some(v) = patch.started_at {
        s.started_at = v;
    }
    if let Some(v) = patch.ended_at {
        s.ended_at = v;
    }
    if let Some(v) = patch.note {
        s.note = v;
    }
    if let Some(v) = patch.billed {
        s.billed = v;
    }
    if let Some(v) = patch.rate_snapshot {
        s.rate_snapshot = v;
    }

    // Re-snapshot the rate only on explicit request when the task changed (§7).
    if reassigned && patch.resnapshot_rate.unwrap_or(false) {
        let rate: f64 = sqlx::query_scalar("SELECT hourly_rate FROM tasks WHERE id=?")
            .bind(&s.task_id)
            .fetch_optional(&db.pool)
            .await
            .map_err(e)?
            .ok_or_else(|| format!("no such task: {}", s.task_id))?;
        s.rate_snapshot = rate;
    }

    if let Some(end) = s.ended_at {
        if end <= s.started_at {
            return Err("ended_at must be after started_at".into());
        }
    }
    s.updated_at = now_ms();

    sqlx::query(
        "UPDATE sessions SET task_id=?, started_at=?, ended_at=?, note=?, rate_snapshot=?, billed=?, updated_at=? WHERE id=?",
    )
    .bind(&s.task_id)
    .bind(s.started_at)
    .bind(s.ended_at)
    .bind(&s.note)
    .bind(s.rate_snapshot)
    .bind(s.billed)
    .bind(s.updated_at)
    .bind(&s.id)
    .execute(&db.pool)
    .await
    .map_err(e)?;

    let view = session_view(&db.pool, &s.id)
        .await
        .map_err(e)?
        .ok_or_else(|| "failed to load updated session".to_string())?;
    // If we just edited the running session, refresh listeners.
    if s.ended_at.is_none() {
        emit_active_changed(&app, &db.pool).await;
    }
    Ok(view)
}

#[tauri::command]
pub async fn delete_session(app: AppHandle, db: State<'_, Db>, id: String) -> CmdResult<()> {
    // If deleting the running session, clear the active pointer afterward.
    let was_active = active_view(&db.pool)
        .await
        .map_err(e)?
        .map(|v| v.id == id)
        .unwrap_or(false);

    // Soft delete (tombstone) so the deletion syncs across devices.
    let now = now_ms();
    sqlx::query("UPDATE sessions SET deleted_at=?, updated_at=? WHERE id=?")
        .bind(now)
        .bind(now)
        .bind(&id)
        .execute(&db.pool)
        .await
        .map_err(e)?;

    if was_active {
        set_active_pointer(&db.pool, None).await.map_err(e)?;
        emit_active_changed(&app, &db.pool).await;
    }
    Ok(())
}
