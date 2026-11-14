# Veylora Release Notes

## v1.0.1 (2026-11-13)

- **Auto-update**: Settings → About → **Check for updates** now downloads a
  newer release from GitHub Releases, verifies its minisign signature against
  the key embedded in the app, installs it, and relaunches. v1.0.0 and older
  builds carry no updater — install v1.0.1 manually once; every release from
  v1.0.1 on is self-updating.
- **Installer**: ships a `.sig` updater signature and a `latest.json`
  manifest alongside the usual `.sha256` sidecar (the updater's source of
  truth for "is there a newer version").
- **Explorer icon cache**: the app broadcasts an icon-change notification on
  startup, so Windows re-reads the executable icon without a manual cache
  wipe.
- **No default credentials**: the hardcoded first-run admin account and its
  default password/PIN constants are gone from the app. A fresh install has
  zero accounts — the first sign-up becomes the admin, later sign-ups are
  regular users. (Existing installs keep their accounts unchanged.)
- **Lock Vault returns to the vault picker**: locking now lands on the
  Create / Open Existing Vault screen instead of the PIN keypad — reopen
  the same vault (or pick a different one), then unlock it.
- Docs/toolchain numbers refreshed (Rust 1.100, 169 Rust + 72 frontend
  tests).

---

# Veylora v1.0.0 Release Notes

## Release Date

2026-11-13

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
2. Verify the SHA-256 of the file you downloaded. The authoritative value is
   printed in the GitHub Release description and shipped next to the
   installer as `Veylora_1.0.0_x64-setup.exe.sha256` (each release asset is
   built by CI, so its hash differs from any local build):

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
- **Auto-update**: Not available in v1.0.0 (added in v1.0.1 — check GitHub
  Releases for new versions and verify the SHA-256 before installing).
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
