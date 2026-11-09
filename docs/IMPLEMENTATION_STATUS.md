# Implementation Status

## Phase 0 — Repository audit
Status: complete
- Repo: empty (greenfield).
- Toolchain: rustc/cargo 1.99.0, node v22.21.1, npm 11.11.0, git present. pnpm and tauri CLI missing (not needed for Phase 1).
- MSVC build environment: The `VC\Tools\MSVC\14.50.35717\include` directory is missing from the VS 18 installation (only `Auxiliary`, `bin`, `lib` present). `cl.exe` exists at `VC\Tools\MSVC\14.50.35717\bin\Hostx64\x64\cl.exe`. Headers `excpt.h` and `stdarg.h` exist in `SDK\ScopeCppSDK\vc15\VC\include`. Windows Kits headers/libs exist at `C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\` and `...\Lib\10.0.26100.0\`.
- Build script: `build.bat` at repo root sets `PATH` (MSVC bin), `INCLUDE` (ScopeCppSDK + Windows Kits ucrt/um/shared), `LIB` (MSVC lib onecore/x64 + Windows Kits lib um/ucrt x64), then `cd` to `src-tauri` and runs `cargo %*`.
- Nonce policy: HKDF(file-key, "nonce", chunk_index) → 24 bytes (deterministic).
- Docs created: ARCHITECTURE.md, CRYPTO_DESIGN.md, IMPLEMENTATION_STATUS.md.

## Phase 1 — Cryptographic foundation
Status: complete
Modules under src-tauri/src/:
  crypto/mod.rs     — shared types, CryptoError (typed, never silent), Result
  crypto/mvk.rs     — Mvk (32B, OS CSPRNG via getrandom), SecretKey, zeroize on drop
  crypto/kdf.rs     — Argon2id PIN (m=64 MiB) & Password (m=256 MiB), t=3, p=1, salt=32B
  crypto/hkdf.rs    — derive_subkey(key, label); derive_subkey_info(key, label, info);
                      frozen labels only; empty/unknown rejected
  crypto/wrap.rs    — wrap_mvk/unwrap_mvk (XChaCha20-Poly1305, per-entry AAD)
  crypto/recovery.rs — recovery-key generation (256-bit CSPRNG), base32 + SHA-256 checksum
  vault/mod.rs      — VaultMeta (versioned serde_json): PIN/password/recovery wrapped MVK copies,
                      add/unwrap entry points, version check on deserialize

Tests (tests/integration.rs, tests/vectors.rs): all pass.
Positive: MVK gen 32B/differs; Argon2id PIN+Password deterministic + salt-sensitive;
HKDF deterministic + label-separated; wrap/unwrap round-trip PIN/password/recovery;
recovery encode/decode round-trip; VaultMeta serialize/deserialize round-trip;
committed test vectors (Argon2id-PIN, Argon2id-Password, HKDF for all 6 labels).
Negative: wrong PIN → typed Aead/UnwrapFailed error; wrong recovery key → typed error;
tampered ciphertext byte → typed error; tampered AAD → typed error; empty/unknown
HKDF label → typed error; recovery single-char mutation → checksum/base32 error;
wrong length → InvalidRecoveryKey; wrong VaultMeta version → UnsupportedVersion.
Committed vectors: tests/vectors.rs (3 tests).

## Phase 2 — Encrypted file format
Status: complete
Modules under src-tauri/src/storage/format/:
  mod.rs      — CHUNK_SIZE=65536, MAGIC="VYLR", VERSION=1, FormatError, FileKey
  header.rs   — header write/read + AEAD (header core encrypted, AAD over magic+version)
  chunk.rs    — chunk encrypt/decrypt with per-chunk nonce + AAD
  finalize.rs — finalize record write/read + AEAD (root_hash || total_length || total_chunks)
  reader.rs   — read_range (random-access), verify_file (full authentication)
  writer.rs   — write_file (streaming writer), write_file_bytes convenience

Format v1 FROZEN (see docs/CRYPTO_DESIGN.md). All 29 lib tests pass, including:
Positive: 1-byte, 65536-byte, 65537-byte, 10 MiB round-trips; random-access ranges;
multi-chunk spans; same-chunk ranges; empty range; verify_file on clean file.
Negative (all return typed errors): tampered chunk ciphertext; reordered chunks;
duplicated chunk; truncated (last 100 bytes); truncated (middle chunk);
cross-file substitution; tampered header ciphertext; tampered header AAD;
wrong file key.
1 GiB test: #[ignore]d, present and compiles; runs via `cargo test -- --ignored`.

Deferred: AeadInPlace → AeadInOut migration (aead 0.6 deprecation, accepted for now).

## Phase 3 — Storage & database
Status: complete
Modules under src-tauri/src/:
  storage/mod.rs      — module re-exports
  storage/vault_dir.rs — VaultDir struct: vault.meta, vault.db, blobs/ paths;
                        create_layout(), ensure_dirs(), fsync_dir()
  storage/blob.rs     — write_blob, read_blob, delete_blob, list_blobs,
                        cleanup_tmp_files; atomic write pattern (temp → fsync → rename → fsync_dir)
  storage/crash.rs    — startup_scan: orphaned tmp cleanup, SQLite integrity check
  database/mod.rs     — Db wrapper around rusqlite::Connection; WAL mode,
                        synchronous=FULL, migrations on open
  database/schema.rs  — MIGRATION_V1: 6 tables (vaults, folders, encrypted_files,
                        favorites, application_settings, backup_history) + schema_migrations
  database/crypto.rs  — encrypt_meta/decrypt_meta: XChaCha20-Poly1305 with metadata-key,
                        random 24B nonce per column, AAD = table || row_id_LE || column
  database/files.rs   — insert_file, get_file, list_files, rename_file, delete_file,
                        move_file; FileRecord with encrypted filename/timestamps
  database/folders.rs — insert_folder, get_folder, list_children, rename_folder;
                        FolderRecord with encrypted name/created_at

Tests: 66 lib tests pass, including:
Positive: atomic write round-trip (create_new with retry); random-access chunk read; DB open + migrate;
insert/get file with encrypted filename; insert/get folder; rename file/folder;
move file between folders; delete file; list files by folder; list child folders;
metadata encryption: different nonces → different ciphertext, same plaintext;
AAD binding: tampered row_id → AEAD failure; wrong key → AEAD failure;
plaintext filename not found in vault.db raw bytes (verified via grep).
Negative: tampered chunk → typed error; tampered header → typed error; wrong file key → typed error;
duplicate file_id → DuplicateBlobName error; wrong metadata_key → typed error;
truncated file → TooSmall/FinalizeAuth error; cross-file substitution → ChunkAuth error;
corrupt vault.db → IntegrityFailed error.
Power-loss simulation: injected Read failure mid-write → no final file, orphaned
.veylora-tmp-* exists, startup_scan removes it, subsequent write with same file_id
succeeds.
fsync_dir on normal directory returns Ok(()) on Windows (degraded to NTFS journal durability).
Deferred: AeadInPlace → AeadInOut migration (aead 0.6 deprecation, accepted for now).
Windows durability: fsync_dir uses FILE_FLAG_BACKUP_SEMANTICS + FlushFileBuffers.
FlushFileBuffers requires SeBackupPrivilege (error code 5 on regular users); returns
success on ERROR_ACCESS_DENIED since NTFS journaling guarantees rename atomicity.
Any other error code from FlushFileBuffers is returned as io::Error. File-level
sync_all always succeeds and guarantees blob content durability before rename.
Docs/CRYPTO_DESIGN.md has a "Durability" section documenting this strategy.

## Phase 4 — Auth, sessions, IPC
Status: complete
Modules under src-tauri/src/:
  auth/mod.rs      — AuthError, UnlockMethod enum
  auth/session.rs  — Session (MVK zeroizing, vault_id, unlock_method, last_activity),
                    SessionManager (Mutex<Option<Session>>), lock/is_locked/touch_activity/with_mvk
  auth/pin.rs      — unlock_with_pin (Argon2id KEK + MVK unwrap from vault.meta)
  auth/password.rs — unlock_with_password (Argon2id KEK + MVK unwrap from vault.meta)
  auth/recovery.rs — unlock_with_recovery (HKDF KEK + MVK unwrap from vault.meta)
  auth/throttle.rs — attempt throttling persisted in SQLite (auth_attempts table),
                    5 failures → 30s lockout, 10 → 5min, 15 → 1h, 20+ → 24h
  auth/auto_lock.rs — spawn_watcher (std::thread, checks every 30s),
                       check_auto_lock locks session on idle timeout
  commands/mod.rs  — Tauri IPC commands: vault_create/open/unlock_pin/password/recovery,
                    lock/status/get_meta (thin wrappers, no key material in args/returns)

Tests: 17 new tests in tests/phase4_auth.rs, all pass.
Positive: wrap/unwrap round-trip PIN/password/recovery; session with MVK via with_mvk;
auto-lock after timeout; auto-lock reset on activity; wrong PIN/password/recovery →
typed AuthError; wrong PIN 5× → check_locked returns Some; successful unlock clears
attempts; lock when already locked is idempotent; unwrap with no vault.meta → VaultNotFound;
tampered vault.meta → VaultCorrupted; recovery key decode/encode round-trip.
Negative: wrong PIN → InvalidCredential; wrong password → InvalidCredential;
wrong recovery key → InvalidCredential; tampered vault.meta → VaultCorrupted;
no vault.meta → VaultNotFound.
IPC boundary: source-scan test confirms no key material types in command signatures.
Windows session-lock: deferred to Phase 8 (#[ignore]d test documents deferral).

## Phase 5 — Protected streaming
Status: complete
- Modules under src-tauri/src/streaming/: protocol.rs, handler.rs, url.rs, token.rs
- Tests: 24 tests in tests/phase5_streaming.rs, all pass.

## Phase 6 — Frontend
Status: complete
- Build environment: `build.bat` at repo root restores MSVC toolchain (PATH/INCLUDE/LIB pointing to VS 18 + Windows Kits 10). Verified `cargo build` clean and `cargo test` passes.
- Frontend: React + Tauri 2 + TypeScript + Vite + Tailwind + shadcn/ui + Lucide + Framer Motion.
- New files: src/lib/vaultFetch.ts (403→callback), src/components/ui/locked-modal.tsx, src/lib/phase6-token-rotation.test.tsx, src/lib/phase6-mid-stream-lock.test.tsx
- Modified: src/routes/pdfviewer/index.tsx (uses vaultFetch, shows LockedModal on 403)
- Tests: 128 Rust tests passing (66 lib + 17 integration + 17 phase4 + 24 phase5 + 6 phase6_protocol + 3 vectors); 1 ignored (1 GiB). Frontend: 24 Vitest tests passing (6 vaultUrl + 8 ipc + 2 pdfviewer + 3 filemanager + 5 unlock + 1 phase6-token-rotation + 1 phase6-mid-stream-lock).
- E2E: backend end-to-end test phase6_e2e_lock_invalidates_token_unlock_rotates verifies lock invalidates token (403) and unlock rotates it (token2 != token1, fetch succeeds).
- Phase 6 E2E verified on 2026-11-09 with 133 Rust + 24 frontend tests passing.
- Token ownership: currently in AppContentInner (sessionToken state, cleared on handleLock). Consider a useVaultSession hook when a second consumer appears.

## Phase 7 — Backup & restore
Status: not started (blocked on Phase 6)

## Phase 8 — Windows release
Status: not started (blocked on Phase 7)
