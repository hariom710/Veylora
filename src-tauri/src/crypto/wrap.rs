//! MVK wrapping / unwrapping with XChaCha20-Poly1305.

use chacha20poly1305::{XChaCha20Poly1305, KeyInit};
use chacha20poly1305::aead::{Aead, Payload};
use hybrid_array::Array;

use crate::crypto::{CryptoError, Mvk, Result, SecretKey};

const NONCE_LEN: usize = 24;

/// The authenticated encrypted form of an MVK under a KEK.
///
/// `nonce` and `ciphertext` are AEAD-authenticated; `aad` (entry metadata such
/// as KDF type, params, salt) is authenticated but not encrypted, so tampering
/// with any of it causes unwrap to fail.
pub struct WrappedPayload {
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>, // MVK (32 bytes) || Poly1305 tag (16 bytes)
}

pub fn wrap_mvk(kek: &SecretKey, mvk: &Mvk, aad: &[u8]) -> Result<WrappedPayload> {
    let cipher = XChaCha20Poly1305::new_from_slice(kek.as_bytes())
        .map_err(|_| CryptoError::Aead)?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce_bytes).map_err(|_| CryptoError::Rng)?;
    let nonce = Array(nonce_bytes);
    let ciphertext = cipher
        .encrypt(&nonce, Payload { aad, msg: mvk.as_bytes() })
        .map_err(|_| CryptoError::Aead)?;
    Ok(WrappedPayload {
        nonce: nonce_bytes,
        ciphertext,
    })
}

pub fn unwrap_mvk(kek: &SecretKey, payload: &WrappedPayload, aad: &[u8]) -> Result<Mvk> {
    let cipher = XChaCha20Poly1305::new_from_slice(kek.as_bytes())
        .map_err(|_| CryptoError::Aead)?;
    let nonce = Array(payload.nonce);
    let plaintext = cipher
        .decrypt(&nonce, Payload { aad, msg: payload.ciphertext.as_slice() })
        .map_err(|_| CryptoError::Aead)?;
    if plaintext.len() != 32 {
        return Err(CryptoError::UnwrapFailed);
    }
    Ok(Mvk::from_array(
        plaintext.try_into().map_err(|_| CryptoError::UnwrapFailed)?,
    ))
}
