---
trigger: model_decision
description: Rust and Tauri backend rules, cryptographic boundaries, and zeroize safety for Veylora.
---
# Rust & Tauri Backend Rules

## Security & Trust Boundaries
- The frontend must never receive raw master vault keys (MVK) or key encryption keys (KEK). IPC returns only metadata, tokens, or byte streams.
- All cryptography (AEAD ChaCha20-Poly1305, Argon2id, HKDF, RNG) must remain strictly within `src-tauri/src/crypto/`.
- All sensitive keys and buffers must implement or derive `Zeroize` / `ZeroizeOnDrop` and be cleared immediately upon lock or drop.
- Database access uses SQLite in WAL mode with `synchronous=FULL`; sensitive columns must be AEAD-encrypted before storage.

## Code Quality & Architecture
- Follow the modular structure: `auth`, `crypto`, `database`, `storage`, `vault`, `streaming`, and `commands`.
- Use `thiserror` for error definitions; do not swallow errors or unwrap in production paths.
- Write atomic file updates via temporary files (`.veylora-tmp-*`) followed by `fsync` and atomic rename.

## Token-Efficient Backend Workflows
- When checking compilation, use `cargo check -p veylora` rather than a full release build.
- When debugging compiler errors, inspect the specific module cited by `rustc` instead of reading all backend files.
- Report compiler diagnostics concisely (focus strictly on the error and the fix).

