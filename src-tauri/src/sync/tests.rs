//! Local half of sync v2, against a real SQLite file with the app's
//! migrations. Run with `cargo test` in src-tauri.

use super::*;
use crate::db::Db;

async fn fresh_db() -> SqlitePool {
    let path = std::env::temp_dir().join(format!("timebar-test-{}.db", uuid::Uuid::new_v4()));
    Db::init(&path).await.expect("init db").pool
}

async fn first_task(pool: &SqlitePool) -> (String, i64) {
    sqlx::query_as("SELECT id, updated_at FROM tasks ORDER BY sort_order LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn insert_running(pool: &SqlitePool, task: &str, id: &str, started_at: i64) {
    sqlx::query(
        "INSERT INTO sessions (id, task_id, started_at, ended_at, rate_snapshot, billed, source, created_at, updated_at)
         VALUES (?, ?, ?, NULL, 0, 0, 'timer', ?, ?)",
    )
    .bind(id).bind(task).bind(started_at).bind(started_at).bind(started_at)
    .execute(pool).await.unwrap();
}

#[tokio::test]
async fn new_rows_start_dirty() {
    let pool = fresh_db().await;
    let clean: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE dirty = 0")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(clean, 0, "every new row must be queued for push");
}

#[tokio::test]
async fn local_edit_never_moves_updated_at_backwards() {
    let pool = fresh_db().await;
    let (id, before) = first_task(&pool).await;
    // Simulate a device whose clock is an hour behind editing the row.
    sqlx::query("UPDATE tasks SET name='Renamed', updated_at=?, dirty=1 WHERE id=?")
        .bind(before - 3_600_000).bind(&id)
        .execute(&pool).await.unwrap();
    let (_, after) = first_task(&pool).await;
    assert_eq!(after, before + 1);
}

#[tokio::test]
async fn reconcile_keeps_only_the_newest_running_session() {
    let pool = fresh_db().await;
    let (task, _) = first_task(&pool).await;
    insert_running(&pool, &task, "a", 1_000_000).await;
    insert_running(&pool, &task, "b", 3_000_000).await;
    insert_running(&pool, &task, "c", 2_000_000).await;

    let closed = reconcile_running(&pool).await.unwrap();
    assert_eq!(closed, 2);

    let rows: Vec<(String, Option<i64>, i64)> =
        sqlx::query_as("SELECT id, ended_at, dirty FROM sessions WHERE id IN ('a','b','c') ORDER BY id")
            .fetch_all(&pool).await.unwrap();
    assert_eq!(rows[0], ("a".into(), Some(3_000_000), 1));
    assert_eq!(rows[1], ("b".into(), None, 1));
    assert_eq!(rows[2], ("c".into(), Some(3_000_000), 1));
    assert_eq!(get_setting(&pool, "active_session_id").await.as_deref(), Some("b"));

    // Idempotent.
    assert_eq!(reconcile_running(&pool).await.unwrap(), 0);
}

#[tokio::test]
async fn reconcile_never_makes_zero_length_sessions() {
    let pool = fresh_db().await;
    let (task, _) = first_task(&pool).await;
    insert_running(&pool, &task, "x", 5_000_000).await;
    insert_running(&pool, &task, "y", 5_000_000).await; // same start
    reconcile_running(&pool).await.unwrap();
    let ended: Vec<(String, i64, Option<i64>)> = sqlx::query_as(
        "SELECT id, started_at, ended_at FROM sessions WHERE id IN ('x','y') AND ended_at IS NOT NULL",
    )
    .fetch_all(&pool).await.unwrap();
    assert_eq!(ended.len(), 1);
    assert!(ended[0].2.unwrap() > ended[0].1);
}

#[tokio::test]
async fn seed_is_discarded_unless_used() {
    let pool = fresh_db().await;
    let tasks: Vec<(String, String)> =
        sqlx::query_as("SELECT id, name FROM tasks ORDER BY sort_order").fetch_all(&pool).await.unwrap();
    assert_eq!(tasks.len(), 6);
    let (edited, tracked) = (&tasks[0].0, &tasks[3].0);

    // Edit one seed task, track real time against another.
    sqlx::query("UPDATE tasks SET name='My real client', updated_at=?, dirty=1 WHERE id=?")
        .bind(now_ms() + 10).bind(edited).execute(&pool).await.unwrap();
    insert_running(&pool, tracked, "mine", now_ms() + 20).await;

    discard_unused_seed(&pool).await.unwrap();

    let left: Vec<String> =
        sqlx::query_scalar("SELECT id FROM tasks ORDER BY sort_order").fetch_all(&pool).await.unwrap();
    assert_eq!(left, vec![edited.clone(), tracked.clone()]);
    let sessions: Vec<String> =
        sqlx::query_scalar("SELECT id FROM sessions").fetch_all(&pool).await.unwrap();
    assert_eq!(sessions, vec!["mine".to_string()], "sample sessions go, real ones stay");
    // Both remaining tasks live in folders, so both folders stay; none were dropped
    // out from under a task.
    let orphans: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks t WHERE t.folder_id IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM folders f WHERE f.id = t.folder_id)",
    )
    .fetch_one(&pool).await.unwrap();
    assert_eq!(orphans, 0);
    assert!(get_setting(&pool, "seed_manifest").await.is_none(), "runs once");
}

#[tokio::test]
async fn untouched_seed_is_removed_completely() {
    let pool = fresh_db().await;
    discard_unused_seed(&pool).await.unwrap();
    for t in ["folders", "tasks", "sessions"] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {t}"))
            .fetch_one(&pool).await.unwrap();
        assert_eq!(n, 0, "{t} should be empty");
    }
}

fn remote_task(id: &str, folder: Option<&str>, name: &str, updated_at: i64, seq: i64) -> SyncTask {
    SyncTask {
        id: id.into(),
        name: name.into(),
        color: "#fff".into(),
        folder_id: folder.map(String::from),
        hourly_rate: 0.0,
        currency: "USD".into(),
        billable: false,
        sort_order: 0,
        archived: false,
        created_at: 1,
        updated_at,
        deleted_at: None,
        user_id: None,
        server_seq: seq,
    }
}

async fn apply_with_fk_off<T: SyncRow>(pool: &SqlitePool, rows: &[T]) -> i64 {
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
    let n = apply_rows(&mut conn, rows).await.unwrap();
    sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    n
}

#[tokio::test]
async fn pull_is_last_write_wins_and_clears_dirty() {
    let pool = fresh_db().await;
    let (id, local_updated) = first_task(&pool).await;

    // Older remote version: ignored, local edit stays queued.
    let n = apply_with_fk_off(&pool, &[remote_task(&id, None, "stale", local_updated - 5, 1)]).await;
    assert_eq!(n, 0);
    let (name, dirty): (String, i64) =
        sqlx::query_as("SELECT name, dirty FROM tasks WHERE id=?").bind(&id).fetch_one(&pool).await.unwrap();
    assert_ne!(name, "stale");
    assert_eq!(dirty, 1);

    // Newer remote version: applied, and not echoed back.
    let n = apply_with_fk_off(&pool, &[remote_task(&id, None, "fresh", local_updated + 5, 2)]).await;
    assert_eq!(n, 1);
    let (name, dirty, updated): (String, i64, i64) =
        sqlx::query_as("SELECT name, dirty, updated_at FROM tasks WHERE id=?").bind(&id).fetch_one(&pool).await.unwrap();
    assert_eq!((name.as_str(), dirty, updated), ("fresh", 0, local_updated + 5));
}

#[tokio::test]
async fn pull_tolerates_child_before_parent() {
    let pool = fresh_db().await;
    // A task whose folder hasn't arrived yet must not abort the pull.
    let n = apply_with_fk_off(&pool, &[remote_task("new-task", Some("folder-not-here-yet"), "t", 10, 1)]).await;
    assert_eq!(n, 1);
    // And foreign keys are back on for normal writes afterwards.
    let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys").fetch_one(&pool).await.unwrap();
    assert_eq!(fk, 1);
}

#[test]
fn server_seq_is_never_sent() {
    let json = serde_json::to_value(remote_task("t", None, "n", 1, 42)).unwrap();
    assert!(json.get("server_seq").is_none());
    assert!(json.get("user_id").is_some());
}
