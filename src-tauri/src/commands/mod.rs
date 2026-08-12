//! IPC command surface (Section 8). One submodule per group. Every command
//! returns `Result<T, String>`; all mutations set `updated_at = now`.

pub mod export;
pub mod queries;
pub mod sessions;
pub mod settings;
pub mod tasks;

use crate::db::now_ms;
use crate::models::SessionView;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};

pub type CmdResult<T> = Result<T, String>;

/// Map any Display error into the `String` half of a command result.
pub fn e<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

/// Row shape for the session + task + folder join used to build a SessionView.
#[derive(sqlx::FromRow)]
struct SessionJoin {
    id: String,
    task_id: String,
    started_at: i64,
    ended_at: Option<i64>,
    note: Option<String>,
    rate_snapshot: f64,
    billed: bool,
    source: String,
    task_name: String,
    task_color: String,
    billable: bool,
    folder_name: Option<String>,
}

const SESSION_VIEW_SQL: &str = "
    SELECT s.id, s.task_id, s.started_at, s.ended_at, s.note, s.rate_snapshot,
           s.billed, s.source,
           t.name AS task_name, t.color AS task_color, t.billable AS billable,
           f.name AS folder_name
    FROM sessions s
    JOIN tasks t ON t.id = s.task_id
    LEFT JOIN folders f ON f.id = t.folder_id
";

fn to_view(r: SessionJoin, now: i64) -> SessionView {
    let is_active = r.ended_at.is_none();
    let end = r.ended_at.unwrap_or(now);
    let duration_ms = (end - r.started_at).max(0);
    let revenue = if r.billable {
        (duration_ms as f64 / 3_600_000.0) * r.rate_snapshot
    } else {
        0.0
    };
    SessionView {
        id: r.id,
        task_id: r.task_id,
        task_name: r.task_name,
        task_color: r.task_color,
        folder_name: r.folder_name,
        started_at: r.started_at,
        ended_at: r.ended_at,
        note: r.note,
        rate_snapshot: r.rate_snapshot,
        billable: r.billable,
        billed: r.billed,
        source: r.source,
        duration_ms,
        revenue,
        is_active,
    }
}

/// Load one session as a SessionView.
pub async fn session_view(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<SessionView>, sqlx::Error> {
    let sql = format!("{SESSION_VIEW_SQL} WHERE s.id = ? AND s.deleted_at IS NULL");
    let row = sqlx::query_as::<_, SessionJoin>(&sql)
        .bind(session_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| to_view(r, now_ms())))
}

/// Load the currently running session (ended_at IS NULL), if any.
pub async fn active_view(pool: &SqlitePool) -> Result<Option<SessionView>, sqlx::Error> {
    let sql = format!(
        "{SESSION_VIEW_SQL} WHERE s.ended_at IS NULL AND s.deleted_at IS NULL ORDER BY s.started_at DESC LIMIT 1"
    );
    let row = sqlx::query_as::<_, SessionJoin>(&sql)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| to_view(r, now_ms())))
}

/// Broadcast the active-session state to all windows (tray + chip + rows).
pub async fn emit_active_changed(app: &AppHandle, pool: &SqlitePool) {
    let payload = active_view(pool).await.ok().flatten();
    let _ = app.emit("active-session-changed", payload);
}
