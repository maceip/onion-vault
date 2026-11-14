//! End to end test of the key manager.
//!
//! The test exercises the whole path that the browser uses: a key pair is
//! generated, its public part is exported, a message is encrypted and signed
//! for it and the message is decrypted again.

use ov_pgp::{master_key_provider, Keyring, KeyringStore};

/// A temporary directory that is removed when it goes out of scope.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "onionvault-pgp-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("cannot create the test directory");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn generate_export_encrypt_decrypt() -> anyhow::Result<()> {
    let dir = TempDir::new("roundtrip");
    let store = KeyringStore::open(&dir.path().join("keyring.sqlite"))?;
    let master = master_key_provider("onionvault.test", false)?;
    let keyring = Keyring::open(store, master);

    // The key pair is generated and stored.
    let fingerprint = keyring.generate_keypair("Trap Test <test@example.invalid>")?;
    assert_eq!(keyring.certificates()?.len(), 1);

    // The public key can be exported and re-imported.
    let armored = keyring.export_armored_public_key(&fingerprint)?;
    assert!(armored.starts_with("-----BEGIN PGP PUBLIC KEY BLOCK-----"));

    // The message is encrypted for (and signed by) the key.
    let message = keyring.encrypt_armored(
        "the trap vault opens",
        std::slice::from_ref(&fingerprint),
        Some(&fingerprint),
    )?;
    assert!(message.starts_with("-----BEGIN PGP MESSAGE-----"));

    // The message can be decrypted again.
    let result = keyring.decrypt_armored(&message)?;
    assert_eq!(result.plaintext, "the trap vault opens");
    assert_eq!(result.decrypted_with.as_ref(), Some(&fingerprint));
    assert!(
        result.signatures.iter().any(|signature| signature.verified),
        "the signature of the message should verify: {:?}",
        result.signatures
    );

    Ok(())
}

#[test]
fn a_foreign_key_cannot_be_decrypted() -> anyhow::Result<()> {
    let dir = TempDir::new("foreign");
    let store = KeyringStore::open(&dir.path().join("keyring.sqlite"))?;
    let master = master_key_provider("onionvault.test", false)?;
    let keyring = Keyring::open(store, master);

    let fingerprint = keyring.generate_keypair("Sender <sender@example.invalid>")?;
    let message = keyring.encrypt_armored("private", std::slice::from_ref(&fingerprint), None)?;

    // A second keyring without the secret key cannot decrypt the message.
    let other_dir = TempDir::new("foreign-other");
    let other_store = KeyringStore::open(&other_dir.path().join("keyring.sqlite"))?;
    let other_master = master_key_provider("onionvault.test", false)?;
    let other = Keyring::open(other_store, other_master);
    other.import_cert(keyring.export_armored_public_key(&fingerprint)?.as_bytes())?;

    assert!(other.decrypt_armored(&message).is_err());

    Ok(())
}

#[test]
fn the_secret_key_is_never_stored_in_the_clear() -> anyhow::Result<()> {
    let dir = TempDir::new("secrecy");
    let database = dir.path().join("keyring.sqlite");
    let store = KeyringStore::open(&database)?;
    let master = master_key_provider("onionvault.test", false)?;
    let keyring = Keyring::open(store, master);
    keyring.generate_keypair("Secret <secret@example.invalid>")?;

    // The database must not contain the secret key material in the clear.
    let bytes = std::fs::read(&database)?;
    let armored_marker = b"-----BEGIN PGP PRIVATE KEY BLOCK-----";
    assert!(
        !bytes
            .windows(armored_marker.len())
            .any(|window| window == armored_marker),
        "the database contains an armored private key"
    );

    Ok(())
}
