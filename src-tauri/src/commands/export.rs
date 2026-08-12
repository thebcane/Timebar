//! CSV export of entries (Section 8). Writes to the app data `exports/` dir
//! and returns the file path.

use super::queries::entries_for;
use super::{e, CmdResult};
use crate::db::Db;
use crate::models::Range;
use crate::timeutil::{format_local, resolve_tz};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn export_csv(
    app: AppHandle,
    db: State<'_, Db>,
    range: Range,
    tz: String,
) -> CmdResult<String> {
    let tzr = resolve_tz(&tz);
    let days = entries_for(&db.pool, range, &tz).await.map_err(e)?;

    let dir = app.path().app_data_dir().map_err(e)?.join("exports");
    std::fs::create_dir_all(&dir).map_err(e)?;
    let path = dir.join(format!("timebar-{}-{}.csv", range.from, range.to));

    let mut wtr = csv::Writer::from_path(&path).map_err(e)?;
    wtr.write_record([
        "date",
        "task",
        "folder",
        "start",
        "end",
        "duration_hours",
        "note",
        "billable",
        "rate",
        "revenue",
        "billed",
    ])
    .map_err(e)?;

    for day in &days {
        for en in &day.entries {
            let hours = en.duration_ms as f64 / 3_600_000.0;
            wtr.write_record([
                day.date.clone(),
                en.task_name.clone(),
                en.folder_name.clone().unwrap_or_default(),
                format_local(en.started_at, tzr),
                format_local(en.ended_at, tzr),
                format!("{hours:.2}"),
                en.note.clone().unwrap_or_default(),
                if en.billable { "yes".into() } else { "no".into() },
                format!("{:.2}", en.revenue / hours.max(1e-9)),
                format!("{:.2}", en.revenue),
                if en.billed { "yes".into() } else { "no".into() },
            ])
            .map_err(e)?;
        }
    }
    wtr.flush().map_err(e)?;
    Ok(path.to_string_lossy().to_string())
}
