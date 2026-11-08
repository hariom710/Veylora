//! Master Vault Key and generic 256-bit secrets.
//!
//! The MVK is generated from the OS CSPRNG and exists only in memory while
//! the vault is unlocked. It is zeroized on drop.

use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::{CryptoError, Result};

/// Master Vault Key (256 bits). Zeroized on drop.
#[derive(Zeroize, ZeroizeOnDrop, Clone)]
pub struct Mvk([u8; 32]);

impl Mvk {
    pub fn generate() -> Result<Self> {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).map_err(|_| CryptoError::Rng)?;
        Ok(Self(bytes))
    }

    pub(crate) fn from_array(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// A generic 256-bit secret (KEK or derived subkey). Zeroized on drop.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SecretKey([u8; 32]);

impl SecretKey {
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(*bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
