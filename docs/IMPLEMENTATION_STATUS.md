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
Windows session-lock: implemented in v1.0 (auth::session_lock; phase4_windows_session_lock_locks_vault replaced the deferral placeholder).

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
Status: complete
- Modules under src-tauri/src/backup/: mod.rs, container.rs, create.rs, restore.rs, verify.rs
- New commands in commands/mod.rs: vault_backup_create, vault_backup_verify, vault_backup_restore, vault_backup_history
- Database migration v3: ALTER TABLE backup_history ADD COLUMN backup_size, ADD COLUMN status
- Frontend: src/routes/backup/index.tsx wired to all four commands; src/lib/ipc.ts has IPC wrappers; src/lib/types.ts has BackupInfo/RestoreInfo/BackupHistoryEntry
- Tests: 15 tests in tests/phase7_backup.rs (14 non-ignored + 1 ignored 100 MiB), all pass
  - Test #1: create backup of vault with 3 files → file exists, size > 0, file_count == 3
  - Test #2: verify created backup → Ok with correct file_count and verified_fields
  - Test #3: restore into fresh vault → files present, blobs match, DB rows match
  - Test #4: round-trip create → restore → verify file contents match
  - Test #5: backup history populated → entry with status "complete"
  - Test #6: tamper manifest → verify returns Corrupted
  - Test #7: tamper blob → verify returns Corrupted (blobs_hash mismatch)
  - Test #8: truncate trailer → verify returns Corrupted
  - Test #9: swap manifest/different meta_len → header AEAD fails
  - Test #10: restore with wrong MVK → header AEAD fails, Corrupted
  - Test #11: restore with existing file_id → BackupError::FileExists, no partial restore
  - Test #12: restore atomic failure → no DB rows added, all-or-nothing
  - Test #13: restore while locked → CommandError::Locked
  - Test #14: create while locked → CommandError::Locked
  - Test #15 (ignored): 100 MiB file create → verify → restore → read → compare
- Frontend Vitest: 3 tests in src/lib/phase7-backup.test.tsx (history render, create backup call, corrupted verify error)
- Container format documented in docs/CRYPTO_DESIGN.md (regions, backup_key derivation, binding chain, all-or-nothing restore)
- Backup/restore flow diagram documented in docs/ARCHITECTURE.md
- Format v1 unchanged, 8 frozen HKDF labels unchanged (backup-key already reserved from Phase 2; "manifest-nonce"/"trailer-nonce" are subkey infos, not new labels)
- No tokio dependency added. Commands are synchronous.
- tauri-plugin-dialog ADDED: Cargo.toml, main.rs plugin registration, capabilities/default.json, package.json, Backup screen wired with saveDialog/openDialog

## Phase 8 — Windows release
Status: fixes applied (frontend assets + dialog plugin); smoke test in progress
- Build environment: `build.bat` at repo root restores MSVC toolchain. CI uses runner defaults.
- Tauri version pinned to 2.10 for compatibility with build environment.
- FIX (frontend assets): `frontendDist` was pointing outside the repo (`../../src-tauri/target/release/dist` resolved from `src-tauri/` to `D:\projets\src-tauri\...`), so early installers shipped no UI and showed "asset not found: index.html". Corrected: `vite.config.ts` `build.outDir` → `dist` (repo root), `tauri.conf.json` `frontendDist` → `../dist`. Installer now embeds `dist/index.html`.
- FIX (dialog plugin): `@tauri-apps/plugin-dialog` was imported as `{ saveDialog, openDialog }`, but the package exports `{ save, open, message, ask, confirm }` — Rollup failed with `"saveDialog" is not exported by .../plugin-dialog/dist-js/index.js` (build-time error, `pnpm build`, src/routes/backup/index.tsx:11). Corrected to `import { save as saveDialog, open as openDialog }`. Backup screen now has "Browse…" buttons wired to `save`/`open` dialogs alongside the native text inputs.
- tauri-plugin-dialog ADDED: Cargo.toml, main.rs plugin registration, capabilities/default.json, package.json, Backup screen wired with saveDialog/openDialog (aliased to save/open).
- Icons: placeholder PNG icons generated at 16/24/32/48/64/128/256/512. Replace with professional artwork before v1.1.
- Windows session-lock: IMPLEMENTED in v1.0 (auth/session_lock.rs, event-driven WTS). Previously deferred on a belief that WTS integration was unavailable in the build environment.
- Windows Hello: DEFERRED to v1.1 (Option B - documented as known limitation).
- Auto-update: DEFERRED to v1.1 (Option B - documented as known limitation).
- Code signing: UNSIGNED (no certificate available). SHA-256 hash published for verification. SmartScreen warning documented in RELEASE_NOTES.md.
- CI/CD: .github/workflows/ci.yml and .github/workflows/release.yml created.
- Supply chain: src-tauri/deny.toml created; cargo-deny passes (advisories, bans, licenses, sources ok).
- Supply chain: pnpm audit — no known vulnerabilities found (vitest 5.1.0, vite 7.3.8, react-router-dom 7.18.5 upgrades).
- Docs: SECURITY.md, RELEASE_NOTES.md, docs/SECURITY_REVIEW.md created.
- Tests: 149 non-ignored Rust + 27 frontend tests pass (same as Phase 7).
- Format v1 unchanged, 8 frozen HKDF labels unchanged.
- No tokio dependency. No new crypto. No new HKDF labels.

## Phase 9 — UI-flow slices (A, B, C) + final release

### Slice A — Login gate & first-run (complete)
- Three-layer auth enforced end to end: Layer 1 app login (app_users in
  `%APPDATA%\com.veylora.app\app.db`, in-memory session), Layer 2 vault selection
  (Welcome screen), Layer 3 vault unlock (PIN/password/recovery). Logout ≠ lock,
  lock ≠ logout.
- Admin account seeded on first start: `b1865741@gmail.com` / `Harry@12345`
  (src-tauri/src/app_auth.rs ADMIN_EMAIL/ADMIN_DEFAULT_PASSWORD). The app flags the
  default password so it can be changed from the Account screen.
- Routes: /login, /welcome (first-run create/open), gated app shell.
- Tests: 7 tests in tests/phase9_app_auth.rs; frontend phase9-login.test.tsx,
  first-run.test.tsx.

### Slice B — Recovery key & file import/export (complete)
- Recovery key shown exactly once at vault creation (recovery screen); Settings
  states honestly that it cannot be re-displayed.
- Import: native open dialog → streaming encrypt → row in encrypted_files.
  Export: user-initiated only, via native save dialog (this is the single path
  where plaintext is written to disk, by explicit user action).
- Viewers (pdf/image/text/office) resolve their file from useParams().fileId and
  read through vaultFetch (403 → LockedModal).
- Tests: 2 tests in tests/sliceb_files.rs (timeouts hardened to 10s after one
  load-flakiness run); frontend sliceb-recovery.test.tsx.

### Slice C — Favorites, recent, settings, shortcuts (complete)
- Backend: database/favorites.rs + list_recent; commands file_favorite /
  file_favorites / file_recent; Settings commands (vault_activity, auto_lock_set,
  valid_pin, read_meta/write_meta, vault_change_pin, vault_change_password,
  vault_backup_history_clear, vault_delete). All 9 handlers + spawn_watcher
  registered in main.rs.
- Auto-lock: AtomicU64 TIMEOUT_SECS in auth/auto_lock.rs (0 = never, default 300 s),
  watcher spawned once in main; frontend heartbeat (mousemove/keydown/pointerdown →
  vaultActivity, throttled 60 s, only when logged in + unlocked); choice persisted
  in localStorage `veylora.autoLockSecs` and re-applied on boot.
- Favorites keyed on blob file_id; Recent = newest import (ORDER BY id DESC).
- Danger zone: vault_delete verifies the PIN before any bytes are removed.
- Shortcuts: Ctrl+L lock; Ctrl+O / Ctrl+F navigate to Files then dispatch
  veylora:import / veylora:focus-search; Ctrl+, settings; Ctrl+Shift+A account;
  Ctrl+1–4 Dashboard/Files/Backup/Settings; skipped when the event target is an
  input/textarea/select; Esc intentionally unbound (native dialogs only).
- Tests: 7 tests in tests/slicec_settings.rs; frontend settings (5), favorites (3)
  plus updated mock factories across all App-rendering suites.

### Fixes made during the slices
- auth/throttle.rs: a single wrong PIN used to persist `locked_until = now + 24h`
  because the threshold loop fell through to the highest tier. Fixed:
  below 5 failures → no lockout (locked_until is set to now, check_locked sees it
  expired); the highest matching tier wins (`.iter().rev()`). Regression test
  phase4_no_lockout_below_five_failures added to tests/phase4_auth.rs.
- Login/signup password field rendered invisible (white text on a white UA input
  background): `src/index.css` defines no shadcn tokens, so `bg-background` was a
  no-op. Fixed in src/components/ui/input.tsx with explicit dark utilities
  (border-slate-700 bg-slate-900 text-slate-50 …; since replaced by the design
  tokens below).
- Cold start skipped the Welcome homepage: `App.tsx` `boot()` auto-opened the
  remembered `veylora.vaultPath`, so `vaultStatus` reported `vault_open: true`
  and the gate rendered Unlock instead of Welcome (a remembered vault also
  rejected any PIN the user had not set for it). Per the UI-flow diagram
  (Login → Welcome → Create/Open → Unlock), `boot()` no longer opens any vault;
  the stored path is opened only by Welcome ("Open") and by `handleLock` within
  the same session. Covered by first-run.test.tsx and phase9-login.test.tsx.
- Launching the installed app opened a terminal window alongside it:
  `main.rs` had no `windows_subsystem` attribute, so Rust linked `veylora.exe`
  as a **console** binary (PE `Subsystem=3`). Added
  `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` — release
  builds are now GUI (`Subsystem=2`, verified on the shipped exe), while debug
  builds keep the console so `tauri dev` logs stay visible.
- Opening an existing vault required hand-typing a full Windows path (and the
  Open form sits below the whole Create form). Welcome now shows an
  "Open a vault" list at the top (`veylora.recentVaults`, newest first, deduped,
  capped at 5, validated on mount via `vaultGetMeta` so a moved vault never
  leaves a dead row; first run after update seeds it from `veylora.vaultPath`),
  prefills the Open path from the last used vault, and adds a Browse… button to
  the Open form. Covered by routes/welcome/index.test.tsx (8 tests); verified
  live on the installed build via WebView2 CDP (login → list → one-click →
  Unlock).
- Apple-Inspired UI Design Brief, Phase 1 (frontend only — no backend, format,
  crypto, or IPC change): `tailwind.config.ts` now carries the exact supplied
  tokens (colors background/surface/elevated/border/foreground/muted/accent/
  success/warning/danger; type scale display 48 … mono 13 with fixed
  line-heights and weights 400–700; radii input/button 10, card 16, dialog 20;
  spacing 4·8·12·16·24·32·48·64·96; boxShadow modal + focus only) and
  `src/index.css` the base layer (dark color-scheme, focus ring, selection).
  Inter loads locally as one .woff2 (`public/fonts/InterVariable.woff2`, no
  Google Fonts). Every screen swept from `slate-*`/rainbow literals to tokens,
  headings put on the exact type scale, card shadows removed (borders only),
  and primary actions became the giant 44px / 17px-600 `#0A84FF` button —
  including a real pre-existing bug: `button.tsx` destructured `variant`/`size`
  without defaults, so the old no-op `bg-primary` had hidden that variant
  styles never applied when callers omitted the prop. Verified on the
  installed build via CDP screenshots (login, welcome, unlock tabs,
  dashboard/nav). 56/56 frontend tests still pass; copy, roles and flow
  untouched.
- Unlock stranded the UI on the Unlock screen (release blocker found by the
  final smoke test): the Unlock screen authenticated correctly, then passed
  the **session token** to `App.handleUnlock`, which replayed it as a PIN via
  a second `vaultUnlockPin(token)` call. That call always fails against the
  real backend, so `setIsLocked(false)` never ran — backend `locked:false`
  while the UI stayed on Unlock (and each attempt logged a phantom throttle
  failure). Present since baf8f2f; hidden in CI because tests mock
  `vaultUnlockPin` to always succeed. Fixed: `handleUnlock` now just records
  the token and flips the lock state (same pattern as
  `handleRecoveryContinue`); the duplicate imports were removed. Regression:
  phase6-token-rotation.test.tsx now asserts `vaultUnlockPin` is called
  exactly once, with the PIN, per unlock.
- File viewers showed nothing (release blocker found by the final smoke
  test, three stacked causes): (1) WebView2/wry only intercepts
  `http://vault.*` — raw `vault://` subresources never reach the handler, so
  `vaultUrl.ts` now emits `http://vault.localhost/file/...` (wry reverts it
  to `vault://localhost/...`; `main.rs` strips the `localhost/` segment so
  the frozen parser sees `vault://file/...`); (2) the response lacked
  `Access-Control-Allow-Origin`, so the browser hid decrypted bytes from JS
  fetch — the closure now echoes the request `Origin` header; (3) the frozen
  `streaming/*` scaffolding cannot serve production data (`decrypt_chunk`
  returns dummy bytes; its tests use 64-hex ids while `file_import` mints
  32-hex ids, so the parser 400s on every real file). Fix without touching
  the freeze: new `file_preview` in `commands/files.rs` (sibling of
  `file_export` — lock gate → constant-time token → row-existence gate →
  `FileKey::derive` → `read_range`, bytes in memory only), called by the
  `main.rs` protocol closure with the same status map as
  `streaming/protocol.rs` (403/404/400/416/500). Phase 5 tests untouched
  and green. Also fixed: `vault_status` hardcoded `file_count:0` — now
  `file_stats()` (COUNT + SUM of plaintext `encrypted_size`, no key),
  zeros only when no vault is open. Regression: preview roundtrip asserts
  in sliceb_files.rs (full bytes, inclusive range, wrong token rejected).

### Disclosures
- rusqlite 0.41 migration: u64 → i64 casts added in streaming/handler.rs and
  backup/restore.rs (integer column types). No format, crypto, or protocol change.
- Import gap resolved: the UI-flow "import" requirement is covered by the Slice B
  import path (dialog → streaming encrypt → DB row); no separate bulk/folder
  import exists.
- Skipped for v1.1 (by agreement, not regressions): light theme, per-folder
  storage breakdown, "Move vault", Settings recovery-key re-display (impossible by design), Windows Hello, auto-update. (Shipped in this pass:
  right-click context menus, move-to-folder + folder rename, batch select;
  since shipped in v1.0: WTS session-lock, browser E2E suite.)
- Frozen surfaces untouched: crypto/*, storage/format/*, storage/blob.rs,
  storage/crash.rs, streaming/*, auth/session.rs, backup/*. Format v1 unchanged;
  8 frozen HKDF labels unchanged (only subkey infos "manifest-nonce"/"trailer-nonce"
  under veylora/v1/backup-key, reserved since Phase 2). No tokio, no new crypto crates.
