//! Menu-bar tray: icon, live `H:MM:SS` title while running, click handling,
//! and the 1-second title updater (Section 3, signature behavior).

use crate::db::{now_ms, Db};
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tauri_plugin_positioner::{Position, WindowExt};

/// Format elapsed milliseconds as `H:MM:SS` (e.g. `2:14:12`).
pub fn format_hms(ms: i64) -> String {
    let total = (ms.max(0)) / 1000;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    format!("{h}:{m:02}:{s:02}")
}

/// Show/hide the quick panel, positioning it centered under the tray icon.
pub fn toggle_quick_panel(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("quick") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            let _ = win.move_window(Position::TrayCenter);
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

/// Open (creating focus on) the main dashboard window.
pub fn open_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Open the main dashboard (from the quick panel's expand button).
#[tauri::command]
pub fn open_dashboard(app: AppHandle) {
    open_main(&app);
}

/// Hide the quick panel (Escape / after selecting a task).
#[tauri::command]
pub fn hide_quick(app: AppHandle) {
    if let Some(w) = app.get_webview_window("quick") {
        let _ = w.hide();
    }
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Dashboard", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit TimeBar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    // Custom monochrome menu-bar icon. `icon_as_template(true)` makes macOS
    // recolor it for light/dark menu bars automatically. Swap this PNG
    // (src-tauri/icons/tray.png) to change the icon — keep it monochrome
    // black-on-transparent at ~44×44 for a crisp @2x template image.
    let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;

    TrayIconBuilder::with_id("main-tray")
        .icon(tray_icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => open_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Let the positioner cache the tray rect for TrayCenter anchoring.
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_quick_panel(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Set the tray title. Idle passes an empty string (not `None`): on macOS
/// `set_title(None)` can leave the previous text visible, while `Some("")`
/// reliably clears it, leaving the icon only.
pub fn set_tray_title(app: &AppHandle, text: Option<String>) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_title(Some(text.unwrap_or_default()));
    }
}

/// Spawn the 1s loop that keeps the tray title in sync with the running
/// session. Source of truth is `started_at`, so it never drifts.
pub fn spawn_title_updater(app: AppHandle) {
    let pool = app.state::<Db>().pool.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut last_title = String::new();
        loop {
            interval.tick().await;
            let started: Option<i64> = sqlx::query_scalar(
                "SELECT started_at FROM sessions WHERE ended_at IS NULL AND deleted_at IS NULL ORDER BY started_at DESC LIMIT 1",
            )
            .fetch_optional(&pool)
            .await
            .ok()
            .flatten();

            // Current-session elapsed; empty string when idle → icon only.
            let title = started.map(|s| format_hms(now_ms() - s)).unwrap_or_default();
            if title != last_title {
                if let Some(tray) = app.tray_by_id("main-tray") {
                    let _ = tray.set_title(Some(title.clone()));
                }
                last_title = title;
            }
        }
    });
}
