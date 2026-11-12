#![allow(missing_docs)]

//! Attempt throttling: persisted in SQLite (auth_attempts table), survives
//! process restart. Cleared on successful unlock.
//!
//! Policy:
//!   5 failures  Ã¢â€ â€™ 30s lockout
//!   10 failures Ã¢â€ â€™ 5min lockout
//!   15 failures Ã¢â€ â€™ 1h lockout
//!   20+ failures Ã¢â€ â€™ 24h lockout

use crate::database::Db;
use rusqlite::OptionalExtension;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Lockout durations in seconds, keyed by failure count thresholds.
const LOCKOUT_PIN: i64 = 30; // 5 failures
const LOCKOUT_PASSWORD: i64 = 300; // 10 failures (5 min)
const LOCKOUT_RECOVERY: i64 = 3600; // 15 failures (1 hour)
const LOCKOUT_MAX: i64 = 86400; // 20+ failures (24 hours)

/// Thresholds: (failed_count, lockout_seconds)
const THRESHOLDS: [(u64, i64); 4] = [
    (5, LOCKOUT_PIN),
    (10, LOCKOUT_PASSWORD),
    (15, LOCKOUT_RECOVERY),
    (20, LOCKOUT_MAX),
];

/// Check if the vault is currently locked out. Returns Some(Instant) when
/// the lockout expires, or None if no lockout is active.
pub fn check_locked(db: &Db, vault_id: u64) -> Option<Instant> {
    let row: Option<i64> = db
        .conn()
        .query_row(
            "SELECT locked_until_unix FROM auth_attempts WHERE vault_id = ?1",
            rusqlite::params![vault_id as i64],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .ok()?;

    let locked_until = row?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    if now < locked_until {
        let remaining = locked_until - now;
        Some(Instant::now() + Duration::from_secs(remaining as u64))
    } else {
        None
    }
}

/// Record a failed attempt: increment failed_count, set locked_until_unix
/// based on the new count.
pub fn record_failure(db: &Db, vault_id: u64) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Get current count
    let current: Option<i64> = db
        .conn()
        .query_row(
            "SELECT failed_count FROM auth_attempts WHERE vault_id = ?1",
            rusqlite::params![vault_id as i64],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .ok()
        .flatten();

    let new_count = current.map_or(1, |c| c + 1);
    // Highest matching tier wins (5→30s, 10→5min, 15→1h, 20+→24h).
    // Below the first threshold: record the attempt, lock nothing —
    // locked_until = now, which check_locked treats as already expired.
    let lockout_seconds = THRESHOLDS
        .iter()
        .rev()
        .find(|(threshold, _)| new_count >= (*threshold as i64))
        .map(|(_, secs)| *secs)
        .unwrap_or(0);

    let locked_until = now + lockout_seconds;

    let _ = db.conn().execute(
        "INSERT INTO auth_attempts (vault_id, failed_count, locked_until_unix, last_attempt_unix)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(vault_id) DO UPDATE SET
           failed_count = ?2,
           locked_until_unix = ?3,
           last_attempt_unix = ?4",
        rusqlite::params![vault_id as i64, new_count, locked_until, now],
    );
}

/// Clear attempts for a vault (called on successful unlock).
pub fn clear_attempts(db: &Db, vault_id: u64) {
    let _ = db.conn().execute(
        "DELETE FROM auth_attempts WHERE vault_id = ?1",
        rusqlite::params![vault_id as i64],
    );
}

/// Get the current failed count for a vault (0 if no row exists).
pub fn failed_count(db: &Db, vault_id: u64) -> u64 {
    db.conn()
        .query_row(
            "SELECT failed_count FROM auth_attempts WHERE vault_id = ?1",
            rusqlite::params![vault_id as i64],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0) as u64
}
