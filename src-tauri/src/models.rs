//! Entities, patch inputs, and computed view structs (Sections 4, 5, 8).
//!
//! Entity structs derive `sqlx::FromRow` and map 1:1 to table columns
//! (snake_case). Everything crossing IPC serializes to camelCase for the
//! frontend. View structs are assembled in Rust so all aggregation stays here.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub color: String,
    pub parent_id: Option<String>,
    pub sort_order: i64,
    pub archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub name: String,
    pub color: String,
    pub folder_id: Option<String>,
    pub hourly_rate: f64,
    pub currency: String,
    pub billable: bool,
    pub sort_order: i64,
    pub archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub task_id: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub note: Option<String>,
    pub rate_snapshot: f64,
    pub billed: bool,
    pub source: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

/// A UTC epoch-ms half-open range `[from, to)`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Range {
    pub from: i64,
    pub to: i64,
}

/// For nullable-settable fields we use `Option<Option<T>>`:
/// missing key -> `None` (leave unchanged); explicit `null` -> `Some(None)`;
/// value -> `Some(Some(v))`. `#[serde(default)]` makes the distinction work.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPatch {
    pub name: Option<String>,
    pub color: Option<String>,
    #[serde(default)]
    pub folder_id: Option<Option<String>>,
    pub hourly_rate: Option<f64>,
    pub currency: Option<String>,
    pub billable: Option<bool>,
    pub sort_order: Option<i64>,
    pub archived: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderPatch {
    pub name: Option<String>,
    pub color: Option<String>,
    #[serde(default)]
    pub parent_id: Option<Option<String>>,
    pub sort_order: Option<i64>,
    pub archived: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPatch {
    pub task_id: Option<String>,
    pub started_at: Option<i64>,
    #[serde(default)]
    pub ended_at: Option<Option<i64>>,
    #[serde(default)]
    pub note: Option<Option<String>>,
    pub billed: Option<bool>,
    pub rate_snapshot: Option<f64>,
    /// When reassigning `task_id`, re-copy the new task's rate into the
    /// snapshot. Default false — historical earnings are preserved (§7).
    pub resnapshot_rate: Option<bool>,
}

// ---------------------------------------------------------------------------
// Views (assembled in Rust)
// ---------------------------------------------------------------------------

/// A single session with joined task info and computed duration/revenue.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub task_id: String,
    pub task_name: String,
    pub task_color: String,
    pub folder_name: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub note: Option<String>,
    pub rate_snapshot: f64,
    pub billable: bool,
    pub billed: bool,
    pub source: String,
    pub duration_ms: i64,
    pub revenue: f64,
    pub is_active: bool,
}

/// A session clamped to one local day (midnight-split, §4). `session_id`
/// still points at the whole underlying session so edits round-trip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub session_id: String,
    pub task_id: String,
    pub task_name: String,
    pub task_color: String,
    pub folder_name: Option<String>,
    pub started_at: i64,
    pub ended_at: i64,
    pub note: Option<String>,
    pub billable: bool,
    pub billed: bool,
    pub source: String,
    pub duration_ms: i64,
    pub revenue: f64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayGroup {
    /// Local calendar day, `YYYY-MM-DD`.
    pub date: String,
    pub total_ms: i64,
    pub total_revenue: f64,
    pub entries: Vec<EntryView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    pub kind: String, // "folder" | "task"
    pub id: String,
    pub name: String,
    pub color: String,
    pub folder_id: Option<String>,
    pub archived: bool,
    pub billable: bool,
    pub hourly_rate: f64,
    pub currency: String,
    pub is_active: bool,
    pub total_ms: i64,
    pub total_revenue: f64,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineBlock {
    pub session_id: String,
    pub task_id: String,
    pub task_name: String,
    pub task_color: String,
    pub note: Option<String>,
    pub date: String,
    pub started_at: i64,
    pub ended_at: i64,
    /// Minutes from local midnight, for positioning on the day axis.
    pub start_min: i32,
    pub end_min: i32,
    pub duration_ms: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportSegment {
    pub task_id: String,
    pub task_name: String,
    pub color: String,
    pub ms: i64,
    pub revenue: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDay {
    pub date: String,
    pub total_ms: i64,
    pub total_revenue: f64,
    pub segments: Vec<ReportSegment>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportTaskTotal {
    pub id: String,
    pub name: String,
    pub color: String,
    pub ms: i64,
    pub revenue: f64,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportView {
    pub days: Vec<ReportDay>,
    pub tasks: Vec<ReportTaskTotal>,
    pub archived: Vec<ReportTaskTotal>,
    pub billing_paid_ms: i64,
    pub billing_paid_revenue: f64,
    pub total_ms: i64,
    pub total_revenue: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub first_day_of_week: i32,
    pub default_currency: String,
    pub active_session_id: Option<String>,
    pub theme: String,
    pub global_shortcut_enabled: bool,
    /// Start TimeBar when you log in (read from the OS, not the database).
    pub launch_at_login: bool,
}
