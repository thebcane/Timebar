//! Supabase sync (Section 12). Local SQLite stays the offline source of truth;
//! this pushes locally-changed rows up and pulls remote changes down,
//! reconciling by `updated_at` (last-write-wins). Auth is email+password.
//!
//! Sync v2 (needs supabase/migrations/20261118_sync_v2.sql on the server):
//! - Push: rows with `dirty = 1` (set by every local write), not "rows newer
//!   than a timestamp", so a device whose clock is behind still pushes.
//! - Pull: rows with a server-assigned `server_seq` above our cursor, paged,
//!   so we never miss a row because another device's clock was off. Plus a
//!   re-check of rows the server changed in the last 2 minutes, for pushes
//!   that committed out of order.
//! - The server ignores pushes older than what it has, so re-pushing is safe.
//! - After pulling, at most one session is left running (newest start wins),
//!   matching the server trigger.

use crate::db::{now_ms, Db};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::{Sqlite, SqlitePool};
use std::sync::LazyLock;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{Mutex, Notify};

const SUPABASE_URL: &str = "https://dekqenjmihzorckyxblr.supabase.co";
const ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImRla3FlbmptaWh6b3Jja3l4YmxyIiwicm9sZSI6ImFub24iLCJpYXQiOjE3ODM5ODI4MzAsImV4cCI6MjA5OTU1ODgzMH0.GTIgsHKCPrX3wv8X7UTHMeJAE1u-HefJ_UZJJQooVT8";

/// Rows per push request and per pull page.
const PAGE: usize = 500;
/// Background sync interval. Local writes also trigger a sync right away.
const POLL_EVERY: Duration = Duration::from_secs(4);
/// How long to wait after a local write before syncing, so a burst of
/// writes (drag-reorder, start-after-stop) goes up as one cycle.
const NUDGE_DEBOUNCE: Duration = Duration::from_millis(300);

type SyncResult<T> = Result<T, String>;

/// One sync cycle at a time (background loop vs. the Sync now button).
static SYNC_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
/// Wakes the background loop early. `notify_one` keeps a permit if the loop
/// is mid-sync, so a write during a sync still gets its own cycle.
static NUDGE: LazyLock<Notify> = LazyLock::new(Notify::new);

/// Ask for a sync soon. Call after any local write. Cheap; does nothing
/// when signed out.
pub fn nudge() {
    NUDGE.notify_one();
}

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

// --- Seed data -------------------------------------------------------------

/// Drop the first-launch sample data if it was never touched, so signing in
/// on a new device doesn't upload "Acme redesign" and friends to every other
/// device. Runs once, at the first sign-in after a fresh install; anything you
/// edited, or tracked time against, is kept.
pub async fn discard_unused_seed(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let Some(raw) = get_setting(pool, "seed_manifest").await else {
        return Ok(());
    };
    let Ok(m) = serde_json::from_str::<serde_json::Value>(&raw) else {
        del_setting(pool, "seed_manifest").await;
        return Ok(());
    };
    let seeded_at = m["seeded_at"].as_i64().unwrap_or(0);
    let ids = |k: &str| -> Vec<String> {
        m[k].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    };

    let mut tx = pool.begin().await?;
    // Every local edit moves updated_at past seeded_at, so `<= seeded_at`
    // means "exactly as seeded".
    for id in ids("sessions") {
        sqlx::query("DELETE FROM sessions WHERE id=? AND updated_at<=?")
            .bind(&id).bind(seeded_at).execute(&mut *tx).await?;
    }
    for id in ids("tasks") {
        sqlx::query(
            "DELETE FROM tasks WHERE id=? AND updated_at<=?
               AND NOT EXISTS (SELECT 1 FROM sessions WHERE task_id=tasks.id)",
        )
        .bind(&id).bind(seeded_at).execute(&mut *tx).await?;
    }
    for id in ids("folders") {
        sqlx::query(
            "DELETE FROM folders WHERE id=? AND updated_at<=?
               AND NOT EXISTS (SELECT 1 FROM tasks WHERE folder_id=folders.id)
               AND NOT EXISTS (SELECT 1 FROM folders c WHERE c.parent_id=folders.id)",
        )
        .bind(&id).bind(seeded_at).execute(&mut *tx).await?;
    }
    sqlx::query("DELETE FROM settings WHERE key='seed_manifest'")
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

// --- Running timer ---------------------------------------------------------

/// Leave at most one session running: the one that started last. Every other
/// running session ends when the winner started (never with zero length).
/// Same rule as the `sessions_single_running` trigger on the server; doing it
/// here too covers the moments before the server has weighed in (offline, or
/// two devices starting timers between syncs). Returns how many it closed.
pub async fn reconcile_running(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let running: Vec<(String, i64)> = sqlx::query_as(
        "SELECT id, started_at FROM sessions
         WHERE ended_at IS NULL AND deleted_at IS NULL
         ORDER BY started_at DESC, id DESC",
    )
    .fetch_all(pool)
    .await?;

    let mut closed = 0;
    if let Some((winner_id, winner_start)) = running.first() {
        let now = now_ms();
        for (id, _) in running.iter().skip(1) {
            let res = sqlx::query(
                "UPDATE sessions SET ended_at=MAX(?, started_at + 1), updated_at=?, dirty=1
                 WHERE id=? AND ended_at IS NULL",
            )
            .bind(winner_start)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
            closed += res.rows_affected() as i64;
        }
        set_setting(pool, "active_session_id", winner_id).await;
    } else {
        del_setting(pool, "active_session_id").await;
    }
    Ok(closed)
}

// --- Row DTOs (snake_case JSON == Postgres columns) ------------------------

type Query<'q> = sqlx::query::Query<'q, Sqlite, SqliteArguments<'q>>;

/// What the generic push/pull needs to know about each synced table.
trait SyncRow:
    Serialize + for<'de> Deserialize<'de> + for<'r> sqlx::FromRow<'r, SqliteRow> + Send + Unpin
{
    const TABLE: &'static str;
    /// Local columns, in the order `bind_all` binds them.
    const COLS: &'static str;
    fn id(&self) -> &str;
    fn updated_at(&self) -> i64;
    fn server_seq(&self) -> i64;
    fn set_user(&mut self, uid: &str);
    fn bind_all<'q>(&'q self, q: Query<'q>) -> Query<'q>;
}

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
    /// Server-owned; read on pull, never sent.
    #[sqlx(default)]
    #[serde(default, skip_serializing)]
    server_seq: i64,
}

impl SyncRow for SyncFolder {
    const TABLE: &'static str = "folders";
    const COLS: &'static str =
        "id,name,color,parent_id,sort_order,archived,created_at,updated_at,deleted_at";
    fn id(&self) -> &str { &self.id }
    fn updated_at(&self) -> i64 { self.updated_at }
    fn server_seq(&self) -> i64 { self.server_seq }
    fn set_user(&mut self, uid: &str) { self.user_id = Some(uid.to_string()); }
    fn bind_all<'q>(&'q self, q: Query<'q>) -> Query<'q> {
        q.bind(&self.id).bind(&self.name).bind(&self.color).bind(&self.parent_id)
            .bind(self.sort_order).bind(self.archived).bind(self.created_at)
            .bind(self.updated_at).bind(self.deleted_at)
    }
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
    #[sqlx(default)]
    #[serde(default, skip_serializing)]
    server_seq: i64,
}

impl SyncRow for SyncTask {
    const TABLE: &'static str = "tasks";
    const COLS: &'static str = "id,name,color,folder_id,hourly_rate,currency,billable,sort_order,archived,created_at,updated_at,deleted_at";
    fn id(&self) -> &str { &self.id }
    fn updated_at(&self) -> i64 { self.updated_at }
    fn server_seq(&self) -> i64 { self.server_seq }
    fn set_user(&mut self, uid: &str) { self.user_id = Some(uid.to_string()); }
    fn bind_all<'q>(&'q self, q: Query<'q>) -> Query<'q> {
        q.bind(&self.id).bind(&self.name).bind(&self.color).bind(&self.folder_id)
            .bind(self.hourly_rate).bind(&self.currency).bind(self.billable)
            .bind(self.sort_order).bind(self.archived).bind(self.created_at)
            .bind(self.updated_at).bind(self.deleted_at)
    }
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
    #[sqlx(default)]
    #[serde(default, skip_serializing)]
    server_seq: i64,
}

impl SyncRow for SyncSession {
    const TABLE: &'static str = "sessions";
    const COLS: &'static str = "id,task_id,started_at,ended_at,note,rate_snapshot,billed,source,created_at,updated_at,deleted_at";
    fn id(&self) -> &str { &self.id }
    fn updated_at(&self) -> i64 { self.updated_at }
    fn server_seq(&self) -> i64 { self.server_seq }
    fn set_user(&mut self, uid: &str) { self.user_id = Some(uid.to_string()); }
    fn bind_all<'q>(&'q self, q: Query<'q>) -> Query<'q> {
        q.bind(&self.id).bind(&self.task_id).bind(self.started_at).bind(self.ended_at)
            .bind(&self.note).bind(self.rate_snapshot).bind(self.billed).bind(&self.source)
            .bind(self.created_at).bind(self.updated_at).bind(self.deleted_at)
    }
}

// --- HTTP ------------------------------------------------------------------

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

/// GET `{resource}?{query}` from PostgREST and parse the rows.
async fn http_get<T: for<'de> Deserialize<'de>>(
    token: &str,
    resource: &str,
    query: &str,
) -> SyncResult<Vec<T>> {
    let resp = client()
        .get(format!("{SUPABASE_URL}/rest/v1/{resource}?{query}"))
        .header("apikey", ANON_KEY)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("pull {resource}: network error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        if text.contains("server_seq") || text.contains("_recent") {
            return Err(format!(
                "pull {resource} failed: the server needs supabase/migrations/20261118_sync_v2.sql ({status})"
            ));
        }
        return Err(format!("pull {resource} failed ({status}): {text}"));
    }
    serde_json::from_str::<Vec<T>>(&text).map_err(|e| format!("pull {resource} parse: {e}"))
}

// --- Generic push / pull ---------------------------------------------------

/// Push every dirty row, a page at a time, and clear `dirty` on the rows that
/// haven't changed again while the request was in flight.
async fn push_table<T: SyncRow>(pool: &SqlitePool, token: &str, uid: &str) -> SyncResult<()> {
    let select = format!(
        "SELECT {} FROM {} WHERE dirty=1 ORDER BY updated_at ASC LIMIT {PAGE}",
        T::COLS,
        T::TABLE
    );
    let clear = format!("UPDATE {} SET dirty=0 WHERE id=? AND updated_at=? AND dirty=1", T::TABLE);
    // Bounded so a row that keeps changing can't spin this forever.
    for _ in 0..100 {
        let mut rows: Vec<T> = sqlx::query_as(&select)
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
        if rows.is_empty() {
            break;
        }
        for r in rows.iter_mut() {
            r.set_user(uid);
        }
        http_push(token, T::TABLE, &rows).await?;

        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
        let mut cleared = 0;
        for r in &rows {
            let res = sqlx::query(&clear)
                .bind(r.id())
                .bind(r.updated_at())
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            cleared += res.rows_affected();
        }
        tx.commit().await.map_err(|e| e.to_string())?;

        if rows.len() < PAGE || cleared == 0 {
            break;
        }
    }
    Ok(())
}

/// Apply pulled rows locally: insert, or overwrite if the pulled row is newer.
/// Returns how many local rows changed.
async fn apply_rows<T: SyncRow>(conn: &mut sqlx::SqliteConnection, rows: &[T]) -> Result<i64, sqlx::Error> {
    use sqlx::Connection;
    let cols: Vec<&str> = T::COLS.split(',').collect();
    let placeholders = vec!["?"; cols.len()].join(",");
    let updates = cols
        .iter()
        .filter(|c| **c != "id")
        .map(|c| format!("{c}=excluded.{c}"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "INSERT INTO {t} ({cols},dirty) VALUES ({placeholders},0)
         ON CONFLICT(id) DO UPDATE SET {updates},dirty=0
         WHERE excluded.updated_at > {t}.updated_at",
        t = T::TABLE,
        cols = T::COLS,
    );
    let mut tx = conn.begin().await?;
    let mut changed = 0;
    for r in rows {
        let res = r.bind_all(sqlx::query(&sql)).execute(&mut *tx).await?;
        changed += res.rows_affected() as i64;
    }
    tx.commit().await?;
    Ok(changed)
}

/// Pull every row the server has changed since our cursor (all pages), apply
/// them, then advance the cursor. Returns how many local rows changed.
async fn pull_table<T: SyncRow>(pool: &SqlitePool, token: &str) -> SyncResult<i64> {
    let key = format!("sync_seq_{}", T::TABLE);
    let start = get_cursor(pool, &key).await;
    let mut cursor = start;
    let mut rows: Vec<T> = Vec::new();
    loop {
        let page: Vec<T> = http_get(
            token,
            T::TABLE,
            &format!("select=*&server_seq=gt.{cursor}&order=server_seq.asc&limit={PAGE}"),
        )
        .await?;
        let n = page.len();
        if let Some(last) = page.last() {
            cursor = last.server_seq();
        }
        rows.extend(page);
        if n < PAGE {
            break;
        }
    }
    // Re-check rows changed in the last couple of minutes at or below the
    // cursor: a row whose push committed a moment after a later-numbered one
    // can land behind the cursor. Re-applying a row we already have is a no-op.
    if start > 0 {
        let recent: Vec<T> = http_get(
            token,
            &format!("{}_recent", T::TABLE),
            &format!("select=*&server_seq=lte.{start}&order=server_seq.asc"),
        )
        .await?;
        rows.extend(recent);
    }
    if rows.is_empty() {
        return Ok(0);
    }

    // Foreign keys off while applying: a page can hold a child before its
    // parent (a task before its folder, say). Every device enforces them on
    // its own writes, so what arrives is consistent once fully applied.
    let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    let result = apply_rows(&mut conn, &rows).await;
    if sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.is_err() {
        // Never hand a connection with FKs off back to the pool.
        conn.close_on_drop();
    }
    let changed = result.map_err(|e| format!("apply {}: {e}", T::TABLE))?;

    set_setting(pool, &key, &cursor.to_string()).await;
    Ok(changed)
}

/// One full sync cycle. Returns the number of local rows changed (pulled rows
/// plus sessions closed by the running-timer rule).
pub async fn sync_once(pool: &SqlitePool) -> SyncResult<i64> {
    let _guard = SYNC_LOCK.lock().await;
    let token = ensure_token(pool).await?;
    let uid = get_setting(pool, "sync_user_id").await.unwrap_or_default();

    push_table::<SyncFolder>(pool, &token, &uid).await?;
    push_table::<SyncTask>(pool, &token, &uid).await?;
    push_table::<SyncSession>(pool, &token, &uid).await?;

    let mut changed = 0;
    changed += pull_table::<SyncFolder>(pool, &token).await?;
    changed += pull_table::<SyncTask>(pool, &token).await?;
    changed += pull_table::<SyncSession>(pool, &token).await?;

    let closed = reconcile_running(pool).await.map_err(|e| e.to_string())?;
    if closed > 0 {
        // Push the closures now rather than on the next cycle.
        push_table::<SyncSession>(pool, &token, &uid).await?;
        changed += closed;
    }

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
        // Before storing the session: the background loop starts syncing the
        // moment a refresh token exists.
        discard_unused_seed(&db.pool).await.map_err(|e| e.to_string())?;
        store_session(&db.pool, &s, &email).await?;
        nudge();
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
    discard_unused_seed(&db.pool).await.map_err(|e| e.to_string())?;
    store_session(&db.pool, &s, &email).await?;
    nudge();
    Ok(())
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
        // v2 pull cursors: signing back in pulls everything again (harmless).
        "sync_seq_folders", "sync_seq_tasks", "sync_seq_sessions",
        // v1 cursors, no longer used.
        "sync_push_folders", "sync_push_tasks", "sync_push_sessions",
        "sync_pull_folders", "sync_pull_tasks", "sync_pull_sessions",
    ] {
        del_setting(&db.pool, key).await;
    }
    Ok(())
}

async fn after_sync(app: &AppHandle, pool: &SqlitePool, result: SyncResult<i64>) -> SyncResult<i64> {
    match result {
        Ok(changed) => {
            if changed > 0 {
                let _ = app.emit("data-changed", ());
                crate::commands::emit_active_changed(app, pool).await;
            }
            Ok(changed)
        }
        Err(e) => {
            set_setting(pool, "sync_error", &e).await;
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn sync_now(app: AppHandle, db: State<'_, Db>) -> Result<i64, String> {
    let result = sync_once(&db.pool).await;
    after_sync(&app, &db.pool, result).await
}

/// Background loop: sync every few seconds while signed in, and soon after
/// any local write (see `nudge`).
pub fn spawn_sync_loop(app: AppHandle) {
    let pool = app.state::<Db>().pool.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(POLL_EVERY);
        loop {
            tokio::select! {
                _ = interval.tick() => {}
                _ = NUDGE.notified() => {
                    tokio::time::sleep(NUDGE_DEBOUNCE).await;
                    interval.reset();
                }
            }
            let signed_in = get_setting(&pool, "sync_refresh_token").await.is_some();
            if !signed_in {
                continue;
            }
            let result = sync_once(&pool).await;
            let _ = after_sync(&app, &pool, result).await;
        }
    });
}

#[cfg(test)]
mod tests;
