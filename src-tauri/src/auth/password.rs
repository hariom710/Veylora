//! Password unlock: derive Argon2id KEK from password + salt, unwrap MVK from vault.meta.

use crate::auth::AuthError;
use crate::auth::session::Session;
use crate::auth::UnlockMethod;
use crate::storage::vault_dir::VaultDir;
use crate::vault::VaultMeta;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Unlock with a password. Derives the Argon2id KEK from password + salt,
/// unwraps the MVK from vault.meta, and returns a Session holding the
/// zeroized MVK.
pub fn unlock_with_password(vault_root: &std::path::Path, password: &[u8]) -> Result<Session, AuthError> {
    let vdir = VaultDir::new(vault_root.to_path_buf());
    let meta_path = &vdir.meta_path;

    // Read vault.meta
    let meta_str = std::fs::read_to_string(meta_path)
        .map_err(|_| AuthError::VaultNotFound)?;
    let meta: VaultMeta = serde_json::from_str(&meta_str)
        .map_err(|_| AuthError::VaultCorrupted)?;

    let mvk = meta.unwrap_with_password(password).map_err(|_| AuthError::InvalidCredential)?;

    // Verify MVK length
    if mvk.as_bytes().len() != 32 {
        return Err(AuthError::InvalidCredential);
    }

    let vault_id = vault_id_from_meta(&meta);
    let session = Session::new(mvk, vault_id, UnlockMethod::Password);

    Ok(session)
}

/// Derive a deterministic vault_id from the serialized VaultMeta (used as a
/// stable identifier for throttling rows).
fn vault_id_from_meta(meta: &VaultMeta) -> u64 {
    let mut hasher = DefaultHasher::new();
    meta.hash(&mut hasher);
    hasher.finish()
}
