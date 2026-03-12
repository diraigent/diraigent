//! Core cryptographic primitives for per-tenant zero-knowledge encryption.
//!
//! Key hierarchy:
//!   access_token → HKDF → KEK → unwraps wrapped_dek → DEK (AES-256-GCM)
//!
//! Encrypted field convention: `enc:v1:<base64(nonce ‖ ciphertext ‖ tag)>`

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, OsRng},
};
use aes_kw::Kek;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use hkdf::Hkdf;
use sha2::Sha256;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;
use zeroize::Zeroize;

const ENC_PREFIX: &str = "enc:v1:";
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const DEK_LEN: usize = 32; // AES-256
const KEK_INFO: &[u8] = b"diraigent-kek-v1";
const CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes

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
    /// Generate a fresh random DEK.
    pub fn generate() -> Self {
        let mut key = [0u8; DEK_LEN];
        OsRng.fill_bytes(&mut key);
        Dek { key }
    }

    /// Construct from raw bytes (e.g. after unwrapping).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() != DEK_LEN {
            return Err(CryptoError::InvalidKeyLength);
        }
        let mut key = [0u8; DEK_LEN];
        key.copy_from_slice(bytes);
        Ok(Dek { key })
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

        // nonce ‖ ciphertext (includes tag appended by aes-gcm)
        let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        Ok(format!("{}{}", ENC_PREFIX, B64.encode(&combined)))
    }

    /// Decrypt a value produced by [`Dek::encrypt`]. Returns the plaintext.
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

    /// Wrap this DEK with a KEK using AES-KW (RFC 3394). Returns base64 wrapped bytes.
    pub fn wrap(&self, kek: &[u8; DEK_LEN]) -> Result<String, CryptoError> {
        let kek = Kek::from(*kek);
        let mut buf = [0u8; DEK_LEN + 8]; // AES-KW adds 8-byte integrity check
        kek.wrap(&self.key, &mut buf)
            .map_err(|_| CryptoError::WrapFailed)?;
        Ok(B64.encode(buf))
    }

    /// Unwrap a DEK from base64 AES-KW ciphertext.
    pub fn unwrap(wrapped_b64: &str, kek: &[u8; DEK_LEN]) -> Result<Self, CryptoError> {
        let wrapped = B64
            .decode(wrapped_b64)
            .map_err(|_| CryptoError::InvalidBase64)?;
        let kek = Kek::from(*kek);
        let mut key = [0u8; DEK_LEN];
        kek.unwrap(&wrapped, &mut key)
            .map_err(|_| CryptoError::UnwrapFailed)?;
        Ok(Dek { key })
    }
}

/// Derive a KEK from a user's access token and the tenant's salt via HKDF-SHA256.
///
/// `HKDF-SHA256(SHA256(access_token), salt, "diraigent-kek-v1")`
pub fn derive_kek(access_token: &str, salt_b64: &str) -> Result<[u8; DEK_LEN], CryptoError> {
    let salt = B64
        .decode(salt_b64)
        .map_err(|_| CryptoError::InvalidBase64)?;

    use sha2::Digest;
    let token_hash = Sha256::digest(access_token.as_bytes());

    let hk = Hkdf::<Sha256>::new(Some(&salt), &token_hash);
    let mut kek = [0u8; DEK_LEN];
    hk.expand(KEK_INFO, &mut kek)
        .map_err(|_| CryptoError::DerivationFailed)?;
    Ok(kek)
}

/// Generate a random 32-byte salt, returned as base64.
pub fn generate_salt() -> String {
    let mut salt = [0u8; 32];
    OsRng.fill_bytes(&mut salt);
    B64.encode(salt)
}

// ── DEK Cache ──

struct CacheEntry {
    dek: Dek,
    inserted: Instant,
}

/// In-memory cache of decrypted DEKs per tenant. TTL-based eviction.
#[derive(Clone)]
pub struct DekCache {
    inner: Arc<RwLock<HashMap<Uuid, CacheEntry>>>,
}

impl Default for DekCache {
    fn default() -> Self {
        Self::new()
    }
}

impl DekCache {
    pub fn new() -> Self {
        DekCache {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the cached DEK for a tenant if it exists and hasn't expired.
    pub async fn get(&self, tenant_id: &Uuid) -> Option<Dek> {
        let cache = self.inner.read().await;
        cache.get(tenant_id).and_then(|entry| {
            if entry.inserted.elapsed() < CACHE_TTL {
                Some(entry.dek.clone())
            } else {
                None
            }
        })
    }

    /// Cache a DEK for a tenant.
    pub async fn put(&self, tenant_id: Uuid, dek: Dek) {
        let mut cache = self.inner.write().await;
        if cache.len() > 1_000 {
            cache.retain(|_, entry| entry.inserted.elapsed() < CACHE_TTL);
        }
        cache.insert(
            tenant_id,
            CacheEntry {
                dek,
                inserted: Instant::now(),
            },
        );
    }

    /// Remove a tenant's cached DEK (e.g. on key rotation).
    pub async fn evict(&self, tenant_id: &Uuid) {
        let mut cache = self.inner.write().await;
        cache.remove(tenant_id);
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
    NotInitialized,
    NoWrappedKey,
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
            Self::NotInitialized => write!(f, "tenant encryption not initialized"),
            Self::NoWrappedKey => write!(f, "no wrapped key found for user"),
        }
    }
}

impl std::error::Error for CryptoError {}

impl From<CryptoError> for crate::error::AppError {
    fn from(e: CryptoError) -> Self {
        match e {
            CryptoError::NotInitialized | CryptoError::NoWrappedKey => {
                crate::error::AppError::Forbidden(e.to_string())
            }
            CryptoError::DecryptionFailed | CryptoError::UnwrapFailed => {
                crate::error::AppError::Unauthorized(e.to_string())
            }
            _ => crate::error::AppError::Internal(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let dek = Dek::generate();
        let plaintext = "hello, world!";
        let aad = "test.field";

        let encrypted = dek.encrypt_str(plaintext, aad).unwrap();
        assert!(encrypted.starts_with(ENC_PREFIX));

        let decrypted = dek.decrypt_str(&encrypted, aad).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_unencrypted_passthrough() {
        let dek = Dek::generate();
        let plain = "just a plain string";
        let result = dek.decrypt_str(plain, "test").unwrap();
        assert_eq!(result, plain);
    }

    #[test]
    fn wrong_aad_fails() {
        let dek = Dek::generate();
        let encrypted = dek.encrypt_str("secret", "correct_aad").unwrap();
        let result = dek.decrypt_str(&encrypted, "wrong_aad");
        assert!(result.is_err());
    }

    #[test]
    fn wrap_unwrap_roundtrip() {
        let dek = Dek::generate();
        let mut k = [0u8; 32];
        OsRng.fill_bytes(&mut k);

        let wrapped = dek.wrap(&k).unwrap();
        let unwrapped = Dek::unwrap(&wrapped, &k).unwrap();

        let ct = dek.encrypt_str("test", "aad").unwrap();
        let pt = unwrapped.decrypt_str(&ct, "aad").unwrap();
        assert_eq!(pt, "test");
    }

    #[test]
    fn derive_kek_deterministic() {
        let salt = generate_salt();
        let kek1 = derive_kek("my-token", &salt).unwrap();
        let kek2 = derive_kek("my-token", &salt).unwrap();
        assert_eq!(kek1, kek2);
    }

    #[test]
    fn json_encrypt_decrypt() {
        let dek = Dek::generate();
        let value = serde_json::json!({"secret": "data", "num": 42});
        let encrypted = dek.encrypt_json(&value, "test.json").unwrap();
        assert!(matches!(encrypted, serde_json::Value::String(ref s) if s.starts_with(ENC_PREFIX)));
        let decrypted = dek.decrypt_json(&encrypted, "test.json").unwrap();
        assert_eq!(decrypted, value);
    }
}
