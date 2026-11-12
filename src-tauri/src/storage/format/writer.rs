//! Streaming writer for format v1.
//!
//! Reads plaintext from `reader`, encrypts chunk-by-chunk, writes the
//! complete on-disk format (header || chunks || finalize) to `dest`.

use crate::crypto::mvk::Mvk;
use crate::storage::format::{
    chunk, finalize, header, FileKey, FormatError, Result, CHUNK_SIZE, NONCE_LEN,
};
use std::io::{Read, Write};

/// Stream-encrypt `reader` into `dest` in format v1 layout.
///
/// Returns the number of bytes written to `dest`.
pub fn write_file(
    reader: &mut dyn Read,
    dest: &mut dyn Write,
    file_key: &FileKey,
    file_id: [u8; 16],
) -> Result<u64> {
    // Phase 1: read all input into chunks (we need totals before writing header).
    let mut chunks: Vec<Vec<u8>> = Vec::new();
    let mut total_length: u64 = 0;
    let mut tmp = [0u8; CHUNK_SIZE as usize];
    loop {
        let n = reader
            .read(&mut tmp)
            .map_err(|e| FormatError::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        chunks.push(tmp[..n].to_vec());
        total_length += n as u64;
    }
    let total_chunks = chunks.len() as u64;

    let hdr = header::Header::new(file_id, total_chunks, total_length);
    let mut written: u64 = 0;

    // Write header.
    written += header::write_header(&hdr, file_key, dest)
        .map_err(|e| FormatError::Io(e.to_string()))? as u64;

    // Write chunks, collecting ciphertext||tag for root_hash.
    let mut chunk_ct_tags: Vec<Vec<u8>> = Vec::with_capacity(chunks.len());
    for (i, plaintext) in chunks.iter().enumerate() {
        let enc = chunk::encrypt_chunk(
            file_key,
            &file_id,
            i as u64,
            total_chunks,
            total_length,
            plaintext,
        )?;
        let nonce = chunk::chunk_nonce(file_key, i as u64);
        dest.write_all(&nonce)
            .map_err(|e| FormatError::Io(e.to_string()))?;
        dest.write_all(&enc)
            .map_err(|e| FormatError::Io(e.to_string()))?;
        written += (NONCE_LEN + enc.len()) as u64;
        chunk_ct_tags.push(enc);
    }

    // Write finalize record.
    let root_hash = finalize::compute_root_hash(&chunk_ct_tags);
    let fin =
        finalize::encrypt_finalize(file_key, &file_id, total_chunks, total_length, &root_hash);
    dest.write_all(&fin)
        .map_err(|e| FormatError::Io(e.to_string()))?;
    written += fin.len() as u64;

    Ok(written)
}

/// Convenience: encrypt an in-memory byte slice into a new Vec<u8>.
pub fn write_file_bytes(
    plaintext: &[u8],
    file_key: &FileKey,
    file_id: [u8; 16],
) -> Result<Vec<u8>> {
    let mut dest: Vec<u8> = Vec::new();
    let mut reader: &[u8] = plaintext;
    write_file(&mut reader, &mut dest, file_key, file_id)?;
    Ok(dest)
}

/// Derive a FileKey from an MVK and file_id.
pub fn derive_file_key(mvk: &Mvk, file_id: [u8; 16]) -> Result<FileKey> {
    FileKey::derive(mvk, &file_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::mvk::Mvk;

    #[test]
    fn write_file_empty() {
        let mvk = Mvk::generate().expect("mvk");
        let file_id = [0x42u8; 16];
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive");

        let mut out = Vec::new();
        let mut reader: &[u8] = b"";
        let written = write_file(&mut reader, &mut out, &file_key, file_id).expect("write");
        assert!(written > 0);
    }

    #[test]
    fn write_file_one_chunk() {
        let mvk = Mvk::generate().expect("mvk");
        let file_id = [0x42u8; 16];
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive");
        let plaintext = vec![0x55u8; 100];

        let mut out = Vec::new();
        let mut reader: &[u8] = &plaintext;
        write_file(&mut reader, &mut out, &file_key, file_id).expect("write");
        assert!(out.len() > plaintext.len());
    }

    #[test]
    fn write_file_multi_chunk() {
        let mvk = Mvk::generate().expect("mvk");
        let file_id = [0x42u8; 16];
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive");
        let plaintext: Vec<u8> = (0..CHUNK_SIZE * 3 + 5).map(|i| (i % 256) as u8).collect();

        let mut out = Vec::new();
        let mut reader: &[u8] = &plaintext;
        write_file(&mut reader, &mut out, &file_key, file_id).expect("write");
        assert!(out.len() > plaintext.len());
    }
}
