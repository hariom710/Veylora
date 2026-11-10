//! Phase 9 — Layer 1 app login: email + password accounts in a separate
//! app-level SQLite database. Deliberately isolated from the vault: it never
//! reads vault files, never sees the MVK, and logging out does not lock the
//! vault (locking the vault does not log you out).

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension};
use subtle::ConstantTimeEq;

use crate::crypto::kdf::derive_password_kek;

/// Seeded on first run with role `admin`. The UI nudges changing it.
pub const ADMIN_EMAIL: &str = "b1865741@gmail.com";
pub const ADMIN_DEFAULT_PASSWORD: &str = "Harry@12345";
/// Default admin PIN (4–6 digits). Changed from vault PIN on first-run.
pub const ADMIN_DEFAULT_PIN: &str = "1729";

const MIN_PASSWORD_LEN: usize = 8;

/// Login state reported to the UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppStatus {
    pub logged_in: bool,
    pub email: Option<String>,
    pub role: Option<String>,
    pub logged_in_since: Option<u64>,
    pub used_default_password: bool,
}

/// One row of the admin user table.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppUser {
    pub id: u64,
    pub email: String,
    pub role: String,
    pub created_at: u64,
    pub last_login: Option<u64>,
}

#[derive(Clone)]
struct AppSession {
    email: String,
    role: String,
    logged_in_since: u64,
    used_default_password: bool,
}

/// Layer-1 session manager. One instance, owned by Tauri as managed state.
pub struct AppAuth {
    db_path: PathBuf,
    session: Mutex<Option<AppSession>>,
}

impl AppAuth {
    /// Production path: `%APPDATA%\com.veylora.app\app.db`. Layer 1 lives
    /// outside any vault, so it must exist before a vault is chosen.
    pub fn new() -> Result<Self, String> {
        let base = std::env::var("APPDATA").map_err(|_| "APPDATA not set".to_string())?;
        let dir = PathBuf::from(base).join("com.veylora.app");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(Self::with_db_path(dir.join("app.db")))
    }

    /// Explicit database location (tests use a temp directory).
    pub fn with_db_path(db_path: PathBuf) -> Self {
        Self {
            db_path,
            session: Mutex::new(None),
        }
    }

    fn open(&self) -> Result<Connection, String> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(&self.db_path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL CHECK (role IN ('admin','user')),
                created_at INTEGER NOT NULL,
                last_login INTEGER
            );",
        )
        .map_err(|e| e.to_string())?;
        self.seed_admin(&conn)?;
        Ok(conn)
    }

    fn seed_admin(&self, conn: &Connection) -> Result<(), String> {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM app_users", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if count == 0 {
            let hash = hash_password(ADMIN_DEFAULT_PASSWORD)?;
            conn.execute(
                "INSERT INTO app_users (email, password_hash, role, created_at)
                 VALUES (?1, ?2, 'admin', ?3)",
                rusqlite::params![ADMIN_EMAIL, hash, now_unix() as i64],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn status(&self) -> AppStatus {
        match self.session.lock().unwrap().clone() {
            Some(s) => AppStatus {
                logged_in: true,
                email: Some(s.email),
                role: Some(s.role),
                logged_in_since: Some(s.logged_in_since),
                used_default_password: s.used_default_password,
            },
            None => AppStatus {
                logged_in: false,
                email: None,
                role: None,
                logged_in_since: None,
                used_default_password: false,
            },
        }
    }

    pub fn login(&self, email: &str, password: &str) -> Result<AppStatus, String> {
        let conn = self.open()?;
        let row: Option<(String, String)> = conn
            .query_row(
                "SELECT password_hash, role FROM app_users
                 WHERE email = ?1 COLLATE NOCASE",
                rusqlite::params![email],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        // Identical message for unknown email and wrong password: never
        // reveal which accounts exist.
        let (stored_hash, role) = row.ok_or_else(|| LOGIN_ERROR.to_string())?;
        if !verify_password(password, &stored_hash) {
            return Err(LOGIN_ERROR.to_string());
        }
        conn.execute(
            "UPDATE app_users SET last_login = ?2
             WHERE email = ?1 COLLATE NOCASE",
            rusqlite::params![email, now_unix() as i64],
        )
        .map_err(|e| e.to_string())?;

        let used_default =
            email.eq_ignore_ascii_case(ADMIN_EMAIL) && password == ADMIN_DEFAULT_PASSWORD;
        *self.session.lock().unwrap() = Some(AppSession {
            email: email.to_string(),
            role,
            logged_in_since: now_unix(),
            used_default_password: used_default,
        });
        Ok(self.status())
    }

    /// Signup always creates role `user` (admin accounts come from an
    /// existing admin session via `create_user`).
    pub fn signup(&self, email: &str, password: &str) -> Result<(), String> {
        let conn = self.open()?;
        self.insert_user(&conn, email, password, "user")
    }

    /// Admin-only user creation from Account → Users.
    pub fn create_user(&self, email: &str, password: &str, role: &str) -> Result<(), String> {
        self.require_admin()?;
        if role != "admin" && role != "user" {
            return Err("Invalid role".to_string());
        }
        let conn = self.open()?;
        self.insert_user(&conn, email, password, role)
    }

    fn insert_user(
        &self,
        conn: &Connection,
        email: &str,
        password: &str,
        role: &str,
    ) -> Result<(), String> {
        if !email.contains('@') || email.len() < 3 {
            return Err("Enter a valid email address".to_string());
        }
        if password.len() < MIN_PASSWORD_LEN {
            return Err(format!(
                "Password must be at least {} characters",
                MIN_PASSWORD_LEN
            ));
        }
        let dup: Option<i64> = conn
            .query_row(
                "SELECT id FROM app_users WHERE email = ?1 COLLATE NOCASE",
                rusqlite::params![email],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if dup.is_some() {
            return Err("Email already in use".to_string());
        }
        let hash = hash_password(password)?;
        conn.execute(
            "INSERT INTO app_users (email, password_hash, role, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![email, hash, role, now_unix() as i64],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn logout(&self) {
        *self.session.lock().unwrap() = None;
    }

    pub fn change_password(&self, old_password: &str, new_password: &str) -> Result<(), String> {
        let session = self.require_session()?;
        if new_password.len() < MIN_PASSWORD_LEN {
            return Err(format!(
                "Password must be at least {} characters",
                MIN_PASSWORD_LEN
            ));
        }
        let conn = self.open()?;
        let stored: Option<String> = conn
            .query_row(
                "SELECT password_hash FROM app_users WHERE email = ?1",
                rusqlite::params![session.email],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let stored = stored.ok_or_else(|| "Invalid password".to_string())?;
        if !verify_password(old_password, &stored) {
            return Err("Invalid password".to_string());
        }
        let hash = hash_password(new_password)?;
        conn.execute(
            "UPDATE app_users SET password_hash = ?2 WHERE email = ?1",
            rusqlite::params![session.email, hash],
        )
        .map_err(|e| e.to_string())?;
        // The default-password banner clears once it is actually changed.
        if let Some(s) = self.session.lock().unwrap().as_mut() {
            s.used_default_password = false;
        }
        Ok(())
    }

    pub fn list_users(&self) -> Result<Vec<AppUser>, String> {
        self.require_admin()?;
        let conn = self.open()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, email, role, created_at, last_login
                 FROM app_users ORDER BY id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(AppUser {
                    id: r.get::<_, i64>(0)? as u64,
                    email: r.get(1)?,
                    role: r.get(2)?,
                    created_at: r.get::<_, i64>(3)? as u64,
                    last_login: r.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn delete_user(&self, id: u64) -> Result<(), String> {
        let session = self.require_admin()?;
        let conn = self.open()?;
        let target: Option<(String, String)> = conn
            .query_row(
                "SELECT email, role FROM app_users WHERE id = ?1",
                rusqlite::params![id as i64],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let (email, role) = target.ok_or_else(|| "User not found".to_string())?;
        // Last-admin guard first: it is the more important invariant, and it
        // must hold even for the admin's own row.
        if role == "admin" {
            let admins: i64 = conn
                .query_row("SELECT COUNT(*) FROM app_users WHERE role = 'admin'", [], |r| {
                    r.get(0)
                })
                .map_err(|e| e.to_string())?;
            if admins <= 1 {
                return Err("Cannot delete the last admin".to_string());
            }
        }
        if email == session.email {
            return Err("Cannot delete your own account".to_string());
        }
        conn.execute(
            "DELETE FROM app_users WHERE id = ?1",
            rusqlite::params![id as i64],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn require_session(&self) -> Result<AppSession, String> {
        self.session
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| "Not logged in".to_string())
    }

    fn require_admin(&self) -> Result<AppSession, String> {
        let session = self.require_session()?;
        if session.role != "admin" {
            return Err("Admin only".to_string());
        }
        Ok(session)
    }
}

const LOGIN_ERROR: &str = "Invalid email or password";

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Argon2id (same parameters as the vault password path) with a fresh
/// 32-byte salt, stored as `salt_hex:key_hex`.
fn hash_password(password: &str) -> Result<String, String> {
    let mut salt = [0u8; 32];
    getrandom::getrandom(&mut salt).map_err(|e| e.to_string())?;
    let key = derive_password_kek(password.as_bytes(), &salt).map_err(|e| e.to_string())?;
    Ok(format!("{}:{}", hex_encode(&salt), hex_encode(key.as_bytes())))
}

fn verify_password(password: &str, stored: &str) -> bool {
    let Some((salt_hex, key_hex)) = stored.split_once(':') else {
        return false;
    };
    let (Some(salt), Some(expected)) = (decode32(salt_hex), decode32(key_hex)) else {
        return false;
    };
    match derive_password_kek(password.as_bytes(), &salt) {
        Ok(key) => bool::from(key.as_bytes().ct_eq(&expected)),
        Err(_) => false,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn decode32(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).ok()?;
        out[i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(out)
}
