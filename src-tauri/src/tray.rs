//! Tray: icon, live timer, click handling, and the 1-second updater
//! (Section 3, signature behavior).
//!
//! macOS shows the running time as text next to the menu-bar icon. Windows
//! tray icons can't show text, so there the icon turns teal while a timer
//! runs and the tooltip carries the task name and elapsed time.

use crate::db::{now_ms, Db};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, PhysicalPosition, WebviewWindow,
};
use tauri_plugin_positioner::{Position, WindowExt};

/// When the quick panel last hid itself because it lost focus.
static LAST_BLUR_HIDE_MS: AtomicI64 = AtomicI64::new(0);

/// Format elapsed milliseconds as `H:MM:SS` (e.g. `2:14:12`).
pub fn format_hms(ms: i64) -> String {
    let total = (ms.max(0)) / 1000;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    format!("{h}:{m:02}:{s:02}")
}

/// Format elapsed milliseconds as `H:MM` (for the tooltip).
fn format_hm(ms: i64) -> String {
    let total = (ms.max(0)) / 60_000;
    format!("{}:{:02}", total / 60, total % 60)
}

/// Record that the quick panel just hid on blur (see `toggle_quick_panel`).
pub fn note_blur_hide() {
    LAST_BLUR_HIDE_MS.store(now_ms(), Ordering::Relaxed);
}

/// Where the quick panel opens relative to the tray icon: under it on macOS
/// (menu bar at the top), above it elsewhere (taskbar at the bottom).
#[cfg(target_os = "macos")]
const PANEL_POSITION: Position = Position::TrayCenter;
#[cfg(not(target_os = "macos"))]
const PANEL_POSITION: Position = Position::TrayBottomCenter;

/// Keep the window fully inside its monitor's usable area (not under the
/// taskbar or off the edge of the screen).
fn clamp_to_work_area(win: &WebviewWindow) {
    let monitor = win
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| win.primary_monitor().ok().flatten());
    let (Some(monitor), Ok(pos), Ok(size)) = (monitor, win.outer_position(), win.outer_size()) else {
        return;
    };
    let area = monitor.work_area();
    let (ax, ay) = (area.position.x, area.position.y);
    let max_x = (ax + area.size.width as i32 - size.width as i32).max(ax);
    let max_y = (ay + area.size.height as i32 - size.height as i32).max(ay);
    let x = pos.x.clamp(ax, max_x);
    let y = pos.y.clamp(ay, max_y);
    if (x, y) != (pos.x, pos.y) {
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
}

/// Fallback when the tray icon's position isn't known yet (the shortcut was
/// used before the icon was ever clicked): the corner nearest the tray.
fn place_in_tray_corner(win: &WebviewWindow) {
    let Some(monitor) = win.primary_monitor().ok().flatten() else {
        return;
    };
    let Ok(size) = win.outer_size() else { return };
    let area = monitor.work_area();
    let margin = (12.0 * monitor.scale_factor()) as i32;
    let x = area.position.x + area.size.width as i32 - size.width as i32 - margin;
    #[cfg(target_os = "macos")]
    let y = area.position.y + margin;
    #[cfg(not(target_os = "macos"))]
    let y = area.position.y + area.size.height as i32 - size.height as i32 - margin;
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

/// Show/hide the quick panel, anchored to the tray icon.
pub fn toggle_quick_panel(app: &AppHandle) {
    let Some(win) = app.get_webview_window("quick") else {
        return;
    };
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
        return;
    }
    // Clicking the tray icon while the panel is open first blurs the panel
    // (which hides it) and then delivers the click. Without this check that
    // click would immediately reopen it. Mostly a Windows thing.
    if now_ms() - LAST_BLUR_HIDE_MS.load(Ordering::Relaxed) < 250 {
        return;
    }
    if win.move_window(PANEL_POSITION).is_err() {
        place_in_tray_corner(&win);
    }
    clamp_to_work_area(&win);
    let _ = win.show();
    let _ = win.set_focus();
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

/// Tray icon for the idle / running state.
///
/// macOS: one monochrome template image that the system recolors for light
/// and dark menu bars (keep src-tauri/icons/tray.png black-on-transparent).
/// Elsewhere: a white icon with a dark outline (readable on light and dark
/// taskbars), and a teal one while a timer runs.
fn tray_image(running: bool) -> tauri::Result<tauri::image::Image<'static>> {
    #[cfg(target_os = "macos")]
    {
        let _ = running;
        tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        if running {
            tauri::image::Image::from_bytes(include_bytes!("../icons/tray-windows-running.png"))
        } else {
            tauri::image::Image::from_bytes(include_bytes!("../icons/tray-windows.png"))
        }
    }
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Dashboard", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit TimeBar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    TrayIconBuilder::with_id("main-tray")
        .icon(tray_image(false)?)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("TimeBar")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => open_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Let the positioner cache the tray rect for tray-relative anchoring.
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
/// reliably clears it, leaving the icon only. No-op on Windows.
pub fn set_tray_title(app: &AppHandle, text: Option<String>) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_title(Some(text.unwrap_or_default()));
    }
}

/// Spawn the 1s loop that keeps the tray in sync with the running session:
/// title (macOS), tooltip (everywhere) and icon (Windows/Linux). Source of
/// truth is `started_at`, so it never drifts.
pub fn spawn_title_updater(app: AppHandle) {
    let pool = app.state::<Db>().pool.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut last_title = String::new();
        let mut last_tooltip = String::new();
        let mut last_running = false;
        loop {
            interval.tick().await;
            let running: Option<(i64, String)> = sqlx::query_as(
                "SELECT s.started_at, t.name FROM sessions s JOIN tasks t ON t.id = s.task_id
                 WHERE s.ended_at IS NULL AND s.deleted_at IS NULL
                 ORDER BY s.started_at DESC LIMIT 1",
            )
            .fetch_optional(&pool)
            .await
            .ok()
            .flatten();
            let Some(tray) = app.tray_by_id("main-tray") else {
                continue;
            };
            let now = now_ms();

            // Current-session elapsed; empty string when idle → icon only.
            let title = running
                .as_ref()
                .map(|(s, _)| format_hms(now - s))
                .unwrap_or_default();
            if title != last_title {
                if !cfg!(windows) {
                    let _ = tray.set_title(Some(title.clone()));
                }
                last_title = title;
            }

            let tooltip = match &running {
                Some((s, name)) => format!("{name} · {}", format_hm(now - s)),
                None => "TimeBar · not tracking".to_string(),
            };
            if tooltip != last_tooltip {
                let _ = tray.set_tooltip(Some(tooltip.clone()));
                last_tooltip = tooltip;
            }

            let is_running = running.is_some();
            if is_running != last_running {
                if !cfg!(target_os = "macos") {
                    if let Ok(img) = tray_image(is_running) {
                        let _ = tray.set_icon(Some(img));
                    }
                }
                last_running = is_running;
            }
        }
    });
}
