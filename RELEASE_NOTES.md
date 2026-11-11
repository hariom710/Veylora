# Veylora v1.0.0 Release Notes

## Release Date

2026-11-09

## What is Veylora?

Veylora is a free, open-source, local-first encrypted file vault for Windows.
It provides encryption-before-storage with a key hierarchy of Master Vault Key
(MVK) → HKDF subkeys → KEK → wraps MVK. Authentication is separate from
encryption. No plaintext is ever written to disk except when the user explicitly
initiates an export of a file to a location of their choice.

## What It Protects Against

- Offline attackers with access to vault files (all data encrypted at rest)
- Tampering with vault data (modified, reordered, duplicated, or truncated
  chunks are detected via AEAD authentication and root hash verification)
- Accidental plaintext leakage (no temp files, no preview caches, no OS
  thumbnails)
- Unauthorized access without valid PIN/password/recovery key

## What It Does NOT Protect Against

- Kernel-level malware or a compromised operating system
- Coerced disclosure of credentials
- Loss of all credentials (PIN + password + recovery key) — without at least
  one valid credential, the vault is permanently unrecoverable by design
- Compromised WebView2 or compromised Rust dependencies

## How to Install

1. Download `Veylora_1.0.0_x64-setup.exe` from the GitHub Releases page.
2. Verify the SHA-256 hash matches the value published below:

   ```
   SHA-256: 353ed801ab8c972864c3f73ad1f6c69617bac71ce703d3507c526f13c0014f8d
   ```

   Verify with:
   ```
   certutil -hashfile Veylora_1.0.0_x64-setup.exe SHA256
   ```

3. Run the installer. It installs to `%LOCALAPPDATA%\Veylora` (no admin
   privileges required).

## ⚠️ Important: Unsigned Installer

The v1.0.0 installer is **unsigned** because no code signing certificate was
available at release time. Windows will show a SmartScreen warning:

1. Windows protected your PC
2. Click "More info"
3. Click "Run anyway"

Before running, verify the SHA-256 hash of the downloaded installer matches
the value published in this release. This ensures the installer has not been
tampered with in transit.

## Known Limitations

- **Windows Hello**: Not available in v1.0. PIN, password, and recovery key
  are the supported unlock methods.
- **Auto-update**: Not available in v1.0. Check GitHub Releases for new
  versions and verify the SHA-256 before installing.
- **Windows session-lock**: Polling-based auto-lock on session lock is
  deferred to v1.1 due to Windows API limitations in the build environment.
- **Real icons**: Placeholder icons are used in v1.0. Replace with
  professional artwork before v1.1.
- **Browser E2E**: Not run in v1.0. Backend end-to-end tests substitute for
  browser E2E testing.
- **Recovery key loss**: If you lose all credentials (PIN, password, and
  recovery key), your data is permanently unrecoverable. Back up your
  recovery key in a safe place.

## Cryptographic Libraries

- argon2 (Argon2id key derivation)
- chacha20poly1305 (XChaCha20-Poly1305 AEAD)
- hkdf (HKDF-SHA256 key derivation)
- sha2 (SHA-256 hashing)
- subtle (constant-time comparison)
- zeroize (secure memory zeroization)
- getrandom (OS CSPRNG)

## Upgrade Notes

N/A — this is the first release.
