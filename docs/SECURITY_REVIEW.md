# Veylora Internal Security Review

This is an internal security review, not an external audit. It documents the
security decisions made during Phases 0-7 and identifies areas for future
improvement.

## Review Checklist

### Key Hierarchy

| Component | Status | Notes |
|-----------|--------|-------|
| MVK (Master Vault Key) | ✓ Reviewed | 256-bit CSPRNG, zeroized on drop, never crosses IPC boundary |
| KEK (Key Encryption Key) | ✓ Reviewed | Derived via Argon2id (PIN/password) or HKDF (recovery) |
| HKDF subkeys | ✓ Reviewed | 8 frozen labels, domain-separated, no label reuse |
| backup_key | ✓ Reviewed | HKDF(MVK, "veylora/v1/backup-key"), same-MVK-only restore |
| Subkey infos | ✓ Reviewed | "manifest-nonce", "trailer-nonce" are subkey infos, not labels |

### File Format v1

| Component | Status | Notes |
|-----------|--------|-------|
| Header | ✓ Reviewed | AEAD-encrypted, AAD over magic+version |
| Chunks | ✓ Reviewed | Per-chunk AEAD with deterministic nonces |
| Finalize record | ✓ Reviewed | AEAD-encrypted, root hash binding |
| Format v1 frozen | ✓ Reviewed | No changes permitted without version bump |

### Metadata Encryption

| Component | Status | Notes |
|-----------|--------|-------|
| Per-column encryption | ✓ Reviewed | XChaCha20-Poly1305 with random nonces |
| AAD binding | ✓ Reviewed | table_name || row_id_LE || column_name |
| Filename privacy | ✓ Reviewed | Plaintext filename not found in raw DB bytes |

### Authentication

| Component | Status | Notes |
|-----------|--------|-------|
| Argon2id parameters | ✓ Reviewed | PIN: m=64 MiB, Password: m=256 MiB, t=3, p=1 |
| Attempt throttling | ✓ Reviewed | Persisted in SQLite, survives restart |
| Session zeroization | ✓ Reviewed | MVK zeroized on drop, lock, auto-lock |
| Session token | ✓ Reviewed | Rotates on unlock, invalidates on lock, constant-time compare |

### Streaming (vault://)

| Component | Status | Notes |
|-----------|--------|-------|
| Token validation | ✓ Reviewed | Constant-time comparison, session required |
| Mid-stream lock | ✓ Reviewed | All-or-nothing, abort on first auth failure |
| Range mapping | ✓ Reviewed | Chunk index mapping, per-chunk auth |

### Backup Container

| Component | Status | Notes |
|-----------|--------|-------|
| backup_key derivation | ✓ Reviewed | HKDF(MVK, "veylora/v1/backup-key") |
| Binding chain | ✓ Reviewed | header AAD → header AEAD → meta_len/meta_hash → manifest AEAD → trailer AEAD → blobs_hash → file_count |
| Same-MVK-only restore | ✓ Reviewed | Documented as intentional security property |
| Atomic restore | ✓ Reviewed | rusqlite transaction, all-or-nothing |

### Deferred Items (Accepted Risks)

| Item | Status | Rationale |
|------|--------|-----------|
| Windows Hello | Deferred to v1.1 | Complex TPM/DPAPI glue needs independent review |
| Auto-update | Implemented (v1.0.1) | `tauri-plugin-updater` verifies each update against the minisign pubkey pinned at build time; the private key lives only in GitHub Secrets + an offline backup — its leak would be a malware channel, so treat it like a code-signing key |
| Windows session-lock | Implemented (v1.0) | Event-driven `WM_WTSSESSION_CHANGE` (Win+L / RDP lock) locks the vault instantly; the OS unlock event never unlocks it |
| Browser E2E | Implemented (v1.0) | Packaged-app CDP smoke drives the real WebView2 in CI |

### Non-Goals (Confirmed Absent)

- No cloud upload
- No incremental backup
- No auto-backup scheduler
- No auto-backup reminders
- No analytics or telemetry

### Recommended Focus for External Audit

1. **HKDF label domain separation** — Verify no label is reused for two purposes
2. **Attempt throttling persistence** — Verify throttling survives restart and cannot be bypassed
3. **Session token rotation** — Verify token invalidation on lock and rotation on unlock
4. **Backup container binding chain** — Verify blobs_hash detects truncation and meta_len binds manifest
5. **Memory zeroization** — Verify MVK and derived keys are zeroized on drop in all code paths
