//! The native DOM PGP engine.
//!
//! OnionVault runs with JavaScript disabled, so the PGP functionality cannot
//! be implemented with an injected content script.  Instead the render
//! process walks Blink's DOM directly through the CEF DOM API
//! (`CefDOMVisitor` / `CefDOMNode`):
//!
//!   * form controls that can hold a PGP message are marked with a synthetic
//!     `data-ov-node-id` attribute and highlighted with an inline style,
//!   * blocks that contain an armored PGP message or an armored public key
//!     are detected the same way,
//!   * the bounding rectangles of all detected nodes are sent to the browser
//!     process so that the native Android UI can anchor its action chips on
//!     top of them.
//!
//! Nothing here executes JavaScript; every DOM change is an inline attribute
//! mutation or a write through the DOM API of CEF.
//!
//! # Writing into a field
//!
//! The DOM API of CEF is deliberately narrow: `CefDOMNode::SetValue` refuses
//! element nodes and only replaces the text of a *text node*, and
//! `CefDOMNode::GetValue` returns the live value of a form control but the
//! empty string for every other element.  Writing a value into a field
//! therefore needs three steps:
//!
//!   1. the value is written (the `value` attribute of a control, the text
//!      node of a block),
//!   2. the *live* value is read back and compared with the intended one --
//!      an attribute write does not mark the control as dirty, so a form
//!      would send the stale text,
//!   3. when the comparison fails, the browser process finishes the job
//!      through the input path of the browser (`select_all`, `del` and the key
//!      events of the text).

use std::sync::{Arc, Mutex};

use cef::rc::Rc;
use cef::{
    wrap_domvisitor, Browser, CefString, CefStringUtf16, Domdocument, Domnode, Domvisitor, Frame,
    ImplDomdocument, ImplDomnode, ImplDomvisitor, Rect, WrapDomvisitor,
};

use crate::ipc::{self, DomFieldMetadata, FieldKind, MutationRequest, MutationResult};

/// Attribute that marks the focused node.
pub use crate::dom_model::FOCUSED_ATTRIBUTE;
/// Attribute that carries the synthetic node id.
pub use crate::dom_model::NODE_ID_ATTRIBUTE;

/// Maximum depth of the DOM traversal; deeper subtrees are ignored to keep
/// the render process responsive on pathological pages.
const MAX_DEPTH: usize = 64;

/// Number of characters of the value that are reported to the UI.
const PREVIEW_LEN: usize = 120;

/// Style of an encryptable field (text area or text input).
pub use crate::dom_model::STYLE_ENCRYPTABLE;
/// Style of the focused, active field.
pub use crate::dom_model::STYLE_FOCUSED;
/// Style of a detected PGP ciphertext.
pub use crate::dom_model::STYLE_PGP_MESSAGE;
/// Style of a detected PGP public key.
pub use crate::dom_model::STYLE_PGP_PUBLIC_KEY;
/// Style of a field whose content was encrypted or decrypted by OnionVault.
pub use crate::dom_model::STYLE_VERIFIED;

/// Marker of an armored PGP message.
pub use crate::dom_model::PGP_MESSAGE_BEGIN;
/// Marker of an armored PGP message.
pub use crate::dom_model::PGP_MESSAGE_END;
/// Marker of an armored PGP public key.
pub use crate::dom_model::PGP_PUBLIC_KEY_BEGIN;

/// What the visitor has to do when it is run.
#[derive(Clone, Debug)]
pub enum VisitMode {
    /// Scan the document and report the interesting nodes.
    Scan,
    /// Replace the content of the node with the given id.
    Mutate(MutationRequest),
    /// Read the value of a node and send it to the browser process.
    ReadValue(ipc::ValueRequest),
}

// The DOM visitor.
//
// It is created through the `wrap_domvisitor!` macro of cef-rs, which turns
// the plain Rust struct into a `CefDOMVisitor` implementation.
wrap_domvisitor! {
    // Visitor that scans the DOM for PGP relevant nodes.
    pub struct PgDomVisitor {
        // The frame the visitor runs in; used to post the results to the
        // browser process.
        frame: Frame,
        // What the visitor has to do.
        mode: VisitMode,
        // The fields that were found; shared with the caller.
        fields: Arc<Mutex<Vec<DomFieldMetadata>>>,
        // The id of the focused form control of the document.
        //
        // A control that the user has typed into keeps a dirty value that
        // the DOM API cannot replace; the input path of the browser can, but
        // only when the control is the focused one.
        focused: Arc<Mutex<Option<String>>>,
    }

    impl Domvisitor {
        // Called by CEF when the DOM is visited.
        fn visit(&self, document: Option<&mut Domdocument>) {
            let Some(document) = document else {
                return;
            };
            match &self.mode {
                VisitMode::Scan => self.scan(document),
                VisitMode::Mutate(request) => self.mutate(document, request),
                VisitMode::ReadValue(request) => self.read_value(document, request),
            }
        }
    }
}

impl PgDomVisitor {
    /// Walk the document and report the interesting nodes.
    fn scan(&self, document: &mut Domdocument) {
        let Some(body) = document.body() else {
            return;
        };

        let title = userfree_to_string(&document.title());
        let mut found = Vec::new();
        let mut path = Vec::new();
        self.walk(&body, &mut path, 0, false, &mut found);

        // The markup of the document is part of the snapshot of the page.
        let markup = document_markup(document);

        if let Ok(mut shared) = self.fields.lock() {
            *shared = found.clone();
        }

        // The browser process anchors its action chips on the reported
        // rectangles and performs the cryptographic operations.
        ipc::send_json(
            &self.frame,
            ipc::MSG_DOM_FIELDS,
            &ipc::DomScanReport {
                title,
                fields: found,
                markup,
            },
        );
    }

    /// Replace the content of the node that carries the requested id.
    fn mutate(&self, document: &mut Domdocument, request: &MutationRequest) {
        let Some(body) = document.body() else {
            return;
        };

        let mut path = Vec::new();
        let mut outcome = MutationOutcome::default();
        self.walk_for_mutation(&body, &mut path, 0, request, &mut outcome);

        if !outcome.found {
            log::warn!(
                "the node {} is gone, the mutation was dropped",
                request.node_id
            );
            self.report_mutation(request, &outcome);
            return;
        }

        // The outcome is reported in every case: the user has to know whether
        // the page carries the new content, because a form that still holds
        // the old text would send it.
        self.report_mutation(request, &outcome);
    }

    /// Report the outcome of a mutation to the browser process.
    fn report_mutation(&self, request: &MutationRequest, outcome: &MutationOutcome) {
        let focused = self
            .focused
            .lock()
            .ok()
            .and_then(|focused| focused.clone())
            .is_some_and(|id| id == request.node_id);

        if outcome.replaced {
            log::info!("the node {} carries the new content", request.node_id);
        } else if outcome.needs_keyboard {
            log::info!(
                "the node {} keeps its old content, the input path has to replace it",
                request.node_id
            );
        } else {
            log::warn!(
                "the node {} could not be changed: {}",
                request.node_id,
                outcome.reason
            );
        }

        let result = MutationResult {
            node_id: request.node_id.clone(),
            applied: outcome.replaced,
            needs_keyboard: outcome.needs_keyboard,
            focused,
            reason: outcome.reason.clone(),
            text: request.replacement.clone(),
        };
        ipc::send_json(&self.frame, ipc::MSG_MUTATION_RESULT, &result);
    }

    /// Read the value of a node and send it to the browser process.
    fn read_value(&self, document: &mut Domdocument, request: &ipc::ValueRequest) {
        let Some(body) = document.body() else {
            return;
        };

        let mut path = Vec::new();
        let mut found = None;
        self.walk_for_value(&body, &mut path, 0, &request.node_id, &mut found);

        let Some(value) = found else {
            log::warn!(
                "the node {} is gone, its value cannot be read",
                request.node_id
            );
            return;
        };

        ipc::send_json(
            &self.frame,
            ipc::MSG_FIELD_VALUE,
            &ipc::FieldValue {
                node_id: request.node_id.clone(),
                value,
            },
        );
    }

    /// Walk that looks for the node whose value has to be read.
    fn walk_for_value(
        &self,
        node: &Domnode,
        path: &mut Vec<usize>,
        depth: usize,
        node_id: &str,
        found: &mut Option<String>,
    ) {
        if depth > MAX_DEPTH || found.is_some() {
            return;
        }

        if !element_tag(node).is_empty() && attribute(node, NODE_ID_ATTRIBUTE) == node_id {
            *found = Some(content_of(node));
            return;
        }

        let mut index = 0usize;
        let mut child = node.first_child();
        while let Some(current) = child {
            path.push(index);
            self.walk_for_value(&current, path, depth + 1, node_id, found);
            path.pop();
            index += 1;
            child = current.next_sibling();
        }
    }

    /// Recursive DOM walk that classifies the nodes.
    fn walk(
        &self,
        node: &Domnode,
        path: &mut Vec<usize>,
        depth: usize,
        inside_match: bool,
        found: &mut Vec<DomFieldMetadata>,
    ) {
        if depth > MAX_DEPTH {
            return;
        }

        let tag = element_tag(node);
        let mut child_inside = inside_match;
        if !tag.is_empty() {
            let node_id = node_id(path, &tag);
            let kinds = self.classify(node, &tag, inside_match);

            // The children of a detected PGP block are not reported again:
            // the block itself is the one that carries the armored text.
            child_inside = inside_match
                || kinds.contains(&FieldKind::PgpMessage)
                || kinds.contains(&FieldKind::PgpPublicKey);

            // Remember the focused form control: a control that the user has
            // typed into can only be changed through the input path of the
            // browser, and that only works while it is focused.
            if has_attribute(node, FOCUSED_ATTRIBUTE) && is_form_control(&tag) {
                if let Ok(mut focused) = self.focused.lock() {
                    *focused = Some(node_id.clone());
                }
            }

            if !kinds.is_empty() {
                self.apply_style(node, &kinds);
                node.set_element_attribute(
                    Some(&CefString::from(NODE_ID_ATTRIBUTE)),
                    Some(&CefString::from(node_id.as_str())),
                );

                let rect = node.element_bounds();
                let value = content_of(node);
                // A PGP block is reported with its complete text: the
                // native UI decrypts the message or imports the key.
                let armored = match kinds[0] {
                    FieldKind::PgpMessage | FieldKind::PgpPublicKey => {
                        let text = value.trim();
                        if text.is_empty() {
                            None
                        } else {
                            Some(text.to_string())
                        }
                    }
                    _ => None,
                };
                found.push(DomFieldMetadata {
                    node_id,
                    kind: kinds[0],
                    tag: tag.clone(),
                    rect: ipc::NodeRect {
                        x: rect.x,
                        y: rect.y,
                        width: rect.width,
                        height: rect.height,
                    },
                    value_preview: preview(&value),
                    armored,
                });
            }
        }

        let mut index = 0usize;
        let mut child = node.first_child();
        while let Some(current) = child {
            path.push(index);
            self.walk(&current, path, depth + 1, child_inside, found);
            path.pop();
            index += 1;
            child = current.next_sibling();
        }
    }

    /// Walk that looks for the node that has to be mutated.
    fn walk_for_mutation(
        &self,
        node: &Domnode,
        path: &mut Vec<usize>,
        depth: usize,
        request: &MutationRequest,
        outcome: &mut MutationOutcome,
    ) {
        if depth > MAX_DEPTH || outcome.found {
            return;
        }

        let tag = element_tag(node);
        if !tag.is_empty() {
            let current_id = attribute(node, NODE_ID_ATTRIBUTE);
            if current_id == request.node_id {
                outcome.found = true;
                *outcome = apply_replacement(node, &request.replacement);
                outcome.found = true;
                // Mark the node as verified / encrypted.
                set_style(node, STYLE_VERIFIED);
                log::info!("mutated the node {} ({tag})", request.node_id);
                return;
            }
        }

        let mut index = 0usize;
        let mut child = node.first_child();
        while let Some(current) = child {
            path.push(index);
            self.walk_for_mutation(&current, path, depth + 1, request, outcome);
            path.pop();
            index += 1;
            child = current.next_sibling();
        }
    }

    /// Classify a node: which PGP relevant roles does it have?
    ///
    /// The decision itself lives in [`crate::dom_model`], so that the tests
    /// can drive it with the HTML of a hidden service.
    fn classify(&self, node: &Domnode, tag: &str, inside_match: bool) -> Vec<FieldKind> {
        let model = model_of(node, tag);
        let probe = self.probe_text(node);
        crate::dom_model::classify_with_text(&model, &probe, inside_match)
    }

    /// The text of a node that is used for the PGP marker detection.
    ///
    /// Only the text that belongs directly to the node is used: the block
    /// that holds the armored text is the block that is detected, not the
    /// form or the body around it.
    fn probe_text(&self, node: &Domnode) -> String {
        let mut probe = direct_text_of(node);
        if !has_element_children(node) {
            probe.push_str(&markup_of(node));
        }
        probe
    }

    /// Apply the highlight that belongs to the detected kinds.
    fn apply_style(&self, node: &Domnode, kinds: &[FieldKind]) {
        let focused = has_attribute(node, FOCUSED_ATTRIBUTE);
        set_style(node, crate::dom_model::inline_style(kinds, focused));
    }
}

/// Outcome of a mutation.
#[derive(Debug, Default)]
struct MutationOutcome {
    /// The node was found.
    found: bool,
    /// The value of the node was replaced.
    replaced: bool,
    /// The control keeps a dirty value that only the input path can change.
    needs_keyboard: bool,
    /// The reason why the replacement failed.
    reason: String,
}

/// Write `text` into the node and verify the result.
///
/// The DOM API of CEF refuses to change the value of an element: the direct
/// call to `CefDOMNode::SetValue` is rejected for every element, and it only
/// replaces the text of a text node.  A form control is therefore written
/// through its `value` attribute and a block through its text child; in both
/// cases the *live* value is read back afterwards, because an attribute
/// write does not mark the control as dirty and a form would send the stale
/// text.
fn apply_replacement(node: &Domnode, text: &str) -> MutationOutcome {
    let mut outcome = MutationOutcome::default();

    if node.is_form_control_element() != 0 {
        // The direct call is the documented way to set the value of a form
        // control; CEF rejects it for elements, which the read back below
        // shows.
        let accepted = node.set_value(Some(&CefString::from(text))) != 0;
        if accepted && same_value(&live_value(node), text) {
            outcome.replaced = true;
            return outcome;
        }

        // The fallback: the `value` attribute.  It reaches the live value of
        // a control that the user has not typed into; a control with a dirty
        // value keeps the text of the user.
        node.set_element_attribute(
            Some(&CefString::from("value")),
            Some(&CefString::from(text)),
        );
        if same_value(&live_value(node), text) {
            outcome.replaced = true;
            return outcome;
        }

        // A text area keeps its default value in a text node; writing it
        // reaches the live value of a control that the user has not typed
        // into.
        for child in text_children(node) {
            let accepted = child.set_value(Some(&CefString::from(text))) != 0;
            if accepted && same_value(&live_value(node), text) {
                outcome.replaced = true;
                return outcome;
            }
        }

        outcome.needs_keyboard = true;
        outcome.reason = "the field still holds the old text".to_string();
        return outcome;
    }

    if node.is_text() != 0 {
        // A text node: this is what `CefDOMNode::SetValue` is meant for.
        let accepted = node.set_value(Some(&CefString::from(text))) != 0;
        let live = value_of(node);
        if accepted && live == text {
            outcome.replaced = true;
        } else {
            outcome.reason = "the text node could not be changed".to_string();
        }
        return outcome;
    }

    // A block: its text lives in the text nodes below it.
    let text_nodes = text_children(node);
    if text_nodes.is_empty() {
        outcome.reason = "the element does not contain any text".to_string();
        return outcome;
    }

    // The first text node carries the new content, the others are emptied.
    let mut applied = text_nodes[0].set_value(Some(&CefString::from(text))) != 0;
    for extra in text_nodes.iter().skip(1) {
        applied &= extra.set_value(Some(&CefString::from(""))) != 0;
    }

    let live = raw_text_of(node);
    if applied && live == text {
        outcome.replaced = true;
    } else {
        outcome.reason = "the text of the element could not be replaced".to_string();
    }
    outcome
}

/// The text nodes below a node.
fn text_children(node: &Domnode) -> Vec<Domnode> {
    let mut text_nodes = Vec::new();
    let mut child = node.first_child();
    while let Some(current) = child {
        if current.is_text() != 0 {
            text_nodes.push(current.clone());
        }
        child = current.next_sibling();
    }
    text_nodes
}

/// Compare the live value of a control with the intended one.
///
/// CEF trims the leading whitespace of the value of a form control; the
/// comparison uses the same normalisation, so that a value that starts with
/// a newline is not reported as a failure.
fn same_value(live: &str, intended: &str) -> bool {
    live.trim_start() == intended.trim_start()
}

/// The live value of a form control.
pub fn live_value(node: &Domnode) -> String {
    value_of(node).trim_start().to_string()
}

/// The text of the direct text children of a node.
pub fn direct_text_of(node: &Domnode) -> String {
    let mut text = String::new();
    let mut child = node.first_child();
    while let Some(current) = child {
        if current.is_text() != 0 {
            text.push_str(&value_of(&current));
        }
        child = current.next_sibling();
    }
    text
}

/// The raw text of the descendants of a node.
///
/// `CefDOMNode::GetValue` returns the empty string for an element that is
/// not a form control, so the text of a block is collected from its text
/// nodes.  The raw text is used instead of the rendered text because the
/// rendered text collapses the line breaks of an armored PGP message.
pub fn raw_text_of(node: &Domnode) -> String {
    let mut text = String::new();
    collect_text(node, 0, &mut text);
    text
}

/// Collect the text of the descendants of a node.
fn collect_text(node: &Domnode, depth: usize, out: &mut String) {
    if depth > MAX_DEPTH {
        return;
    }
    if node.is_text() != 0 {
        out.push_str(&value_of(node));
        return;
    }
    let mut child = node.first_child();
    while let Some(current) = child {
        collect_text(&current, depth + 1, out);
        child = current.next_sibling();
    }
}

/// The content of a node.
///
/// A form control carries its live value, every other element the text of
/// its descendants.
pub fn content_of(node: &Domnode) -> String {
    if node.is_form_control_element() != 0 {
        live_value(node)
    } else if node.is_text() != 0 {
        value_of(node)
    } else {
        raw_text_of(node)
    }
}

/// The tag name of an element node (empty for other node types).
pub fn element_tag(node: &Domnode) -> String {
    userfree_to_string(&node.element_tag_name())
}

/// The value of a node.
pub fn value_of(node: &Domnode) -> String {
    userfree_to_string(&node.value())
}

/// The markup of a document.
///
/// The DOM API of CEF cannot return the markup of the document itself, but
/// the head and the body carry everything that matters for the snapshot.
pub fn document_markup(document: &Domdocument) -> String {
    let mut markup = String::new();
    if let Some(head) = document.head() {
        markup.push_str(&markup_of(&head));
    }
    if let Some(body) = document.body() {
        markup.push_str(&markup_of(&body));
    }
    markup
}

/// The outer HTML of a node.
pub fn markup_of(node: &Domnode) -> String {
    userfree_to_string(&node.as_markup())
}

/// The value of an attribute (empty when it is not present).
pub fn attribute(node: &Domnode, name: &str) -> String {
    userfree_to_string(&node.element_attribute(Some(&CefString::from(name))))
}

/// `true` when the attribute is present.
pub fn has_attribute(node: &Domnode, name: &str) -> bool {
    node.has_element_attribute(Some(&CefString::from(name))) != 0
}

/// `true` when the node has at least one element child.
pub fn has_element_children(node: &Domnode) -> bool {
    let mut child = node.first_child();
    while let Some(current) = child {
        if current.is_element() != 0 {
            return true;
        }
        child = current.next_sibling();
    }
    false
}

/// `true` when the node is a form control whose value can be replaced.
pub fn is_form_control(tag: &str) -> bool {
    matches!(tag.to_ascii_uppercase().as_str(), "TEXTAREA" | "INPUT")
}

/// `true` when the tag can carry a PGP block.
pub fn is_pgp_container(tag: &str) -> bool {
    matches!(
        tag.to_ascii_uppercase().as_str(),
        "PRE" | "CODE" | "TEXTAREA" | "P" | "BLOCKQUOTE" | "DIV"
    )
}

/// Set the inline style of a node, keeping the styles the page brought along.
pub fn set_style(node: &Domnode, style: &str) {
    let existing = attribute(node, "style");
    let combined = if existing.trim().is_empty() {
        style.to_string()
    } else {
        format!("{} {}", existing.trim_end_matches(';'), style)
    };
    node.set_element_attribute(
        Some(&CefString::from("style")),
        Some(&CefString::from(combined.as_str())),
    );
}

/// Mark a node as the focused one.
pub fn mark_focused(node: &Domnode) {
    node.set_element_attribute(
        Some(&CefString::from(FOCUSED_ATTRIBUTE)),
        Some(&CefString::from("1")),
    );
}

/// A deterministic synthetic id for a node.
///
/// The id is derived from the position of the node in the document, so that
/// the same node keeps its id across rescans and the native UI can anchor
/// its chips on a stable identifier.
pub fn node_id(path: &[usize], tag: &str) -> String {
    crate::dom_model::synthetic_id(path, tag)
}

/// Map a node of the DOM of CEF onto the model.
///
/// Only the attributes that the classification needs are mapped: the model
/// is a decision helper, not a copy of the DOM.
pub fn model_of(node: &Domnode, tag: &str) -> crate::dom_model::HtmlNode {
    let mut model = crate::dom_model::HtmlNode {
        tag: tag.to_ascii_lowercase(),
        ..crate::dom_model::HtmlNode::default()
    };
    let input_type = attribute(node, "type");
    if !input_type.is_empty() {
        model.attributes.push(("type".to_string(), input_type));
    }
    if has_attribute(node, "disabled") {
        model
            .attributes
            .push(("disabled".to_string(), "disabled".to_string()));
    }
    if has_attribute(node, "readonly") {
        model
            .attributes
            .push(("readonly".to_string(), "readonly".to_string()));
    }
    model
}

/// Shorten a value for the preview that is shown in the native UI.
pub fn preview(value: &str) -> String {
    let mut chars = value.chars();
    let head: String = chars.by_ref().take(PREVIEW_LEN).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// Convert a CEF string into a Rust `String`.
pub fn userfree_to_string(value: &cef::CefStringUserfree) -> String {
    CefStringUtf16::from(value).to_string()
}

/// Build a DOM visitor for a scan run.
pub fn scan_visitor(
    frame: Frame,
    fields: Arc<Mutex<Vec<DomFieldMetadata>>>,
    focused: Arc<Mutex<Option<String>>>,
) -> Domvisitor {
    PgDomVisitor::new(frame, VisitMode::Scan, fields, focused)
}

/// Build a DOM visitor that performs a mutation.
pub fn mutation_visitor(
    frame: Frame,
    request: MutationRequest,
    focused: Arc<Mutex<Option<String>>>,
) -> Domvisitor {
    PgDomVisitor::new(
        frame,
        VisitMode::Mutate(request),
        Arc::new(Mutex::new(Vec::new())),
        focused,
    )
}

/// Build a DOM visitor that reads the value of a node.
pub fn read_value_visitor(
    frame: Frame,
    request: ipc::ValueRequest,
    focused: Arc<Mutex<Option<String>>>,
) -> Domvisitor {
    PgDomVisitor::new(
        frame,
        VisitMode::ReadValue(request),
        Arc::new(Mutex::new(Vec::new())),
        focused,
    )
}

/// The browser of a frame, for handlers that need it.
pub fn browser_of(browser: Option<&mut Browser>) -> Option<Browser> {
    browser.map(|b| b.clone())
}

/// The bounding rectangle of a node.
pub fn bounds_of(node: &Domnode) -> Rect {
    node.element_bounds()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_leading_whitespace_is_ignored_by_the_verification() {
        // CEF trims the leading whitespace of the value of a form control;
        // the verification has to use the same normalisation, otherwise a
        // value that starts with a newline would be reported as a failure.
        assert!(same_value("hello", "hello"));
        assert!(same_value("hello", "\n  hello"));
        assert!(same_value("\nhello", "hello"));
        assert!(!same_value("hello", "world"));
        assert!(!same_value("", "hello"));
        // The trailing whitespace matters: the control would send it.
        assert!(!same_value("hello ", "hello"));
    }

    #[test]
    fn the_node_id_is_stable() {
        let first = node_id(&[0, 2, 1], "TEXTAREA");
        let second = node_id(&[0, 2, 1], "TEXTAREA");
        assert_eq!(first, second);
        assert!(first.starts_with("ov-"));
        assert_ne!(first, node_id(&[0, 2], "TEXTAREA"));
        assert_ne!(first, node_id(&[0, 2, 1], "INPUT"));
    }
}
