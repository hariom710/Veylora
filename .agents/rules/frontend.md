---
trigger: model_decision
description: Frontend UI rules, React/TypeScript architecture, and secure Tauri IPC communication.
---
# Frontend & Tauri IPC Rules

## Architecture & Security Boundary
- Frontend code (React + TypeScript) is an untrusted presentation layer running in WebView2.
- Never implement cryptographic algorithms or store unencrypted keys in frontend memory, localStorage, or state.
- Communicate with the backend strictly through typed Tauri commands (`invoke`) and the custom `vault://` protocol for streaming decrypted bytes.
- Do not cache decrypted binary data or files in local browser storage or disk caches.

## Token-Efficient Frontend Workflows
- Modify only the relevant component, hook, or state slice.
- Reuse existing design tokens, components, and utility functions instead of introducing duplicate helpers or styling libraries.
- Avoid scanning all frontend components when fixing a localized UI issue.
- Verify TypeScript types locally with `tsc --noEmit` on changed files when needed.

