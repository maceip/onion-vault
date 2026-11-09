//! The input path of the browser.
//!
//! The DOM API of CEF cannot change the value of a form control that the
//! user has typed into: Blink keeps the dirty value of the control and a DOM
//! write only touches the default value, so a form would post the old text.
//! The input path of the browser can do it: the selection of the focused
//! control is deleted and the new text is sent as key events, exactly like a
//! keyboard would do it.
//!
//! No JavaScript is involved: the commands are the editing commands of the
//! browser and the text travels as character events.

use cef::{Browser, Frame, ImplBrowser, ImplBrowserHost, ImplFrame, KeyEvent, KeyEventType};

/// Replace the content of the focused field with `text`.
///
/// The function returns `false` when the browser host is not available.  The
/// caller reports the failure to the user: a field that still holds the old
/// text would send it, which is the failure mode the whole verification
/// exists for.
pub fn replace_focused_text(browser: &mut Browser, frame: &mut Frame, text: &str) -> bool {
    // Select the content of the focused control and delete it.
    frame.select_all();
    frame.del();

    let Some(host) = browser.host() else {
        log::error!("the browser has no host, the field cannot be changed");
        return false;
    };

    // The text is sent as character events, the way an input method commits
    // it.  The characters are sent as UTF-16 code units, so that a text
    // outside of the basic plane arrives as its surrogate pair.
    for unit in text.encode_utf16() {
        let event = character_event(unit);
        host.send_key_event(Some(&event));
    }

    true
}

/// A key event that carries a single character.
fn character_event(character: u16) -> KeyEvent {
    KeyEvent {
        size: std::mem::size_of::<KeyEvent>(),
        type_: KeyEventType::CHAR,
        modifiers: 0,
        windows_key_code: 0,
        native_key_code: 0,
        is_system_key: 0,
        character,
        unmodified_character: character,
        focus_on_editable_field: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_character_event_carries_the_character() {
        let event = character_event('a' as u16);
        assert_eq!(event.type_, KeyEventType::CHAR);
        assert_eq!(event.character, 'a' as u16);
        assert_eq!(event.unmodified_character, 'a' as u16);
    }
}
