//! Vault metadata: versioned, authenticated wrapped MVK copies.
//!
//! Each wrapped MVK copy is individually AEAD-authenticated (XChaCha20-Poly1305),
//! so the whole VaultMeta is authenticated through its entries. The version
//! field is checked on deserialize to reject header tampering.

use serde::{Deserialize, Serialize};

use crate::crypto::hkdf::{LABEL_RECOVERY_WRAP, derive_subkey};
use crate::crypto::kdf::{derive_password_kek, derive_pin_kek, PASSWORD_MEM_KIB, PIN_MEM_KIB, SALT_LEN};
use crate::crypto::wrap::{unwrap_mvk, wrap_mvk, WrappedPayload};
use crate::crypto::{CryptoError, Mvk, Result, SecretKey};

const VAULT_META_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct VaultMeta {
    pub version: u32,
    pub pin: Option<WrappedEntry>,
    pub password: Option<WrappedEntry>,
    pub recovery: Option<WrappedEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct WrappedEntry {
    pub kdf: KdfType,
    pub params: KdfParams,
    pub salt: Vec<u8>,
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub enum KdfType {
    Argon2idPin,
    Argon2idPassword,
    RecoveryHkdf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub enum KdfParams {
    Argon2id { mem_kib: u32, time: u32, lanes: u32 },
    Hkdf,
}

fn random_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    getrandom::getrandom(&mut salt).expect("OS RNG failure");
    salt
}

/// Authenticated data for a wrapped entry: KDF type, params, and salt.
/// Prevents substitution of one wrapping entry for another.
fn entry_aad(kdf: &KdfType, params: &KdfParams, salt: &[u8]) -> Vec<u8> {
    let mut aad = Vec::new();
    aad.push(match kdf {
        KdfType::Argon2idPin => 0,
        KdfType::Argon2idPassword => 1,
        KdfType::RecoveryHkdf => 2,
    });
    match params {
        KdfParams::Argon2id { mem_kib, time, lanes } => {
            aad.extend_from_slice(&mem_kib.to_le_bytes());
            aad.extend_from_slice(&time.to_le_bytes());
            aad.extend_from_slice(&lanes.to_le_bytes());
        }
        KdfParams::Hkdf => {
            aad.extend_from_slice(&[0u8; 12]);
        }
    }
    aad.extend_from_slice(salt);
    aad
}

impl VaultMeta {
    pub fn new() -> Self {
        Self {
            version: VAULT_META_VERSION,
            pin: None,
            password: None,
            recovery: None,
        }
    }

    pub fn add_pin_entry(&mut self, pin: &[u8], mvk: &Mvk) -> Result<()> {
        let salt = random_salt();
        let kek = derive_pin_kek(pin, &salt)?;
        let aad = entry_aad(&KdfType::Argon2idPin, &KdfParams::Argon2id { mem_kib: PIN_MEM_KIB, time: 3, lanes: 1 }, &salt);
        let payload = wrap_mvk(&kek, mvk, &aad)?;
        self.pin = Some(WrappedEntry {
            kdf: KdfType::Argon2idPin,
            params: KdfParams::Argon2id {
                mem_kib: PIN_MEM_KIB,
                time: 3,
                lanes: 1,
            },
            salt: salt.to_vec(),
            nonce: payload.nonce,
            ciphertext: payload.ciphertext,
        });
        Ok(())
    }

    pub fn add_password_entry(&mut self, password: &[u8], mvk: &Mvk) -> Result<()> {
        let salt = random_salt();
        let kek = derive_password_kek(password, &salt)?;
        let aad = entry_aad(&KdfType::Argon2idPassword, &KdfParams::Argon2id { mem_kib: PASSWORD_MEM_KIB, time: 3, lanes: 1 }, &salt);
        let payload = wrap_mvk(&kek, mvk, &aad)?;
        self.password = Some(WrappedEntry {
            kdf: KdfType::Argon2idPassword,
            params: KdfParams::Argon2id {
                mem_kib: PASSWORD_MEM_KIB,
                time: 3,
                lanes: 1,
            },
            salt: salt.to_vec(),
            nonce: payload.nonce,
            ciphertext: payload.ciphertext,
        });
        Ok(())
    }

    pub fn add_recovery_entry(&mut self, recovery_key: &[u8; 32], mvk: &Mvk) -> Result<()> {
        let kek = SecretKey::from_bytes(&derive_subkey(recovery_key, LABEL_RECOVERY_WRAP)?);
        let aad = entry_aad(&KdfType::RecoveryHkdf, &KdfParams::Hkdf, &[]);
        let payload = wrap_mvk(&kek, mvk, &aad)?;
        self.recovery = Some(WrappedEntry {
            kdf: KdfType::RecoveryHkdf,
            params: KdfParams::Hkdf,
            salt: Vec::new(),
            nonce: payload.nonce,
            ciphertext: payload.ciphertext,
        });
        Ok(())
    }

    pub fn unwrap_with_pin(&self, pin: &[u8]) -> Result<Mvk> {
        let entry = self.pin.as_ref().ok_or(CryptoError::UnwrapFailed)?;
        let salt: [u8; SALT_LEN] = entry
            .salt
            .clone()
            .try_into()
            .map_err(|_| CryptoError::UnwrapFailed)?;
        let kek = derive_pin_kek(pin, &salt)?;
        let aad = entry_aad(&entry.kdf, &entry.params, &entry.salt);
        let payload = WrappedPayload {
            nonce: entry.nonce,
            ciphertext: entry.ciphertext.clone(),
        };
        unwrap_mvk(&kek, &payload, &aad)
    }

    pub fn unwrap_with_password(&self, password: &[u8]) -> Result<Mvk> {
        let entry = self.password.as_ref().ok_or(CryptoError::UnwrapFailed)?;
        let salt: [u8; SALT_LEN] = entry
            .salt
            .clone()
            .try_into()
            .map_err(|_| CryptoError::UnwrapFailed)?;
        let kek = derive_password_kek(password, &salt)?;
        let aad = entry_aad(&entry.kdf, &entry.params, &entry.salt);
        let payload = WrappedPayload {
            nonce: entry.nonce,
            ciphertext: entry.ciphertext.clone(),
        };
        unwrap_mvk(&kek, &payload, &aad)
    }

    pub fn unwrap_with_recovery(&self, recovery_key: &[u8; 32]) -> Result<Mvk> {
        let entry = self.recovery.as_ref().ok_or(CryptoError::UnwrapFailed)?;
        let kek = SecretKey::from_bytes(&derive_subkey(recovery_key, LABEL_RECOVERY_WRAP)?);
        let aad = entry_aad(&entry.kdf, &entry.params, &entry.salt);
        let payload = WrappedPayload {
            nonce: entry.nonce,
            ciphertext: entry.ciphertext.clone(),
        };
        unwrap_mvk(&kek, &payload, &aad)
    }

    pub fn serialize(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|_| CryptoError::Serialization)
    }

    pub fn deserialize(s: &str) -> Result<Self> {
        let meta: Self = serde_json::from_str(s).map_err(|_| CryptoError::Serialization)?;
        if meta.version != VAULT_META_VERSION {
            return Err(CryptoError::UnsupportedVersion);
        }
        Ok(meta)
    }
}
