//! String manipulation and cryptography utilities.
//!
//! This module provides utility functions for:
//! - AES-256-GCM encryption and decryption for sensitive data (e.g., passwords)
//! - Base64 encoding/decoding for storage and transport

use crate::error::Error;
use aes_gcm::{
    Aes256Gcm,
    aead::{Aead, AeadCore, KeyInit, Nonce, OsRng},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

type Result<T, E = Error> = std::result::Result<T, E>;

/// Master encryption key for AES-256-GCM cipher.
///
/// WARNING: In production, this should be stored securely (e.g., keychain, env var)
/// rather than hardcoded in the binary.
const MASTER_KEY: &[u8; 32] = b"DfcGuiSecretKey2026GoldwindTeam!";

/// Encrypts a plaintext string using AES-256-GCM encryption.
///
/// The encrypted data is encoded as Base64 for easy storage and transport.
/// Each encryption uses a randomly generated nonce for security.
///
/// # Algorithm Details
/// - **Cipher**: AES-256-GCM (Galois/Counter Mode)
/// - **Key size**: 256 bits (32 bytes)
/// - **Nonce**: 96 bits (12 bytes), randomly generated per encryption
/// - **Authentication**: Built-in authenticated encryption (AEAD)
///
/// # Storage Format
/// The output Base64 string contains: `[nonce (12 bytes)][ciphertext (variable)]`
///
/// # Arguments
/// * `plain_text` - The plaintext string to encrypt
///
/// # Returns
/// A Base64-encoded string containing the nonce and ciphertext
pub fn encrypt(plain_text: &str) -> Result<String> {
    // Initialize AES-256-GCM cipher with master key
    let cipher = Aes256Gcm::new(MASTER_KEY.into());

    // Generate a random 96-bit nonce (number used once)
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    // Encrypt the plaintext
    let ciphertext = cipher
        .encrypt(&nonce, plain_text.as_bytes())
        .map_err(|e| Error::Invalid {
            message: format!("Encryption failed: {e}"),
        })?;

    // Combine nonce and ciphertext for storage
    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);

    // Encode as Base64 for safe storage/transport
    Ok(BASE64.encode(combined))
}

/// Decrypts a Base64-encoded ciphertext encrypted with AES-256-GCM.
///
/// Expects the input to be in the format produced by `encrypt()`:
/// `[nonce (12 bytes)][ciphertext (variable)]` encoded as Base64.
///
/// # Arguments
/// * `cipher_text` - Base64-encoded string containing nonce and ciphertext
///
/// # Returns
/// The decrypted plaintext string
pub fn decrypt(cipher_text: &str) -> Result<String> {
    // Decode from Base64
    let data = BASE64.decode(cipher_text).map_err(|e| Error::Invalid {
        message: format!("Base64 decode failed: {e}"),
    })?;

    // Validate minimum length (nonce is 12 bytes)
    if data.len() < 12 {
        return Err(Error::Invalid {
            message: "Ciphertext too short".to_string(),
        });
    }

    // Initialize cipher with master key
    let cipher = Aes256Gcm::new(MASTER_KEY.into());

    // Extract nonce from first 12 bytes
    let nonce_bytes = &data[0..12];
    let nonce = Nonce::<Aes256Gcm>::from_slice(nonce_bytes);

    // Extract ciphertext from remaining bytes
    let ciphertext = &data[12..];

    // Decrypt and verify authenticity
    let plaintext_bytes = cipher.decrypt(nonce, ciphertext).map_err(|e| Error::Invalid {
        message: format!("Decryption failed: {e}"),
    })?;

    // Convert decrypted bytes to UTF-8 string
    String::from_utf8(plaintext_bytes).map_err(|e| Error::Invalid {
        message: format!("UTF-8 decode failed: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let original = "my_secret_password";
        let encrypted = encrypt(original).expect("Encryption failed");
        let decrypted = decrypt(&encrypted).expect("Decryption failed");
        assert_eq!(original, decrypted);
    }

    #[test]
    fn test_encrypt_produces_different_ciphertext() {
        let original = "test";
        let encrypted1 = encrypt(original).expect("Encryption failed");
        let encrypted2 = encrypt(original).expect("Encryption failed");
        // Due to random nonce, ciphertexts should be different
        assert_ne!(encrypted1, encrypted2);
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        let result = decrypt("not_valid_base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_too_short() {
        let result = decrypt("AQIDBA=="); // Only 4 bytes
        assert!(result.is_err());
    }
}
