//! Cryptographic primitives for the KeyFlow vault.
//!
//! - Key derivation: Argon2id (RFC 9106), memory-hard, tuned per OWASP's
//!   password-storage cheat sheet for a *local* KDF (higher cost than a
//!   server would use, since the only rate limiter is the attacker's own
//!   hardware).
//! - Encryption: AES-256-GCM, a NIST-standard authenticated cipher. A
//!   fresh random 96-bit nonce is generated for every encryption call;
//!   nonces are never reused with the same key.
//! - Randomness: the OS CSPRNG only (`rand::rngs::OsRng`), never a
//!   userspace PRNG.
//!
//! No cryptography is invented here — only composition of audited,
//! widely-used primitives from the RustCrypto project.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::{KeyflowError, Result};

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 12;
pub const KEY_LEN: usize = 32;

/// Argon2id tuning parameters, persisted alongside the vault so a vault
/// created under one cost profile stays decryptable if defaults change
/// later. Values follow OWASP's "Argon2id, high" guidance for a
/// desktop/local threat model: 64 MiB memory, 3 iterations, 4 lanes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct KdfParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 4,
        }
    }
}

impl KdfParams {
    fn to_argon2_params(self) -> Result<Params> {
        Params::new(self.memory_kib, self.iterations, self.parallelism, Some(KEY_LEN))
            .map_err(|_| KeyflowError::CorruptVault)
    }
}

/// A derived 256-bit vault key. Zeroized on drop; never `Debug`- or
/// `Display`-formatted, never logged, never serialized.
#[derive(Clone, ZeroizeOnDrop)]
pub struct VaultKey([u8; KEY_LEN]);

impl VaultKey {
    pub fn derive(master_password: &str, salt: &[u8; SALT_LEN], params: KdfParams) -> Result<Self> {
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params.to_argon2_params()?);
        let mut out = [0u8; KEY_LEN];
        argon2
            .hash_password_into(master_password.as_bytes(), salt, &mut out)
            .map_err(|_| KeyflowError::CorruptVault)?;
        Ok(Self(out))
    }

    fn cipher(&self) -> Aes256Gcm {
        Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.0))
    }
}

/// Generates a cryptographically secure random salt for KDF input.
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    salt
}

fn generate_nonce() -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

/// An encrypted blob: a random nonce plus the AES-256-GCM ciphertext
/// (which includes the authentication tag). Safe to serialize and store;
/// contains no recoverable plaintext without the vault key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedBlob {
    #[serde(with = "base64_bytes")]
    pub nonce: Vec<u8>,
    #[serde(with = "base64_bytes")]
    pub ciphertext: Vec<u8>,
}

/// Encrypts `plaintext` with the vault key using AES-256-GCM.
///
/// `aad` (associated data) is authenticated but not encrypted — used to
/// bind the ciphertext to the vault's format version so an old blob can't
/// be replayed under a newer schema.
pub fn seal(key: &VaultKey, plaintext: &[u8], aad: &[u8]) -> Result<SealedBlob> {
    let nonce_bytes = generate_nonce();
    let cipher = key.cipher();
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload { msg: plaintext, aad },
        )
        .map_err(|_| KeyflowError::CorruptVault)?;
    Ok(SealedBlob {
        nonce: nonce_bytes.to_vec(),
        ciphertext,
    })
}

/// Decrypts a [`SealedBlob`]. Fails authentication (and returns
/// `AuthenticationFailed`) if the key is wrong, the AAD doesn't match, or
/// the blob was tampered with in any way — GCM's tag check covers all
/// three uniformly, so we deliberately don't distinguish them to callers
/// (that would leak an oracle).
pub fn open(key: &VaultKey, blob: &SealedBlob, aad: &[u8]) -> Result<Vec<u8>> {
    if blob.nonce.len() != NONCE_LEN {
        return Err(KeyflowError::CorruptVault);
    }
    let cipher = key.cipher();
    let mut plaintext = cipher
        .decrypt(
            Nonce::from_slice(&blob.nonce),
            Payload {
                msg: &blob.ciphertext,
                aad,
            },
        )
        .map_err(|_| KeyflowError::AuthenticationFailed)?;
    let result = plaintext.clone();
    plaintext.zeroize();
    Ok(result)
}

mod base64_bytes {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        STANDARD.decode(s.as_bytes()).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_roundtrip() {
        let salt = generate_salt();
        let key = VaultKey::derive("correct horse battery staple", &salt, KdfParams::default()).unwrap();
        let plaintext = b"top secret credential data";
        let blob = seal(&key, plaintext, b"vault-v1").unwrap();
        let opened = open(&key, &blob, b"vault-v1").unwrap();
        assert_eq!(opened, plaintext);
    }

    #[test]
    fn wrong_password_fails_authentication() {
        let salt = generate_salt();
        let right_key = VaultKey::derive("right-password", &salt, KdfParams::default()).unwrap();
        let wrong_key = VaultKey::derive("wrong-password", &salt, KdfParams::default()).unwrap();
        let blob = seal(&right_key, b"secret", b"aad").unwrap();
        assert!(matches!(open(&wrong_key, &blob, b"aad"), Err(KeyflowError::AuthenticationFailed)));
    }

    #[test]
    fn tampered_ciphertext_fails_authentication() {
        let salt = generate_salt();
        let key = VaultKey::derive("password", &salt, KdfParams::default()).unwrap();
        let mut blob = seal(&key, b"secret", b"aad").unwrap();
        let last = blob.ciphertext.len() - 1;
        blob.ciphertext[last] ^= 0xFF;
        assert!(open(&key, &blob, b"aad").is_err());
    }

    #[test]
    fn mismatched_aad_fails_authentication() {
        let salt = generate_salt();
        let key = VaultKey::derive("password", &salt, KdfParams::default()).unwrap();
        let blob = seal(&key, b"secret", b"vault-v1").unwrap();
        assert!(open(&key, &blob, b"vault-v2").is_err());
    }

    #[test]
    fn nonces_are_not_reused_across_calls() {
        let salt = generate_salt();
        let key = VaultKey::derive("password", &salt, KdfParams::default()).unwrap();
        let a = seal(&key, b"same plaintext", b"aad").unwrap();
        let b = seal(&key, b"same plaintext", b"aad").unwrap();
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.ciphertext, b.ciphertext);
    }

    #[test]
    fn same_password_and_salt_derive_identical_keys() {
        let salt = generate_salt();
        let k1 = VaultKey::derive("pw", &salt, KdfParams::default()).unwrap();
        let k2 = VaultKey::derive("pw", &salt, KdfParams::default()).unwrap();
        // Indirect check: a blob sealed under k1 must open under k2.
        let blob = seal(&k1, b"x", b"").unwrap();
        assert!(open(&k2, &blob, b"").is_ok());
    }
}
