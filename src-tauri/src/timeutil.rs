//! Local-day bucketing and midnight-splitting (§4). All time math that needs
//! a timezone lives here so it is done once and identically everywhere.

use chrono::{DateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;

/// Parse an IANA tz string (e.g. "America/New_York"); fall back to UTC.
pub fn resolve_tz(tz: &str) -> Tz {
    tz.parse().unwrap_or(chrono_tz::UTC)
}

/// One piece of a session that falls within a single local calendar day.
#[derive(Debug, Clone)]
pub struct DaySeg {
    /// `YYYY-MM-DD` in local time.
    pub date: String,
    /// Local midnight of `date`, epoch ms.
    pub day_start_ms: i64,
    /// Clamped interval, epoch ms.
    pub start_ms: i64,
    pub end_ms: i64,
}

#[allow(dead_code)] // start_min/end_min/duration_ms are convenience accessors
impl DaySeg {
    pub fn duration_ms(&self) -> i64 {
        (self.end_ms - self.start_ms).max(0)
    }
    /// Minutes from local midnight — for calendar positioning.
    pub fn start_min(&self) -> i32 {
        ((self.start_ms - self.day_start_ms) / 60_000) as i32
    }
    pub fn end_min(&self) -> i32 {
        ((self.end_ms - self.day_start_ms) / 60_000) as i32
    }
}

fn local_midnight_ms(date: chrono::NaiveDate, tz: Tz) -> i64 {
    let naive = date.and_hms_opt(0, 0, 0).unwrap();
    tz.from_local_datetime(&naive)
        .earliest()
        .map(|dt| dt.timestamp_millis())
        // DST "spring forward" can skip local midnight; nudge forward an hour.
        .unwrap_or_else(|| {
            let alt = date.and_hms_opt(1, 0, 0).unwrap();
            tz.from_local_datetime(&alt)
                .earliest()
                .map(|dt| dt.timestamp_millis())
                .unwrap_or(0)
        })
}

/// Split `[start_ms, end_ms)` into per-local-day segments.
pub fn split_by_local_day(start_ms: i64, end_ms: i64, tz: Tz) -> Vec<DaySeg> {
    let mut out = Vec::new();
    if end_ms <= start_ms {
        return out;
    }
    let mut cur = start_ms;
    // Guard against pathological loops (e.g. bad tz math).
    let mut guard = 0;
    while cur < end_ms && guard < 4000 {
        guard += 1;
        let local = DateTime::<Utc>::from_timestamp_millis(cur)
            .unwrap_or_else(Utc::now)
            .with_timezone(&tz);
        let date = local.date_naive();
        let day_start_ms = local_midnight_ms(date, tz);
        let next_midnight_ms = local_midnight_ms(date.succ_opt().unwrap(), tz);
        let seg_end = end_ms.min(next_midnight_ms);
        out.push(DaySeg {
            date: date.format("%Y-%m-%d").to_string(),
            day_start_ms,
            start_ms: cur,
            end_ms: seg_end,
        });
        if seg_end <= cur {
            break; // safety
        }
        cur = seg_end;
    }
    out
}

/// The local `YYYY-MM-DD` a given instant falls on.
#[allow(dead_code)]
pub fn local_date_string(ms: i64, tz: Tz) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .unwrap_or_else(Utc::now)
        .with_timezone(&tz)
        .format("%Y-%m-%d")
        .to_string()
}

/// Format an instant as local `YYYY-MM-DD HH:MM` (used by CSV export).
pub fn format_local(ms: i64, tz: Tz) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .unwrap_or_else(Utc::now)
        .with_timezone(&tz)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

/// Minutes from local midnight for an instant.
#[allow(dead_code)]
pub fn local_minutes(ms: i64, tz: Tz) -> i32 {
    let local = DateTime::<Utc>::from_timestamp_millis(ms)
        .unwrap_or_else(Utc::now)
        .with_timezone(&tz);
    (local.hour() * 60 + local.minute()) as i32
}
