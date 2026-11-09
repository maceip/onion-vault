//! Integration tests for the OpenPGP key manager of OnionVault.
//!
//! The tests prove the three properties the application relies on:
//!
//!   * the secret key material is wrapped by the master key before it
//!     reaches the database, and only the master key can unwrap it again
//!     (the software master key is the stand in for the Android Keystore),
//!   * a message that is encrypted for one recipient and signed by another
//!     is decrypted byte for byte, and the signature is attributed to the
//!     certificate of the signer,
//!   * a message that is encrypted for somebody else cannot be decrypted
//!     with the wrong key, and the database never holds a secret key in
//!     the clear.
//!
//! Every test is hermetic: it works on a temporary directory below
//! `std::env::temp_dir()` that is removed when the test ends.  No network
//! access and no external service is required.
//!
//! The key pairs are generated with sequoia-openpgp: the first test uses the
//! key manager's own generation path, the other tests generate the
//! certificates directly, because the key manager attributes a signature to
//! the certificate whose *primary* key made it.

use std::{io::Write, path::Path, sync::Arc};

use ov_pgp::{Keyring, KeyringStore, MasterKeyProvider, SoftwareMasterKey};
use sequoia_openpgp::{
    armor,
    cert::{Cert, CertBuilder, CipherSuite},
    parse::Parse,
    serialize::Marshal,
    types::{KeyFlags, PublicKeyAlgorithm},
};

/// A temporary directory that is removed when it goes out of scope.
struct TempDir(std::path::PathBuf);

impl TempDir {
    /// Create a temporary directory that belongs to this test.
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("ov-pgp-it-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("cannot create the test directory");
        Self(path)
    }

    /// The path of the directory.
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Generate an ephemeral certificate with an Ed25519 primary key.
///
/// The primary key is signing capable, so that the key manager can
/// attribute a signature to the certificate that owns it, and the
/// certificate carries an ECDH subkey for the transport encryption.  This
/// mirrors what the key manager itself generates, but with a signing
/// primary key.
fn generate_cert(userid: &str) -> anyhow::Result<Cert> {
    let (cert, _revocation) = CertBuilder::new()
        .set_cipher_suite(CipherSuite::Cv25519)
        .set_primary_key_flags(KeyFlags::empty().set_certification().set_signing())
        .add_userid(userid)
        .add_subkey(KeyFlags::empty().set_transport_encryption(), None, None)
        .generate()?;
    Ok(cert)
}

/// The transferable secret key of `cert`, in its binary form.
fn transferable_secret_key(cert: &Cert) -> anyhow::Result<Vec<u8>> {
    let mut binary = Vec::new();
    cert.clone().into_tsk().serialize(&mut binary)?;
    Ok(binary)
}

/// The armored transferable secret key of `cert`.
fn armored_secret_key(cert: &Cert) -> anyhow::Result<String> {
    let binary = transferable_secret_key(cert)?;
    let mut sink = Vec::new();
    {
        let mut writer = armor::Writer::new(&mut sink, armor::Kind::SecretKey)?;
        writer.write_all(&binary)?;
        writer.finalize()?;
    }
    Ok(String::from_utf8(sink)?)
}

/// Parse an armored certificate.
fn parse_cert(armored: &str) -> anyhow::Result<Cert> {
    let mut reader =
        armor::Reader::from_bytes(armored.as_bytes(), armor::ReaderMode::Tolerant(None));
    let mut binary = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut binary)?;
    Cert::from_bytes(&binary)
}

/// `true` when `needle` occurs in `haystack`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// The contents of the database and of its write ahead log.
///
/// SQLite runs the key ring in WAL mode, so the rows that were written are
/// still in the log file; both files have to be scanned.
fn database_files(database: &Path) -> Vec<Vec<u8>> {
    let candidates = [
        database.to_path_buf(),
        database.with_extension("sqlite-wal"),
    ];
    candidates
        .iter()
        .filter(|path| path.exists())
        .map(|path| std::fs::read(path).expect("cannot read the key ring database"))
        .collect()
}

/// The master key of a fresh key ring.
///
/// The software master key is the stand in for the AES-256-GCM boundary of
/// the Android Keystore.
fn master_key() -> anyhow::Result<Arc<SoftwareMasterKey>> {
    Ok(Arc::new(SoftwareMasterKey::generate()?))
}

/// The key manager generates two independent key pairs with an Ed25519
/// primary key, and it stores the secret part of both of them.
#[test]
fn two_ephemeral_key_pairs_are_generated_with_an_ed25519_primary_key() -> anyhow::Result<()> {
    let dir = TempDir::new("generation");
    let store = KeyringStore::open(&dir.path().join("keyring.sqlite"))?;
    let keyring = Keyring::open(store, master_key()?);

    let alice = keyring.generate_keypair("Alice <alice@example.invalid>")?;
    let bob = keyring.generate_keypair("Bob <bob@example.invalid>")?;
    assert_ne!(alice, bob, "every key pair must have its own fingerprint");

    let certificates = keyring.certificates()?;
    assert_eq!(
        certificates.len(),
        2,
        "both key pairs must be in the key ring"
    );
    for stored in &certificates {
        assert!(
            stored.has_secret,
            "the secret key of {} must be stored",
            stored.fingerprint
        );
        let cert = parse_cert(&stored.armored)?;
        assert_eq!(cert.fingerprint(), stored.fingerprint);
        assert_eq!(
            cert.primary_key().pk_algo(),
            PublicKeyAlgorithm::EdDSA,
            "the primary key of {} must be an Ed25519 key",
            stored.fingerprint
        );
        assert!(
            cert.keys().secret().next().is_none(),
            "the exported certificate must not carry the secret key"
        );
    }

    Ok(())
}

/// The master key wraps the secret key material: the wrapped blob differs
/// from the plaintext, unwrapping returns the original bytes, and a
/// different key cannot unwrap the blob.
#[test]
fn the_master_key_wraps_the_secret_key_material() -> anyhow::Result<()> {
    let dir = TempDir::new("wrapping");
    let database = dir.path().join("keyring.sqlite");
    let master = master_key()?;

    let fingerprint = {
        let store = KeyringStore::open(&database)?;
        let keyring = Keyring::open(store, master.clone());
        keyring.generate_keypair("Sender <sender@example.invalid>")?
    };

    // A second connection to the database shows what the key manager
    // wrote to the disk.
    let store = KeyringStore::open(&database)?;
    let wrapped = store
        .secret(&fingerprint)?
        .expect("the secret key must be stored in the database");
    let plaintext = master.unwrap(&wrapped)?;
    assert_ne!(
        wrapped, plaintext,
        "the secret key must not be stored in the clear"
    );

    // The unwrapped material really is a transferable secret key.
    let cert = Cert::from_bytes(&plaintext)?;
    assert!(
        cert.keys().secret().next().is_some(),
        "the unwrapped material must carry the secret key"
    );

    // Wrapping the same material again produces a blob that is neither
    // the plaintext nor the blob in the database, and it unwraps to the
    // original bytes.
    let wrapped_again = master.wrap(&plaintext)?;
    assert_ne!(wrapped_again, plaintext);
    assert_ne!(wrapped_again, wrapped, "every wrap must use a fresh nonce");
    assert_eq!(master.unwrap(&wrapped_again)?, plaintext);

    // A different master key cannot unwrap either blob.
    let other = SoftwareMasterKey::generate()?;
    assert!(other.unwrap(&wrapped).is_err());
    assert!(other.unwrap(&wrapped_again).is_err());

    Ok(())
}

/// A message that is encrypted for Bob and signed by Alice is decrypted
/// byte for byte, and the signature is verified and attributed to Alice.
#[test]
fn a_message_encrypted_for_bob_and_signed_by_alice_is_decrypted_and_verified() -> anyhow::Result<()>
{
    let dir = TempDir::new("roundtrip");
    let store = KeyringStore::open(&dir.path().join("keyring.sqlite"))?;
    let keyring = Keyring::open(store, master_key()?);

    let alice = generate_cert("Alice <alice@example.invalid>")?;
    let bob = generate_cert("Bob <bob@example.invalid>")?;
    let fingerprint_alice = keyring.import_cert(&transferable_secret_key(&alice)?)?;
    let fingerprint_bob = keyring.import_cert(&transferable_secret_key(&bob)?)?;
    assert_eq!(fingerprint_alice, alice.fingerprint());
    assert_eq!(fingerprint_bob, bob.fingerprint());

    // The plaintext contains a multi byte character, so that the test
    // really compares bytes and not characters.
    let plaintext = "onion vault says: 🧅 42\n";
    let message = keyring.encrypt_armored(
        plaintext,
        std::slice::from_ref(&fingerprint_bob),
        Some(&fingerprint_alice),
    )?;
    assert!(message.starts_with("-----BEGIN PGP MESSAGE-----"));
    assert!(
        !message.contains(plaintext),
        "the armored message must not contain the plaintext"
    );

    let result = keyring.decrypt_armored(&message)?;
    assert_eq!(
        result.plaintext.as_bytes(),
        plaintext.as_bytes(),
        "the decrypted plaintext must be the original one"
    );
    assert_eq!(
        result.decrypted_with.as_ref(),
        Some(&fingerprint_bob),
        "the message must have been decrypted with Bob's key"
    );

    let signature = result
        .signatures
        .iter()
        .find(|signature| signature.verified)
        .expect("the signature of Alice must be verified");
    assert_eq!(
        signature.fingerprint.as_ref(),
        Some(&fingerprint_alice),
        "the signature must be attributed to Alice"
    );
    assert_eq!(signature.error, None);

    Ok(())
}

/// A message that is encrypted for Alice cannot be decrypted with Bob's
/// key, not even when Bob's secret key is available.
#[test]
fn a_message_encrypted_for_alice_cannot_be_decrypted_with_bob() -> anyhow::Result<()> {
    let dir = TempDir::new("foreign");
    let store = KeyringStore::open(&dir.path().join("keyring.sqlite"))?;
    let keyring = Keyring::open(store, master_key()?);

    let alice = generate_cert("Alice <alice@example.invalid>")?;
    let bob = generate_cert("Bob <bob@example.invalid>")?;
    let fingerprint_alice = keyring.import_cert(&transferable_secret_key(&alice)?)?;
    keyring.import_cert(&transferable_secret_key(&bob)?)?;

    let message = keyring.encrypt_armored("private", &[fingerprint_alice], None)?;
    assert!(message.starts_with("-----BEGIN PGP MESSAGE-----"));

    // A second key ring that only holds Bob, including his secret key.
    let other_dir = TempDir::new("foreign-other");
    let other_store = KeyringStore::open(&other_dir.path().join("keyring.sqlite"))?;
    let other = Keyring::open(other_store, master_key()?);
    other.import_cert(&transferable_secret_key(&bob)?)?;

    assert!(
        other.decrypt_armored(&message).is_err(),
        "Bob must not be able to decrypt a message that is encrypted for Alice"
    );

    Ok(())
}

/// The database never contains the secret key in the clear: neither the
/// armored private key block nor the binary secret key material are ever
/// written to the disk, while the public certificate is.
#[test]
fn the_keyring_database_never_holds_the_secret_key_in_the_clear() -> anyhow::Result<()> {
    let dir = TempDir::new("secrecy");
    let database = dir.path().join("keyring.sqlite");
    let store = KeyringStore::open(&database)?;
    let keyring = Keyring::open(store, master_key()?);

    let alice = generate_cert("Alice <alice@example.invalid>")?;
    let fingerprint = keyring.import_cert(&transferable_secret_key(&alice)?)?;
    keyring.generate_keypair("Bob <bob@example.invalid>")?;

    // The armored secret key of Alice is the marker that must never show
    // up in the database.
    let armored_secret = armored_secret_key(&alice)?;
    assert!(armored_secret.starts_with("-----BEGIN PGP PRIVATE KEY BLOCK-----"));

    let files = database_files(&database);
    assert!(!files.is_empty(), "the key ring database must exist");
    for bytes in &files {
        assert!(
            !contains(bytes, b"-----BEGIN PGP PRIVATE KEY BLOCK-----"),
            "the database contains an armored private key"
        );
    }

    // The tail of the armored secret key is the base64 of the secret key
    // material; it must not be in the database either.  (The head of the
    // block is the public part, which is stored in the clear on purpose.)
    let payload: Vec<&str> = armored_secret
        .lines()
        .filter(|line| !line.starts_with("-----") && !line.contains(": "))
        .collect();
    let tail = payload
        .last()
        .expect("the armored secret key must have a payload")
        .to_string();
    for bytes in &files {
        assert!(
            !contains(bytes, tail.as_bytes()),
            "the database contains the base64 of the secret key material"
        );
    }

    // The binary secret key material must not be in the database either.
    let binary_secret = transferable_secret_key(&alice)?;
    for bytes in &files {
        assert!(
            !contains(bytes, &binary_secret),
            "the database contains the binary secret key material"
        );
    }

    // The public certificate is stored in the clear, so the scan above is
    // not vacuous.
    let armored_public = keyring.export_armored_public_key(&fingerprint)?;
    assert!(
        files
            .iter()
            .any(|bytes| contains(bytes, armored_public.as_bytes())),
        "the public certificate must be stored in the clear"
    );

    Ok(())
}
