use subtle::ConstantTimeEq;
use std::sync::Mutex;

/// TokenStore owns the current session token and provides validation.
/// The token is a random 32-byte value hex-encoded (64 chars).
/// It is rotated on every successful unlock and invalidated on lock().
///
/// The store is owned by SessionManager and lives for the lifetime of the
/// vault session. It is not persisted to disk.
pub struct TokenStore {
    /// Current token hex string (64 chars = 32 bytes)
    token: Mutex<Option<String>>,
}

impl TokenStore {
    /// Create a new TokenStore with no token (vault initially locked).
    pub fn new() -> Self {
        Self { token: Mutex::new(None) }
    }

    /// Generate a new random 32-byte token, replacing any existing one.
    pub fn generate(&self) -> String {
        use getrandom::getrandom;
        let mut bytes = [0u8; 32];
        getrandom(&mut bytes).expect("getrandom failed for token generation");
        let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
        let mut guard = self.token.lock().unwrap();
        *guard = Some(hex.clone());
        hex
    }

    /// Validate a token candidate against the stored token using constant-time
    /// comparison via subtle::ConstantTimeEq. Returns true if they match.
    pub fn validate(&self, candidate: &str) -> bool {
        let guard = self.token.lock().unwrap();
        let stored = guard.as_ref();
        match stored {
            Some(stored_token) => stored_token.as_bytes().ct_eq(candidate.as_bytes()).into(),
            None => false,
        }
    }

    /// Invalidate the current token (call on lock()).
    pub fn invalidate(&self) {
        let mut guard = self.token.lock().unwrap();
        *guard = None;
    }

    /// Get the current token for rotation on unlock.
    pub fn current(&self) -> Option<String> {
        self.token.lock().unwrap().clone()
    }
}