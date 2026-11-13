//! The OpenPGP key manager.
//!
//! Everything that touches key material lives here: key generation, import,
//! export, encryption and decryption.  The browser (CEF) never sees a private
//! key: the browser process asks the key manager for the result of a
//! cryptographic operation and only ever receives the resulting text.

use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::Arc,
};

use anyhow::{anyhow, bail, Context, Result};
use sequoia_openpgp as openpgp;
use sequoia_openpgp::{
    armor,
    cert::{Cert, CertBuilder, CipherSuite},
    crypto::SessionKey,
    parse::{
        stream::{DecryptionHelper, DecryptorBuilder, MessageLayer, VerificationHelper},
        Parse,
    },
    policy::StandardPolicy,
    serialize::{
        stream::{Armorer, Encryptor2, LiteralWriter, Message, Signer},
        Marshal,
    },
    types::SymmetricAlgorithm,
    Fingerprint, KeyHandle, KeyID,
};

use crate::{
    master_key::MasterKeyProvider,
    store::{KeyringStore, StoredCertificate},
};

/// The result of a decryption.
#[derive(Clone, Debug)]
pub struct DecryptionResult {
    /// The decrypted plaintext.
    pub plaintext: String,
    /// The signatures that were found in the message.
    pub signatures: Vec<SignatureInfo>,
    /// The fingerprint of the secret key the message was decrypted with.
    pub decrypted_with: Option<Fingerprint>,
}

/// Information about a single signature.
#[derive(Clone, Debug)]
pub struct SignatureInfo {
    /// The key id of the signer.
    pub key_id: KeyID,
    /// The fingerprint of the signer, if the certificate is in the keyring.
    pub fingerprint: Option<Fingerprint>,
    /// `true` when the signature could be verified.
    pub verified: bool,
    /// A human readable reason for a failed verification.
    pub error: Option<String>,
}

/// The key manager.
pub struct Keyring {
    store: KeyringStore,
    master: Arc<dyn MasterKeyProvider>,
    policy: StandardPolicy<'static>,
}

impl Keyring {
    /// Open a keyring that is protected by `master`.
    pub fn open(store: KeyringStore, master: Arc<dyn MasterKeyProvider>) -> Self {
        Self {
            store,
            master,
            policy: StandardPolicy::new(),
        }
    }

    /// A description of the master key backend.
    pub fn master_key_description(&self) -> String {
        self.master.describe()
    }

    /// The policy that is applied to all certificates.
    pub fn policy(&self) -> &StandardPolicy<'static> {
        &self.policy
    }

    /// All certificates in the keyring.
    pub fn certificates(&self) -> Result<Vec<StoredCertificate>> {
        self.store.certificates()
    }

    /// Generate a new key pair for `userid` and store it.
    ///
    /// The private key is wrapped with the master key before it reaches the
    /// database.
    pub fn generate_keypair(&self, userid: &str) -> Result<Fingerprint> {
        let (cert, _signature) = CertBuilder::general_purpose(CipherSuite::default(), Some(userid))
            .generate()
            .map_err(|e| anyhow!("cannot generate a key pair: {e}"))?;

        let fingerprint = cert.fingerprint();

        // The transferable secret key: every packet of the certificate,
        // including the secret key material.
        let tsk = serialize_transferable_secret_key(&cert)?
            .ok_or_else(|| anyhow!("the generated key has no secret material"))?;
        let wrapped = self.master.wrap(&tsk)?;

        let armored = armor_public_key(&cert)?;
        self.store.put_certificate(&fingerprint, &armored)?;
        self.store.put_secret(&fingerprint, &wrapped)?;

        log::info!("generated a new key pair {fingerprint} for {userid:?}");
        Ok(fingerprint)
    }

    /// Import a certificate (armored or binary) into the keyring.
    ///
    /// Secret keys are imported as well; their material is wrapped with the
    /// master key before it is written to the database.
    pub fn import_cert(&self, data: &[u8]) -> Result<Fingerprint> {
        let cert = parse_cert(data)?;
        let fingerprint = cert.fingerprint();

        let armored = armor_public_key(&cert)?;
        self.store.put_certificate(&fingerprint, &armored)?;

        // Only store a secret key when the imported certificate carries one.
        if let Some(tsk) = serialize_transferable_secret_key(&cert)? {
            let wrapped = self.master.wrap(&tsk)?;
            self.store.put_secret(&fingerprint, &wrapped)?;
            log::info!("imported the certificate {fingerprint} including its secret key");
        } else {
            log::info!("imported the certificate {fingerprint}");
        }

        Ok(fingerprint)
    }

    /// The armored public key of `fingerprint`.
    pub fn export_armored_public_key(&self, fingerprint: &Fingerprint) -> Result<String> {
        self.store
            .certificate(fingerprint)?
            .ok_or_else(|| anyhow!("no certificate {fingerprint} in the keyring"))
    }

    /// Remove the secret key of `fingerprint`, keeping the public part.
    pub fn delete_secret_key(&self, fingerprint: &Fingerprint) -> Result<bool> {
        self.store.remove_secret(fingerprint)
    }

    /// Encrypt `plaintext` for `recipients` and optionally sign it.
    ///
    /// The result is an ASCII armored OpenPGP message.
    pub fn encrypt_armored(
        &self,
        plaintext: &str,
        recipients: &[Fingerprint],
        sign_with: Option<&Fingerprint>,
    ) -> Result<String> {
        if recipients.is_empty() {
            bail!("no recipient given");
        }

        // The certificates have to outlive the references to their keys.
        let certs = recipients
            .iter()
            .map(|fingerprint| self.certificate_for(fingerprint))
            .collect::<Result<Vec<_>>>()?;

        let mut recipient_keys = Vec::new();
        for (fingerprint, cert) in recipients.iter().zip(certs.iter()) {
            let policy = &self.policy;
            let keys = cert
                .keys()
                .with_policy(policy, None)
                .supported()
                .alive()
                .revoked(false)
                .for_transport_encryption()
                .map(|key| key.key())
                .collect::<Vec<_>>();
            if keys.is_empty() {
                bail!("the certificate {fingerprint} has no usable encryption subkey");
            }
            recipient_keys.extend(keys);
        }

        let signer = match sign_with {
            Some(fingerprint) => Some(self.signing_keypair(fingerprint)?),
            None => None,
        };

        let mut sink = Vec::new();
        {
            let message = Message::new(&mut sink);
            let message = Armorer::new(message)
                .kind(armor::Kind::Message)
                .build()
                .map_err(|e| anyhow!("cannot armourize the message: {e}"))?;
            let message = match signer {
                Some(keypair) => Signer::new(message, keypair)
                    .build()
                    .map_err(|e| anyhow!("cannot sign the message: {e}"))?,
                None => message,
            };
            let message = Encryptor2::for_recipients(message, recipient_keys)
                .build()
                .map_err(|e| anyhow!("cannot encrypt the message: {e}"))?;
            let mut literal = LiteralWriter::new(message)
                .format(openpgp::types::DataFormat::Text)
                .build()
                .map_err(|e| anyhow!("cannot create the literal writer: {e}"))?;
            literal
                .write_all(plaintext.as_bytes())
                .map_err(|e| anyhow!("cannot write the plaintext: {e}"))?;
            literal
                .finalize()
                .map_err(|e| anyhow!("cannot finalise the message: {e}"))?;
        }

        String::from_utf8(sink).map_err(|e| anyhow!("the armored message is not valid UTF-8: {e}"))
    }

    /// Decrypt an ASCII armored OpenPGP message.
    pub fn decrypt_armored(&self, ciphertext: &str) -> Result<DecryptionResult> {
        let payload = dearmor_if_needed(ciphertext.as_bytes())?;
        let helper = KeyringHelper {
            store: &self.store,
            master: self.master.as_ref(),
            signatures: Vec::new(),
            decrypted_with: None,
        };

        let mut decryptor = DecryptorBuilder::from_bytes(&payload)
            .map_err(|e| anyhow!("cannot read the OpenPGP message: {e}"))?
            .with_policy(&self.policy, None, helper)
            .map_err(|e| anyhow!("cannot decrypt the OpenPGP message: {e}"))?;

        let mut plaintext = Vec::new();
        decryptor
            .read_to_end(&mut plaintext)
            .map_err(|e| anyhow!("cannot read the decrypted message: {e}"))?;
        let helper = decryptor.into_helper();

        Ok(DecryptionResult {
            plaintext: String::from_utf8(plaintext)
                .map_err(|e| anyhow!("the decrypted message is not valid UTF-8: {e}"))?,
            signatures: helper.signatures,
            decrypted_with: helper.decrypted_with,
        })
    }

    /// The certificate of `fingerprint`.
    fn certificate_for(&self, fingerprint: &Fingerprint) -> Result<Cert> {
        let armored = self
            .store
            .certificate(fingerprint)?
            .ok_or_else(|| anyhow!("no certificate {fingerprint} in the keyring"))?;
        parse_cert(armored.as_bytes())
    }

    /// The signing key of `fingerprint`.
    ///
    /// The primary key of a generated certificate is only certification
    /// capable; the signatures are therefore made with the signing subkey.
    fn signing_keypair(&self, fingerprint: &Fingerprint) -> Result<openpgp::crypto::KeyPair> {
        let wrapped = self
            .store
            .secret(fingerprint)?
            .ok_or_else(|| anyhow!("no secret key {fingerprint} in the keyring"))?;
        let tsk = self
            .master
            .unwrap(&wrapped)
            .context("cannot unwrap the secret key")?;
        let cert = parse_cert(&tsk)?;

        let policy = &self.policy;
        let candidates = cert
            .keys()
            .with_policy(policy, None)
            .supported()
            .alive()
            .revoked(false)
            .for_signing()
            .collect::<Vec<_>>();

        for candidate in candidates {
            let secret = match candidate.key().clone().parts_into_secret() {
                Ok(secret) => secret,
                Err(err) => {
                    log::warn!("the key {fingerprint} has no secret material: {err:#}");
                    continue;
                }
            };
            match secret.into_keypair() {
                Ok(keypair) => return Ok(keypair),
                Err(err) => {
                    log::warn!("cannot use the signing key of {fingerprint}: {err:#}");
                }
            }
        }

        bail!("the key {fingerprint} has no usable signing key")
    }
}

/// Helper that feeds sequoia with the certificates and secret keys of the
/// keyring and collects the signature verification results.
struct KeyringHelper<'a> {
    store: &'a KeyringStore,
    master: &'a dyn MasterKeyProvider,
    signatures: Vec<SignatureInfo>,
    decrypted_with: Option<Fingerprint>,
}

impl KeyringHelper<'_> {
    /// All certificates of the keyring that carry a secret key.
    fn secret_certificates(&self) -> Result<Vec<(Fingerprint, Cert)>> {
        let mut out = Vec::new();
        for stored in self.store.certificates()? {
            let Some(wrapped) = self.store.secret(&stored.fingerprint)? else {
                continue;
            };
            let tsk = self
                .master
                .unwrap(&wrapped)
                .with_context(|| format!("cannot unwrap the secret key {}", stored.fingerprint))?;
            let cert = parse_cert(&tsk)?;
            out.push((stored.fingerprint, cert));
        }
        Ok(out)
    }
}

impl VerificationHelper for KeyringHelper<'_> {
    fn get_certs(&mut self, keyids: &[KeyHandle]) -> Result<Vec<Cert>> {
        let mut certs = Vec::new();
        for stored in self.store.certificates()? {
            let Ok(cert) = parse_cert(stored.armored.as_bytes()) else {
                continue;
            };
            let matches = cert
                .keys()
                .with_policy(&StandardPolicy::new(), None)
                .any(|key| {
                    let handle = key.key().key_handle();
                    keyids.iter().any(|wanted| handle.aliases(wanted))
                });
            if matches {
                certs.push(cert);
            }
        }
        Ok(certs)
    }

    fn check(&mut self, structure: openpgp::parse::stream::MessageStructure) -> Result<()> {
        for layer in structure {
            let MessageLayer::SignatureGroup { results } = layer else {
                continue;
            };
            for result in results {
                match result {
                    Ok(good) => {
                        let key_id = signature_key_id(good.sig);
                        let fingerprint = self.fingerprint_of(&key_id);
                        self.signatures.push(SignatureInfo {
                            key_id,
                            fingerprint,
                            verified: true,
                            error: None,
                        });
                    }
                    Err(err) => {
                        let key_id = verification_error_key_id(&err);
                        let fingerprint = self.fingerprint_of(&key_id);
                        self.signatures.push(SignatureInfo {
                            key_id,
                            fingerprint,
                            verified: false,
                            error: Some(err.to_string()),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

impl KeyringHelper<'_> {
    /// The fingerprint of the certificate that owns `key_id`.
    fn fingerprint_of(&self, key_id: &KeyID) -> Option<Fingerprint> {
        let stored = self.store.certificates().ok()?;
        for cert in stored {
            if cert.fingerprint.aliases(KeyHandle::KeyID(key_id.clone())) {
                return Some(cert.fingerprint);
            }
        }
        None
    }
}

impl DecryptionHelper for KeyringHelper<'_> {
    fn decrypt<D>(
        &mut self,
        pkesks: &[openpgp::packet::PKESK],
        _skesks: &[openpgp::packet::SKESK],
        sym_algo: Option<SymmetricAlgorithm>,
        mut decrypt: D,
    ) -> Result<Option<Fingerprint>>
    where
        D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool,
    {
        let secrets = self.secret_certificates()?;

        for pkesk in pkesks {
            let recipient = pkesk.recipient();
            for (fingerprint, cert) in &secrets {
                for key in cert.keys().secret() {
                    let key = key.key();
                    if key.keyid() != *recipient {
                        continue;
                    }
                    let mut keypair = match key.clone().parts_into_secret().map(|key| {
                        key.into_keypair()
                            .map_err(|e| anyhow!("cannot build the decryption key: {e}"))
                    }) {
                        Ok(Ok(keypair)) => keypair,
                        Ok(Err(err)) => {
                            log::warn!("cannot use the secret key {fingerprint}: {err:#}");
                            continue;
                        }
                        Err(err) => {
                            log::warn!("cannot use the secret key {fingerprint}: {err:#}");
                            continue;
                        }
                    };

                    let Some((algo, session_key)) = pkesk.decrypt(&mut keypair, None) else {
                        continue;
                    };
                    let algo = sym_algo.unwrap_or(algo);
                    if decrypt(algo, &session_key) {
                        self.decrypted_with = Some(fingerprint.clone());
                        return Ok(Some(fingerprint.clone()));
                    }
                }
            }
        }

        Ok(None)
    }
}

/// The key id of the signer of `signature`.
fn signature_key_id(signature: &openpgp::packet::Signature) -> KeyID {
    signature
        .get_issuers()
        .first()
        .map(key_id_of)
        .unwrap_or_else(KeyID::wildcard)
}

/// The key id of the signer of a failed signature.
fn verification_error_key_id(error: &openpgp::parse::stream::VerificationError<'_>) -> KeyID {
    use openpgp::parse::stream::VerificationError::*;
    let signature = match error {
        MalformedSignature { sig, .. } => sig,
        MissingKey { sig } => sig,
        UnboundKey { sig, .. } => sig,
        BadKey { sig, .. } => sig,
        BadSignature { sig, .. } => sig,
    };
    signature_key_id(signature)
}

/// The key id of a key handle.
fn key_id_of(handle: &KeyHandle) -> KeyID {
    match handle {
        KeyHandle::KeyID(key_id) => key_id.clone(),
        KeyHandle::Fingerprint(fingerprint) => {
            let bytes = fingerprint.as_bytes();
            let start = bytes.len().saturating_sub(8);
            KeyID::from_bytes(&bytes[start..])
        }
    }
}

/// Parse a certificate from armored or binary input.
fn parse_cert(data: &[u8]) -> Result<Cert> {
    match Cert::from_bytes(data) {
        Ok(cert) => Ok(cert),
        Err(first) => {
            let binary = dearmor_if_needed(data)?;
            if binary == data {
                bail!("cannot read the certificate: {first}");
            }
            Cert::from_bytes(&binary).map_err(|e| anyhow!("cannot read the certificate: {e}"))
        }
    }
}

/// Serialize the transferable secret key of `cert`.
///
/// Returns `None` when the certificate does not carry any secret material.
fn serialize_transferable_secret_key(cert: &Cert) -> Result<Option<Vec<u8>>> {
    if cert.keys().secret().next().is_none() {
        return Ok(None);
    }

    let mut buffer = Vec::new();
    let tsk = cert.clone().into_tsk();
    tsk.serialize(&mut buffer)
        .map_err(|e| anyhow!("cannot serialize the secret key: {e}"))?;
    Ok(Some(buffer))
}

/// ASCII armor the public part of `cert`.
fn armor_public_key(cert: &Cert) -> Result<String> {
    let mut public = Vec::new();
    cert.serialize(&mut public)
        .map_err(|e| anyhow!("cannot serialize the certificate: {e}"))?;

    let mut sink = Vec::new();
    {
        let mut writer = armor::Writer::new(&mut sink, armor::Kind::PublicKey)
            .map_err(|e| anyhow!("cannot create the armor writer: {e}"))?;
        writer
            .write_all(&public)
            .map_err(|e| anyhow!("cannot write the certificate: {e}"))?;
        writer
            .finalize()
            .map_err(|e| anyhow!("cannot finalize the armor: {e}"))?;
    }
    String::from_utf8(sink).map_err(|e| anyhow!("the armored certificate is not valid UTF-8: {e}"))
}

/// Strip the ASCII armor of `data`, if it carries any.
fn dearmor_if_needed(data: &[u8]) -> Result<Vec<u8>> {
    let is_armored = std::str::from_utf8(data)
        .map(|text| text.contains("-----BEGIN PGP"))
        .unwrap_or(false);
    if !is_armored {
        return Ok(data.to_vec());
    }

    // The armor reader takes care of the base64 decoding and of the CRC
    // checksum; it is tolerant about small deviations in the header.
    let mut reader = armor::Reader::from_bytes(data, armor::ReaderMode::Tolerant(None));
    let mut payload = Vec::new();
    reader
        .read_to_end(&mut payload)
        .map_err(|e| anyhow!("cannot decode the armored payload: {e}"))?;
    Ok(payload)
}

/// Helper that maps a key id to a fingerprint.
#[allow(dead_code)]
fn fingerprint_map(certs: &[Cert]) -> HashMap<KeyID, Fingerprint> {
    let mut map = HashMap::new();
    for cert in certs {
        map.insert(cert.keyid(), cert.fingerprint());
    }
    map
}
