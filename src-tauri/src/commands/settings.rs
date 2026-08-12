//! App settings (Section 8). `active_session_id` is returned authoritatively
//! from the running row, not just the stored pointer.

use super::{e, CmdResult};
use crate::db::Db;
use crate::models::Settings;
use std::collections::HashMap;
use tauri::{AppHandle, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

/// The quick-panel toggle shortcut (Cmd+Shift+T). Kept here so registration and
/// the runtime enable/disable command stay in sync.
pub fn quick_toggle_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyT)
}

#[tauri::command]
pub async fn get_settings(db: State<'_, Db>) -> CmdResult<Settings> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
        .fetch_all(&db.pool)
        .await
        .map_err(e)?;
    let map: HashMap<String, String> = rows.into_iter().collect();

    let active: Option<String> = sqlx::query_scalar(
        "SELECT id FROM sessions WHERE ended_at IS NULL AND deleted_at IS NULL ORDER BY started_at DESC LIMIT 1",
    )
    .fetch_optional(&db.pool)
    .await
    .map_err(e)?;

    Ok(Settings {
        first_day_of_week: map
            .get("first_day_of_week")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
        default_currency: map
            .get("default_currency")
            .cloned()
            .unwrap_or_else(|| "USD".into()),
        active_session_id: active,
        theme: map.get("theme").cloned().unwrap_or_else(|| "dark".into()),
        // Default on when unset.
        global_shortcut_enabled: map.get("global_shortcut_enabled").map(|v| v != "0").unwrap_or(true),
    })
}

#[tauri::command]
pub async fn update_setting(db: State<'_, Db>, key: String, value: String) -> CmdResult<()> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
    )
    .bind(&key)
    .bind(&value)
    .execute(&db.pool)
    .await
    .map_err(e)?;
    Ok(())
}

/// Enable/disable the global quick-panel shortcut and persist the choice.
#[tauri::command]
pub async fn set_global_shortcut(app: AppHandle, db: State<'_, Db>, enabled: bool) -> CmdResult<()> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('global_shortcut_enabled', ?)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
    )
    .bind(if enabled { "1" } else { "0" })
    .execute(&db.pool)
    .await
    .map_err(e)?;

    let sc = quick_toggle_shortcut();
    if enabled {
        let _ = app.global_shortcut().register(sc);
    } else {
        let _ = app.global_shortcut().unregister(sc);
    }
    Ok(())
}
