//! Minimal crypto module for orchestra workers.
//!
//! Mirrors the API's `crypto.rs` conventions:
//!   - `enc:v1:<base64(nonce ‖ ciphertext ‖ tag)>` format
//!   - AES-256-GCM with AAD
//!   - HKDF-SHA256 for KEK derivation
//!
//! The orchestra needs crypto when the tenant uses passphrase-mode encryption
//! (Phase 2). In login-derived mode the API decrypts transparently, so the
//! orchestra receives plaintext.

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, OsRng},
};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use hkdf::Hkdf;
use sha2::Sha256;
use std::fmt;
use zeroize::Zeroize;

const ENC_PREFIX: &str = "enc:v1:";
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const DEK_LEN: usize = 32; // AES-256
const KEK_INFO: &[u8] = b"diraigent-kek-v1";

/// A Data Encryption Key — 32 bytes for AES-256-GCM.
#[derive(Clone)]
pub struct Dek {
    key: [u8; DEK_LEN],
}

impl Drop for Dek {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

impl Dek {
    /// Construct from raw bytes (e.g. from base64-decoded env var).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() != DEK_LEN {
            return Err(CryptoError::InvalidKeyLength);
        }
        let mut key = [0u8; DEK_LEN];
        key.copy_from_slice(bytes);
        Ok(Dek { key })
    }

    /// Construct from a base64-encoded string (e.g. `DIRAIGENT_DEK` env var).
    pub fn from_base64(b64: &str) -> Result<Self, CryptoError> {
        let bytes = B64.decode(b64).map_err(|_| CryptoError::InvalidBase64)?;
        Self::from_bytes(&bytes)
    }

    /// Encrypt plaintext with AES-256-GCM and the given AAD tag.
    /// Returns `enc:v1:<base64(nonce ‖ ciphertext ‖ tag)>`.
    pub fn encrypt(&self, plaintext: &[u8], aad: &str) -> Result<String, CryptoError> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.key).map_err(|_| CryptoError::InvalidKeyLength)?;
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let payload = aes_gcm::aead::Payload {
            msg: plaintext,
            aad: aad.as_bytes(),
        };
        let ciphertext = cipher
            .encrypt(nonce, payload)
            .map_err(|_| CryptoError::EncryptionFailed)?;

        let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        Ok(format!("{}{}", ENC_PREFIX, B64.encode(&combined)))
    }

    /// Decrypt a value produced by [`Dek::encrypt`]. Returns the plaintext bytes.
    pub fn decrypt(&self, value: &str, aad: &str) -> Result<Vec<u8>, CryptoError> {
        let encoded = value
            .strip_prefix(ENC_PREFIX)
            .ok_or(CryptoError::NotEncrypted)?;

        let combined = B64
            .decode(encoded)
            .map_err(|_| CryptoError::InvalidBase64)?;

        if combined.len() < NONCE_LEN + TAG_LEN {
            return Err(CryptoError::InvalidCiphertext);
        }

        let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);

        let cipher =
            Aes256Gcm::new_from_slice(&self.key).map_err(|_| CryptoError::InvalidKeyLength)?;

        let payload = aes_gcm::aead::Payload {
            msg: ciphertext,
            aad: aad.as_bytes(),
        };

        cipher
            .decrypt(nonce, payload)
            .map_err(|_| CryptoError::DecryptionFailed)
    }

    /// Decrypt a string field. If not encrypted (no `enc:v1:` prefix), returns as-is.
    pub fn decrypt_str(&self, value: &str, aad: &str) -> Result<String, CryptoError> {
        if !value.starts_with(ENC_PREFIX) {
            return Ok(value.to_string());
        }
        let bytes = self.decrypt(value, aad)?;
        String::from_utf8(bytes).map_err(|_| CryptoError::InvalidUtf8)
    }

    /// Encrypt a string field. Returns `enc:v1:...`.
    pub fn encrypt_str(&self, value: &str, aad: &str) -> Result<String, CryptoError> {
        self.encrypt(value.as_bytes(), aad)
    }

    /// Decrypt a JSON value. If it's a string starting with `enc:v1:`, decrypt and parse.
    /// Otherwise return as-is.
    pub fn decrypt_json(
        &self,
        value: &serde_json::Value,
        aad: &str,
    ) -> Result<serde_json::Value, CryptoError> {
        if let serde_json::Value::String(s) = value
            && s.starts_with(ENC_PREFIX)
        {
            let decrypted = self.decrypt_str(s, aad)?;
            return serde_json::from_str(&decrypted).map_err(|_| CryptoError::InvalidJson);
        }
        Ok(value.clone())
    }

    /// Encrypt a JSON value by serializing to string then encrypting.
    pub fn encrypt_json(
        &self,
        value: &serde_json::Value,
        aad: &str,
    ) -> Result<serde_json::Value, CryptoError> {
        let json_str = serde_json::to_string(value).map_err(|_| CryptoError::EncryptionFailed)?;
        let encrypted = self.encrypt_str(&json_str, aad)?;
        Ok(serde_json::Value::String(encrypted))
    }

    /// Unwrap a DEK from base64 AES-KW ciphertext.
    pub fn unwrap(wrapped_b64: &str, kek: &[u8; DEK_LEN]) -> Result<Self, CryptoError> {
        let wrapped = B64
            .decode(wrapped_b64)
            .map_err(|_| CryptoError::InvalidBase64)?;
        let kek = aes_kw::Kek::from(*kek);
        let mut key = [0u8; DEK_LEN];
        kek.unwrap(&wrapped, &mut key)
            .map_err(|_| CryptoError::UnwrapFailed)?;
        Ok(Dek { key })
    }
}

/// Derive a KEK from a passphrase and salt via HKDF-SHA256.
///
/// `HKDF-SHA256(SHA256(passphrase), salt, "diraigent-kek-v1")`
pub fn derive_kek(passphrase: &str, salt_b64: &str) -> Result<[u8; DEK_LEN], CryptoError> {
    let salt = B64
        .decode(salt_b64)
        .map_err(|_| CryptoError::InvalidBase64)?;

    use sha2::Digest;
    let hash = Sha256::digest(passphrase.as_bytes());

    let hk = Hkdf::<Sha256>::new(Some(&salt), &hash);
    let mut kek = [0u8; DEK_LEN];
    hk.expand(KEK_INFO, &mut kek)
        .map_err(|_| CryptoError::DerivationFailed)?;
    Ok(kek)
}

/// Returns true if the value looks like an encrypted field.
pub fn is_encrypted(value: &str) -> bool {
    value.starts_with(ENC_PREFIX)
}

/// Returns true if the JSON value (or any nested value) contains an encrypted string.
/// Used as a fast pre-check to skip the full recursive walk when nothing is encrypted.
fn contains_encrypted(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(s) => s.starts_with(ENC_PREFIX),
        serde_json::Value::Object(map) => map.values().any(contains_encrypted),
        serde_json::Value::Array(arr) => arr.iter().any(contains_encrypted),
        _ => false,
    }
}

/// Recursively walk a JSON value and decrypt any encrypted string fields.
/// Uses the field path as AAD (e.g. "task.context", "knowledge.content").
///
/// Performs a fast pre-check: if no encrypted strings are present, returns immediately.
pub fn decrypt_json_recursive(dek: &Dek, value: &mut serde_json::Value, path: &str) {
    if !contains_encrypted(value) {
        return;
    }
    decrypt_json_recursive_inner(dek, value, path);
}

fn decrypt_json_recursive_inner(dek: &Dek, value: &mut serde_json::Value, path: &str) {
    match value {
        serde_json::Value::String(s) if s.starts_with(ENC_PREFIX) => {
            match dek.decrypt_str(s, path) {
                Ok(decrypted) => {
                    // Try to parse as JSON; if it parses, replace with the parsed value
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                        *value = parsed;
                    } else {
                        *s = decrypted;
                    }
                }
                Err(e) => {
                    tracing::warn!("failed to decrypt field {path}: {e}");
                }
            }
        }
        serde_json::Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                if let Some(v) = map.get_mut(&key) {
                    decrypt_json_recursive(dek, v, &child_path);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, v) in arr.iter_mut().enumerate() {
                let child_path = format!("{path}[{i}]");
                decrypt_json_recursive(dek, v, &child_path);
            }
        }
        _ => {}
    }
}

// ── Errors ──

#[derive(Debug)]
pub enum CryptoError {
    InvalidKeyLength,
    EncryptionFailed,
    DecryptionFailed,
    WrapFailed,
    UnwrapFailed,
    DerivationFailed,
    InvalidBase64,
    NotEncrypted,
    InvalidCiphertext,
    InvalidUtf8,
    InvalidJson,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKeyLength => write!(f, "invalid key length"),
            Self::EncryptionFailed => write!(f, "encryption failed"),
            Self::DecryptionFailed => write!(f, "decryption failed — wrong key or corrupted data"),
            Self::WrapFailed => write!(f, "AES-KW wrap failed"),
            Self::UnwrapFailed => write!(f, "AES-KW unwrap failed — wrong KEK"),
            Self::DerivationFailed => write!(f, "key derivation failed"),
            Self::InvalidBase64 => write!(f, "invalid base64"),
            Self::NotEncrypted => write!(f, "value is not encrypted"),
            Self::InvalidCiphertext => write!(f, "invalid ciphertext format"),
            Self::InvalidUtf8 => write!(f, "decrypted value is not valid UTF-8"),
            Self::InvalidJson => write!(f, "decrypted value is not valid JSON"),
        }
    }
}

impl std::error::Error for CryptoError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let dek = Dek::from_bytes(&[0x42; 32]).unwrap();
        let encrypted = dek.encrypt_str("hello", "test.field").unwrap();
        assert!(encrypted.starts_with(ENC_PREFIX));
        let decrypted = dek.decrypt_str(&encrypted, "test.field").unwrap();
        assert_eq!(decrypted, "hello");
    }

    #[test]
    fn decrypt_passthrough_unencrypted() {
        let dek = Dek::from_bytes(&[0x42; 32]).unwrap();
        let result = dek.decrypt_str("plain text", "test").unwrap();
        assert_eq!(result, "plain text");
    }

    #[test]
    fn from_base64_roundtrip() {
        let b64 = B64.encode([0xAB; 32]);
        let dek = Dek::from_base64(&b64).unwrap();
        let encrypted = dek.encrypt_str("secret", "aad").unwrap();
        let dek2 = Dek::from_base64(&b64).unwrap();
        let decrypted = dek2.decrypt_str(&encrypted, "aad").unwrap();
        assert_eq!(decrypted, "secret");
    }

    #[test]
    fn derive_kek_deterministic() {
        let salt = B64.encode([0x01; 32]);
        let kek1 = derive_kek("my-passphrase", &salt).unwrap();
        let kek2 = derive_kek("my-passphrase", &salt).unwrap();
        assert_eq!(kek1, kek2);
    }

    #[test]
    fn recursive_decrypt() {
        let dek = Dek::from_bytes(&[0x42; 32]).unwrap();
        let encrypted = dek.encrypt_str("secret-value", "obj.field").unwrap();
        let mut value = serde_json::json!({
            "field": encrypted,
            "plain": "not encrypted",
        });
        decrypt_json_recursive(&dek, &mut value, "obj");
        assert_eq!(value["field"], "secret-value");
        assert_eq!(value["plain"], "not encrypted");
    }
}
