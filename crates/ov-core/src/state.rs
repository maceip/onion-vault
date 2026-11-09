//! The shared state of the browser engine.

use std::sync::Arc;

use cef::Browser;
use ov_cef::{BrowserSlot, OvConfig, SnapshotCollector};
use ov_pgp::Keyring;
use ov_tor::TorService;

use crate::surface::SurfaceRenderer;

/// Everything that the JNI layer, the CEF handlers and the Tor client share.
pub struct EngineState {
    /// The browser, once CEF created it.
    pub slot: Arc<BrowserSlot>,
    /// The OpenPGP key manager.
    pub keyring: Arc<Keyring>,
    /// The embedded Tor client.
    pub tor: Arc<TorService>,
    /// The renderer that copies the frames into the Android surface.
    pub surface: Arc<SurfaceRenderer>,
    /// The configuration of the CEF engine.
    pub config: Arc<OvConfig>,
    /// The field actions that the native UI requested.
    pub pending: ov_cef::PendingActions,
    /// The collector of the page snapshots.
    ///
    /// The snapshots are the state of the pages that were visited: the
    /// native UI shows them in the preview of the predictive back gesture.
    pub snapshots: Arc<SnapshotCollector>,
}

impl EngineState {
    /// The browser, if CEF created it already.
    pub fn browser(&self) -> Option<Browser> {
        self.slot.get()
    }

    /// The fingerprints of the certificates that own a secret key.
    ///
    /// These are the "own" keys of the user: messages are encrypted for
    /// them and their public key is published when the user asks for it.
    pub fn own_fingerprints(&self) -> Vec<sequoia_openpgp::Fingerprint> {
        match self.keyring.certificates() {
            Ok(certificates) => certificates
                .into_iter()
                .filter(|certificate| certificate.has_secret)
                .map(|certificate| certificate.fingerprint)
                .collect(),
            Err(err) => {
                log::error!("cannot read the keyring: {err:#}");
                Vec::new()
            }
        }
    }
}
