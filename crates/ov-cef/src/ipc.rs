//! The IPC between the CEF render process and the CEF browser process.
//!
//! Trap does not use JavaScript, so the two processes talk through
//! `CefProcessMessage`s:
//!
//! | message                | direction        | payload                    |
//! |------------------------|------------------|----------------------------|
//! | `OV_DOM_FIELDS`        | render → browser | the detected DOM fields    |
//! | `OV_REQ_DECRYPT`       | render → browser | an armored PGP message     |
//! | `OV_MUTATE_DOM_NODE`   | browser → render | a replacement for a node   |
//! | `OV_MUTATION_RESULT`   | render → browser | the outcome of a mutation  |
//! | `OV_DECRYPT_RESULT`    | browser → render | the decrypted plaintext    |
//! | `OV_NAVIGATION`        | browser → native | the current address        |
//!
//! The payload of a message is a JSON document in the first argument of the
//! message; the JSON schema is described by the types in this module.

use cef::{
    process_message_create, CefString, Frame, ImplFrame, ImplListValue, ImplProcessMessage,
    ProcessId, ProcessMessage,
};
use serde::{de::DeserializeOwned, Serialize};

/// The render process reports the fields it found.
pub const MSG_DOM_FIELDS: &str = "OV_DOM_FIELDS";
/// The render process asks the browser process to decrypt a PGP message.
pub const MSG_REQ_DECRYPT: &str = "OV_REQ_DECRYPT";
/// The browser process asks the render process to replace a node's content.
pub const MSG_MUTATE_DOM_NODE: &str = "OV_MUTATE_DOM_NODE";
/// The render process reports the outcome of a mutation.
pub const MSG_MUTATION_RESULT: &str = "OV_MUTATION_RESULT";
/// The browser process sends the decrypted plaintext back to the render process.
pub const MSG_DECRYPT_RESULT: &str = "OV_DECRYPT_RESULT";
/// The browser process reports a navigation to the native UI.
pub const MSG_NAVIGATION: &str = "OV_NAVIGATION";
/// The browser process asks the render process for the value of a node.
pub const MSG_REQ_FIELD_VALUE: &str = "OV_REQ_FIELD_VALUE";
/// The render process answers with the value of a node.
pub const MSG_FIELD_VALUE: &str = "OV_FIELD_VALUE";
/// The browser process asks the render process to read a node again.
///
/// The message is used to verify a change that the browser process made
/// through the input path of the browser.
pub const MSG_READ_VALUE: &str = "OV_READ_VALUE";

/// What the browser process wants to do with the value of a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldAction {
    /// Encrypt the value for the given recipients.
    Encrypt,
    /// Replace the value with the armored public key of the local user.
    InsertPublicKey,
}

/// The request for the value of a DOM node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct FieldValueRequest {
    /// The id of the node.
    pub node_id: String,
    /// What should happen with the value.
    pub action: FieldAction,
    /// The fingerprints of the recipients (only for [`FieldAction::Encrypt`]).
    pub recipients: Vec<String>,
}

/// The request for the value of a single node.
///
/// The render process only needs the id of the node; the action of the
/// browser process is remembered on its side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct ValueRequest {
    /// The id of the node.
    pub node_id: String,
}

/// The value of a DOM node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct FieldValue {
    /// The id of the node.
    pub node_id: String,
    /// The value of the node.
    pub value: String,
}

/// The role of a detected DOM node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    /// A `<textarea>` that can hold a PGP message.
    Textarea,
    /// An `<input type="text">` that can hold a PGP message.
    TextInput,
    /// A block that contains an armored PGP message.
    PgpMessage,
    /// A block that contains an armored PGP public key.
    PgpPublicKey,
}

impl FieldKind {
    /// A short label for the native UI.
    pub fn label(&self) -> &'static str {
        match self {
            FieldKind::Textarea => "Text area",
            FieldKind::TextInput => "Text field",
            FieldKind::PgpMessage => "PGP message",
            FieldKind::PgpPublicKey => "PGP public key",
        }
    }

    /// `true` when the node can hold an encrypted message.
    pub fn is_encryptable(&self) -> bool {
        matches!(self, FieldKind::Textarea | FieldKind::TextInput)
    }

    /// `true` when the node contains something that can be decrypted.
    pub fn is_decryptable(&self) -> bool {
        matches!(self, FieldKind::PgpMessage)
    }

    /// `true` when the node contains a public key that can be imported.
    pub fn is_importable(&self) -> bool {
        matches!(self, FieldKind::PgpPublicKey)
    }
}

/// A rectangle in device independent pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct NodeRect {
    /// The left edge.
    pub x: i32,
    /// The top edge.
    pub y: i32,
    /// The width.
    pub width: i32,
    /// The height.
    pub height: i32,
}

/// Everything the native UI needs to know about a detected node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct DomFieldMetadata {
    /// The synthetic id of the node (`data-ov-node-id`).
    pub node_id: String,
    /// The role of the node.
    pub kind: FieldKind,
    /// The tag name of the node.
    pub tag: String,
    /// The bounding rectangle of the node.
    pub rect: NodeRect,
    /// A shortened preview of the current value.
    pub value_preview: String,
    /// The complete armored text of a PGP block.
    ///
    /// The field is only filled for the blocks that carry an armored PGP
    /// message or an armored public key; the native UI needs the complete
    /// text to decrypt the message or to import the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub armored: Option<String>,
}

/// The result of a DOM scan in the render process.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct DomScanReport {
    /// The title of the document.
    pub title: String,
    /// The nodes that were detected.
    pub fields: Vec<DomFieldMetadata>,
    /// The markup of the document.
    ///
    /// The snapshot of the page needs the state of the document: the text of
    /// a text area that was encrypted or decrypted has to survive a back
    /// navigation, even when the hidden service cannot be reached any more.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub markup: String,
}

/// The request to replace the content of a node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct MutationRequest {
    /// The id of the node that has to be replaced.
    pub node_id: String,
    /// The new content.
    pub replacement: String,
}

/// The outcome of a mutation.
///
/// The DOM API of CEF can only change a *text node*; a form control keeps
/// the dirty value that the user typed into it.  The result therefore tells
/// the browser process whether the replacement was applied and whether the
/// input path of the browser has to finish the job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct MutationResult {
    /// The id of the node that was changed.
    pub node_id: String,
    /// `true` when the node carries the new content.
    pub applied: bool,
    /// `true` when the control can only be changed through the input path.
    pub needs_keyboard: bool,
    /// `true` when the node is the focused element of the document.
    pub focused: bool,
    /// The reason why the replacement failed (empty when it succeeded).
    #[serde(default)]
    pub reason: String,
    /// The text that was to be written into the node.
    #[serde(default)]
    pub text: String,
}

/// The request to decrypt an armored PGP message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct DecryptRequest {
    /// The id of the node the message came from.
    pub node_id: String,
    /// The armored PGP message.
    pub armored: String,
}

/// The result of a decryption.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct DecryptResponse {
    /// The id of the node the message came from.
    pub node_id: String,
    /// The decrypted plaintext.
    pub plaintext: String,
    /// `true` when a signature of the message could be verified.
    pub signature_verified: bool,
    /// The fingerprint of the signer, when the signature was verified.
    pub signer: Option<String>,
}

/// A navigation that the browser process reports to the native UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct NavigationInfo {
    /// The URL of the document.
    pub url: String,
    /// The title of the document.
    pub title: String,
    /// `true` when the navigation was blocked by the leak prevention rules.
    pub blocked: bool,
}

/// Send a JSON payload to the browser process.
pub fn send_json<T: Serialize>(frame: &Frame, name: &str, payload: &T) -> bool {
    let text = match serde_json::to_string(payload) {
        Ok(text) => text,
        Err(err) => {
            log::error!("cannot serialize the payload of {name}: {err}");
            return false;
        }
    };
    send_text(frame, name, &text)
}

/// Send a JSON payload to the render process.
pub fn send_json_to_renderer<T: Serialize>(frame: &Frame, name: &str, payload: &T) -> bool {
    let text = match serde_json::to_string(payload) {
        Ok(text) => text,
        Err(err) => {
            log::error!("cannot serialize the payload of {name}: {err}");
            return false;
        }
    };
    send_text_to(frame, name, &text, ProcessId::RENDERER)
}

/// Send a plain text payload to the browser process.
pub fn send_text(frame: &Frame, name: &str, text: &str) -> bool {
    send_text_to(frame, name, text, ProcessId::BROWSER)
}

/// Send a plain text payload to the given process.
pub fn send_text_to(frame: &Frame, name: &str, text: &str, target: ProcessId) -> bool {
    let Some(mut message) = process_message_create(Some(&CefString::from(name))) else {
        log::error!("cannot create the process message {name}");
        return false;
    };
    let Some(list) = message.argument_list() else {
        log::error!("the process message {name} has no argument list");
        return false;
    };
    list.set_size(1);
    list.set_string(0, Some(&CefString::from(text)));
    frame.send_process_message(target, Some(&mut message));
    true
}

/// Read a JSON payload from a process message.
pub fn read_json<T: DeserializeOwned>(message: &mut ProcessMessage) -> Option<T> {
    let text = read_text(message)?;
    match serde_json::from_str(&text) {
        Ok(value) => Some(value),
        Err(err) => {
            log::error!("cannot deserialize the process message: {err}");
            None
        }
    }
}

/// Read the first argument of a process message as text.
pub fn read_text(message: &mut ProcessMessage) -> Option<String> {
    let list = message.argument_list()?;
    if list.size() == 0 {
        return None;
    }
    let value = list.string(0);
    Some(crate::dom_pgp::userfree_to_string(&value))
}

/// The name of a process message.
pub fn message_name(message: &mut ProcessMessage) -> String {
    crate::dom_pgp::userfree_to_string(&message.name())
}
