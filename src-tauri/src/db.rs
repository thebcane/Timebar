//! SQLite source of truth: pool setup, migrations, and first-run seed data.

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

/// Shared handle placed in Tauri managed state.
pub struct Db {
    pub pool: SqlitePool,
}

/// Current UTC time as epoch milliseconds — the one clock the app trusts.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl Db {
    /// Open (creating if needed) the database at `db_path`, run migrations,
    /// and seed sample data on first run.
    pub async fn init(db_path: &Path) -> Result<Self, sqlx::Error> {
        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
            .create_if_missing(true)
            .foreign_keys(true)
            // WAL lets the 1s tray reader run concurrently with command writers
            // instead of contending on the single rollback-journal lock.
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        let db = Db { pool };
        db.seed_if_empty().await?;
        db.ensure_settings().await?;
        Ok(db)
    }

    /// Seed default settings rows if absent (idempotent).
    async fn ensure_settings(&self) -> Result<(), sqlx::Error> {
        let defaults = [
            ("first_day_of_week", "1"),
            ("default_currency", "USD"),
            ("theme", "dark"),
        ];
        for (k, v) in defaults {
            sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES (?, ?)")
                .bind(k)
                .bind(v)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    async fn seed_if_empty(&self) -> Result<(), sqlx::Error> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks")
            .fetch_one(&self.pool)
            .await?;
        if count > 0 {
            return Ok(());
        }

        let now = now_ms();
        let id = || uuid::Uuid::new_v4().to_string();

        // Folders ---------------------------------------------------------
        let client = id();
        let personal = id();
        for (fid, name, color) in [
            (&client, "Client Work", "#3B82F6"),
            (&personal, "Personal", "#22C55E"),
        ] {
            sqlx::query(
                "INSERT INTO folders (id, name, color, parent_id, sort_order, archived, created_at, updated_at)
                 VALUES (?, ?, ?, NULL, 0, 0, ?, ?)",
            )
            .bind(fid)
            .bind(name)
            .bind(color)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }

        // Tasks -----------------------------------------------------------
        // (name, color, folder, hourly_rate, billable)
        let task_defs: [(&str, &str, Option<&String>, f64, bool); 6] = [
            ("Acme redesign", "#38BDF8", Some(&client), 120.0, true),
            ("Bug triage", "#F59E0B", Some(&client), 90.0, true),
            ("Internal tooling", "#8B5CF6", Some(&client), 100.0, true),
            ("Learning Rust", "#22C55E", Some(&personal), 0.0, false),
            ("Admin & email", "#FB923C", None, 0.0, false),
            ("Side project", "#EC4899", Some(&personal), 0.0, false),
        ];
        let mut task_ids = Vec::new();
        for (i, (name, color, folder, rate, billable)) in task_defs.iter().enumerate() {
            let tid = id();
            sqlx::query(
                "INSERT INTO tasks (id, name, color, folder_id, hourly_rate, currency, billable, sort_order, archived, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, 'USD', ?, ?, 0, ?, ?)",
            )
            .bind(&tid)
            .bind(*name)
            .bind(*color)
            .bind(folder.map(|s| s.as_str()))
            .bind(rate)
            .bind(*billable)
            .bind(i as i64)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
            task_ids.push((tid, *rate));
        }

        // Sessions over the past several days ------------------------------
        const HOUR: i64 = 3_600_000;
        const DAY: i64 = 24 * HOUR;
        // (day_offset_back, task_index, start_hour_local, duration_minutes, note)
        let session_defs: [(i64, usize, i64, i64, Option<&str>); 12] = [
            (6, 0, 9, 95, Some("Wireframes")),
            (6, 4, 11, 30, None),
            (5, 1, 10, 50, Some("Prod incident")),
            (5, 0, 14, 120, Some("Hi-fi mockups")),
            (4, 2, 9, 75, Some("CI pipeline")),
            (4, 3, 20, 60, Some("Ownership chapter")),
            (3, 0, 10, 140, None),
            (3, 5, 21, 45, Some("Prototype")),
            (2, 1, 13, 80, Some("Triage backlog")),
            (2, 4, 16, 25, None),
            (1, 2, 9, 110, Some("Refactor build")),
            (1, 0, 15, 65, Some("Design review")),
        ];
        // Anchor to local midnight of today so seed data lands on real days.
        for (back, ti, hour, mins, note) in session_defs {
            let (tid, rate) = &task_ids[ti];
            let day_start = now - (now % DAY); // rough UTC midnight; fine for dev seed
            let start = day_start - back * DAY + hour * HOUR;
            let end = start + mins * 60_000;
            let billable_rate = *rate;
            sqlx::query(
                "INSERT INTO sessions (id, task_id, started_at, ended_at, note, rate_snapshot, billed, source, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, 0, 'timer', ?, ?)",
            )
            .bind(id())
            .bind(tid)
            .bind(start)
            .bind(end)
            .bind(note)
            .bind(billable_rate)
            .bind(start)
            .bind(end)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }
}
