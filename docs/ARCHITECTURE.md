# Veylora Architecture

## Trust boundary

```
┌──────────────────────────────────────────────────────────────┐
│  React + TypeScript (WebView2)                               │
│  Screens, file manager, viewers                              │
│  Talks ONLY to Rust via Tauri IPC + vault://                 │
└──────────────────────────┬───────────────────────────────────┘
                           │ IPC (no keys) / vault:// (byte ranges)
┌──────────────────────────▼───────────────────────────────────┐
│  Rust backend (trust boundary)                               │
│  auth/        PIN, password, sessions, auto-lock, throttling │
│  crypto/      MVK, KEK, HKDF, AEAD, chunk format, zeroize    │
│  vault/       open/create/lock, key hierarchy, recovery      │
│  storage/     encrypted blobs, atomic writes, crash safety   │
│  database/    SQLite via SQLx, encrypted metadata columns    │
│  file_manager/import, export, rename, delete, folders        │
│  backup/      encrypted backups, integrity verification      │
│  security/    attempt counters, event log, sanitization      │
│  streaming/   vault:// protocol, Range handling, auth checks │
│  platform/    DPAPI / Windows Hello glue                     │
└──────────────────────────┬───────────────────────────────────┘
                           │
┌──────────────────────────▼───────────────────────────────────┐
│  Encrypted vault on disk                                     │
│  vault.meta   version, KDF params, wrapped MVK copies        │
│  vault.db     SQLite (sensitive columns AEAD-encrypted)      │
│  blobs/<id>   chunked AEAD ciphertext per file (single file) │
│  blobs/tmp-*  temp files during atomic writes                │
└──────────────────────────────────────────────────────────────┘

### Atomic write pattern

Every write that can leave the vault inconsistent uses the same pattern:
1. Write to `blobs/.veylora-tmp-<random>` inside the vault dir (same volume).
2. `fsync` the temp file.
3. `rename` over the destination (atomic on NTFS).
4. `fsync_dir` the parent directory (best-effort on Windows; NTFS journaling guarantees rename durability).

SQLite runs in WAL mode with `synchronous=FULL`.
```

## Hard trust-boundary rules

1. The frontend never sees the MVK or any raw key material. IPC returns metadata, statuses, and streams — never keys.
2. The frontend never performs cryptography. All AEAD, KDF, HKDF, and RNG happen in Rust.
3. Plaintext bytes only leave Rust through the `vault://` protocol, scoped to an authenticated session, for exact byte ranges only. No temp files, no disk cache, no plaintext in `AppData`.

## Session Lifecycle

```
┌─────────┐    Unlock     ┌──────────┐    Auto-lock/Lock    ┌─────────┐
│ Locked  │ ─────────────►│ Unlocked │ ───────────────────► │ Locked  │
└─────────┘               └──────────┘                       └─────────┘
     ▲                                              │
     │                                              │
     └──────────────────────────────────────────────┘
                    MVK zeroized on lock
```

**Session states:**
- **Locked**: No session active; MVK is not in memory.
- **Unlocked**: Session active; MVK is in a `zeroize` buffer.

**Session transitions:**
- `create`/`unlock`: `Locked` → `Unlocked` (MVK decrypted from `vault.meta` into a zeroizing buffer)
- `lock`: `Unlocked` → `Locked` (MVK zeroized, session dropped)
- `auto-lock`: `Unlocked` → `Locked` (after idle timeout, default 5 minutes)

**Zeroization:** The MVK and all derived keys live in `zeroize` buffers and are wiped when the session is dropped (on lock, close, or auto-lock).

## Module map (`src-tauri/src`)

| Module | Responsibility |
|---|---|
| `auth` | PIN/password/recovery auth, sessions, auto-lock, attempt throttling |
| `crypto` | MVK generation, Argon2id/HKDF, AEAD, zeroize, nonce policy |
| `vault` | open/create/lock, key hierarchy, recovery |
| `storage` | blob read/write, atomic writes, crash-recovery scan |
| `database` | SQLite WAL, encrypted metadata columns |
| `file_manager` | import/export/rename/delete/folders |
| `backup` | encrypted backup container, manifest, restore |
| `security` | attempt counters, event log |
| `streaming` | `vault://` protocol, Range mapping, per-chunk auth |
| `platform` | Windows DPAPI / Windows Hello glue |
| `commands` | Tauri IPC command handlers (thin layer, no crypto) |

## Data flow

1. **Unlock**: frontend sends PIN/password via IPC → Rust derives KEK → unwraps MVK → session object holds zeroized MVK → frontend receives session token only.
2. **Import**: frontend streams file bytes via `vault://` upload → Rust encrypts chunks → writes blobs atomically → writes encrypted metadata to SQLite.
3. **Preview**: frontend requests `vault://file/<id>?range=...` with session token → Rust maps range to chunks → decrypts + authenticates per chunk → streams bytes.
4. **Lock**: Rust zeroizes MVK + derived keys, invalidates session token, closes outstanding `vault://` streams.
