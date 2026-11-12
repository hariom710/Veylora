//! Recovery-key generation and encoding.
//!
//! A recovery key is 256 bits of OS randomness. It wraps the MVK via HKDF
//! (label "veylora/v1/recovery-wrap") -- never Argon2id, since it already has
//! 256 bits of entropy.

use sha2::{Digest, Sha256};

use crate::crypto::{CryptoError, Result};

const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const ENCODED_LEN: usize = 52; // base32 of 32 bytes, no padding
const CHECKSUM_LEN: usize = 4; // hex of first 2 SHA-256 bytes

pub fn generate_recovery_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key).expect("OS RNG failure");
    key
}

pub fn encode_recovery_key(key: &[u8; 32]) -> String {
    let encoded = base32_encode(key);
    let mut hasher = Sha256::new();
    hasher.update(key);
    let digest = hasher.finalize();
    let checksum = format!("{:02x}{:02x}", digest[0], digest[1]);
    format!("{}{}", encoded, checksum)
}

pub fn decode_recovery_key(s: &str) -> Result<[u8; 32]> {
    if s.len() != ENCODED_LEN + CHECKSUM_LEN {
        return Err(CryptoError::InvalidRecoveryKey);
    }
    let (encoded, checksum) = s.split_at(ENCODED_LEN);
    let key = base32_decode(encoded)?;
    let mut hasher = Sha256::new();
    hasher.update(key);
    let digest = hasher.finalize();
    let expected = format!("{:02x}{:02x}", digest[0], digest[1]);
    if expected != checksum {
        return Err(CryptoError::ChecksumMismatch);
    }
    Ok(key)
}

fn base32_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() * 8).div_ceil(5));
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for &b in bytes {
        buffer = (buffer << 8) | (b as u32);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let idx = ((buffer >> bits) & 0x1F) as usize;
            out.push(ALPHABET[idx] as char);
        }
    }
    if bits > 0 {
        let idx = ((buffer << (5 - bits)) & 0x1F) as usize;
        out.push(ALPHABET[idx] as char);
    }
    out
}

fn base32_decode(s: &str) -> Result<[u8; 32]> {
    let mut buffer: u32 = 0;
    let mut bits = 0;
    let mut out = [0u8; 32];
    let mut out_pos = 0;
    for c in s.chars() {
        let ch = c.to_ascii_uppercase() as u8;
        let idx = ALPHABET
            .iter()
            .position(|&x| x == ch)
            .ok_or(CryptoError::InvalidBase32)?;
        buffer = (buffer << 5) | (idx as u32);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            if out_pos >= out.len() {
                return Err(CryptoError::InvalidBase32);
            }
            out[out_pos] = (buffer >> bits) as u8;
            out_pos += 1;
        }
    }
    if out_pos != 32 {
        return Err(CryptoError::InvalidBase32);
    }
    Ok(out)
}
