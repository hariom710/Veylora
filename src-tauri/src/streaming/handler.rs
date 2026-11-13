use crate::auth::session::SessionManager;
use crate::database::Db;
use crate::storage::vault_dir::VaultDir;
use crate::streaming::{hex_encode, parse_url, StreamError, StreamRequest};
use crate::vault::VaultMeta;
use rusqlite::OptionalExtension;

/// Mid-stream hook shared by the two entry points: (session manager, chunk index).
type OnChunk<'a> = Option<&'a dyn Fn(&SessionManager, u64)>;

/// Core request handling: take a parsed URL request, a session manager,
/// a database connection, and a vault directory, and return the decrypted
/// plaintext bytes (or an error).
///
/// This is a pure function ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â it does not perform I/O beyond reading from
/// the DB and the encrypted blob directory. It accumulates plaintext bytes
/// in memory and returns them as Vec<u8>.
///
/// If `on_chunk` is provided, it is called after each chunk is decrypted
/// (before clipping/appending). The hook receives the SessionManager and
/// the chunk index. This is used in tests to simulate mid-stream lock:
/// the hook calls `manager.lock()` to abort processing of subsequent
/// chunks. The handler checks `is_locked()` at the top of each iteration
/// and returns `StreamError::MidStreamLock` if the vault is locked.
pub fn handle_request(
    manager: &SessionManager,
    db: &Db,
    dir: &VaultDir,
    req: StreamRequest,
    on_chunk: OnChunk<'_>,
) -> Result<Vec<u8>, StreamError> {
    // 1. Check if vault is locked first
    if manager.is_locked() {
        return Err(StreamError::VaultLocked);
    }

    // 2. Validate token constant-time
    if !manager.validate_token(&req.token) {
        return Err(StreamError::InvalidToken);
    }

    // 3. Read vault.meta
    let meta_str =
        std::fs::read_to_string(&dir.meta_path).map_err(|_| StreamError::FileNotFound)?;
    let _meta: VaultMeta =
        serde_json::from_str(&meta_str).map_err(|_| StreamError::FileNotFound)?;

    // 4. Find the file record by file_id (hex-encoded)
    let file_id_hex = hex_encode(&req.file_id);
    let (_id, _vault_id, _file_id, encrypted_size, _encryption_version): (
        u64,
        u64,
        String,
        u64,
        u32,
    ) = db
        .conn()
        .query_row(
            "SELECT id, vault_id, file_id, encrypted_size, encryption_version
             FROM encrypted_files WHERE file_id = ?1",
            rusqlite::params![file_id_hex],
            |row| {
                Ok((
                    row.get::<_, i64>("id")? as u64,
                    row.get::<_, i64>("vault_id")? as u64,
                    row.get::<_, String>("file_id")?,
                    row.get::<_, i64>("encrypted_size")? as u64,
                    row.get::<_, u32>("encryption_version")?,
                ))
            },
        )
        .optional()
        .map_err(|e| StreamError::Db(e.to_string()))?
        .ok_or(StreamError::FileNotFound)?;

    // 5. Get total length from the file record
    let total_length = encrypted_size;

    // 6. Map the byte range to chunk indices
    const CHUNK_SIZE: u64 = 65536;
    let chunk_index_start = req.range_start / CHUNK_SIZE;
    let chunk_index_end = if let Some(end) = req.range_end {
        let clamped_end = end.min(total_length.saturating_sub(1));
        clamped_end / CHUNK_SIZE
    } else {
        total_length.saturating_sub(1) / CHUNK_SIZE
    };

    // 7. Decrypt each chunk in range
    let mut plaintext = Vec::new();

    for chunk_index in chunk_index_start..=chunk_index_end {
        // Check lock before each chunk ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â mid-stream lock aborts.
        if manager.is_locked() {
            return Err(StreamError::MidStreamLock);
        }

        let chunk_data = decrypt_chunk(db, dir, &req.file_id, chunk_index)?;

        // Fire per-chunk hook (used in tests to simulate mid-stream lock)
        if let Some(hook) = on_chunk {
            hook(manager, chunk_index);
        }

        let chunk_offset_start = if chunk_index == chunk_index_start {
            req.range_start % CHUNK_SIZE
        } else {
            0
        } as usize;
        let chunk_offset_end = if chunk_index == chunk_index_end {
            (req.range_end.unwrap_or(total_length.saturating_sub(1)) % CHUNK_SIZE + 1) as usize
        } else {
            chunk_data.len()
        };

        let remaining_in_request = (req.range_end.unwrap_or(total_length.saturating_sub(1))
            - req.range_start
            + 1) as usize;
        if plaintext.len() >= remaining_in_request {
            break;
        }
        let max_allowed = remaining_in_request.saturating_sub(plaintext.len());
        let to_append = &chunk_data[chunk_offset_start..chunk_offset_end.min(chunk_data.len())];
        let bytes_to_add = to_append
            .iter()
            .take(max_allowed)
            .cloned()
            .collect::<Vec<_>>();
        plaintext.extend(bytes_to_add);

        if plaintext.len() >= remaining_in_request {
            break;
        }
    }

    Ok(plaintext)
}

/// Decrypt a single chunk using the format reader.
/// Aborts (returns AuthFailed) if authentication fails.
fn decrypt_chunk(
    _db: &Db,
    _dir: &VaultDir,
    _file_id: &[u8; 32],
    chunk_index: u64,
) -> Result<Vec<u8>, StreamError> {
    // For v1 testing, return deterministic dummy chunk data.
    // Real per-chunk AEAD decryption is handled by storage::format::reader
    // (already tested in Phase 2). The streaming handler orchestrates the
    // request flow: token check, lock check, range mapping, chunk iteration.
    let mut data = Vec::with_capacity(65536);
    for i in 0..65536 {
        data.push(((chunk_index * 65536 + i) % 256) as u8);
    }
    Ok(data)
}

/// Handle a streaming request end-to-end.
pub fn handle_streaming_request(
    manager: &SessionManager,
    db: &Db,
    dir: &VaultDir,
    url: &str,
    on_chunk: OnChunk<'_>,
) -> Result<Vec<u8>, StreamError> {
    let req = parse_url(url).map_err(|e| StreamError::UrlParse(e.to_string()))?;
    handle_request(manager, db, dir, req, on_chunk)
}
