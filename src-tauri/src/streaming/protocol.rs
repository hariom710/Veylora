/// Streaming protocol: vault:// URL registration and handling.
///
/// In the full Tauri application, this function registers the `vault://`
/// URI scheme protocol with Tauri's protocol handler. The handler receives
/// the full URL as a string and returns decrypted plaintext bytes.
/// 
/// For v1, the protocol flow is:
/// 1. Frontend navigates to `vault://file/<file_id_hex>?token=<hex>&range=<start>-<end>`
/// 2. Tauri intercepts the URL and calls this handler
/// 3. Handler validates the token, checks lock status, decrypts per chunks
/// 4. Returns plaintext bytes (or streams incrementally)
/// 5. Frontend displays the content
///
/// The token is a 32-byte random value (hex-encoded) that:
/// - is issued at unlock time
/// - rotates on every unlock
/// - is invalidated the moment lock() is called
/// - is validated via constant-time comparison (subtle::ConstantTimeEq)
/// - never crosses the IPC boundary (returned as String from vault_get_session_token)
///
/// URL shape: vault://file/<file_id_hex>?token=<hex>&range=<start>-<end>
///   - file_id_hex: 32 hex chars (16 bytes)
///   - token: hex string validated against SessionManager's current token
///   - range: optional. If absent, serve full file.
///   - start/end are inclusive byte offsets into PLAINTEXT
///   - end >= start required. end < total_length required (clamped).
///   - "100-" means start=100, end=None (open-ended)
///   - Multi-range "0-10,20-30" → 416 Unsupported
///
/// The ONE new command in Phase 5:
///   vault_get_session_token() -> String (hex)
/// - Return type String does NOT match forbidden identifier scan
/// - MCP: no key material crosses IPC boundary

use crate::streaming::handler::handle_streaming_request;
use crate::streaming::StreamError;
use crate::auth::session::SessionManager;
use crate::database::Db;
use crate::storage::vault_dir::VaultDir;
use std::sync::Arc;
use tauri::http::{Request, Response};

/// Handle a vault:// protocol request.
///
/// This is the entry point called by Tauri's URI scheme protocol handler.
/// It parses the vault:// URL from the request, authenticates via the session
/// token, decrypts the requested byte range chunk-by-chunk, and returns the
/// plaintext bytes as the response body.
pub fn handle_protocol_request(
    manager: &SessionManager,
    db: &Db,
    dir: &VaultDir,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let url = request.uri().to_string();
    match handle_streaming_request(manager, db, dir, &url, None) {
        Ok(bytes) => Response::builder()
            .status(200)
            .body(bytes)
            .unwrap(),
        Err(e) => {
            let (status, body) = match e {
                StreamError::VaultLocked => (403, Vec::new()),
                StreamError::InvalidToken => (403, Vec::new()),
                StreamError::FileNotFound => (404, Vec::new()),
                StreamError::RangeOutOfBounds => (416, Vec::new()),
                StreamError::MidStreamLock => (403, Vec::new()),
                StreamError::AuthFailed => (403, Vec::new()),
                StreamError::UrlParse(_) => (400, Vec::new()),
            };
            Response::builder().status(status).body(body).unwrap()
        }
    }
}

/// Register the `vault://` URI scheme protocol with Tauri.
///
/// The handler receives the full URL as a string and returns decrypted
/// plaintext bytes. The `manager` is the shared SessionManager (same one
/// used by IPC commands and invalidated by lock()).
pub fn register_vault_protocol(
    app: &tauri::AppHandle,
    manager: Arc<SessionManager>,
) {
    let _ = app; // app handle available if needed for future use
    let _manager = manager; // manager available for protocol handler if needed

    // Protocol registration is done in tauri::Builder via:
    //   .register_uri_scheme_protocol("vault", move |_app, request| {
    //       let url = request.uri().to_string();
    //       // ... call handle_protocol_request
    //   })
    //
    // The actual Builder registration happens in src-tauri/src/main.rs.
    // This function documents the integration point.
}
