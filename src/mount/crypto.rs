use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::RngCore;

use crate::error::{IronweaveError, Result};

/// Encrypt plaintext using AES-256-GCM with a base64-encoded 32-byte key.
/// Returns base64-encoded `nonce || ciphertext`.
pub fn encrypt(plaintext: &str, master_key_b64: &str) -> Result<String> {
    let key_bytes = BASE64.decode(master_key_b64)
        .map_err(|e| IronweaveError::Internal(format!("invalid master key: {}", e)))?;
    if key_bytes.len() != 32 {
        return Err(IronweaveError::Internal("master key must be 32 bytes".to_string()));
    }

    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| IronweaveError::Internal(format!("cipher init: {}", e)))?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| IronweaveError::Internal(format!("encryption failed: {}", e)))?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(BASE64.encode(&combined))
}

/// Decrypt base64-encoded `nonce || ciphertext` using AES-256-GCM.
pub fn decrypt(encrypted_b64: &str, master_key_b64: &str) -> Result<String> {
    let key_bytes = BASE64.decode(master_key_b64)
        .map_err(|e| IronweaveError::Internal(format!("invalid master key: {}", e)))?;
    if key_bytes.len() != 32 {
        return Err(IronweaveError::Internal("master key must be 32 bytes".to_string()));
    }

    let combined = BASE64.decode(encrypted_b64)
        .map_err(|e| IronweaveError::Internal(format!("invalid ciphertext: {}", e)))?;
    if combined.len() < 12 {
        return Err(IronweaveError::Internal("ciphertext too short".to_string()));
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| IronweaveError::Internal(format!("cipher init: {}", e)))?;

    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| IronweaveError::Internal(format!("decryption failed: {}", e)))?;

    String::from_utf8(plaintext)
        .map_err(|e| IronweaveError::Internal(format!("invalid utf-8: {}", e)))
}

/// Generate a random 32-byte key, returned as base64.
pub fn generate_key() -> String {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    BASE64.encode(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> String {
        generate_key()
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = test_key();
        let plaintext = "hello world secret credential";
        let encrypted = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn different_nonces() {
        let key = test_key();
        let plaintext = "same plaintext";
        let enc1 = encrypt(plaintext, &key).unwrap();
        let enc2 = encrypt(plaintext, &key).unwrap();
        assert_ne!(enc1, enc2, "two encryptions of the same plaintext should differ");
    }

    #[test]
    fn wrong_key_fails() {
        let key1 = test_key();
        let key2 = test_key();
        let encrypted = encrypt("secret", &key1).unwrap();
        let result = decrypt(&encrypted, &key2);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_key_length() {
        let short_key = BASE64.encode(&[0u8; 16]);
        let result = encrypt("test", &short_key);
        assert!(result.is_err());
        assert!(format!("{:?}", result.unwrap_err()).contains("32 bytes"));
    }

    #[test]
    fn empty_string_roundtrip() {
        let key = test_key();
        let encrypted = encrypt("", &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, "");
    }

    #[test]
    fn generate_key_length() {
        let key = generate_key();
        let decoded = BASE64.decode(&key).unwrap();
        assert_eq!(decoded.len(), 32);
    }
}
