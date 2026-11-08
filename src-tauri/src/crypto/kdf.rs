//! Argon2id KEK derivation for PIN and password paths.

use argon2::{Algorithm, Argon2, Params, Version};

use crate::crypto::{CryptoError, Result, SecretKey};

pub const SALT_LEN: usize = 32;
pub const ARGON2_TIME: u32 = 3;
pub const ARGON2_LANES: u32 = 1;

/// Memory cost for PIN: 64 MiB = 65536 KiB.
pub const PIN_MEM_KIB: u32 = 64 * 1024;
/// Memory cost for password: 256 MiB = 262144 KiB.
pub const PASSWORD_MEM_KIB: u32 = 256 * 1024;

pub fn derive_pin_kek(pin: &[u8], salt: &[u8; SALT_LEN]) -> Result<SecretKey> {
    derive_argon2id_kek(pin, salt, PIN_MEM_KIB)
}

pub fn derive_password_kek(password: &[u8], salt: &[u8; SALT_LEN]) -> Result<SecretKey> {
    derive_argon2id_kek(password, salt, PASSWORD_MEM_KIB)
}

fn derive_argon2id_kek(secret: &[u8], salt: &[u8; SALT_LEN], mem_kib: u32) -> Result<SecretKey> {
    let params = Params::new(mem_kib, ARGON2_TIME, ARGON2_LANES, None)
        .map_err(|e| CryptoError::Argon2(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut output = [0u8; 32];
    argon2
        .hash_password_into(secret, salt, &mut output)
        .map_err(|e| CryptoError::Argon2(e.to_string()))?;
    Ok(SecretKey::from_bytes(&output))
}
