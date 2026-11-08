//! Integration tests for the Phase 1 cryptographic foundation.
//!
//! Covers MVK generation, Argon2id KEK derivation (PIN + password), HKDF
//! domain-separated subkeys, MVK wrap/unwrap (PIN/password/recovery), VaultMeta
//! round-trip, recovery-key encode/decode with checksum, and negative cases.

use veylora::crypto::hkdf::{
    derive_subkey, LABEL_METADATA_KEY, LABEL_RECOVERY_WRAP, LABEL_VAULT_KEY,
};
use veylora::crypto::kdf::{derive_pin_kek, derive_password_kek};
use veylora::crypto::mvk::Mvk;
use veylora::crypto::recovery::{decode_recovery_key, encode_recovery_key, generate_recovery_key};
use veylora::crypto::wrap::{unwrap_mvk, wrap_mvk};
use veylora::crypto::{
    CryptoError, SecretKey,
};
use veylora::vault::VaultMeta;

#[test]
fn mvk_generation_returns_32_bytes_and_differs() {
    let a = Mvk::generate().expect("MVK generation failed");
    let b = Mvk::generate().expect("MVK generation failed");
    assert_eq!(a.as_bytes().len(), 32);
    assert_ne!(a.as_bytes(), b.as_bytes());
}

#[test]
fn argon2id_pin_deterministic_and_salt_sensitive() {
    let salt_a = [0x11u8; 32];
    let salt_b = [0x22u8; 32];
    let k1 = derive_pin_kek(b"1234", &salt_a).expect("PIN KEK derivation failed");
    let k2 = derive_pin_kek(b"1234", &salt_a).expect("PIN KEK derivation failed");
    let k3 = derive_pin_kek(b"1234", &salt_b).expect("PIN KEK derivation failed");
    assert_eq!(k1.as_bytes(), k2.as_bytes(), "same PIN+salt must yield same KEK");
    assert_ne!(k1.as_bytes(), k3.as_bytes(), "different salt must yield different KEK");
}

#[test]
fn argon2id_password_deterministic_and_salt_sensitive() {
    let salt_a = [0x33u8; 32];
    let salt_b = [0x44u8; 32];
    let k1 = derive_password_kek(b"correct horse battery staple", &salt_a)
        .expect("Password KEK derivation failed");
    let k2 = derive_password_kek(b"correct horse battery staple", &salt_a)
        .expect("Password KEK derivation failed");
    let k3 = derive_password_kek(b"correct horse battery staple", &salt_b)
        .expect("Password KEK derivation failed");
    assert_eq!(k1.as_bytes(), k2.as_bytes());
    assert_ne!(k1.as_bytes(), k3.as_bytes());
}

#[test]
fn hkdf_deterministic_and_label_separated() {
    let key = [0x42u8; 32];
    let a = derive_subkey(&key, LABEL_VAULT_KEY).expect("HKDF vault-key failed");
    let b = derive_subkey(&key, LABEL_VAULT_KEY).expect("HKDF vault-key failed");
    let c = derive_subkey(&key, LABEL_METADATA_KEY).expect("HKDF metadata-key failed");
    assert_eq!(a, b, "same key+label must yield same subkey");
    assert_ne!(a, c, "different label must yield different subkey");
}

#[test]
fn hkdf_rejects_empty_and_unknown_labels() {
    let key = [0x42u8; 32];
    assert!(derive_subkey(&key, b"").is_err(), "empty label must be rejected");
    assert!(
        derive_subkey(&key, b"not-a-valid-label").is_err(),
        "unknown label must be rejected"
    );
    // file-key prefix with a file_id suffix is allowed
    assert!(
        derive_subkey(&key, b"veylora/v1/file-key/abc123").is_ok(),
        "file-key/<file_id> label must be accepted"
    );
}

#[test]
fn wrap_unwrap_roundtrip_pin() {
    let mvk = Mvk::generate().expect("MVK generation failed");
    let kek = derive_pin_kek(b"1234", &[0x55u8; 32]).expect("PIN KEK failed");
    let payload = wrap_mvk(&kek, &mvk, b"pin-aad").expect("wrap failed");
    let unwrapped = unwrap_mvk(&kek, &payload, b"pin-aad").expect("unwrap failed");
    assert_eq!(unwrapped.as_bytes(), mvk.as_bytes());
}

#[test]
fn wrap_unwrap_roundtrip_password() {
    let mvk = Mvk::generate().expect("MVK generation failed");
    let kek = derive_password_kek(b"correct horse battery staple", &[0x66u8; 32])
        .expect("Password KEK failed");
    let payload = wrap_mvk(&kek, &mvk, b"password-aad").expect("wrap failed");
    let unwrapped = unwrap_mvk(&kek, &payload, b"password-aad").expect("unwrap failed");
    assert_eq!(unwrapped.as_bytes(), mvk.as_bytes());
}

#[test]
fn wrap_unwrap_roundtrip_recovery() {
    let mvk = Mvk::generate().expect("MVK generation failed");
    let recovery_key = generate_recovery_key();
    let kek = SecretKey::from_bytes(&derive_subkey(&recovery_key, LABEL_RECOVERY_WRAP)
        .expect("recovery HKDF failed"));
    let payload = wrap_mvk(&kek, &mvk, b"recovery-aad").expect("wrap failed");
    let unwrapped = unwrap_mvk(&kek, &payload, b"recovery-aad").expect("unwrap failed");
    assert_eq!(unwrapped.as_bytes(), mvk.as_bytes());
}

#[test]
fn wrong_pin_fails_typed_error() {
    let mvk = Mvk::generate().expect("MVK generation failed");
    let kek = derive_pin_kek(b"1234", &[0x55u8; 32]).expect("PIN KEK failed");
    let payload = wrap_mvk(&kek, &mvk, b"pin-aad").expect("wrap failed");
    let wrong_kek = derive_pin_kek(b"0000", &[0x55u8; 32]).expect("PIN KEK failed");
    let err = unwrap_mvk(&wrong_kek, &payload, b"pin-aad").err().expect("expected unwrap error");
    assert!(
        matches!(err, CryptoError::Aead | CryptoError::UnwrapFailed),
        "expected typed auth error, got {:?}",
        err
    );
}

#[test]
fn wrong_recovery_key_fails_typed_error() {
    let mvk = Mvk::generate().expect("MVK generation failed");
    let good = generate_recovery_key();
    let bad = generate_recovery_key();
    let good_kek = SecretKey::from_bytes(&derive_subkey(&good, LABEL_RECOVERY_WRAP).expect("HKDF"));
    let bad_kek = SecretKey::from_bytes(&derive_subkey(&bad, LABEL_RECOVERY_WRAP).expect("HKDF"));
    let payload = wrap_mvk(&good_kek, &mvk, b"recovery-aad").expect("wrap failed");
    let err = unwrap_mvk(&bad_kek, &payload, b"recovery-aad").err().expect("expected unwrap error");
    assert!(
        matches!(err, CryptoError::Aead | CryptoError::UnwrapFailed),
        "expected typed auth error, got {:?}",
        err
    );
}

#[test]
fn tampered_ciphertext_fails() {
    let mvk = Mvk::generate().expect("MVK generation failed");
    let kek = derive_pin_kek(b"1234", &[0x55u8; 32]).expect("PIN KEK failed");
    let mut payload = wrap_mvk(&kek, &mvk, b"pin-aad").expect("wrap failed");
    // flip one byte in the ciphertext
    payload.ciphertext[0] ^= 0x01;
    let err = unwrap_mvk(&kek, &payload, b"pin-aad").err().expect("expected unwrap error");
    assert!(
        matches!(err, CryptoError::Aead | CryptoError::UnwrapFailed),
        "expected typed auth error, got {:?}",
        err
    );
}

#[test]
fn tampered_aad_fails() {
    let mvk = Mvk::generate().expect("MVK generation failed");
    let kek = derive_pin_kek(b"1234", &[0x55u8; 32]).expect("PIN KEK failed");
    let payload = wrap_mvk(&kek, &mvk, b"pin-aad").expect("wrap failed");
    let err = unwrap_mvk(&kek, &payload, b"tampered-aad").err().expect("expected unwrap error");
    assert!(
        matches!(err, CryptoError::Aead | CryptoError::UnwrapFailed),
        "expected typed auth error, got {:?}",
        err
    );
}

#[test]
fn recovery_key_encode_decode_roundtrip() {
    let key = generate_recovery_key();
    let encoded = encode_recovery_key(&key);
    let decoded = decode_recovery_key(&encoded).expect("decode failed");
    assert_eq!(decoded, key);
}

#[test]
fn recovery_key_checksum_rejects_single_char_mutation() {
    let key = generate_recovery_key();
    let encoded = encode_recovery_key(&key);
    // mutate a character in the encoded base32 portion
    let c = encoded.chars().next().unwrap();
    let replacement = if c == 'A' { 'B' } else { 'A' };
    let mut chars: Vec<char> = encoded.chars().collect();
    chars[0] = replacement;
    let encoded: String = chars.into_iter().collect();
    let err = decode_recovery_key(&encoded).unwrap_err();
    assert!(
        matches!(err, CryptoError::ChecksumMismatch | CryptoError::InvalidBase32),
        "expected ChecksumMismatch/InvalidBase32, got {:?}",
        err
    );
}

#[test]
fn recovery_key_rejects_wrong_length() {
    let err = decode_recovery_key("SHORT").unwrap_err();
    assert!(matches!(err, CryptoError::InvalidRecoveryKey));
}

#[test]
fn vault_meta_serialize_deserialize_roundtrip() {
    let mvk = Mvk::generate().expect("MVK generation failed");
    let mut meta = VaultMeta::new();
    meta.add_pin_entry(b"1234", &mvk).expect("add pin entry");
    meta.add_password_entry(b"correct horse battery staple", &mvk).expect("add password entry");
    let recovery_key = generate_recovery_key();
    meta.add_recovery_entry(&recovery_key, &mvk).expect("add recovery entry");

    let json = meta.serialize().expect("serialize");
    let restored = VaultMeta::deserialize(&json).expect("deserialize");
    assert_eq!(restored.version, 1);
    assert!(restored.pin.is_some());
    assert!(restored.password.is_some());
    assert!(restored.recovery.is_some());
}

#[test]
fn vault_meta_rejects_wrong_version() {
    let mut meta = VaultMeta::new();
    let json = meta.serialize().expect("serialize");
    // tamper with version field
    let tampered = json.replace("\"version\":1", "\"version\":2");
    let err = VaultMeta::deserialize(&tampered).unwrap_err();
    assert!(
        matches!(err, CryptoError::UnsupportedVersion),
        "expected UnsupportedVersion, got {:?}",
        err
    );
}


