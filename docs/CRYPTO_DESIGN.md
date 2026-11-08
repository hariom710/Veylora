# Veylora Crypto Design

## Key hierarchy

```
MVK (256-bit, OS CSPRNG, in-memory only)
  │ HKDF-SHA256 label "veylora/v1/vault-key"
  ├─► vault-key        (blob AEAD key)
  ├─► metadata-key     (metadata AEAD key)
  └─► file-key(file_id) per file → chunk keys / nonces
```

## How the MVK is protected (wrapped by KEKs)

| Method | KDF | Params | Wrap |
|---|---|---|---|
| PIN | Argon2id | m=64 MiB, t=3, p=1, salt=32B random | AEAD wrap of MVK |
| Password | Argon2id | m=256 MiB, t=3, p=1, salt=32B random | AEAD wrap of MVK |
| Recovery key | HKDF-SHA256 | label "veylora/v1/recovery-wrap" (no Argon2id) | AEAD wrap of MVK |
| Windows Hello (optional) | gates DPAPI/TPM-sealed KEK | — | — |

Each wrapped MVK copy stored in `vault.meta` with KDF type, params, salt, nonce, ciphertext. Params versioned so they can be raised later without re-encrypting blobs.

## HKDF domain labels (never reused)

- `veylora/v1/vault-key`
- `veylora/v1/metadata-key`
- `veylora/v1/recovery-wrap`
- `veylora/v1/file-key/<file_id>`
- `veylora/v1/backup-key`
- `veylora/v1/nonce` (chunk nonce derivation)

Single `derive_subkey` function requires a label argument; no label reused for two purposes.

## Memory hygiene

MVK and all derived keys live in `zeroize` buffers. Wiped on drop, on lock, on close.

## File format v1

Header (authenticated):
```
magic      "VYLR"        (4 bytes)
version    u32 = 1        (4 bytes)
file_id    [16] random    (16 bytes)
chunk_size u32 = 65536    (4 bytes)
total_chunks u64          (8 bytes)
total_length  u64         (8 bytes)
header_nonce [24]         (24 bytes)
header_tag   [16] AEAD    (16 bytes)
```

Chunk i:
```
nonce[24] ‖ ciphertext ‖ tag[16]
```

Finalize record:
```
finalize_nonce[24] ‖ AEAD(root_hash SHA-256 ‖ total_length) ‖ tag[16]
```

## Chunk AEAD

- Algorithm: XChaCha20-Poly1305
- Key: `file-key(file_id)`
- Nonce: HKDF(file-key, "nonce", chunk_index) → 24 bytes (deterministic, per-chunk-unique)
- AAD: magic ‖ version ‖ file_id ‖ chunk_index ‖ total_chunks ‖ total_length

## Nonce policy (chosen, frozen)

**HKDF(file-key, "nonce", chunk_index) → 24 bytes.** Deterministic, no random stored per file, zero collision risk, reproducible test vectors. One policy; never mixed with random prefixes.

## Range request mapping

```
first = start / chunk_size
last  = (end - 1) / chunk_size
```
Each chunk decrypted and authenticated independently; stream aborts on first auth failure.

## What the format detects

Modified chunks (AEAD tag), missing/duplicated/reordered chunks (chunk index in AAD), cross-file substitution (file_id in AAD), truncation (finalize record).

## Crate versions (resolved, Phase 1)

| Crate | Version | Role |
|---|---|---|
| argon2 | 0.5.3 | Argon2id KEK derivation |
| chacha20poly1305 | 0.11.0 | XChaCha20-Poly1305 AEAD (wrap/mvk, Phase 2 chunk/header/finalize AEAD) |
| getrandom | 0.2.17 | OS CSPRNG |
| hkdf | 0.12.4 | HKDF-SHA256 subkeys |
| sha2 | 0.10.9 | Recovery-key checksum |
| serde / serde_json | 1 | VaultMeta JSON serialization |
| thiserror | 1.0.69 | Typed CryptoError |
| zeroize | 1.9.1 | `#[derive(Zeroize, ZeroizeOnDrop)]` on key types |
| hybrid-array | 0.4 | AEAD nonce type (matches aead crate exactly) |

### Deliberate type choices

- `CryptoError::Argon2(String)` (not `#[from] argon2::Error`): `argon2::Error` in v0.5.3 does not implement `StdError`, so `thiserror #[from]` cannot be used. The error message is carried as a String.
- `AeadInPlace` (deprecated in `aead` 0.6): the stable, working API in aead 0.6.1. Migration to `AeadInOut` is a refactor, deferred to a later phase.

## VaultMeta serialization

`vault.meta` is a serde_json document with a `version` field (currently `1`). On deserialize, a version mismatch returns `UnsupportedVersion`. Each wrapped MVK entry stores: KDF type, KDF params, salt, nonce, and AEAD ciphertext. Each entry is individually AEAD-authenticated: the nonce and ciphertext are encrypted, while KDF type, params, and salt form the authenticated-associated-data (AAD). This prevents substitution of one wrapping entry for another and detects header/AAD tampering.

## Recovery key encoding

32 random bytes are encoded as base32 (alphabet `A-Z2-7`, no padding, 52 chars) followed by a 4-char hex checksum = first two bytes of SHA-256(key). Total 56 printable ASCII characters. The checksum rejects single-character mutations and transcription errors.

## File format v1 (FROZEN)

Format version `1`. Changes after this point require a `format_version` bump.

### On-disk layout

**Header** (written first, before chunks):

| Field | Size | Value |
|---|---|---|
| magic | 4 bytes | `"VYLR"` |
| format_version | 4 bytes | u32 LE = 1 |
| file_id | 16 bytes | random |
| chunk_size | 4 bytes | u32 LE = 65536 |
| total_chunks | 8 bytes | u64 LE |
| total_length | 8 bytes | u64 LE |
| header_nonce | 24 bytes | HKDF(file-key, "header-nonce") |
| header_ciphertext | N bytes | AEAD over header core |
| header_tag | 16 bytes | Poly1305 tag |

**Header core** (encrypted, 32 bytes): `file_id || chunk_size || total_chunks || total_length`

**Header AAD** (authenticated, not encrypted): `"VYLR" || format_version_LE(4 bytes)`

**Chunk i** (contiguous, after header):

| Field | Size | Value |
|---|---|---|
| nonce | 24 bytes | `HKDF(file-key, "nonce", chunk_index)` |
| ciphertext | plaintext_len bytes | XChaCha20-Poly1305(plaintext_i) |
| tag | 16 bytes | Poly1305 tag |

**Chunk AAD** (authenticated, not encrypted): `"VYLR" || format_version_LE || file_id || chunk_index_u64_LE || total_chunks_u64_LE || total_length_u64_LE`

All chunks except the last are exactly 65536 plaintext bytes. The last chunk may be short: `plaintext_len = total_length - (total_chunks - 1) * 65536`.

**Finalize record** (after last chunk):

| Field | Size | Value |
|---|---|---|
| nonce | 24 bytes | `HKDF(file-key, "finalize-nonce")` |
| ciphertext | N bytes | AEAD over `root_hash || total_length || total_chunks` |
| tag | 16 bytes | Poly1305 tag |

**Finalize AAD**: `"VYLR" || format_version_LE || file_id`

**Finalize plaintext** (encrypted): `root_hash[32] || total_length_u64_LE || total_chunks_u64_LE`

### Key derivation

- `file_key = HKDF(MVK, label = "veylora/v1/file-key/" || file_id_hex)` where `file_id_hex` is the lowercase hex encoding of the 16-byte `file_id`.
- Header nonce: `HKDF(file-key, "veylora/v1/header-nonce")` → 24 bytes.
- Chunk nonce i: `HKDF(file-key, "veylora/v1/nonce", chunk_index_le_bytes)` → 24 bytes.
- Finalize nonce: `HKDF(file-key, "veylora/v1/finalize-nonce")` → 24 bytes.

### root_hash

`root_hash = SHA-256( chunk0_ciphertext || chunk0_tag || chunk1_ciphertext || chunk1_tag || ... )`

Concatenated in chunk order. A reordering attack produces a different root_hash.

### Validation

A file is valid only if all of the following hold:
1. Header authenticates (header AEAD tag valid).
2. Every requested chunk authenticates (chunk AEAD tag valid).
3. Finalize record authenticates (finalize AEAD tag valid) AND `root_hash` matches the recomputed hash of all chunk ciphertexts||tags AND `total_length`/`total_chunks` match.

Partial validation is never reported as success.

### Metadata encryption (database columns)

Metadata columns (filenames, folder names, tags, timestamps) are encrypted with
XChaCha20-Poly1305 using `metadata-key` (derived from MVK via HKDF label
`veylora/v1/metadata-key`). Each encrypted column value is stored as
`nonce[24] || ciphertext`, where the nonce is 24 bytes of OS CSPRNG randomness
generated per column value (not derived via HKDF). The AAD for each encryption
is `table_name || row_id_LE(8 bytes) || column_name`, binding the ciphertext to
its specific table, row, and column.

### What the format detects

- Modified chunks (AEAD tag failure).
- Missing/duplicated/reordered chunks (chunk index in AAD + root_hash binding).
- Cross-file substitution (file_id in AAD).
- Truncation (finalize record + length binding).
- Header tampering (header AEAD + AAD over magic/version).

### HKDF domain labels (frozen)

| Label | Purpose |
|---|---|
| `veylora/v1/vault-key` | Blob AEAD key from MVK |
| `veylora/v1/metadata-key` | Metadata AEAD key from MVK |
| `veylora/v1/recovery-wrap` | Recovery KEK from recovery key |
| `veylora/v1/backup-key` | Backup container key from MVK |
| `veylora/v1/file-key/<file_id>` | Per-file key from MVK (file_id appended at call site) |
| `veylora/v1/nonce` | Chunk nonce (chunk index appended to info) |
| `veylora/v1/header-nonce` | Header nonce |
| `veylora/v1/finalize-nonce` | Finalize record nonce |

No label is reused for two purposes.

## Threat model

### In scope
- Casual local access (same Windows account, no PIN/password).
- Stolen/lost laptop (offline attacker with disk).
- Offline attacker with copied vault files.
- Tampering with vault data (modified/reordered/duplicated/truncated chunks).
- Accidental plaintext leakage via the app (no temp files, no thumbnail caches).

### Out of scope
- Kernel-level malware / compromised OS.
- Coerced disclosure.
- Weak user-chosen passwords.
- Loss of all credentials (PIN + password + recovery key → unrecoverable; by design).
- Forensic recovery on SSDs (wear leveling).
- Compromised WebView2 or compromised Rust dependency.

## Durability

All blob writes use the atomic write pattern:
1. Write to `blobs/.veylora-tmp-<random-hex>` inside the vault directory (same volume).
2. `sync_all` the temp file to flush data to disk.
3. `rename` over the final path `blobs/<file_id_hex>` (atomic on NTFS).
4. `fsync_dir` the parent directory to flush metadata.

### Temp file naming

The temp file path is `blobs/.veylora-tmp-<random-hex>` where `<random-hex>` is
128 bits of OS CSPRNG output (16 bytes, 32 hex characters). To avoid the
theoretical truncate-existing-file race, `write_blob` uses `OpenOptions`
with `create_new(true)`: if the temp path already exists, it regenerates the
suffix and retries up to 3 times before returning a typed error. The collision
probability is 2^-128 per attempt.

### File-level sync_all

`sync_all` is called on the temp file before `rename`. It always succeeds on
Windows (no special privilege required) and guarantees the encrypted blob
contents are durable before the rename makes them visible.

### Directory fsync on Windows

`fsync_dir` opens the directory handle with `FILE_FLAG_BACKUP_SEMANTICS` and
calls `FlushFileBuffers`. This requires the `SeBackupPrivilege`, which regular
users do not have. `FlushFileBuffers` returns `ERROR_ACCESS_DENIED` (5) in
that case. The function returns `Ok(())` for `ERROR_ACCESS_DENIED` — NTFS
transactional journaling guarantees rename atomicity and metadata consistency
even without directory flush. Any other error code from `FlushFileBuffers` is
returned as an `io::Error` so unexpected failure modes surface rather than
being silently absorbed.

## Threat model

### In scope
