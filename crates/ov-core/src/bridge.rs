//! The bridge between the CEF browser process and the native application.
//!
//! The bridge receives the DOM scan results and the decryption requests of
//! the render process.  All cryptographic operations happen here, in the
//! browser process: the sandboxed render process never sees a private key
//! and never sees the decrypted plaintext.

use std::sync::Arc;

use ov_cef::{
    BrowserBridge, DecryptRequest, DecryptResponse, DomScanReport, FieldAction, FieldValue,
    MutationResult, NavigationInfo, PageSnapshot,
};

use crate::{native::NativeArg, state::EngineState};

/// The implementation of the CEF bridge.
pub struct OvBridge {
    /// The shared engine state.
    state: Arc<EngineState>,
}

impl OvBridge {
    /// Create the bridge.
    pub fn new(state: Arc<EngineState>) -> Self {
        Self { state }
    }

    /// Send the DOM scan report to the native UI.
    fn report_dom_scan(&self, report: &DomScanReport) {
        let payload = match serde_json::to_string(report) {
            Ok(payload) => payload,
            Err(err) => {
                log::error!("cannot serialize the DOM scan report: {err}");
                return;
            }
        };
        crate::native::call_native("onDomScan", &[NativeArg::String(payload)]);
    }

    /// Show the decrypted plaintext in the native sheet.
    ///
    /// The plaintext is deliberately *not* written back into the page: a
    /// hostile hidden service could read it back through CSS (for example
    /// with an `input[value^="a"]` rule that leaks the first character to a
    /// remote server).
    fn show_plaintext(&self, node_id: &str, title: &str, body: &str, verified: bool) {
        crate::native::call_native(
            "showPlaintext",
            &[
                NativeArg::String(node_id.to_string()),
                NativeArg::String(title.to_string()),
                NativeArg::String(body.to_string()),
                NativeArg::Bool(verified),
            ],
        );
    }

    /// Report the outcome of a DOM mutation to the native UI.
    ///
    /// The user has to know whether the page carries the new content: a
    /// field that still holds the old text would send it.
    fn report_mutation(&self, result: &MutationResult) {
        let payload = match serde_json::to_string(result) {
            Ok(payload) => payload,
            Err(err) => {
                log::error!("cannot serialize the mutation result: {err}");
                return;
            }
        };
        crate::native::call_native("onMutationResult", &[NativeArg::String(payload)]);
    }

    /// Tell the native UI that the back stack changed.
    ///
    /// The snapshot itself stays in the engine: the UI pulls it when it
    /// needs it, because a frame is too big for a callback.
    fn report_snapshot(&self, snapshot: &PageSnapshot) {
        log::info!(
            "the snapshot of {} is complete ({} bytes of markup)",
            snapshot.url,
            snapshot.markup.len()
        );
        crate::native::call_native("onPageSnapshot", &[NativeArg::String(snapshot.url.clone())]);
    }

    /// Report a navigation to the native UI.
    fn report_navigation(&self, info: &NavigationInfo) {
        let payload = match serde_json::to_string(info) {
            Ok(payload) => payload,
            Err(err) => {
                log::error!("cannot serialize the navigation: {err}");
                return;
            }
        };
        crate::native::call_native("onNavigation", &[NativeArg::String(payload)]);
    }
}

impl BrowserBridge for OvBridge {
    fn on_dom_scan(&self, report: DomScanReport) {
        self.report_dom_scan(&report);
    }

    fn on_decrypt_request(&self, request: DecryptRequest) -> Option<DecryptResponse> {
        let result = match self.state.keyring.decrypt_armored(&request.armored) {
            Ok(result) => result,
            Err(err) => {
                log::error!("cannot decrypt the PGP message: {err:#}");
                crate::native::call_native(
                    "showError",
                    &[NativeArg::String(format!(
                        "cannot decrypt the message: {err:#}"
                    ))],
                );
                return None;
            }
        };

        let verified = result.signatures.iter().any(|signature| signature.verified);

        // The plaintext goes into the native sheet, never into the page.
        self.show_plaintext(
            &request.node_id,
            "Decrypted message",
            &result.plaintext,
            verified,
        );

        // The user decides whether the plaintext is written back into the
        // page; that happens through `nativeInsertPlaintext`.
        None
    }

    fn on_field_value(
        &self,
        value: FieldValue,
        action: FieldAction,
        recipients: &[String],
    ) -> Option<String> {
        match action {
            FieldAction::Encrypt => {
                let fingerprints = if recipients.is_empty() {
                    self.state.own_fingerprints()
                } else {
                    let mut parsed = Vec::new();
                    for recipient in recipients {
                        match ov_pgp::parse_fingerprint(recipient) {
                            Ok(fingerprint) => parsed.push(fingerprint),
                            Err(err) => {
                                log::warn!("cannot read the recipient {recipient}: {err:#}");
                            }
                        }
                    }
                    parsed
                };
                if fingerprints.is_empty() {
                    crate::native::call_native(
                        "showError",
                        &[NativeArg::String(
                            "there is no key to encrypt the message with".to_string(),
                        )],
                    );
                    return None;
                }
                match self
                    .state
                    .keyring
                    .encrypt_armored(&value.value, &fingerprints, None)
                {
                    Ok(armored) => Some(armored),
                    Err(err) => {
                        log::error!("cannot encrypt the message: {err:#}");
                        crate::native::call_native(
                            "showError",
                            &[NativeArg::String(format!(
                                "cannot encrypt the message: {err:#}"
                            ))],
                        );
                        None
                    }
                }
            }
            FieldAction::InsertPublicKey => {
                let fingerprints = self.state.own_fingerprints();
                let Some(fingerprint) = fingerprints.first() else {
                    crate::native::call_native(
                        "showError",
                        &[NativeArg::String(
                            "there is no own key to publish".to_string(),
                        )],
                    );
                    return None;
                };
                match self.state.keyring.export_armored_public_key(fingerprint) {
                    Ok(armored) => Some(armored),
                    Err(err) => {
                        log::error!("cannot export the public key: {err:#}");
                        None
                    }
                }
            }
        }
    }

    fn on_mutation_result(&self, result: MutationResult) {
        self.report_mutation(&result);
    }

    fn on_snapshot(&self, snapshot: PageSnapshot) {
        self.report_snapshot(&snapshot);
    }

    fn on_navigation(&self, info: NavigationInfo) {
        self.report_navigation(&info);
    }
}
