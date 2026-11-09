//! Streaming module: vault:// protocol, URL parsing, token management,
//! per-chunk decryption, and request handling.
/*!
 * No key material leaves this module. The Session holds the MVK in a
 * zeroizing buffer; request handlers borrow it via with_mvk() closures.
 */

pub mod url;
pub mod token;
pub mod handler;
pub mod protocol;

pub use url::parse_url;
pub use handler::handle_streaming_request;

/// Errors that can occur during streaming request handling.
#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("vault locked: no active session")]
    VaultLocked,
    #[error("invalid URL: {0}")]
    UrlParse(String),
    #[error("invalid token")]
    InvalidToken,
    #[error("file not found")]
    FileNotFound,
    #[error("range request out of bounds")]
    RangeOutOfBounds,
    #[error("authentication failed on chunk")]
    AuthFailed,
    #[error("mid-stream lock: chunk not sent")]
    MidStreamLock,
}

/// Parsed components of a vault:// URL.
#[derive(Debug, Clone)]
pub struct StreamRequest {
    pub file_id: [u8; 32],
    pub token: String,
    pub range_start: u64,
    pub range_end: Option<u64>,
    pub is_head: bool,
}

/// hex-encode a [u8; 32] into 64 hex chars.
pub fn hex_encode(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}