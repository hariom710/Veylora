#![allow(missing_docs)]

//! Veylora cryptographic foundation (Phase 1).
//!
//! Provides the MVK, Argon2id KEK derivation, HKDF domain-separated subkeys,
//! MVK wrapping for PIN/password/recovery paths, and recovery-key encoding.
//! All key material is zeroized on drop.

pub mod auth;
pub mod commands;
pub mod crypto;
pub mod database;
pub mod storage;
pub mod vault;
