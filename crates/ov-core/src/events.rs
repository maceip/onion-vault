//! Translation of the Android input events into the CEF input events.
//!
//! CEF expects the same event objects that a desktop Chromium uses: a
//! `CefMouseEvent` with window coordinates, a `CefTouchEvent` for touch
//! screens and a `CefKeyEvent` with a *Windows* virtual key code.  Android
//! uses its own key codes, so they have to be translated.

use cef::{
    KeyEvent, KeyEventType, MouseButtonType, MouseEvent, PointerType, TouchEvent, TouchEventType,
};

/// The modifiers of a CEF event.
pub mod modifiers {
    /// No modifier is pressed.
    pub const NONE: u32 = 0;
    /// The shift key.
    pub const SHIFT: u32 = 1 << 0;
    /// The control key.
    pub const CTRL: u32 = 1 << 1;
    /// The alt key.
    pub const ALT: u32 = 1 << 2;
    /// The meta key.
    pub const META: u32 = 1 << 3;
}

/// The Android key codes that Trap translates.
pub mod android_key {
    /// The back button.
    pub const BACK: i32 = 4;
    /// The dpad up key.
    pub const DPAD_UP: i32 = 19;
    /// The dpad down key.
    pub const DPAD_DOWN: i32 = 20;
    /// The dpad left key.
    pub const DPAD_LEFT: i32 = 21;
    /// The dpad right key.
    pub const DPAD_RIGHT: i32 = 22;
    /// The tab key.
    pub const TAB: i32 = 61;
    /// The space key.
    pub const SPACE: i32 = 62;
    /// The enter key.
    pub const ENTER: i32 = 66;
    /// The backspace key.
    pub const DEL: i32 = 67;
    /// The page up key.
    pub const PAGE_UP: i32 = 92;
    /// The page down key.
    pub const PAGE_DOWN: i32 = 93;
    /// The escape key.
    pub const ESCAPE: i32 = 111;
    /// The delete key.
    pub const FORWARD_DEL: i32 = 112;
    /// The home key.
    pub const MOVE_HOME: i32 = 122;
    /// The end key.
    pub const MOVE_END: i32 = 123;
}

/// The Windows virtual key codes that CEF expects.
mod vk {
    /// Backspace.
    pub const BACK: i32 = 0x08;
    /// Tab.
    pub const TAB: i32 = 0x09;
    /// Return.
    pub const RETURN: i32 = 0x0D;
    /// Shift.
    pub const SHIFT: i32 = 0x10;
    /// Control.
    pub const CONTROL: i32 = 0x11;
    /// Alt.
    pub const MENU: i32 = 0x12;
    /// Escape.
    pub const ESCAPE: i32 = 0x1B;
    /// Space.
    pub const SPACE: i32 = 0x20;
    /// Page up.
    pub const PRIOR: i32 = 0x21;
    /// Page down.
    pub const NEXT: i32 = 0x22;
    /// End.
    pub const END: i32 = 0x23;
    /// Home.
    pub const HOME: i32 = 0x24;
    /// Left arrow.
    pub const LEFT: i32 = 0x25;
    /// Up arrow.
    pub const UP: i32 = 0x26;
    /// Right arrow.
    pub const RIGHT: i32 = 0x27;
    /// Down arrow.
    pub const DOWN: i32 = 0x28;
    /// Insert.
    #[allow(dead_code)]
    pub const INSERT: i32 = 0x2D;
    /// Delete.
    pub const DELETE: i32 = 0x2E;
    /// The first function key.
    pub const F1: i32 = 0x70;
}

/// Translate an Android key code into a Windows virtual key code.
pub fn android_key_to_vk(code: i32) -> i32 {
    match code {
        android_key::BACK => vk::BACK,
        android_key::TAB => vk::TAB,
        android_key::ENTER => vk::RETURN,
        android_key::SPACE => vk::SPACE,
        android_key::DEL => vk::BACK,
        android_key::FORWARD_DEL => vk::DELETE,
        android_key::ESCAPE => vk::ESCAPE,
        android_key::PAGE_UP => vk::PRIOR,
        android_key::PAGE_DOWN => vk::NEXT,
        android_key::MOVE_HOME => vk::HOME,
        android_key::MOVE_END => vk::END,
        android_key::DPAD_LEFT => vk::LEFT,
        android_key::DPAD_UP => vk::UP,
        android_key::DPAD_RIGHT => vk::RIGHT,
        android_key::DPAD_DOWN => vk::DOWN,
        59 | 60 => vk::SHIFT,
        113 | 114 => vk::CONTROL,
        57 | 58 => vk::MENU,
        117 | 118 => 0x5B, // VK_LWIN / VK_RWIN
        code if (131..=140).contains(&code) => vk::F1 + (code - 131),
        code if (0x20..0x7F).contains(&code) => code,
        _ => 0,
    }
}

/// Build a CEF mouse event from the Android pointer position.
pub fn mouse_event(x: f32, y: f32, modifiers: u32) -> MouseEvent {
    MouseEvent {
        x: x as i32,
        y: y as i32,
        modifiers,
    }
}

/// The action of an Android `MotionEvent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchAction {
    /// The finger went down.
    Down,
    /// The finger moved.
    Move,
    /// The finger went up.
    Up,
    /// The gesture was cancelled.
    Cancel,
}

impl TouchAction {
    /// The CEF touch event type.
    pub fn cef_type(&self) -> TouchEventType {
        match self {
            TouchAction::Down => TouchEventType::PRESSED,
            TouchAction::Move => TouchEventType::MOVED,
            TouchAction::Up => TouchEventType::RELEASED,
            TouchAction::Cancel => TouchEventType::CANCELLED,
        }
    }

    /// Parse the Android action of a `MotionEvent`.
    ///
    /// The action is masked with `ACTION_POINTER_INDEX_MASK`; only the
    /// primary pointer is forwarded to CEF.
    pub fn from_android(action: i32) -> Option<TouchAction> {
        match action & 0xFF {
            0 => Some(TouchAction::Down),
            1 => Some(TouchAction::Up),
            2 => Some(TouchAction::Move),
            3 => Some(TouchAction::Cancel),
            _ => None,
        }
    }
}

/// Build a CEF touch event.
pub fn touch_event(action: TouchAction, x: f32, y: f32, id: i32, modifiers: u32) -> TouchEvent {
    TouchEvent {
        id,
        x,
        y,
        radius_x: 0.0,
        radius_y: 0.0,
        rotation_angle: 0.0,
        pressure: if action == TouchAction::Up { 0.0 } else { 1.0 },
        type_: action.cef_type(),
        modifiers,
        pointer_type: PointerType::TOUCH,
    }
}

/// The kind of a key event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// The key went down.
    Down,
    /// The key went up.
    Up,
    /// A character was typed.
    Char,
}

impl KeyAction {
    /// The CEF key event type.
    pub fn cef_type(&self) -> KeyEventType {
        match self {
            KeyAction::Down => KeyEventType::RAWKEYDOWN,
            KeyAction::Up => KeyEventType::KEYUP,
            KeyAction::Char => KeyEventType::CHAR,
        }
    }
}

/// Build a CEF key event.
pub fn key_event(
    action: KeyAction,
    android_code: i32,
    character: u16,
    modifiers: u32,
    is_system_key: bool,
) -> KeyEvent {
    KeyEvent {
        size: std::mem::size_of::<KeyEvent>(),
        type_: action.cef_type(),
        modifiers,
        windows_key_code: android_key_to_vk(android_code),
        native_key_code: android_code,
        is_system_key: is_system_key as i32,
        character,
        unmodified_character: character,
        focus_on_editable_field: 0,
    }
}

/// The mouse button of an Android pointer event.
pub fn mouse_button(button: i32) -> MouseButtonType {
    match button {
        1 => MouseButtonType::MIDDLE,
        2 => MouseButtonType::RIGHT,
        _ => MouseButtonType::LEFT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrow_keys_are_translated() {
        assert_eq!(android_key_to_vk(android_key::DPAD_LEFT), vk::LEFT);
        assert_eq!(android_key_to_vk(android_key::DPAD_DOWN), vk::DOWN);
    }

    #[test]
    fn the_function_keys_are_translated() {
        assert_eq!(android_key_to_vk(131), vk::F1);
        assert_eq!(android_key_to_vk(140), vk::F1 + 9);
    }

    #[test]
    fn the_touch_actions_are_parsed() {
        assert_eq!(TouchAction::from_android(0), Some(TouchAction::Down));
        assert_eq!(TouchAction::from_android(1), Some(TouchAction::Up));
        assert_eq!(TouchAction::from_android(2), Some(TouchAction::Move));
        assert_eq!(TouchAction::from_android(5), None);
    }

    #[test]
    fn the_mouse_position_is_converted_to_integers() {
        let event = mouse_event(10.7, 20.2, modifiers::NONE);
        assert_eq!(event.x, 10);
        assert_eq!(event.y, 20);
    }
}
