use crate::streaming::{StreamError, StreamRequest};

/// Parse a vault:// URL into a StreamRequest.
///
/// Expected shape: vault://file/<file_id_hex>?token=<hex>&range=<start>-<end>
///   - file_id_hex: exactly 64 hex characters (32 bytes)
///   - token: any hex string (validated against SessionManager)
///   - range: optional. If absent, serve full file.
///   - start/inclusive, end/inclusive byte offsets into PLAINTEXT.
///   - end >= start required. If end >= total_length, clamped to total_length-1.
///   - "100-" means start=100, end=None (open-ended).
///   - Multi-range "0-10,20-30" is REJECTED (returns UrlParse error).
///
/// Returns a StreamRequest.
pub fn parse_url(url: &str) -> Result<StreamRequest, StreamError> {
    // Strip "vault://" prefix
    let url = url.trim();
    if !url.starts_with("vault://") {
        return Err(StreamError::UrlParse("missing vault:// prefix".to_string()));
    }
    let rest = &url["vault://".len()..];

    // Expect "file/<file_id_hex>?token=<hex>&range=..."
    let mut parts = rest.splitn(2, '?');
    let path_part = parts.next().ok_or(StreamError::UrlParse(
        "missing '?' separator in vault URL".to_string(),
    ))?;

    let query_part = parts.next().unwrap_or("");

    // Parse path: "file/<file_id_hex>"
    let path_parts: Vec<&str> = path_part.splitn(2, '/').collect();
    if path_parts.len() != 2 || path_parts[0] != "file" {
        return Err(StreamError::UrlParse(format!(
            "expected 'file/<hex>' path, got: {}",
            path_part
        )));
    }

    let file_id_hex = path_parts[1];
    if file_id_hex.len() != 64 {
        return Err(StreamError::UrlParse(format!(
            "file_id_hex must be 64 hex chars (32 bytes), got {}: {}",
            file_id_hex.len(),
            file_id_hex
        )));
    }

    // Parse file_id into [u8; 32]
    let file_id = parse_hex_32(file_id_hex).map_err(|_| {
        StreamError::UrlParse(format!(
            "file_id_hex must be valid hex, got: {}",
            file_id_hex
        ))
    })?;

    // Parse query: token=<hex>&range=...
    let mut token = String::new();
    let mut range_start: Option<u64> = None;
    let mut range_end: Option<u64> = None;
    let mut is_head = false;

    for qpart in query_part.split('&') {
        let mut kv = qpart.splitn(2, '=');
        let key = kv.next().unwrap_or("");
        let value = kv.next().unwrap_or("");

        match key {
            "token" => {
                token = value.to_string();
            }
            "range" => {
                // Parse range "start-end" or "start-"
                let mut re = value.splitn(2, '-');
                let start_str = re.next().unwrap_or("").trim();
                let end_str = re.next();

                if start_str.is_empty() {
                    return Err(StreamError::UrlParse(
                        "range start must not be empty".to_string(),
                    ));
                }

                let start = start_str.parse::<u64>().map_err(|_| {
                    StreamError::UrlParse(format!("range start must be u64, got: {}", start_str))
                })?;

                if let Some(es) = end_str {
                    if es.is_empty() {
                        range_end = None; // open-ended "100-"
                    } else {
                        let end = es.parse::<u64>().map_err(|_| {
                            StreamError::UrlParse(format!("range end must be u64, got: {}", es))
                        })?;
                        if end < start {
                            return Err(StreamError::UrlParse(format!(
                                "range end ({}) must be >= start ({})",
                                es, start_str
                            )));
                        }
                        range_end = Some(end);
                    }
                } else {
                    range_end = None; // open-ended
                }
                range_start = Some(start);
            }
            "head" => {
                is_head = value.parse::<bool>().unwrap_or(false);
            }
            _ => {}
        }
    }

    if token.is_empty() {
        return Err(StreamError::UrlParse(
            "missing 'token' query parameter".to_string(),
        ));
    }

    Ok(StreamRequest {
        file_id,
        token,
        range_start: range_start.unwrap_or(0),
        range_end,
        is_head,
    })
}

/// Parse exactly 64 hex characters into [u8; 32].
fn parse_hex_32(s: &str) -> Result<[u8; 32], StreamError> {
    if s.len() != 64 {
        return Err(StreamError::UrlParse(format!(
            "file_id_hex must be 64 hex chars (32 bytes), got {}",
            s.len()
        )));
    }
    let mut b = [0u8; 32];
    for (i, byte) in b.iter_mut().enumerate() {
        let high = hex_char_to_nibble(s.chars().nth(2 * i).unwrap_or('0'))?;
        let low = hex_char_to_nibble(s.chars().nth(2 * i + 1).unwrap_or('0'))?;
        *byte = (high << 4) | low;
    }
    Ok(b)
}

/// Convert a hex char to its nibble value (0-15).
fn hex_char_to_nibble(c: char) -> Result<u8, StreamError> {
    match c {
        '0'..='9' => Ok(c as u8 - b'0'),
        'a'..='f' => Ok(c as u8 - b'a' + 10),
        'A'..='F' => Ok(c as u8 - b'A' + 10),
        _ => Err(StreamError::UrlParse(format!("invalid hex char: '{c}'"))),
    }
}
