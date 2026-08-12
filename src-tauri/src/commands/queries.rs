//! Read-side aggregation (Section 8). Sessions are fetched once per call and
//! split into local days here (§4), so every surface agrees on totals.

use super::{e, CmdResult};
use crate::db::{now_ms, Db};
use crate::models::{
    DayGroup, EntryView, Range, ReportDay, ReportSegment, ReportTaskTotal, ReportView, Task,
    TimelineBlock, TreeNode,
};
use crate::timeutil::{resolve_tz, split_by_local_day};
use crate::models::Folder;
use sqlx::SqlitePool;
use std::collections::{BTreeMap, HashMap};
use tauri::State;

/// Joined row for aggregation.
#[derive(sqlx::FromRow, Clone)]
struct AggRow {
    session_id: String,
    task_id: String,
    task_name: String,
    task_color: String,
    task_archived: bool,
    folder_name: Option<String>,
    started_at: i64,
    ended_at: Option<i64>,
    note: Option<String>,
    billable: bool,
    billed: bool,
    rate_snapshot: f64,
    source: String,
}

const AGG_SQL: &str = "
    SELECT s.id AS session_id, s.task_id, s.started_at, s.ended_at, s.note,
           s.billed, s.rate_snapshot, s.source,
           t.name AS task_name, t.color AS task_color, t.archived AS task_archived,
           t.billable AS billable, f.name AS folder_name
    FROM sessions s
    JOIN tasks t ON t.id = s.task_id
    LEFT JOIN folders f ON f.id = t.folder_id
    WHERE s.deleted_at IS NULL AND t.deleted_at IS NULL
      AND s.started_at < ? AND COALESCE(s.ended_at, ?) > ?
    ORDER BY s.started_at
";

async fn fetch_overlapping(
    pool: &SqlitePool,
    range: Range,
    now: i64,
) -> Result<Vec<AggRow>, sqlx::Error> {
    sqlx::query_as::<_, AggRow>(AGG_SQL)
        .bind(range.to)
        .bind(now)
        .bind(range.from)
        .fetch_all(pool)
        .await
}

#[inline]
fn seg_revenue(ms: i64, billable: bool, rate: f64) -> f64 {
    if billable {
        (ms as f64 / 3_600_000.0) * rate
    } else {
        0.0
    }
}

#[derive(Default)]
struct DayAcc {
    total_ms: i64,
    total_rev: f64,
    entries: Vec<EntryView>,
}

/// Shared day-grouping used by both `get_time_entries` and CSV export.
pub async fn entries_for(
    pool: &SqlitePool,
    range: Range,
    tz: &str,
) -> Result<Vec<DayGroup>, sqlx::Error> {
    let tzr = resolve_tz(tz);
    let now = now_ms();
    let rows = fetch_overlapping(pool, range, now).await?;

    let mut map: BTreeMap<String, DayAcc> = BTreeMap::new();
    for r in rows {
        let is_active = r.ended_at.is_none();
        let eff_end = r.ended_at.unwrap_or(now);
        for seg in split_by_local_day(r.started_at, eff_end, tzr) {
            let s = seg.start_ms.max(range.from);
            let en = seg.end_ms.min(range.to);
            if en <= s {
                continue;
            }
            let dur = en - s;
            let rev = seg_revenue(dur, r.billable, r.rate_snapshot);
            let acc = map.entry(seg.date.clone()).or_default();
            acc.total_ms += dur;
            acc.total_rev += rev;
            acc.entries.push(EntryView {
                session_id: r.session_id.clone(),
                task_id: r.task_id.clone(),
                task_name: r.task_name.clone(),
                task_color: r.task_color.clone(),
                folder_name: r.folder_name.clone(),
                started_at: s,
                ended_at: en,
                note: r.note.clone(),
                billable: r.billable,
                billed: r.billed,
                source: r.source.clone(),
                duration_ms: dur,
                revenue: rev,
                is_active,
            });
        }
    }

    let mut days: Vec<DayGroup> = map
        .into_iter()
        .map(|(date, acc)| {
            let mut entries = acc.entries;
            entries.sort_by_key(|x| x.started_at);
            DayGroup {
                date,
                total_ms: acc.total_ms,
                total_revenue: acc.total_rev,
                entries,
            }
        })
        .collect();
    // Most-recent day first.
    days.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(days)
}

#[tauri::command]
pub async fn get_time_entries(
    db: State<'_, Db>,
    range: Range,
    tz: String,
) -> CmdResult<Vec<DayGroup>> {
    entries_for(&db.pool, range, &tz).await.map_err(e)
}

#[tauri::command]
pub async fn get_timeline(
    db: State<'_, Db>,
    range: Range,
    tz: String,
) -> CmdResult<Vec<TimelineBlock>> {
    let tzr = resolve_tz(&tz);
    let now = now_ms();
    let rows = fetch_overlapping(&db.pool, range, now).await.map_err(e)?;

    let mut blocks = Vec::new();
    for r in rows {
        let is_active = r.ended_at.is_none();
        let eff_end = r.ended_at.unwrap_or(now);
        for seg in split_by_local_day(r.started_at, eff_end, tzr) {
            let s = seg.start_ms.max(range.from);
            let en = seg.end_ms.min(range.to);
            if en <= s {
                continue;
            }
            let start_min = ((s - seg.day_start_ms) / 60_000) as i32;
            let end_min = ((en - seg.day_start_ms) / 60_000) as i32;
            blocks.push(TimelineBlock {
                session_id: r.session_id.clone(),
                task_id: r.task_id.clone(),
                task_name: r.task_name.clone(),
                task_color: r.task_color.clone(),
                note: r.note.clone(),
                date: seg.date.clone(),
                started_at: s,
                ended_at: en,
                start_min,
                end_min,
                duration_ms: en - s,
                is_active,
            });
        }
    }
    Ok(blocks)
}

// --- Task tree with rolled-up totals ---------------------------------------

fn task_node(t: &Task, totals: &HashMap<String, (i64, f64)>, active: &Option<String>) -> TreeNode {
    let (ms, rev) = totals.get(&t.id).copied().unwrap_or((0, 0.0));
    TreeNode {
        kind: "task".into(),
        id: t.id.clone(),
        name: t.name.clone(),
        color: t.color.clone(),
        folder_id: t.folder_id.clone(),
        archived: t.archived,
        billable: t.billable,
        hourly_rate: t.hourly_rate,
        currency: t.currency.clone(),
        is_active: active.as_deref() == Some(t.id.as_str()),
        total_ms: ms,
        total_revenue: rev,
        children: vec![],
    }
}

fn folder_node(
    f: &Folder,
    folders: &[Folder],
    tasks: &[Task],
    totals: &HashMap<String, (i64, f64)>,
    active: &Option<String>,
) -> TreeNode {
    let mut children: Vec<TreeNode> = Vec::new();
    for sf in folders.iter().filter(|x| x.parent_id.as_deref() == Some(f.id.as_str())) {
        children.push(folder_node(sf, folders, tasks, totals, active));
    }
    for t in tasks.iter().filter(|x| x.folder_id.as_deref() == Some(f.id.as_str())) {
        children.push(task_node(t, totals, active));
    }
    let total_ms = children.iter().map(|c| c.total_ms).sum();
    let total_revenue = children.iter().map(|c| c.total_revenue).sum();
    TreeNode {
        kind: "folder".into(),
        id: f.id.clone(),
        name: f.name.clone(),
        color: f.color.clone(),
        folder_id: None,
        archived: f.archived,
        billable: false,
        hourly_rate: 0.0,
        currency: "USD".into(),
        is_active: false,
        total_ms,
        total_revenue,
        children,
    }
}

#[tauri::command]
pub async fn get_task_tree_with_totals(
    db: State<'_, Db>,
    range: Range,
    tz: String,
    include_archived: bool,
) -> CmdResult<Vec<TreeNode>> {
    let tzr = resolve_tz(&tz);
    let now = now_ms();
    let rows = fetch_overlapping(&db.pool, range, now).await.map_err(e)?;

    let mut totals: HashMap<String, (i64, f64)> = HashMap::new();
    for r in &rows {
        let eff_end = r.ended_at.unwrap_or(now);
        for seg in split_by_local_day(r.started_at, eff_end, tzr) {
            let s = seg.start_ms.max(range.from);
            let en = seg.end_ms.min(range.to);
            if en <= s {
                continue;
            }
            let dur = en - s;
            let rev = seg_revenue(dur, r.billable, r.rate_snapshot);
            let entry = totals.entry(r.task_id.clone()).or_default();
            entry.0 += dur;
            entry.1 += rev;
        }
    }

    let arch_filter = if include_archived {
        "WHERE deleted_at IS NULL"
    } else {
        "WHERE deleted_at IS NULL AND archived=0"
    };
    let folders = sqlx::query_as::<_, Folder>(&format!(
        "SELECT id, name, color, parent_id, sort_order, archived, created_at, updated_at, deleted_at FROM folders {arch_filter} ORDER BY sort_order, name"
    ))
    .fetch_all(&db.pool)
    .await
    .map_err(e)?;
    let tasks = sqlx::query_as::<_, Task>(&format!(
        "SELECT id, name, color, folder_id, hourly_rate, currency, billable, sort_order, archived, created_at, updated_at, deleted_at FROM tasks {arch_filter} ORDER BY sort_order, name"
    ))
    .fetch_all(&db.pool)
    .await
    .map_err(e)?;

    let active: Option<String> = sqlx::query_scalar(
        "SELECT task_id FROM sessions WHERE ended_at IS NULL AND deleted_at IS NULL ORDER BY started_at DESC LIMIT 1",
    )
    .fetch_optional(&db.pool)
    .await
    .map_err(e)?;

    let mut out: Vec<TreeNode> = Vec::new();
    for f in folders.iter().filter(|x| x.parent_id.is_none()) {
        out.push(folder_node(f, &folders, &tasks, &totals, &active));
    }
    for t in tasks.iter().filter(|x| x.folder_id.is_none()) {
        out.push(task_node(t, &totals, &active));
    }
    Ok(out)
}

// --- Report ----------------------------------------------------------------

#[tauri::command]
pub async fn get_report(
    db: State<'_, Db>,
    range: Range,
    tz: String,
    mode: String,
) -> CmdResult<ReportView> {
    let _ = mode; // Both time and money are returned; the FE toggle is instant.
    let tzr = resolve_tz(&tz);
    let now = now_ms();
    let rows = fetch_overlapping(&db.pool, range, now).await.map_err(e)?;

    let mut day_map: BTreeMap<String, HashMap<String, (i64, f64)>> = BTreeMap::new();
    let mut task_meta: HashMap<String, (String, String, bool)> = HashMap::new();
    let mut task_tot: HashMap<String, (i64, f64)> = HashMap::new();
    let mut paid_ms = 0i64;
    let mut paid_rev = 0.0;
    let mut total_ms = 0i64;
    let mut total_rev = 0.0;

    for r in &rows {
        task_meta
            .entry(r.task_id.clone())
            .or_insert((r.task_name.clone(), r.task_color.clone(), r.task_archived));
        let eff_end = r.ended_at.unwrap_or(now);
        for seg in split_by_local_day(r.started_at, eff_end, tzr) {
            let s = seg.start_ms.max(range.from);
            let en = seg.end_ms.min(range.to);
            if en <= s {
                continue;
            }
            let dur = en - s;
            let rev = seg_revenue(dur, r.billable, r.rate_snapshot);
            let dmap = day_map.entry(seg.date.clone()).or_default();
            let cell = dmap.entry(r.task_id.clone()).or_default();
            cell.0 += dur;
            cell.1 += rev;
            let tt = task_tot.entry(r.task_id.clone()).or_default();
            tt.0 += dur;
            tt.1 += rev;
            total_ms += dur;
            total_rev += rev;
            if r.billed {
                paid_ms += dur;
                paid_rev += rev;
            }
        }
    }

    let days: Vec<ReportDay> = day_map
        .into_iter()
        .map(|(date, tmap)| {
            let mut segments: Vec<ReportSegment> = tmap
                .into_iter()
                .map(|(tid, (ms, rev))| {
                    let (name, color, _) = task_meta.get(&tid).cloned().unwrap_or_default();
                    ReportSegment {
                        task_id: tid,
                        task_name: name,
                        color,
                        ms,
                        revenue: rev,
                    }
                })
                .collect();
            segments.sort_by(|a, b| b.ms.cmp(&a.ms));
            let tms = segments.iter().map(|s| s.ms).sum();
            let trev = segments.iter().map(|s| s.revenue).sum();
            ReportDay {
                date,
                total_ms: tms,
                total_revenue: trev,
                segments,
            }
        })
        .collect();

    let mut tasks: Vec<ReportTaskTotal> = Vec::new();
    let mut archived: Vec<ReportTaskTotal> = Vec::new();
    for (tid, (ms, rev)) in task_tot {
        let (name, color, is_arch) = task_meta.get(&tid).cloned().unwrap_or_default();
        let item = ReportTaskTotal {
            id: tid,
            name,
            color,
            ms,
            revenue: rev,
            archived: is_arch,
        };
        if is_arch {
            archived.push(item);
        } else {
            tasks.push(item);
        }
    }
    tasks.sort_by(|a, b| b.ms.cmp(&a.ms));
    archived.sort_by(|a, b| b.ms.cmp(&a.ms));

    Ok(ReportView {
        days,
        tasks,
        archived,
        billing_paid_ms: paid_ms,
        billing_paid_revenue: paid_rev,
        total_ms,
        total_revenue: total_rev,
    })
}
