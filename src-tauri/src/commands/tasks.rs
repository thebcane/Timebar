//! Task and folder CRUD (Section 8).

use super::{e, CmdResult};
use crate::db::{now_ms, Db};
use crate::models::{Folder, FolderPatch, Task, TaskPatch};
use tauri::State;

const FOLDER_COLS: &str =
    "id, name, color, parent_id, sort_order, archived, created_at, updated_at, deleted_at";
const TASK_COLS: &str = "id, name, color, folder_id, hourly_rate, currency, billable, sort_order, archived, created_at, updated_at, deleted_at";

// --- Folders ---------------------------------------------------------------

#[tauri::command]
pub async fn list_folders(db: State<'_, Db>) -> CmdResult<Vec<Folder>> {
    let sql = format!("SELECT {FOLDER_COLS} FROM folders WHERE deleted_at IS NULL ORDER BY sort_order, name");
    sqlx::query_as::<_, Folder>(&sql)
        .fetch_all(&db.pool)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn create_folder(
    db: State<'_, Db>,
    name: String,
    color: String,
    parent_id: Option<String>,
) -> CmdResult<Folder> {
    let now = now_ms();
    let folder = Folder {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        color,
        parent_id,
        sort_order: 0,
        archived: false,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    sqlx::query(
        "INSERT INTO folders (id, name, color, parent_id, sort_order, archived, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&folder.id)
    .bind(&folder.name)
    .bind(&folder.color)
    .bind(&folder.parent_id)
    .bind(folder.sort_order)
    .bind(folder.archived)
    .bind(folder.created_at)
    .bind(folder.updated_at)
    .execute(&db.pool)
    .await
    .map_err(e)?;
    Ok(folder)
}

#[tauri::command]
pub async fn update_folder(
    db: State<'_, Db>,
    id: String,
    patch: FolderPatch,
) -> CmdResult<Folder> {
    let sql = format!("SELECT {FOLDER_COLS} FROM folders WHERE id = ?");
    let mut f = sqlx::query_as::<_, Folder>(&sql)
        .bind(&id)
        .fetch_one(&db.pool)
        .await
        .map_err(e)?;

    if let Some(v) = patch.name {
        f.name = v;
    }
    if let Some(v) = patch.color {
        f.color = v;
    }
    if let Some(v) = patch.parent_id {
        f.parent_id = v;
    }
    if let Some(v) = patch.sort_order {
        f.sort_order = v;
    }
    if let Some(v) = patch.archived {
        f.archived = v;
    }
    f.updated_at = now_ms();

    sqlx::query(
        "UPDATE folders SET name=?, color=?, parent_id=?, sort_order=?, archived=?, updated_at=? WHERE id=?",
    )
    .bind(&f.name)
    .bind(&f.color)
    .bind(&f.parent_id)
    .bind(f.sort_order)
    .bind(f.archived)
    .bind(f.updated_at)
    .bind(&f.id)
    .execute(&db.pool)
    .await
    .map_err(e)?;
    Ok(f)
}

/// Soft-delete a folder and move its children to the top level. Soft delete
/// (a `deleted_at` tombstone) so the deletion syncs across devices; child
/// tasks/subfolders are re-parented to top level rather than deleted.
#[tauri::command]
pub async fn delete_folder(db: State<'_, Db>, id: String) -> CmdResult<()> {
    let now = now_ms();
    let mut tx = db.pool.begin().await.map_err(e)?;
    sqlx::query("UPDATE folders SET deleted_at=?, updated_at=? WHERE id=?")
        .bind(now)
        .bind(now)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(e)?;
    sqlx::query("UPDATE tasks SET folder_id=NULL, updated_at=? WHERE folder_id=?")
        .bind(now)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(e)?;
    sqlx::query("UPDATE folders SET parent_id=NULL, updated_at=? WHERE parent_id=?")
        .bind(now)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(e)?;
    tx.commit().await.map_err(e)?;
    Ok(())
}

#[tauri::command]
pub async fn archive_folder(db: State<'_, Db>, id: String, archived: bool) -> CmdResult<()> {
    sqlx::query("UPDATE folders SET archived=?, updated_at=? WHERE id=?")
        .bind(archived)
        .bind(now_ms())
        .bind(&id)
        .execute(&db.pool)
        .await
        .map_err(e)?;
    Ok(())
}

// --- Tasks -----------------------------------------------------------------

#[tauri::command]
pub async fn list_tasks(db: State<'_, Db>, include_archived: bool) -> CmdResult<Vec<Task>> {
    let sql = if include_archived {
        format!("SELECT {TASK_COLS} FROM tasks WHERE deleted_at IS NULL ORDER BY sort_order, name")
    } else {
        format!("SELECT {TASK_COLS} FROM tasks WHERE deleted_at IS NULL AND archived=0 ORDER BY sort_order, name")
    };
    sqlx::query_as::<_, Task>(&sql)
        .fetch_all(&db.pool)
        .await
        .map_err(e)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_task(
    db: State<'_, Db>,
    name: String,
    color: String,
    folder_id: Option<String>,
    hourly_rate: Option<f64>,
    currency: Option<String>,
    billable: Option<bool>,
) -> CmdResult<Task> {
    let now = now_ms();
    let task = Task {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        color,
        folder_id,
        hourly_rate: hourly_rate.unwrap_or(0.0),
        currency: currency.unwrap_or_else(|| "USD".into()),
        billable: billable.unwrap_or(false),
        sort_order: 0,
        archived: false,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    sqlx::query(
        "INSERT INTO tasks (id, name, color, folder_id, hourly_rate, currency, billable, sort_order, archived, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&task.id)
    .bind(&task.name)
    .bind(&task.color)
    .bind(&task.folder_id)
    .bind(task.hourly_rate)
    .bind(&task.currency)
    .bind(task.billable)
    .bind(task.sort_order)
    .bind(task.archived)
    .bind(task.created_at)
    .bind(task.updated_at)
    .execute(&db.pool)
    .await
    .map_err(e)?;
    Ok(task)
}

#[tauri::command]
pub async fn update_task(db: State<'_, Db>, id: String, patch: TaskPatch) -> CmdResult<Task> {
    let sql = format!("SELECT {TASK_COLS} FROM tasks WHERE id = ?");
    let mut t = sqlx::query_as::<_, Task>(&sql)
        .bind(&id)
        .fetch_one(&db.pool)
        .await
        .map_err(e)?;
    let old_rate = t.hourly_rate;

    if let Some(v) = patch.name {
        t.name = v;
    }
    if let Some(v) = patch.color {
        t.color = v;
    }
    if let Some(v) = patch.folder_id {
        t.folder_id = v;
    }
    if let Some(v) = patch.hourly_rate {
        t.hourly_rate = v;
    }
    if let Some(v) = patch.currency {
        t.currency = v;
    }
    if let Some(v) = patch.billable {
        t.billable = v;
    }
    if let Some(v) = patch.sort_order {
        t.sort_order = v;
    }
    if let Some(v) = patch.archived {
        t.archived = v;
    }
    t.updated_at = now_ms();

    sqlx::query(
        "UPDATE tasks SET name=?, color=?, folder_id=?, hourly_rate=?, currency=?, billable=?, sort_order=?, archived=?, updated_at=? WHERE id=?",
    )
    .bind(&t.name)
    .bind(&t.color)
    .bind(&t.folder_id)
    .bind(t.hourly_rate)
    .bind(&t.currency)
    .bind(t.billable)
    .bind(t.sort_order)
    .bind(t.archived)
    .bind(t.updated_at)
    .bind(&t.id)
    .execute(&db.pool)
    .await
    .map_err(e)?;

    // If the rate changed, apply it to this task's existing sessions so
    // already-tracked time bills at the new rate (e.g. setting a rate after
    // the fact). `updated_at` bumps so the change syncs across devices.
    if (t.hourly_rate - old_rate).abs() > f64::EPSILON {
        sqlx::query(
            "UPDATE sessions SET rate_snapshot=?, updated_at=? WHERE task_id=? AND deleted_at IS NULL",
        )
        .bind(t.hourly_rate)
        .bind(t.updated_at)
        .bind(&t.id)
        .execute(&db.pool)
        .await
        .map_err(e)?;
    }

    Ok(t)
}

#[tauri::command]
pub async fn archive_task(db: State<'_, Db>, id: String, archived: bool) -> CmdResult<()> {
    sqlx::query("UPDATE tasks SET archived=?, updated_at=? WHERE id=?")
        .bind(archived)
        .bind(now_ms())
        .bind(&id)
        .execute(&db.pool)
        .await
        .map_err(e)?;
    Ok(())
}

/// Soft-delete a task and its sessions (a `deleted_at` tombstone so the
/// deletion syncs across devices). FE confirms first.
#[tauri::command]
pub async fn delete_task(db: State<'_, Db>, id: String) -> CmdResult<()> {
    let now = now_ms();
    let mut tx = db.pool.begin().await.map_err(e)?;
    sqlx::query("UPDATE tasks SET deleted_at=?, updated_at=? WHERE id=?")
        .bind(now)
        .bind(now)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(e)?;
    sqlx::query("UPDATE sessions SET deleted_at=?, updated_at=? WHERE task_id=? AND deleted_at IS NULL")
        .bind(now)
        .bind(now)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(e)?;
    tx.commit().await.map_err(e)?;
    Ok(())
}

/// Persist a new manual ordering. `entity` is "task" or "folder".
#[tauri::command]
pub async fn reorder(db: State<'_, Db>, entity: String, ordered_ids: Vec<String>) -> CmdResult<()> {
    let table = match entity.as_str() {
        "task" => "tasks",
        "folder" => "folders",
        other => return Err(format!("unknown entity: {other}")),
    };
    let now = now_ms();
    let mut tx = db.pool.begin().await.map_err(e)?;
    for (i, id) in ordered_ids.iter().enumerate() {
        let sql = format!("UPDATE {table} SET sort_order=?, updated_at=? WHERE id=?");
        sqlx::query(&sql)
            .bind(i as i64)
            .bind(now)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(e)?;
    }
    tx.commit().await.map_err(e)?;
    Ok(())
}
