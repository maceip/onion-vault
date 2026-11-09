//! The headless test of the DOM engine.
//!
//! The test drives the DOM engine with the HTML of a hidden service and
//! proves three things:
//!
//!   1. the form controls and the PGP blocks are detected,
//!   2. they receive a synthetic id and an inline style, so that the native
//!      UI can anchor its chips on them, and
//!   3. the engine never evaluates a script: the fixture contains a script
//!      that would change the document, and the engine ignores it.
//!
//! The test runs without CEF: the decisions of the DOM engine live in
//! `ov_cef::dom_model`, which the visitor of the render process uses as well.

use ov_cef::dom_model::{self, WriteTarget};
use ov_cef::ipc::FieldKind;

/// The HTML of a hidden service with a form and an armored message.
const HIDDEN_SERVICE: &str = r#"<!doctype html>
<html>
<head>
  <title>SecureDrop</title>
  <script>document.getElementById("body").value = "hacked";</script>
</head>
<body>
  <h1>Send a message</h1>
  <form action="/send" method="post">
    <label for="body">Your message</label>
    <textarea id="body" name="body" rows="8" cols="60">Write your message here.</textarea>
    <input type="text" name="subject" value="Subject">
    <input type="submit" value="Send">
    <pre id="answer">-----BEGIN PGP MESSAGE-----

hQEMAyEAD2G1UN2v0dV3e6xk9kq0Vv4j7wL5nQ1pR8sT3uX2cB6mN0oP4rS7vW9yZ1
-----END PGP MESSAGE-----</pre>
  </form>
  <div id="key">-----BEGIN PGP PUBLIC KEY BLOCK-----

mDMEY1234567890abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ12
-----END PGP PUBLIC KEY BLOCK-----</div>
</body>
</html>
"#;

/// The test area of a hidden service that is not a text field.
const READONLY: &str = r#"<input type="text" name="locked" value="x" readonly>"#;

/// The fields of the fixture are detected.
#[test]
fn the_fields_of_a_hidden_service_are_detected() {
    let document = dom_model::parse_html(HIDDEN_SERVICE);
    let fields = dom_model::scan(&document);

    let kinds: Vec<FieldKind> = fields.iter().map(|field| field.kind).collect();
    assert!(
        kinds.contains(&FieldKind::Textarea),
        "the text area was not detected: {kinds:?}"
    );
    assert!(
        kinds.contains(&FieldKind::PgpMessage),
        "the armored message was not detected: {kinds:?}"
    );
    assert!(
        kinds.contains(&FieldKind::PgpPublicKey),
        "the public key was not detected: {kinds:?}"
    );
    assert!(
        kinds.contains(&FieldKind::TextInput),
        "the text field was not detected: {kinds:?}"
    );
}

/// Every detected node receives a synthetic id and an inline style.
#[test]
fn the_detected_nodes_receive_an_id_and_a_style() {
    let document = dom_model::parse_html(HIDDEN_SERVICE);
    let fields = dom_model::scan(&document);
    assert!(!fields.is_empty());

    for field in &fields {
        assert!(
            field.node_id.starts_with("ov-"),
            "the node {} has no synthetic id",
            field.node_id
        );
        assert!(
            field.style.contains("outline"),
            "the node {} has no inline outline: {}",
            field.node_id,
            field.style
        );
    }

    // The ids are unique: the native UI addresses the nodes with them.
    let mut ids: Vec<&str> = fields.iter().map(|field| field.node_id.as_str()).collect();
    ids.sort_unstable();
    let count = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), count, "two nodes share the same id");
}

/// The style of a PGP block differs from the style of a text field.
#[test]
fn the_style_tells_the_roles_apart() {
    let document = dom_model::parse_html(HIDDEN_SERVICE);
    let fields = dom_model::scan(&document);

    let message = fields
        .iter()
        .find(|field| field.kind == FieldKind::PgpMessage)
        .expect("the armored message was not detected");
    assert!(message.style.contains("dashed"), "{}", message.style);

    let area = fields
        .iter()
        .find(|field| field.kind == FieldKind::Textarea)
        .expect("the text area was not detected");
    assert!(area.style.contains("solid"), "{}", area.style);
}

/// A read only field is not an encryptable field.
#[test]
fn a_read_only_field_is_ignored() {
    let document = dom_model::parse_html(READONLY);
    let fields = dom_model::scan(&document);
    assert!(
        fields.is_empty(),
        "the read only field was detected: {fields:?}"
    );
}

/// The value of a control is written through its value attribute, a block
/// through its text node.
#[test]
fn the_value_is_written_through_the_right_target() {
    let document = dom_model::parse_html(HIDDEN_SERVICE);

    let area = dom_model::find_by_tag(&document, "textarea").expect("the text area was not parsed");
    assert_eq!(dom_model::write_target(area), WriteTarget::ValueAttribute);

    let block = dom_model::find_by_tag(&document, "pre").expect("the block was not parsed");
    assert_eq!(dom_model::write_target(block), WriteTarget::TextNode);
}

/// The engine never evaluates a script.
///
/// The fixture contains a script that would overwrite the text of the text
/// area.  The engine reads the document and finds the original text, so the
/// script was not executed.
#[test]
fn no_line_of_javascript_is_executed() {
    let document = dom_model::parse_html(HIDDEN_SERVICE);

    // The script element is parsed like every other element, but nothing is
    // evaluated: the text of the text area is the one of the document.
    let area = dom_model::find_by_tag(&document, "textarea").expect("the text area was not parsed");
    assert_eq!(area.text_content(), "Write your message here.");

    // The script of the document is part of the tree, but the engine does
    // not run it: the document keeps its original text.
    assert!(document.text_content().contains("hacked"));
    assert!(document.text_content().contains("Write your message here."));
}
