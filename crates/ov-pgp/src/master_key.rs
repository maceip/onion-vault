//! Wrapping of the secret key material.
//!
//! Transferable secret keys (TSKs) must never touch the disk in the clear.
//! PlugTalk therefore encrypts every TSK with an AES-256-GCM *master
//! wrapping key* before it is written into the SQLite keyring.
//!
//! On Android the master wrapping key lives inside the hardware backed
//! `AndroidKeyStore` (see [`crate::keystore`]); the cipher operations are
//! performed by the Keystore itself, so the key material never enters the
//! application process.  On other platforms (unit tests, desktop builds) an
//! equivalent software key is used.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, bail, Result};
use rand_core::OsRng;

/// Version byte of the wrapping format.
const FORMAT_VERSION: u8 = 0x01;
/// Length of the GCM nonce in bytes.
const NONCE_LEN: usize = 12;
/// Length of the AES-256 key in bytes.
const KEY_LEN: usize = 32;

/// A key that can wrap and unwrap secret key material.
pub trait MasterKeyProvider: Send + Sync {
    /// Encrypt `plaintext` and return an opaque blob.
    fn wrap(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
    /// Decrypt a blob produced by [`MasterKeyProvider::wrap`].
    fn unwrap(&self, blob: &[u8]) -> Result<Vec<u8>>;
    /// A human readable description of the backend, shown in the UI.
    fn describe(&self) -> String;
}

/// Split a wrapping blob into its nonce and ciphertext.
fn split_blob(blob: &[u8]) -> Result<(&[u8], &[u8])> {
    if blob.len() < 1 + NONCE_LEN + 16 {
        bail!("the wrapped blob is too short ({} bytes)", blob.len());
    }
    if blob[0] != FORMAT_VERSION {
        bail!("unsupported wrapping format version {}", blob[0]);
    }
    Ok((&blob[1..1 + NONCE_LEN], &blob[1 + NONCE_LEN..]))
}

/// A software master key, used on platforms without an Android Keystore.
pub struct SoftwareMasterKey {
    key: [u8; KEY_LEN],
}

impl SoftwareMasterKey {
    /// Generate a fresh master key from the operating system CSPRNG.
    pub fn generate() -> Result<Self> {
        let mut key = [0u8; KEY_LEN];
        getrandom::fill(&mut key).map_err(|e| anyhow!("the system CSPRNG failed: {e}"))?;
        Ok(Self { key })
    }

    /// Import a master key from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != KEY_LEN {
            bail!(
                "a master key must be {KEY_LEN} bytes long, got {}",
                bytes.len()
            );
        }
        let mut key = [0u8; KEY_LEN];
        key.copy_from_slice(bytes);
        Ok(Self { key })
    }
}

impl Drop for SoftwareMasterKey {
    fn drop(&mut self) {
        // Best effort scrubbing of the key material.
        let mut key = self.key;
        key.fill(0);
    }
}

impl MasterKeyProvider for SoftwareMasterKey {
    fn wrap(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow!("cannot wrap the secret key: {e}"))?;

        let mut blob = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
        blob.push(FORMAT_VERSION);
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    fn unwrap(&self, blob: &[u8]) -> Result<Vec<u8>> {
        let (nonce, ciphertext) = split_blob(blob)?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| anyhow!("cannot unwrap the secret key: wrong master key or damaged data"))
    }

    fn describe(&self) -> String {
        "software master key (AES-256-GCM)".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrapped_blob_round_trips() {
        let master = SoftwareMasterKey::generate().unwrap();
        let secret = b"transferable secret key material";
        let blob = master.wrap(secret).unwrap();
        assert_ne!(blob, secret.to_vec());
        assert_eq!(master.unwrap(&blob).unwrap(), secret);
    }

    #[test]
    fn a_damaged_blob_is_rejected() {
        let master = SoftwareMasterKey::generate().unwrap();
        let mut blob = master.wrap(b"secret").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0xff;
        assert!(master.unwrap(&blob).is_err());
    }

    #[test]
    fn another_key_cannot_unwrap() {
        let a = SoftwareMasterKey::generate().unwrap();
        let b = SoftwareMasterKey::generate().unwrap();
        let blob = a.wrap(b"secret").unwrap();
        assert!(b.unwrap(&blob).is_err());
    }
}
