//! A small model of the DOM of a page.
//!
//! The DOM API of CEF is only available inside the render process, so the
//! parts of the DOM engine that decide *what* has to happen are kept free of
//! CEF and live here:
//!
//!   * the parser that reads the HTML of a hidden service into a tree,
//!   * the classification of the nodes (a text field, a PGP block, ...),
//!   * the synthetic id and the inline style of a node, and
//!   * the decision how a value is written into a node.
//!
//! The visitor of the render process maps the real DOM onto this model, and
//! the tests drive the model with the HTML of a hidden service.  Both paths
//! therefore use the same code.

use crate::ipc::FieldKind;

/// Attribute that carries the synthetic node id.
pub const NODE_ID_ATTRIBUTE: &str = "data-ov-node-id";

/// Attribute that marks the focused node.
pub const FOCUSED_ATTRIBUTE: &str = "data-ov-focused";

/// Marker of an armored PGP message.
pub const PGP_MESSAGE_BEGIN: &str = "-----BEGIN PGP MESSAGE-----";

/// Marker of an armored PGP message.
pub const PGP_MESSAGE_END: &str = "-----END PGP MESSAGE-----";

/// Marker of an armored PGP public key.
pub const PGP_PUBLIC_KEY_BEGIN: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----";

/// Style of an encryptable field (text area or text input).
pub const STYLE_ENCRYPTABLE: &str = "outline: 2px solid #00E5FF !important; \
outline-offset: -2px !important; background-color: rgba(0, 229, 255, 0.06) !important;";

/// Style of the focused, active field.
pub const STYLE_FOCUSED: &str = "outline: 3px solid #00FF66 !important; \
box-shadow: inset 0 0 0 2px #00FF66 !important;";

/// Style of a detected PGP ciphertext.
pub const STYLE_PGP_MESSAGE: &str = "outline: 2px dashed #FFB300 !important; \
background-color: rgba(255, 179, 0, 0.08) !important;";

/// Style of a detected PGP public key.
pub const STYLE_PGP_PUBLIC_KEY: &str = "outline: 2px dotted #CE93D8 !important;";

/// Style of a field whose content was encrypted or decrypted by Trap.
pub const STYLE_VERIFIED: &str = "outline: 2px solid #00FF66 !important; \
outline-offset: -2px !important; background-color: rgba(0, 255, 102, 0.06) !important;";

/// A node of the document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HtmlNode {
    /// The name of the tag (empty for the text nodes and for the root).
    pub tag: String,
    /// The attributes of the node.
    pub attributes: Vec<(String, String)>,
    /// The text of the node (only for the text nodes).
    pub text: String,
    /// The children of the node.
    pub children: Vec<HtmlNode>,
}

impl HtmlNode {
    /// A text node.
    pub fn text_node(text: &str) -> Self {
        Self {
            text: text.to_string(),
            ..Self::default()
        }
    }

    /// `true` when the node is a text node.
    pub fn is_text(&self) -> bool {
        self.tag.is_empty() && !self.text.is_empty()
    }

    /// The value of an attribute (empty when it is not present).
    pub fn attribute(&self, name: &str) -> String {
        self.attributes
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    }

    /// `true` when the attribute is present.
    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(name))
    }

    /// The text of the direct text children of the node.
    ///
    /// The classification uses it: only the innermost block that holds the
    /// armored text is a PGP block, not the form or the body around it.
    pub fn direct_text(&self) -> String {
        self.text_children()
            .iter()
            .map(|child| child.text.clone())
            .collect()
    }

    /// The text of the node and of its descendants.
    pub fn text_content(&self) -> String {
        let mut text = String::new();
        collect_text(self, &mut text);
        text
    }

    /// The text nodes below the node.
    pub fn text_children(&self) -> Vec<&HtmlNode> {
        self.children
            .iter()
            .filter(|child| child.is_text())
            .collect()
    }

    /// The elements below the node.
    pub fn elements(&self) -> Vec<&HtmlNode> {
        self.children
            .iter()
            .filter(|child| !child.tag.is_empty())
            .collect()
    }
}

/// Collect the text of a node and of its descendants.
fn collect_text(node: &HtmlNode, out: &mut String) {
    if node.is_text() {
        out.push_str(&node.text);
        return;
    }
    for child in &node.children {
        collect_text(child, out);
    }
}

/// The roles of a node.
///
/// The result can carry more than one role: a text area that holds an
/// armored message is a field *and* a PGP block.
pub fn classify(node: &HtmlNode, inside_match: bool) -> Vec<FieldKind> {
    classify_with_text(node, &node.direct_text(), inside_match)
}

/// The roles of a node, with the text that is used for the detection.
///
/// The render process has the text of a node in its own hands, so the text
/// is passed in instead of being read from the node.
pub fn classify_with_text(node: &HtmlNode, probe: &str, inside_match: bool) -> Vec<FieldKind> {
    let mut kinds = Vec::new();
    let tag = node.tag.to_ascii_uppercase();

    if tag == "TEXTAREA" {
        kinds.push(FieldKind::Textarea);
    } else if tag == "INPUT" {
        let input_type = node.attribute("type").to_ascii_lowercase();
        let disabled = node.has_attribute("disabled");
        let readonly = node.has_attribute("readonly");
        if (input_type.is_empty() || input_type == "text") && !disabled && !readonly {
            kinds.push(FieldKind::TextInput);
        }
    }

    // A node that is nested inside an already detected PGP block must not be
    // reported a second time.
    if !inside_match {
        if probe.contains(PGP_MESSAGE_BEGIN) && probe.contains(PGP_MESSAGE_END) {
            kinds.push(FieldKind::PgpMessage);
        } else if probe.contains(PGP_PUBLIC_KEY_BEGIN) {
            kinds.push(FieldKind::PgpPublicKey);
        }
    }

    kinds
}

/// The inline style of a node.
///
/// The style is set as an inline attribute, so that the page cannot undo it
/// with its own stylesheet and so that no script is needed.
pub fn inline_style(kinds: &[FieldKind], focused: bool) -> &'static str {
    if focused {
        STYLE_FOCUSED
    } else if kinds.contains(&FieldKind::PgpMessage) {
        STYLE_PGP_MESSAGE
    } else if kinds.contains(&FieldKind::PgpPublicKey) {
        STYLE_PGP_PUBLIC_KEY
    } else {
        STYLE_ENCRYPTABLE
    }
}

/// The style of a node that was changed by Trap.
pub fn verified_style() -> &'static str {
    STYLE_VERIFIED
}

/// A deterministic synthetic id for a node.
///
/// The id is derived from the position of the node in the document, so that
/// the same node keeps its id across rescans and the native UI can anchor
/// its chips on a stable identifier.
pub fn synthetic_id(path: &[usize], tag: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |byte: u64| {
        hash ^= byte;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    };
    for index in path {
        mix(*index as u64);
    }
    mix(0xff);
    for byte in tag.as_bytes() {
        mix(*byte as u64);
    }
    format!("ov-{hash:016x}")
}

/// How a value is written into a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteTarget {
    /// The `value` attribute of a form control.
    ValueAttribute,
    /// The text node of a block.
    TextNode,
    /// The input path of the browser: the control has to be focused.
    InputPath,
}

/// Decide how a value is written into a node.
///
/// The DOM API of CEF refuses to change the value of an element: it only
/// replaces the text of a text node.  A form control is therefore written
/// through its `value` attribute, a block through its text child, and a
/// control that the user has typed into (a dirty value) can only be changed
/// through the input path of the browser.
pub fn write_target(node: &HtmlNode) -> WriteTarget {
    let tag = node.tag.to_ascii_uppercase();
    if tag == "TEXTAREA" || tag == "INPUT" {
        return WriteTarget::ValueAttribute;
    }
    if node.is_text() {
        return WriteTarget::TextNode;
    }
    if !node.text_children().is_empty() {
        return WriteTarget::TextNode;
    }
    WriteTarget::InputPath
}

/// A node that the DOM engine acts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedField {
    /// The synthetic id of the node.
    pub node_id: String,
    /// The role of the node.
    pub kind: FieldKind,
    /// The name of the tag of the node.
    pub tag: String,
    /// The inline style that the node receives.
    pub style: &'static str,
    /// The position of the node in the document.
    pub path: Vec<usize>,
}

/// Scan a document and report the nodes that the DOM engine acts on.
///
/// The scan is the model of the walk of the render process: it marks the
/// form controls and the PGP blocks with a synthetic id and an inline
/// style, so that the native UI can anchor its chips on them.  No script is
/// evaluated: the document is only read.
pub fn scan(root: &HtmlNode) -> Vec<DetectedField> {
    let mut found = Vec::new();
    walk(root, &mut Vec::new(), 0, false, &mut found);
    found
}

/// The recursive walk of the scan.
fn walk(
    node: &HtmlNode,
    path: &mut Vec<usize>,
    depth: usize,
    inside_match: bool,
    found: &mut Vec<DetectedField>,
) {
    if depth > MAX_DEPTH {
        return;
    }

    let mut child_inside = inside_match;
    if !node.tag.is_empty() {
        let kinds = classify(node, inside_match);
        if !kinds.is_empty() {
            let node_id = synthetic_id(path, &node.tag);
            found.push(DetectedField {
                node_id,
                kind: kinds[0],
                tag: node.tag.clone(),
                style: inline_style(&kinds, node.has_attribute(FOCUSED_ATTRIBUTE)),
                path: path.clone(),
            });
        }
        // The children of a detected PGP block are not reported again: the
        // block itself is the one that carries the armored text.
        child_inside = inside_match
            || kinds.contains(&FieldKind::PgpMessage)
            || kinds.contains(&FieldKind::PgpPublicKey);
    }

    for (index, child) in node.children.iter().enumerate() {
        path.push(index);
        walk(child, path, depth + 1, child_inside, found);
        path.pop();
    }
}

/// The maximum depth of the scan.
const MAX_DEPTH: usize = 64;

/// `true` when the tag can carry a PGP block.
pub fn is_pgp_container(tag: &str) -> bool {
    matches!(
        tag.to_ascii_uppercase().as_str(),
        "PRE" | "CODE" | "TEXTAREA" | "P" | "BLOCKQUOTE" | "DIV"
    )
}

/// Find an element with the given tag.
///
/// The search walks the whole tree; it is used by the diagnostics and by the
/// tests.
pub fn find_by_tag<'a>(node: &'a HtmlNode, tag: &str) -> Option<&'a HtmlNode> {
    if node.tag.eq_ignore_ascii_case(tag) {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_by_tag(child, tag) {
            return Some(found);
        }
    }
    None
}

/// Parse the HTML of a page into a tree.
///
/// The parser understands the tags that a hidden service uses: elements,
/// attributes, text and the comments.  It is deliberately small: it serves
/// the tests and the diagnostics, while the render process uses the DOM of
/// Blink.
pub fn parse_html(html: &str) -> HtmlNode {
    let mut root = HtmlNode::default();
    let bytes: Vec<char> = html.chars().collect();
    let mut index = 0usize;
    let mut stack: Vec<HtmlNode> = Vec::new();

    while index < bytes.len() {
        if bytes[index] == '<' {
            if html_at(&bytes, index, "<!--") {
                // A comment: skip it.
                let end = find_at(&bytes, index + 4, "-->").unwrap_or(bytes.len());
                index = end + 3;
                continue;
            }
            if html_at(&bytes, index, "<!") || html_at(&bytes, index, "<?") {
                let end = find_char(&bytes, index, '>').unwrap_or(bytes.len());
                index = end + 1;
                continue;
            }
            if html_at(&bytes, index, "</") {
                let end = find_char(&bytes, index, '>').unwrap_or(bytes.len());
                let name: String = bytes[index + 2..end.min(bytes.len())]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_ascii_lowercase();
                // Close the element.
                if let Some(position) = stack.iter().rposition(|node| node.tag == name) {
                    while stack.len() > position {
                        let node = stack.pop().unwrap();
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(node);
                        } else {
                            root.children.push(node);
                        }
                    }
                }
                index = end + 1;
                continue;
            }

            // An opening tag.
            let end = find_char(&bytes, index, '>').unwrap_or(bytes.len());
            let raw: String = bytes[index + 1..end.min(bytes.len())].iter().collect();
            let (tag, attributes) = parse_tag(&raw);
            let node = HtmlNode {
                tag,
                attributes,
                ..HtmlNode::default()
            };
            // The void elements have no content.
            if is_void_element(&node.tag) {
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root.children.push(node);
                }
            } else {
                stack.push(node);
            }
            index = end + 1;
            continue;
        }

        // Text.
        let end = find_char(&bytes, index, '<').unwrap_or(bytes.len());
        let text: String = bytes[index..end].iter().collect();
        if !text.trim().is_empty() {
            let node = HtmlNode::text_node(&text);
            if let Some(parent) = stack.last_mut() {
                parent.children.push(node);
            } else {
                root.children.push(node);
            }
        }
        index = end;
    }

    // Close the elements that are still open.
    while let Some(node) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(node);
        } else {
            root.children.push(node);
        }
    }

    root
}

/// `true` when the text starts with the needle.
fn html_at(chars: &[char], index: usize, needle: &str) -> bool {
    let needle: Vec<char> = needle.chars().collect();
    if index + needle.len() > chars.len() {
        return false;
    }
    chars[index..index + needle.len()] == needle[..]
}

/// The index of the next occurrence of the needle.
fn find_at(chars: &[char], from: usize, needle: &str) -> Option<usize> {
    let needle: Vec<char> = needle.chars().collect();
    if needle.is_empty() || chars.len() < needle.len() {
        return None;
    }
    (from..=chars.len() - needle.len())
        .find(|index| chars[*index..*index + needle.len()] == needle[..])
}

/// The index of the next occurrence of the character.
fn find_char(chars: &[char], from: usize, needle: char) -> Option<usize> {
    (from..chars.len()).find(|index| chars[*index] == needle)
}

/// Parse the name and the attributes of a tag.
fn parse_tag(raw: &str) -> (String, Vec<(String, String)>) {
    let raw = raw.trim();
    let mut chars = raw.chars().peekable();
    let mut name = String::new();
    for character in chars.by_ref() {
        if character.is_whitespace() {
            break;
        }
        name.push(character);
    }

    let mut attributes = Vec::new();
    let rest: String = chars.collect();
    let mut remaining = rest.as_str();

    while !remaining.is_empty() {
        // Skip the whitespace.
        let trimmed = remaining.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('/') {
            break;
        }
        // The name of the attribute.
        let end = trimmed
            .find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(trimmed.len());
        let key = trimmed[..end].to_ascii_lowercase();
        if key.is_empty() {
            break;
        }
        let after = &trimmed[end..];
        if let Some(after) = after.strip_prefix('=') {
            // The value is quoted or it ends at the next whitespace.
            if let Some(stripped) = after.strip_prefix('"') {
                let close = stripped.find('"').unwrap_or(stripped.len());
                attributes.push((key, stripped[..close].to_string()));
                remaining = &stripped[(close + 1).min(stripped.len())..];
                continue;
            }
            if let Some(stripped) = after.strip_prefix('\'') {
                let close = stripped.find('\'').unwrap_or(stripped.len());
                attributes.push((key, stripped[..close].to_string()));
                remaining = &stripped[(close + 1).min(stripped.len())..];
                continue;
            }
            let close = after.find(char::is_whitespace).unwrap_or(after.len());
            attributes.push((key, after[..close].to_string()));
            remaining = &after[close..];
            continue;
        }
        // A boolean attribute (`readonly`, `disabled`, ...).
        attributes.push((key.clone(), key));
        remaining = after;
    }

    (name.to_ascii_lowercase(), attributes)
}

/// `true` when the element cannot have children.
fn is_void_element(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_area_is_detected() {
        let html =
            "<html><body><form><textarea name=\"body\">hello</textarea></form></body></html>";
        let root = parse_html(html);
        let mut elements = root.elements();
        while let Some(node) = elements.first().copied() {
            elements = node.elements();
            if let Some(area) = elements.iter().find(|node| node.tag == "textarea") {
                let kinds = classify(area, false);
                assert_eq!(kinds, vec![FieldKind::Textarea]);
                return;
            }
        }
        panic!("the text area was not parsed");
    }

    #[test]
    fn a_pgp_block_is_detected() {
        let html = "<pre>-----BEGIN PGP MESSAGE-----\n\nhQEMA\n-----END PGP MESSAGE-----</pre>";
        let root = parse_html(html);
        let elements = root.elements();
        let block = elements
            .iter()
            .find(|node| node.tag == "pre")
            .expect("the block was not parsed");
        let kinds = classify(block, false);
        assert_eq!(kinds, vec![FieldKind::PgpMessage]);
    }

    #[test]
    fn the_write_target_of_a_control_is_its_value() {
        let root = parse_html("<input type=\"text\" value=\"x\">");
        let elements = root.elements();
        let input = elements
            .iter()
            .find(|node| node.tag == "input")
            .expect("the input was not parsed");
        assert_eq!(write_target(input), WriteTarget::ValueAttribute);
    }
}
