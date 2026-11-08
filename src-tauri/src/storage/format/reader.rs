//! Random-access reader for format v1.
//!
//! read_range maps a byte range to chunk indices, decrypts and authenticates
//! each chunk independently, and returns only the requested bytes.
//! verify_file authenticates every chunk and the finalize record.

use crate::storage::format::{
    chunk, finalize, header, CHUNK_SIZE, NONCE_LEN, TAG_LEN, FormatError, FileKey, Result,
};

/// Read bytes [start, end) from an encrypted file on disk.
///
/// `file` is the complete on-disk file contents. Returns only the requested
/// plaintext bytes. Every chunk touched is authenticated; any failure aborts.
pub fn read_range(
    file: &[u8],
    start: u64,
    end: u64,
    file_key: &FileKey,
) -> Result<Vec<u8>> {
    let (hdr, header_len) = header::read_header(file, file_key)?;

    if start > end {
        return Err(FormatError::InvalidRange);
    }
    if end > hdr.total_length {
        return Err(FormatError::InvalidRange);
    }
    if start == end {
        return Ok(Vec::new());
    }

    let first_chunk = start / CHUNK_SIZE;
    let last_chunk = if end == 0 { 0 } else { (end - 1) / CHUNK_SIZE };

    let mut offset = header_len;
    let mut out = Vec::new();

    for i in 0..=last_chunk {
        let pt_len = chunk::chunk_plaintext_len(hdr.total_length, hdr.total_chunks, i);
        let ct_tag_len = pt_len + TAG_LEN;
        let chunk_on_disk_len = NONCE_LEN + ct_tag_len;

        if offset + chunk_on_disk_len > file.len() {
            return Err(FormatError::TooSmall);
        }

        if i >= first_chunk {
            let _nonce = &file[offset..offset + NONCE_LEN];
            let ct_tag = &file[offset + NONCE_LEN..offset + chunk_on_disk_len];

            let plaintext = chunk::decrypt_chunk(
                file_key,
                &hdr.file_id,
                i,
                hdr.total_chunks,
                hdr.total_length,
                ct_tag,
            )?;

            // Copy the relevant slice from this chunk's plaintext.
            let chunk_start = i * CHUNK_SIZE;
            let lo = if start > chunk_start { start - chunk_start } else { 0 };
            let hi = if end < chunk_start + pt_len as u64 {
                (end - chunk_start) as usize
            } else {
                pt_len
            };
            if hi > lo as usize {
                out.extend_from_slice(&plaintext[lo as usize..hi]);
            }
        }

        offset += chunk_on_disk_len;
    }

    // After all chunks, the finalize record should follow.
    // (Not verified here; use verify_file for full validation.)
    Ok(out)
}

/// Verify the entire file: header, every chunk, and the finalize record.
pub fn verify_file(file: &[u8], file_key: &FileKey) -> Result<()> {
    let (hdr, header_len) = header::read_header(file, file_key)?;

    let mut offset = header_len;
    let mut chunk_ct_tags: Vec<Vec<u8>> = Vec::with_capacity(hdr.total_chunks as usize);

    for i in 0..hdr.total_chunks {
        let pt_len = chunk::chunk_plaintext_len(hdr.total_length, hdr.total_chunks, i);
        let ct_tag_len = pt_len + TAG_LEN;
        let chunk_on_disk_len = NONCE_LEN + ct_tag_len;

        if offset + chunk_on_disk_len > file.len() {
            return Err(FormatError::TooSmall);
        }

        let ct_tag = &file[offset + NONCE_LEN..offset + chunk_on_disk_len];
        // Decrypt to authenticate (plaintext discarded).
        let _ = chunk::decrypt_chunk(
            file_key,
            &hdr.file_id,
            i,
            hdr.total_chunks,
            hdr.total_length,
            ct_tag,
        )?;
        chunk_ct_tags.push(ct_tag.to_vec());

        offset += chunk_on_disk_len;
    }

    // Finalize record follows the last chunk.
    let fin_start = offset;
    if fin_start + NONCE_LEN + TAG_LEN > file.len() {
        return Err(FormatError::TooSmall);
    }
    let fin_ct_tag = &file[fin_start..];

    let root_hash = crate::storage::format::finalize::compute_root_hash(&chunk_ct_tags);
    finalize::decrypt_finalize(
        file_key,
        &hdr.file_id,
        hdr.total_chunks,
        hdr.total_length,
        &root_hash,
        fin_ct_tag,
    )?;

    Ok(())
}

/// Convenience: encrypt plaintext bytes, then read them back via read_range.
pub fn round_trip_bytes(
    plaintext: &[u8],
    file_key: &FileKey,
    file_id: [u8; 16],
) -> Result<Vec<u8>> {
    let encoded = crate::storage::format::writer::write_file_bytes(plaintext, file_key, file_id)?;
    read_range(&encoded, 0, plaintext.len() as u64, file_key)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::mvk::Mvk;

    fn setup() -> (Mvk, FileKey, [u8; 16]) {
        let mvk = Mvk::generate().expect("mvk");
        let file_id = [0x42u8; 16];
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive");
        (mvk, file_key, file_id)
    }

    #[test]
    fn read_range_one_byte() {
        let (_mvk, file_key, file_id) = setup();
        let plaintext = vec![0x7fu8; 1];
        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");
        let out = read_range(&encoded, 0, 1, &file_key).expect("read");
        assert_eq!(out, plaintext);
    }

    #[test]
    fn read_range_exact_one_chunk() {
        let (_mvk, file_key, file_id) = setup();
        let plaintext: Vec<u8> = (0..CHUNK_SIZE as usize).map(|i| (i % 256) as u8).collect();
        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");
        let out = read_range(&encoded, 0, CHUNK_SIZE, &file_key).expect("read");
        assert_eq!(out, plaintext);
    }

    #[test]
    fn read_range_one_chunk_plus_one_byte() {
        let (_mvk, file_key, file_id) = setup();
        let plaintext: Vec<u8> = (0..CHUNK_SIZE as usize + 1).map(|i| (i % 256) as u8).collect();
        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");
        let out = read_range(&encoded, 0, CHUNK_SIZE + 1, &file_key).expect("read");
        assert_eq!(out, plaintext);
    }

    #[test]
    fn read_range_multi_chunk_10mb() {
        let (_mvk, file_key, file_id) = setup();
        let size: usize = 10 * 1024 * 1024;
        let plaintext: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");

        // Full read.
        let all = read_range(&encoded, 0, size as u64, &file_key).expect("read");
        assert_eq!(all, plaintext);

        // 5 random ranges.
        let ranges = [
            (0, 100),
            (CHUNK_SIZE - 50, CHUNK_SIZE + 50),
            (CHUNK_SIZE * 2, CHUNK_SIZE * 2 + 1000),
            ((size - 100) as u64, size as u64),
            (CHUNK_SIZE * 5 + 123, CHUNK_SIZE * 5 + 456),
        ];
        for (s, e) in ranges {
            let out = read_range(&encoded, s, e, &file_key).expect("read range");
            assert_eq!(out, plaintext[s as usize..e as usize], "range {}-{}", s, e);
        }
    }

    #[test]
    fn read_range_spans_chunks() {
        let (_mvk, file_key, file_id) = setup();
        // range [65530, 65550) spans the end of chunk 0 and start of chunk 1.
        let plaintext: Vec<u8> = (0..CHUNK_SIZE as usize * 2).map(|i| (i % 256) as u8).collect();
        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");
        let out = read_range(&encoded, CHUNK_SIZE - 50, CHUNK_SIZE + 50, &file_key)
            .expect("read");
        assert_eq!(out, plaintext[(CHUNK_SIZE - 50) as usize..(CHUNK_SIZE + 50) as usize]);
    }

    #[test]
    fn read_range_same_chunk() {
        let (_mvk, file_key, file_id) = setup();
        let plaintext: Vec<u8> = (0..CHUNK_SIZE as usize).map(|i| (i % 256) as u8).collect();
        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");
        let out = read_range(&encoded, 100, 200, &file_key).expect("read");
        assert_eq!(out, plaintext[100..200]);
    }

    #[test]
    fn read_range_empty() {
        let (_mvk, file_key, file_id) = setup();
        let plaintext = vec![1u8; 100];
        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");
        let out = read_range(&encoded, 10, 10, &file_key).expect("read");
        assert!(out.is_empty());
    }

    #[test]
    fn verify_file_ok() {
        let (_mvk, file_key, file_id) = setup();
        let plaintext: Vec<u8> = (0..CHUNK_SIZE as usize * 3 + 7).map(|i| (i % 256) as u8).collect();
        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");
        verify_file(&encoded, &file_key).expect("verify");
    }

    #[test]
    fn negative_tampered_chunk_3() {
        let (_mvk, file_key, file_id) = setup();
        let plaintext: Vec<u8> = (0..CHUNK_SIZE as usize * 4).map(|i| (i % 256) as u8).collect();
        let mut encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");
        // Find chunk 3's ciphertext region and flip a byte.
        let (hdr, header_len) = header::read_header(&encoded, &file_key).expect("header");
        let mut offset = header_len;
        for i in 0..=3u64 {
            let pt_len = chunk::chunk_plaintext_len(hdr.total_length, hdr.total_chunks, i);
            let chunk_on_disk_len = NONCE_LEN + pt_len + TAG_LEN;
            if i == 3 {
                encoded[offset + NONCE_LEN] ^= 0x01; // flip ciphertext byte
                break;
            }
            offset += chunk_on_disk_len;
        }
        // read_range across chunk 3 should fail.
        let err = read_range(
            &encoded,
            3 * CHUNK_SIZE,
            4 * CHUNK_SIZE,
            &file_key,
        )
        .unwrap_err();
        assert!(matches!(err, FormatError::ChunkAuth(3)));
        let err = verify_file(&encoded, &file_key).unwrap_err();
        assert!(matches!(err, FormatError::ChunkAuth(3)));
    }

    #[test]
    fn negative_reorder_chunks() {
        let (_mvk, file_key, file_id) = setup();
        let plaintext: Vec<u8> = (0..CHUNK_SIZE as usize * 3).map(|i| (i % 256) as u8).collect();
        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");

        // Reorder: swap chunk 1 and chunk 2 on disk.
        let (hdr, header_len) = header::read_header(&encoded, &file_key).expect("header");
        let mut offsets = Vec::new();
        let mut offset = header_len;
        for i in 0..hdr.total_chunks {
            offsets.push(offset);
            let pt_len = chunk::chunk_plaintext_len(hdr.total_length, hdr.total_chunks, i);
            offset += NONCE_LEN + pt_len + TAG_LEN;
        }
        let len1 = NONCE_LEN
            + chunk::chunk_plaintext_len(hdr.total_length, hdr.total_chunks, 1)
            + TAG_LEN;
        let len2 = NONCE_LEN
            + chunk::chunk_plaintext_len(hdr.total_length, hdr.total_chunks, 2)
            + TAG_LEN;
        let mut reordered = encoded.clone();
        reordered
            .splice(offsets[1]..offsets[1] + len1, encoded[offsets[2]..offsets[2] + len2].iter().cloned());
        // Now splice chunk 2's original location with chunk 1's original bytes
        // (lengths are equal since all chunks are full).
        reordered
            .splice(offsets[2]..offsets[2] + len2, encoded[offsets[1]..offsets[1] + len1].iter().cloned());

        let err = verify_file(&reordered, &file_key).unwrap_err();
        assert!(
            matches!(err, FormatError::RootHashMismatch | FormatError::ChunkAuth(_)),
            "expected root hash or chunk auth error, got {:?}",
            err
        );
    }

    #[test]
    fn negative_duplicate_chunk() {
        let (_mvk, file_key, file_id) = setup();
        let plaintext: Vec<u8> = (0..CHUNK_SIZE as usize * 3).map(|i| (i % 256) as u8).collect();
        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");

        // Duplicate chunk 1: insert it again after chunk 2.
        let (hdr, header_len) = header::read_header(&encoded, &file_key).expect("header");
        let mut offsets = Vec::new();
        let mut lens = Vec::new();
        let mut offset = header_len;
        for i in 0..hdr.total_chunks {
            offsets.push(offset);
            let pt_len = chunk::chunk_plaintext_len(hdr.total_length, hdr.total_chunks, i);
            let cl = NONCE_LEN + pt_len + TAG_LEN;
            lens.push(cl);
            offset += cl;
        }
        let chunk1_bytes = &encoded[offsets[1]..offsets[1] + lens[1]];
        let insert_at = offset; // after last chunk (where finalize starts)
        let mut duplicated = encoded.clone();
        duplicated.splice(
            insert_at..insert_at,
            chunk1_bytes.iter().cloned(),
        );

        let err = verify_file(&duplicated, &file_key).unwrap_err();
        assert!(
            matches!(
                err,
                FormatError::RootHashMismatch
                    | FormatError::ChunkAuth(_)
                    | FormatError::FinalizeAuth
            ),
            "expected root hash, chunk auth, or finalize auth error, got {:?}",
            err
        );
    }

    #[test]
    fn negative_truncate_last_100_bytes() {
        let (_mvk, file_key, file_id) = setup();
        let plaintext: Vec<u8> = (0..CHUNK_SIZE as usize * 2 + 100).map(|i| (i % 256) as u8).collect();
        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");
        let truncated = &encoded[..encoded.len() - 100];

        // Header and first chunks still read OK.
        let _ = read_range(&truncated, 0, CHUNK_SIZE, &file_key).expect("read first chunk");
        // verify_file should fail (finalize missing/corrupt).
        let err = verify_file(truncated, &file_key).unwrap_err();
        assert!(
            matches!(err, FormatError::TooSmall | FormatError::FinalizeAuth),
            "expected too-small or finalize auth error, got {:?}",
            err
        );
    }

    #[test]
    fn negative_truncate_middle_chunk() {
        let (_mvk, file_key, file_id) = setup();
        let plaintext: Vec<u8> = (0..CHUNK_SIZE as usize * 3).map(|i| (i % 256) as u8).collect();
        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");

        // Remove chunk 1 from the middle.
        let (hdr, header_len) = header::read_header(&encoded, &file_key).expect("header");
        let mut offsets = Vec::new();
        let mut lens = Vec::new();
        let mut offset = header_len;
        for i in 0..hdr.total_chunks {
            offsets.push(offset);
            let pt_len = chunk::chunk_plaintext_len(hdr.total_length, hdr.total_chunks, i);
            let cl = NONCE_LEN + pt_len + TAG_LEN;
            lens.push(cl);
            offset += cl;
        }
        let mut truncated = encoded.clone();
        truncated.splice(offsets[1]..offsets[1] + lens[1], std::iter::empty::<u8>());

        let err = verify_file(&truncated, &file_key).unwrap_err();
        assert!(
            matches!(err, FormatError::ChunkAuth(_) | FormatError::RootHashMismatch | FormatError::FinalizeAuth),
            "expected auth/root/finalize error, got {:?}",
            err
        );
    }

    #[test]
    fn negative_cross_file_substitution() {
        let (_mvk, file_key, file_id_a) = setup();
        let file_id_b = [0x43u8; 16];
        let file_key_b = FileKey::derive(&_mvk, &file_id_b).expect("derive b");
        let plaintext_a: Vec<u8> = (0..CHUNK_SIZE as usize * 2).map(|i| (i % 256) as u8).collect();
        let plaintext_b: Vec<u8> = (0..CHUNK_SIZE as usize * 2).map(|i| ((i + 100) % 256) as u8).collect();
        let encoded_a = crate::storage::format::writer::write_file_bytes(
            &plaintext_a, &file_key, file_id_a,
        )
        .expect("write a");
        let encoded_b = crate::storage::format::writer::write_file_bytes(
            &plaintext_b, &file_key_b, file_id_b,
        )
        .expect("write b");

        // Swap chunk 0 of A with chunk 0 of B (different file_ids).
        let (hdr_a, header_len_a) = header::read_header(&encoded_a, &file_key).expect("header a");
        let (hdr_b, header_len_b) = header::read_header(&encoded_b, &file_key_b).expect("header b");

        let pt_len_a = chunk::chunk_plaintext_len(hdr_a.total_length, hdr_a.total_chunks, 0);
        let ct_tag_len_a = pt_len_a + TAG_LEN;
        let chunk_a_len = NONCE_LEN + ct_tag_len_a;

        let pt_len_b = chunk::chunk_plaintext_len(hdr_b.total_length, hdr_b.total_chunks, 0);
        let ct_tag_len_b = pt_len_b + TAG_LEN;
        let chunk_b_len = NONCE_LEN + ct_tag_len_b;

        // Since both files are same size, chunk lengths are equal.
        let mut substituted = encoded_a.clone();
        substituted.splice(
            header_len_a..header_len_a + chunk_a_len,
            encoded_b[header_len_b..header_len_b + chunk_b_len].iter().cloned(),
        );

        let err = read_range(&substituted, 0, CHUNK_SIZE, &file_key).unwrap_err();
        assert!(matches!(err, FormatError::ChunkAuth(0)));
    }

    #[test]
    fn negative_tampered_header_ciphertext() {
        let (_mvk, file_key, file_id) = setup();
        let plaintext = vec![1u8; 100];
        let mut encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");
        let ct_start = header::HEADER_FIXED_LEN + NONCE_LEN;
        encoded[ct_start] ^= 0x01;

        let err = read_range(&encoded, 0, 100, &file_key).unwrap_err();
        assert!(matches!(err, FormatError::HeaderAuth));
    }

    #[test]
    fn negative_tampered_header_aad() {
        let (_mvk, file_key, file_id) = setup();
        let plaintext = vec![1u8; 100];
        let mut encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write");
        encoded[0] ^= 0xFF; // tamper magic byte (in AAD)

        let err = read_range(&encoded, 0, 100, &file_key).unwrap_err();
        assert!(matches!(err, FormatError::BadMagic));
    }

    #[test]
    fn negative_wrong_file_key() {
        let mvk_a = Mvk::generate().expect("mvk a");
        let mvk_b = Mvk::generate().expect("mvk b");
        let file_id = [0x42u8; 16];
        let file_key_a = FileKey::derive(&mvk_a, &file_id).expect("derive a");
        let file_key_b = FileKey::derive(&mvk_b, &file_id).expect("derive b");
        let plaintext = vec![1u8; 100];
        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key_a, file_id,
        )
        .expect("write");

        let err = read_range(&encoded, 0, 100, &file_key_b).unwrap_err();
        assert!(
            matches!(err, FormatError::HeaderAuth),
            "expected header auth error, got {:?}",
            err
        );
    }

    #[ignore]
    #[test]
    fn large_file_1gib() {
        let (_mvk, file_key, file_id) = setup();
        let size: u64 = 1024 * 1024 * 1024; // 1 GiB
        let pattern: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let mut plaintext = vec![0u8; size as usize];
        for chunk in plaintext.chunks_mut(256) {
            chunk.copy_from_slice(&pattern);
        }

        let encoded = crate::storage::format::writer::write_file_bytes(
            &plaintext, &file_key, file_id,
        )
        .expect("write 1 GiB");

        verify_file(&encoded, &file_key).expect("verify 1 GiB");

        let ranges = [
            (0u64, 1024),
            (size / 2, size / 2 + 1024),
            (size - 1024, size),
            (CHUNK_SIZE * 3 + 100, CHUNK_SIZE * 3 + 200),
            (CHUNK_SIZE * 1000 + 5000, CHUNK_SIZE * 1000 + 6000),
        ];
        for (s, e) in ranges {
            let out = read_range(&encoded, s, e, &file_key).expect("read range");
            assert_eq!(
                out,
                plaintext[s as usize..e as usize],
                "range {}-{}",
                s,
                e
            );
        }
    }
}
