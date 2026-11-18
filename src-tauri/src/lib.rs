//! TimeBar app setup: database, tray, activation policy, windows, IPC.

mod commands;
mod db;
mod models;
mod sync;
mod timeutil;
mod tray;

use db::Db;
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Cmd+Shift+T toggles the quick panel by default (configurable later, §7).
    let toggle_shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyT);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state == ShortcutState::Pressed
                        && shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::KeyT)
                    {
                        tray::toggle_quick_panel(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            // tasks & folders
            commands::tasks::list_folders,
            commands::tasks::create_folder,
            commands::tasks::update_folder,
            commands::tasks::archive_folder,
            commands::tasks::delete_folder,
            commands::tasks::list_tasks,
            commands::tasks::create_task,
            commands::tasks::update_task,
            commands::tasks::archive_task,
            commands::tasks::delete_task,
            commands::tasks::reorder,
            // sessions & timer
            commands::sessions::get_active_session,
            commands::sessions::get_session,
            commands::sessions::start_task,
            commands::sessions::stop_active,
            commands::sessions::create_manual_session,
            commands::sessions::update_session,
            commands::sessions::delete_session,
            // queries
            commands::queries::get_time_entries,
            commands::queries::get_timeline,
            commands::queries::get_task_tree_with_totals,
            commands::queries::get_report,
            // settings
            commands::settings::get_settings,
            commands::settings::update_setting,
            commands::settings::set_global_shortcut,
            // export
            commands::export::export_csv,
            // windows
            tray::open_dashboard,
            tray::hide_quick,
            // sync
            sync::sync_status,
            sync::sync_sign_in,
            sync::sync_sign_up,
            sync::sync_sign_out,
            sync::sync_change_password,
            sync::sync_now,
        ])
        .setup(move |app| {
            // --- Database ------------------------------------------------
            // TIMEBAR_DATA_DIR points the app at another database, e.g. to run
            // a second copy as a separate "device" when testing sync.
            let data_dir = std::env::var_os("TIMEBAR_DATA_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| app.path().app_data_dir().expect("resolve app data dir"));
            std::fs::create_dir_all(&data_dir).ok();
            let db_path = data_dir.join("timebar.db");
            let db = tauri::async_runtime::block_on(Db::init(&db_path)).expect("initialize database");
            app.manage(db);

            // --- macOS accessory app (menu bar, no Dock icon) ------------
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // --- Tray + live title loop ----------------------------------
            let handle = app.handle().clone();
            tray::build_tray(&handle)?;
            tray::spawn_title_updater(handle.clone());

            // --- Background sync loop (idle until signed in) -------------
            sync::spawn_sync_loop(handle.clone());

            // --- Quick panel hides on blur -------------------------------
            if let Some(quick) = app.get_webview_window("quick") {
                let q = quick.clone();
                quick.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = q.hide();
                    }
                });
            }

            // --- Main window: red X hides (don't destroy) so the tray's
            //     "Open Dashboard" can reopen it. Quit is via the tray menu.
            if let Some(main) = app.get_webview_window("main") {
                let m = main.clone();
                main.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = m.hide();
                    }
                });
            }

            // --- Global shortcut (best-effort, only if enabled) ----------
            let shortcut_enabled = tauri::async_runtime::block_on(async {
                sqlx::query_scalar::<_, String>(
                    "SELECT value FROM settings WHERE key='global_shortcut_enabled'",
                )
                .fetch_optional(&app.state::<Db>().pool)
                .await
                .ok()
                .flatten()
                .map(|v| v != "0")
                .unwrap_or(true)
            });
            if shortcut_enabled {
                let _ = app.global_shortcut().register(toggle_shortcut);
            }

            // --- Dev-only: self-test the start/stop path (no click) ------
            if std::env::var("TIMEBAR_SELFTEST").is_ok() {
                let pool = app.state::<Db>().pool.clone();
                tauri::async_runtime::block_on(async {
                    let tid: Option<String> =
                        sqlx::query_scalar("SELECT id FROM tasks ORDER BY sort_order LIMIT 1")
                            .fetch_optional(&pool)
                            .await
                            .ok()
                            .flatten();
                    if let Some(tid) = tid {
                        match commands::sessions::do_start(&pool, &tid, db::now_ms()).await {
                            Ok(sid) => {
                                let running: i64 = sqlx::query_scalar(
                                    "SELECT COUNT(*) FROM sessions WHERE ended_at IS NULL",
                                )
                                .fetch_one(&pool)
                                .await
                                .unwrap_or(-1);
                                println!("SELFTEST: start ok -> session {sid}, running rows = {running}");
                                // Clean up so we don't leave a dangling timer.
                                let _ = sqlx::query(
                                    "UPDATE sessions SET ended_at=?, updated_at=?, dirty=1 WHERE id=?",
                                )
                                .bind(db::now_ms())
                                .bind(db::now_ms())
                                .bind(&sid)
                                .execute(&pool)
                                .await;
                                let _ = sqlx::query("DELETE FROM settings WHERE key='active_session_id'")
                                    .execute(&pool)
                                    .await;
                                println!("SELFTEST: cleanup done");
                            }
                            Err(e) => println!("SELFTEST: start FAILED -> {e}"),
                        }
                    }
                });
            }

            // --- Dev-only: force windows visible for screenshotting ------
            if std::env::var("TIMEBAR_SHOW_ALL").is_ok() {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
                if let Some(w) = app.get_webview_window("quick") {
                    let _ = w.set_always_on_top(false);
                    let _ = w.show();
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
