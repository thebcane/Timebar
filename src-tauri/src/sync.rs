//! Supabase sync (Section 12). Local SQLite stays the offline source of truth;
//! this pushes locally-changed rows up and pulls remote changes down,
//! reconciling by `updated_at` (last-write-wins). Auth is email+password.

use crate::db::{now_ms, Db};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

const SUPABASE_URL: &str = "https://dekqenjmihzorckyxblr.supabase.co";
const ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImRla3FlbmptaWh6b3Jja3l4YmxyIiwicm9sZSI6ImFub24iLCJpYXQiOjE3ODM5ODI4MzAsImV4cCI6MjA5OTU1ODgzMH0.GTIgsHKCPrX3wv8X7UTHMeJAE1u-HefJ_UZJJQooVT8";

type SyncResult<T> = Result<T, String>;

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

// --- settings-backed key/value (cursors, tokens) ---------------------------

async fn get_setting(pool: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key=?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

async fn set_setting(pool: &SqlitePool, key: &str, value: &str) {
    let _ = sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await;
}

async fn del_setting(pool: &SqlitePool, key: &str) {
    let _ = sqlx::query("DELETE FROM settings WHERE key=?")
        .bind(key)
        .execute(pool)
        .await;
}

async fn get_cursor(pool: &SqlitePool, key: &str) -> i64 {
    get_setting(pool, key)
        .await
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

// --- Auth ------------------------------------------------------------------

#[derive(Deserialize)]
struct AuthUser {
    id: String,
}

#[derive(Deserialize)]
struct AuthSession {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_at: Option<i64>, // unix seconds
    user: Option<AuthUser>,
}

async fn store_session(pool: &SqlitePool, s: &AuthSession, email: &str) -> SyncResult<()> {
    let access = s
        .access_token
        .as_ref()
        .ok_or("no access token in response")?;
    let refresh = s
        .refresh_token
        .as_ref()
        .ok_or("no refresh token in response")?;
    let uid = s.user.as_ref().map(|u| u.id.clone()).unwrap_or_default();
    set_setting(pool, "sync_email", email).await;
    set_setting(pool, "sync_access_token", access).await;
    set_setting(pool, "sync_refresh_token", refresh).await;
    set_setting(pool, "sync_user_id", &uid).await;
    set_setting(pool, "sync_expires_at", &s.expires_at.unwrap_or(0).to_string()).await;
    Ok(())
}

async fn post_auth(path: &str, body: serde_json::Value) -> SyncResult<AuthSession> {
    let resp = client()
        .post(format!("{SUPABASE_URL}/auth/v1/{path}"))
        .header("apikey", ANON_KEY)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("auth failed ({status}): {text}"));
    }
    serde_json::from_str::<AuthSession>(&text).map_err(|e| format!("parse error: {e}"))
}

/// Refresh the access token if it is missing or within 60s of expiry.
async fn ensure_token(pool: &SqlitePool) -> SyncResult<String> {
    let access = get_setting(pool, "sync_access_token").await;
    let expires_at = get_cursor(pool, "sync_expires_at").await;
    let now_secs = now_ms() / 1000;
    if let Some(tok) = access {
        if expires_at == 0 || now_secs < expires_at - 60 {
            return Ok(tok);
        }
    }
    // Refresh.
    let refresh = get_setting(pool, "sync_refresh_token")
        .await
        .ok_or("not signed in")?;
    let s = post_auth(
        "token?grant_type=refresh_token",
        serde_json::json!({ "refresh_token": refresh }),
    )
    .await?;
    let email = get_setting(pool, "sync_email").await.unwrap_or_default();
    store_session(pool, &s, &email).await?;
    s.access_token.ok_or_else(|| "refresh returned no token".into())
}

// --- Row DTOs (snake_case JSON == Postgres columns) ------------------------

#[derive(Serialize, Deserialize, sqlx::FromRow)]
struct SyncFolder {
    id: String,
    name: String,
    color: String,
    parent_id: Option<String>,
    sort_order: i64,
    archived: bool,
    created_at: i64,
    updated_at: i64,
    deleted_at: Option<i64>,
    #[sqlx(default)]
    user_id: Option<String>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
struct SyncTask {
    id: String,
    name: String,
    color: String,
    folder_id: Option<String>,
    hourly_rate: f64,
    currency: String,
    billable: bool,
    sort_order: i64,
    archived: bool,
    created_at: i64,
    updated_at: i64,
    deleted_at: Option<i64>,
    #[sqlx(default)]
    user_id: Option<String>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
struct SyncSession {
    id: String,
    task_id: String,
    started_at: i64,
    ended_at: Option<i64>,
    note: Option<String>,
    rate_snapshot: f64,
    billed: bool,
    source: String,
    created_at: i64,
    updated_at: i64,
    deleted_at: Option<i64>,
    #[sqlx(default)]
    user_id: Option<String>,
}

// --- Generic HTTP push/pull ------------------------------------------------

async fn http_push<T: Serialize>(token: &str, table: &str, rows: &[T]) -> SyncResult<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let resp = client()
        .post(format!("{SUPABASE_URL}/rest/v1/{table}?on_conflict=id"))
        .header("apikey", ANON_KEY)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header("Prefer", "resolution=merge-duplicates,return=minimal")
        .json(rows)
        .send()
        .await
        .map_err(|e| format!("push {table}: network error: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("push {table} failed ({status}): {text}"));
    }
    Ok(())
}

async fn http_pull<T: for<'de> Deserialize<'de>>(
    token: &str,
    table: &str,
    since: i64,
) -> SyncResult<Vec<T>> {
    let resp = client()
        .get(format!(
            "{SUPABASE_URL}/rest/v1/{table}?select=*&updated_at=gt.{since}&order=updated_at.asc"
        ))
        .header("apikey", ANON_KEY)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("pull {table}: network error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("pull {table} failed ({status}): {text}"));
    }
    serde_json::from_str::<Vec<T>>(&text).map_err(|e| format!("pull {table} parse: {e}"))
}

// --- Per-table sync --------------------------------------------------------

/// Advance the push cursor past freshly-pulled rows so they aren't echoed back.
async fn advance_cursors(pool: &SqlitePool, table: &str, pulled_max: i64) {
    set_setting(pool, &format!("sync_pull_{table}"), &pulled_max.to_string()).await;
    let push_cur = get_cursor(pool, &format!("sync_push_{table}")).await;
    if pulled_max > push_cur {
        set_setting(pool, &format!("sync_push_{table}"), &pulled_max.to_string()).await;
    }
}

async fn sync_folders(pool: &SqlitePool, token: &str, uid: &str) -> SyncResult<i64> {
    let push_cur = get_cursor(pool, "sync_push_folders").await;
    let mut rows = sqlx::query_as::<_, SyncFolder>(
        "SELECT id,name,color,parent_id,sort_order,archived,created_at,updated_at,deleted_at FROM folders WHERE updated_at > ? ORDER BY updated_at ASC",
    )
    .bind(push_cur)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let mut pushed_max = push_cur;
    for r in rows.iter_mut() {
        r.user_id = Some(uid.to_string());
        pushed_max = pushed_max.max(r.updated_at);
    }
    http_push(token, "folders", &rows).await?;
    if pushed_max > push_cur {
        set_setting(pool, "sync_push_folders", &pushed_max.to_string()).await;
    }

    let pull_cur = get_cursor(pool, "sync_pull_folders").await;
    let remote: Vec<SyncFolder> = http_pull(token, "folders", pull_cur).await?;
    let mut maxu = pull_cur;
    let mut changed = 0i64;
    for r in &remote {
        maxu = maxu.max(r.updated_at);
        let res = sqlx::query(
            "INSERT INTO folders (id,name,color,parent_id,sort_order,archived,created_at,updated_at,deleted_at)
             VALUES (?,?,?,?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name,color=excluded.color,parent_id=excluded.parent_id,
               sort_order=excluded.sort_order,archived=excluded.archived,created_at=excluded.created_at,
               updated_at=excluded.updated_at,deleted_at=excluded.deleted_at
             WHERE excluded.updated_at > folders.updated_at",
        )
        .bind(&r.id).bind(&r.name).bind(&r.color).bind(&r.parent_id).bind(r.sort_order)
        .bind(r.archived).bind(r.created_at).bind(r.updated_at).bind(r.deleted_at)
        .execute(pool).await.map_err(|e| e.to_string())?;
        changed += res.rows_affected() as i64;
    }
    advance_cursors(pool, "folders", maxu).await;
    Ok(changed)
}

async fn sync_tasks(pool: &SqlitePool, token: &str, uid: &str) -> SyncResult<i64> {
    let push_cur = get_cursor(pool, "sync_push_tasks").await;
    let mut rows = sqlx::query_as::<_, SyncTask>(
        "SELECT id,name,color,folder_id,hourly_rate,currency,billable,sort_order,archived,created_at,updated_at,deleted_at FROM tasks WHERE updated_at > ? ORDER BY updated_at ASC",
    )
    .bind(push_cur)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let mut pushed_max = push_cur;
    for r in rows.iter_mut() {
        r.user_id = Some(uid.to_string());
        pushed_max = pushed_max.max(r.updated_at);
    }
    http_push(token, "tasks", &rows).await?;
    if pushed_max > push_cur {
        set_setting(pool, "sync_push_tasks", &pushed_max.to_string()).await;
    }

    let pull_cur = get_cursor(pool, "sync_pull_tasks").await;
    let remote: Vec<SyncTask> = http_pull(token, "tasks", pull_cur).await?;
    let mut maxu = pull_cur;
    let mut changed = 0i64;
    for r in &remote {
        maxu = maxu.max(r.updated_at);
        let res = sqlx::query(
            "INSERT INTO tasks (id,name,color,folder_id,hourly_rate,currency,billable,sort_order,archived,created_at,updated_at,deleted_at)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name,color=excluded.color,folder_id=excluded.folder_id,
               hourly_rate=excluded.hourly_rate,currency=excluded.currency,billable=excluded.billable,
               sort_order=excluded.sort_order,archived=excluded.archived,created_at=excluded.created_at,
               updated_at=excluded.updated_at,deleted_at=excluded.deleted_at
             WHERE excluded.updated_at > tasks.updated_at",
        )
        .bind(&r.id).bind(&r.name).bind(&r.color).bind(&r.folder_id).bind(r.hourly_rate)
        .bind(&r.currency).bind(r.billable).bind(r.sort_order).bind(r.archived)
        .bind(r.created_at).bind(r.updated_at).bind(r.deleted_at)
        .execute(pool).await.map_err(|e| e.to_string())?;
        changed += res.rows_affected() as i64;
    }
    advance_cursors(pool, "tasks", maxu).await;
    Ok(changed)
}

async fn sync_sessions(pool: &SqlitePool, token: &str, uid: &str) -> SyncResult<i64> {
    let push_cur = get_cursor(pool, "sync_push_sessions").await;
    let mut rows = sqlx::query_as::<_, SyncSession>(
        "SELECT id,task_id,started_at,ended_at,note,rate_snapshot,billed,source,created_at,updated_at,deleted_at FROM sessions WHERE updated_at > ? ORDER BY updated_at ASC",
    )
    .bind(push_cur)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let mut pushed_max = push_cur;
    for r in rows.iter_mut() {
        r.user_id = Some(uid.to_string());
        pushed_max = pushed_max.max(r.updated_at);
    }
    http_push(token, "sessions", &rows).await?;
    if pushed_max > push_cur {
        set_setting(pool, "sync_push_sessions", &pushed_max.to_string()).await;
    }

    let pull_cur = get_cursor(pool, "sync_pull_sessions").await;
    let remote: Vec<SyncSession> = http_pull(token, "sessions", pull_cur).await?;
    let mut maxu = pull_cur;
    let mut changed = 0i64;
    for r in &remote {
        maxu = maxu.max(r.updated_at);
        let res = sqlx::query(
            "INSERT INTO sessions (id,task_id,started_at,ended_at,note,rate_snapshot,billed,source,created_at,updated_at,deleted_at)
             VALUES (?,?,?,?,?,?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET task_id=excluded.task_id,started_at=excluded.started_at,ended_at=excluded.ended_at,
               note=excluded.note,rate_snapshot=excluded.rate_snapshot,billed=excluded.billed,source=excluded.source,
               created_at=excluded.created_at,updated_at=excluded.updated_at,deleted_at=excluded.deleted_at
             WHERE excluded.updated_at > sessions.updated_at",
        )
        .bind(&r.id).bind(&r.task_id).bind(r.started_at).bind(r.ended_at).bind(&r.note)
        .bind(r.rate_snapshot).bind(r.billed).bind(&r.source).bind(r.created_at).bind(r.updated_at).bind(r.deleted_at)
        .execute(pool).await.map_err(|e| e.to_string())?;
        changed += res.rows_affected() as i64;
    }
    advance_cursors(pool, "sessions", maxu).await;
    Ok(changed)
}

/// One full sync cycle. Returns the number of local rows changed by the pull.
pub async fn sync_once(pool: &SqlitePool) -> SyncResult<i64> {
    let token = ensure_token(pool).await?;
    let uid = get_setting(pool, "sync_user_id").await.unwrap_or_default();
    // Order matters only for FK-ish readability; LWW converges regardless.
    let mut changed = 0;
    changed += sync_folders(pool, &token, &uid).await?;
    changed += sync_tasks(pool, &token, &uid).await?;
    changed += sync_sessions(pool, &token, &uid).await?;
    set_setting(pool, "sync_last", &now_ms().to_string()).await;
    del_setting(pool, "sync_error").await;
    Ok(changed)
}

// --- Commands --------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    signed_in: bool,
    email: Option<String>,
    last_sync: Option<i64>,
    last_error: Option<String>,
}

#[tauri::command]
pub async fn sync_status(db: State<'_, Db>) -> Result<SyncStatus, String> {
    let email = get_setting(&db.pool, "sync_email").await;
    let signed_in = get_setting(&db.pool, "sync_refresh_token").await.is_some();
    let last_sync = get_setting(&db.pool, "sync_last").await.and_then(|v| v.parse().ok());
    let last_error = get_setting(&db.pool, "sync_error").await;
    Ok(SyncStatus { signed_in, email: if signed_in { email } else { None }, last_sync, last_error })
}

#[tauri::command]
pub async fn sync_sign_up(db: State<'_, Db>, email: String, password: String) -> Result<String, String> {
    let s = post_auth("signup", serde_json::json!({ "email": email, "password": password })).await?;
    if s.access_token.is_some() {
        store_session(&db.pool, &s, &email).await?;
        Ok("signed-in".into())
    } else {
        // Email confirmation is on: user must confirm before signing in.
        Ok("confirm-email".into())
    }
}

#[tauri::command]
pub async fn sync_sign_in(db: State<'_, Db>, email: String, password: String) -> Result<(), String> {
    let s = post_auth(
        "token?grant_type=password",
        serde_json::json!({ "email": email, "password": password }),
    )
    .await?;
    store_session(&db.pool, &s, &email).await
}

/// Change the signed-in user's password (Supabase requires ≥ 6 chars).
#[tauri::command]
pub async fn sync_change_password(db: State<'_, Db>, password: String) -> Result<(), String> {
    let token = ensure_token(&db.pool).await?;
    let resp = client()
        .put(format!("{SUPABASE_URL}/auth/v1/user"))
        .header("apikey", ANON_KEY)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "password": password }))
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("couldn't change password ({status}): {text}"));
    }
    Ok(())
}

#[tauri::command]
pub async fn sync_sign_out(db: State<'_, Db>) -> Result<(), String> {
    for key in [
        "sync_email", "sync_access_token", "sync_refresh_token", "sync_user_id",
        "sync_expires_at", "sync_last", "sync_error",
    ] {
        del_setting(&db.pool, key).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn sync_now(app: AppHandle, db: State<'_, Db>) -> Result<i64, String> {
    match sync_once(&db.pool).await {
        Ok(changed) => {
            if changed > 0 {
                let _ = app.emit("data-changed", ());
                crate::commands::emit_active_changed(&app, &db.pool).await;
            }
            Ok(changed)
        }
        Err(e) => {
            set_setting(&db.pool, "sync_error", &e).await;
            Err(e)
        }
    }
}

/// Background loop: sync every few seconds while signed in.
pub fn spawn_sync_loop(app: AppHandle) {
    let pool = app.state::<Db>().pool.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(8));
        loop {
            interval.tick().await;
            let signed_in = get_setting(&pool, "sync_refresh_token").await.is_some();
            if !signed_in {
                continue;
            }
            match sync_once(&pool).await {
                Ok(changed) if changed > 0 => {
                    let _ = app.emit("data-changed", ());
                    crate::commands::emit_active_changed(&app, &pool).await;
                }
                Ok(_) => {}
                Err(e) => {
                    set_setting(&pool, "sync_error", &e).await;
                }
            }
        }
    });
}
