//! Hardware backed OpenPGP key manager for OnionVault.
//!
//! The crate provides a Sequoia (RFC 4880 / RFC 9580) based keyring that
//!
//!   * stores certificates in a SQLite database inside the application
//!     sandbox,
//!   * never writes transferable secret keys to disk in the clear: they are
//!     wrapped with an AES-256-GCM master key that lives in the Android
//!     hardware Keystore,
//!   * performs all cryptographic operations in the browser process, so that
//!     the sandboxed CEF render process never sees a private key.

pub mod crypto;
pub mod keystore;
pub mod master_key;
pub mod store;

pub use crypto::{DecryptionResult, Keyring, SignatureInfo};
pub use master_key::{MasterKeyProvider, SoftwareMasterKey};
pub use store::{KeyringStore, StoredCertificate};

use std::sync::Arc;

use anyhow::{Context, Result};
use sequoia_openpgp::Fingerprint;

/// Parse a fingerprint in its hexadecimal representation.
pub fn parse_fingerprint(text: &str) -> Result<Fingerprint> {
    let trimmed = text.trim();
    let bytes = hex::decode(trimmed.replace(' ', ""))
        .with_context(|| format!("{trimmed} is not a hexadecimal fingerprint"))?;
    if bytes.len() != 20 && bytes.len() != 32 {
        anyhow::bail!("{trimmed} is not a valid fingerprint");
    }
    Ok(Fingerprint::from_bytes(&bytes))
}

/// The user id of an armored certificate.
///
/// The keyring stores the armored certificate; the user id is read from it
/// for the key manager of the native UI.
pub fn user_id_of(armored: &str) -> Option<String> {
    use sequoia_openpgp::parse::Parse;

    let cert = sequoia_openpgp::Cert::from_bytes(armored.as_bytes()).ok()?;
    let user_id = cert.userids().next()?;
    Some(String::from_utf8_lossy(user_id.value()).into_owned())
}

/// Build the master key provider for the current platform.
///
/// On Android the master wrapping key is created inside the hardware backed
/// `AndroidKeyStore`; everywhere else a software master key is used.
#[cfg(target_os = "android")]
pub fn master_key_provider(
    vm: jni::JavaVM,
    alias: &str,
    require_user_auth: bool,
) -> Result<Arc<dyn MasterKeyProvider>> {
    let provider = keystore::AndroidKeystoreMasterKey::open(vm, alias, require_user_auth)?;
    Ok(Arc::new(provider))
}

/// Build the master key provider for the current platform.
///
/// The Keystore is not available outside of Android, so a software master
/// key is used instead.
#[cfg(not(target_os = "android"))]
pub fn master_key_provider(
    _alias: &str,
    _require_user_auth: bool,
) -> Result<Arc<dyn MasterKeyProvider>> {
    log::warn!(
        "the Android Keystore is not available on this platform, using a software master key"
    );
    Ok(Arc::new(SoftwareMasterKey::generate()?))
}
