use crate::auth::session::SessionManager;
use crate::crypto::mvk::Mvk;
use crate::database::Db;
use crate::storage::format::{read_header, read_range, FileKey};
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

    // 4. Row-existence gate: never serve an orphan blob (a crash between
    // write_blob and insert_file can leave one behind).
    let file_id_hex = hex_encode(&req.file_id);
    db.conn()
        .query_row(
            "SELECT id FROM encrypted_files WHERE file_id = ?1",
            rusqlite::params![file_id_hex],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| StreamError::Db(e.to_string()))?
        .ok_or(StreamError::FileNotFound)?;

    // 5. Load the encrypted blob and derive its file key. Key material stays
    // inside the closure (FileKey zeroizes on drop).
    let (blob, file_key) = manager
        .with_mvk(|mvk| -> Result<_, StreamError> {
            let file_key = FileKey::derive(&Mvk::from_array(*mvk), &req.file_id)
                .map_err(|e| StreamError::Db(e.to_string()))?;
            let blob = std::fs::read(dir.blob_path(&req.file_id))
                .map_err(|_| StreamError::FileNotFound)?;
            Ok((blob, file_key))
        })
        .map_err(|_| StreamError::VaultLocked)??;

    // 6. Plaintext bounds come from the format header — the DB's
    // encrypted_size counts chunk overhead and is not a plaintext length.
    let (header, _) = read_header(&blob, &file_key).map_err(|_| StreamError::AuthFailed)?;
    let total_length = header.total_length;
    if total_length == 0 {
        return Ok(Vec::new());
    }
    if req.range_start >= total_length {
        return Err(StreamError::RangeOutOfBounds);
    }

    // 7. Clamp the requested range (inclusive) to the file.
    let range_start = req.range_start;
    let range_end = req
        .range_end
        .unwrap_or(total_length - 1)
        .min(total_length - 1);

    // 8. Map the byte range to chunk indices
    const CHUNK_SIZE: u64 = 65536;
    let chunk_index_start = range_start / CHUNK_SIZE;
    let chunk_index_end = range_end / CHUNK_SIZE;

    // 9. Decrypt each chunk in range. read_range authenticates every chunk
    // it touches; any failure aborts with AuthFailed (no unauthenticated
    // bytes are ever returned).
    let mut plaintext = Vec::new();

    for chunk_index in chunk_index_start..=chunk_index_end {
        // Check lock before each chunk — mid-stream lock aborts.
        if manager.is_locked() {
            return Err(StreamError::MidStreamLock);
        }

        let chunk_start = chunk_index * CHUNK_SIZE;
        let chunk_end = (chunk_start + CHUNK_SIZE).min(total_length);
        let chunk_data = read_range(&blob, chunk_start, chunk_end, &file_key)
            .map_err(|_| StreamError::AuthFailed)?;

        // Fire per-chunk hook (used in tests to simulate mid-stream lock)
        if let Some(hook) = on_chunk {
            hook(manager, chunk_index);
        }

        let chunk_offset_start = if chunk_index == chunk_index_start {
            (range_start % CHUNK_SIZE) as usize
        } else {
            0
        };
        let chunk_offset_end = if chunk_index == chunk_index_end {
            (range_end % CHUNK_SIZE + 1) as usize
        } else {
            chunk_data.len()
        };

        // The clamped range maps exactly onto these slices: they tile
        // [range_start, range_end] with no gap or overlap.
        plaintext.extend_from_slice(&chunk_data[chunk_offset_start..chunk_offset_end]);
    }

    Ok(plaintext)
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
