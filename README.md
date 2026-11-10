# Veylora

<p align="center">
  <strong>Your Files. Your Privacy. Your Control.</strong><br>
  A high-security, zero-knowledge local encrypted file vault built with <strong>Tauri 2</strong>, <strong>Rust</strong>, and <strong>React</strong>.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Security-ChaCha20--Poly1305-blue?style=flat-square" alt="Encryption">
  <img src="https://img.shields.io/badge/KDF-Argon2id-blueviolet?style=flat-square" alt="KDF">
  <img src="https://img.shields.io/badge/Backend-Rust%20%2F%20Tauri%202-orange?style=flat-square" alt="Backend">
  <img src="https://img.shields.io/badge/Frontend-React%2018%20%2B%20TypeScript-61dafb?style=flat-square" alt="Frontend">
  <img src="https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-green?style=flat-square" alt="License">
</p>

---

## Overview

**Veylora** is a desktop personal vault application engineered for maximum data privacy and resilience. It provides tamper-proof local file encryption, in-memory chunk-based file streaming (PDFs, Videos, Images), atomic crash-safe writes, and robust backup verification without ever exposing plaintext keys to disk or the frontend.

### Core Guarantee
> **Zero Plaintext on Disk**: Plaintext file contents never touch persistent disk storage outside of an explicit, user-initiated export. Decrypted bytes are streamed exclusively in memory through ephemeral sessions via the custom `vault://` protocol.

---

## Key Features

- **Three-Layer Security Model**:
  - **Layer 1 (App Access)**: Local user accounts (Admin/User) stored in `app_users` for app entry.
  - **Layer 2 (Vault Selection)**: Choose, create, or switch between distinct encrypted vaults.
  - **Layer 3 (Vault Encryption)**: Independent encryption keys derived via PIN, Master Password, or One-Time Recovery Key.
- **Cryptographic Primitives**:
  - **AEAD Encryption**: ChaCha20-Poly1305 for file chunk blobs and database metadata.
  - **Key Derivation**: Argon2id with memory-hard parameters for PIN and Password KEKs.
  - **Key Hierarchy**: Master Vault Key (MVK) wrapped using 3 separate KEKs (PIN, Password, Recovery).
  - **Key Separation**: HKDF-SHA256 for context-separated chunk nonces and derivation labels.
  - **Memory Safety**: `Zeroize` and `ZeroizeOnDrop` ensure all sensitive key material is wiped from RAM immediately on vault lock.
- **Protected In-Memory Streaming (`vault://`)**:
  - Direct, range-capable byte streaming for media playback.
  - Built-in PDF reader, HTML5 Video player (with seeking), and full Image viewer.
  - **Mid-stream lock**: Instantly terminates active streams with HTTP 403 when the vault is locked.
- **Atomic File System Storage**:
  - Chunks written to temporary files (`.veylora-tmp-*`) with `fsync` before atomic renaming on NTFS/ext4.
  - Encrypted SQLite metadata database running in WAL mode with `synchronous=FULL`.
- **Encrypted Backup & Recovery**:
  - Encrypted self-contained backup containers.
  - Cryptographic integrity verification without decrypting file contents.
  - Atomic restoration with overwrite conflict resolution.
- **Account & Security Management**:
  - Configurable auto-lock timers (1, 5, 15, 30 minutes).
  - Brute-force throttling with progressive exponential backoff.
  - One-time recovery key display upon vault creation.

---

## Architecture & Trust Boundaries

```
┌─────────────────────────────────────────────────────────────────┐
│ React + TypeScript Presentation Layer (WebView2)                │
│ - Screens: Login, Welcome, Unlock, Dashboard, FileManager       │
│ - Viewers: PDF (PDF.js), Video (HTML5 Range), Image Viewer      │
│ - Talks ONLY via Tauri IPC commands and vault:// protocol       │
└────────────────────────────┬────────────────────────────────────┘
                             │ IPC (no raw keys) / vault:// (in-memory ranges)
┌────────────────────────────▼────────────────────────────────────┐
│ Rust Backend Core (Trust Boundary)                              │
│ ├── auth/        - PIN, Password, Sessions, Auto-lock, Throttling│
│ ├── crypto/      - MVK, KEK, HKDF, AEAD, Zeroize safety          │
│ ├── database/    - SQLite in WAL mode; AEAD-encrypted columns    │
│ ├── storage/     - Chunked AEAD ciphertext blobs, atomic rename  │
│ ├── streaming/   - vault:// protocol, token rotation, auth checks│
│ └── backup/      - Encrypted container creation, verify, restore │
└────────────────────────────┬────────────────────────────────────┘
                             │ Atomic Writes & Encrypted Storage
┌────────────────────────────▼────────────────────────────────────┐
│ Encrypted Vault on Disk                                         │
│ ├── vault.meta   - Argon2id salt, KDF params, wrapped MVKs      │
│ ├── vault.db     - SQLite DB (sensitive metadata AEAD encrypted) │
│ └── blobs/<id>   - Chunked AEAD ciphertext per file              │
└─────────────────────────────────────────────────────────────────┘
```

---

## User Journey & Screen Flow

1. **Login & Signup (`Layer 1`)**:
   - Initial cold-start login.
   - Seeded default administrator: `b1865741@gmail.com` / `Harry@12345`.
2. **Welcome Screen (`Layer 2`)**:
   - Create a brand new vault or open an existing vault path.
   - Admin password change warning banner if running default credentials.
3. **Vault Creation & Recovery Key**:
   - Choose vault path, numeric PIN (4–6 digits), and Master Password (min 8 chars).
   - Generates a single-view, monospace **Recovery Key** (`VLR-XXXX-...`).
4. **Vault Unlock (`Layer 3`)**:
   - Unlock with PIN keypad, Password, or Recovery Key to unwrap MVK into RAM.
5. **Main Application**:
   - **Dashboard**: Storage statistics, quick import, recent items, and favorites.
   - **File Manager**: Folder organization, search, drag-and-drop import, and safe export.
   - **Viewers**: Embedded PDF, video, and image previewers.
   - **Backup**: Create, verify, and restore backup archives.
   - **Settings & Account**: Credentials rotation, auto-lock timeout, user management.

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| <kbd>Ctrl</kbd> + <kbd>L</kbd> | Lock Vault immediately |
| <kbd>Ctrl</kbd> + <kbd>O</kbd> | Navigate to File Manager & trigger Import dialog |
| <kbd>Ctrl</kbd> + <kbd>F</kbd> | Navigate to File Manager & focus Search bar |
| <kbd>Ctrl</kbd> + <kbd>,</kbd> | Open Settings |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>A</kbd> | Open Account management |
| <kbd>Ctrl</kbd> + <kbd>1</kbd> .. <kbd>4</kbd> | Quick switch between Dashboard, Files, Backup, Settings |
| <kbd>Esc</kbd> | Close viewer / modal / exit fullscreen |

---

## Development & Build Setup

### Prerequisites

- **Rust toolchain**: `rustc` and `cargo` (1.80+ or 1.99+)
- **Node.js**: `v20+` or `v22+` with `pnpm`
- **Windows Build Tools** (for Windows): Visual Studio MSVC toolchain with C++ build tools and Windows 10/11 SDK.

### 1. Install Frontend Dependencies
```bash
pnpm install
```

### 2. Run in Development Mode
To start both the Vite development server and the Tauri desktop window:
```bash
pnpm tauri dev
```

### 3. Run Frontend Tests
```bash
pnpm test
```
*(48 unit and integration tests covering auth gates, viewers, token rotation, and UI states)*

### 4. Run Rust Backend Tests
On Windows with MSVC:
```bash
build.bat test
```
Or directly via Cargo:
```bash
cd src-tauri
cargo test
```
*(161 Rust tests verifying cryptographic invariants, zeroization, atomic storage, and protocol handlers)*

### 5. Build Desktop Installer
```bash
pnpm build
cd src-tauri
cargo tauri build
```
The output installer will be packaged in `src-tauri/target/release/bundle/nsis/`.

---

## Security Invariants

1. **Zero Raw Keys in Frontend**: Tauri commands never return raw MVK or KEK key material.
2. **Zeroize on Drop**: All key containers (`Mvk`, `Kek`, session tokens) implement or derive `Zeroize` / `ZeroizeOnDrop`.
3. **Constant-Time Comparisons**: Token checks and authentication validations utilize constant-time equality comparisons (`subtle::ConstantTimeEq`).
4. **Crash Durability**: Every write follows the sequence: `write .tmp` → `fsync` → `atomic rename` → `fsync parent dir`.
5. **No Network Telemetry**: Veylora is fully offline. No analytics, tracking, or network telemetry calls are made.

---

## License

This project is licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](http://www.apache.org/licenses/LICENSE-2.0))
- MIT License ([LICENSE-MIT](http://opensource.org/licenses/MIT))

at your option.
