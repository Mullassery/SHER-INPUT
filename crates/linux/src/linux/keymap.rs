//! Linux evdev keycode -> SHER-Input [`PhysicalKey`] / [`PointerButton`] translation.
//! This is the entire surface where Linux-specific codes are allowed to exist
//! (section 25) — nothing outside this module ever sees an `evdev::Key`.

use evdev::Key;
use sher_input_core::{PhysicalKey, PointerButton};

pub fn map_key(key: Key) -> PhysicalKey {
    match key {
        Key::KEY_A => PhysicalKey::KeyA,
        Key::KEY_B => PhysicalKey::KeyB,
        Key::KEY_C => PhysicalKey::KeyC,
        Key::KEY_D => PhysicalKey::KeyD,
        Key::KEY_E => PhysicalKey::KeyE,
        Key::KEY_F => PhysicalKey::KeyF,
        Key::KEY_G => PhysicalKey::KeyG,
        Key::KEY_H => PhysicalKey::KeyH,
        Key::KEY_I => PhysicalKey::KeyI,
        Key::KEY_J => PhysicalKey::KeyJ,
        Key::KEY_K => PhysicalKey::KeyK,
        Key::KEY_L => PhysicalKey::KeyL,
        Key::KEY_M => PhysicalKey::KeyM,
        Key::KEY_N => PhysicalKey::KeyN,
        Key::KEY_O => PhysicalKey::KeyO,
        Key::KEY_P => PhysicalKey::KeyP,
        Key::KEY_Q => PhysicalKey::KeyQ,
        Key::KEY_R => PhysicalKey::KeyR,
        Key::KEY_S => PhysicalKey::KeyS,
        Key::KEY_T => PhysicalKey::KeyT,
        Key::KEY_U => PhysicalKey::KeyU,
        Key::KEY_V => PhysicalKey::KeyV,
        Key::KEY_W => PhysicalKey::KeyW,
        Key::KEY_X => PhysicalKey::KeyX,
        Key::KEY_Y => PhysicalKey::KeyY,
        Key::KEY_Z => PhysicalKey::KeyZ,
        Key::KEY_0 => PhysicalKey::Digit0,
        Key::KEY_1 => PhysicalKey::Digit1,
        Key::KEY_2 => PhysicalKey::Digit2,
        Key::KEY_3 => PhysicalKey::Digit3,
        Key::KEY_4 => PhysicalKey::Digit4,
        Key::KEY_5 => PhysicalKey::Digit5,
        Key::KEY_6 => PhysicalKey::Digit6,
        Key::KEY_7 => PhysicalKey::Digit7,
        Key::KEY_8 => PhysicalKey::Digit8,
        Key::KEY_9 => PhysicalKey::Digit9,
        Key::KEY_ENTER => PhysicalKey::Enter,
        Key::KEY_ESC => PhysicalKey::Escape,
        Key::KEY_BACKSPACE => PhysicalKey::Backspace,
        Key::KEY_TAB => PhysicalKey::Tab,
        Key::KEY_SPACE => PhysicalKey::Space,
        Key::KEY_MINUS => PhysicalKey::Minus,
        Key::KEY_EQUAL => PhysicalKey::Equal,
        Key::KEY_LEFTBRACE => PhysicalKey::BracketLeft,
        Key::KEY_RIGHTBRACE => PhysicalKey::BracketRight,
        Key::KEY_BACKSLASH => PhysicalKey::Backslash,
        Key::KEY_SEMICOLON => PhysicalKey::Semicolon,
        Key::KEY_APOSTROPHE => PhysicalKey::Quote,
        Key::KEY_GRAVE => PhysicalKey::Grave,
        Key::KEY_COMMA => PhysicalKey::Comma,
        Key::KEY_DOT => PhysicalKey::Period,
        Key::KEY_SLASH => PhysicalKey::Slash,
        Key::KEY_CAPSLOCK => PhysicalKey::CapsLock,
        Key::KEY_SCROLLLOCK => PhysicalKey::ScrollLock,
        Key::KEY_NUMLOCK => PhysicalKey::NumLock,
        Key::KEY_F1 => PhysicalKey::F1,
        Key::KEY_F2 => PhysicalKey::F2,
        Key::KEY_F3 => PhysicalKey::F3,
        Key::KEY_F4 => PhysicalKey::F4,
        Key::KEY_F5 => PhysicalKey::F5,
        Key::KEY_F6 => PhysicalKey::F6,
        Key::KEY_F7 => PhysicalKey::F7,
        Key::KEY_F8 => PhysicalKey::F8,
        Key::KEY_F9 => PhysicalKey::F9,
        Key::KEY_F10 => PhysicalKey::F10,
        Key::KEY_F11 => PhysicalKey::F11,
        Key::KEY_F12 => PhysicalKey::F12,
        Key::KEY_SYSRQ => PhysicalKey::PrintScreen,
        Key::KEY_PAUSE => PhysicalKey::Pause,
        Key::KEY_INSERT => PhysicalKey::Insert,
        Key::KEY_HOME => PhysicalKey::Home,
        Key::KEY_PAGEUP => PhysicalKey::PageUp,
        Key::KEY_DELETE => PhysicalKey::Delete,
        Key::KEY_END => PhysicalKey::End,
        Key::KEY_PAGEDOWN => PhysicalKey::PageDown,
        Key::KEY_RIGHT => PhysicalKey::ArrowRight,
        Key::KEY_LEFT => PhysicalKey::ArrowLeft,
        Key::KEY_DOWN => PhysicalKey::ArrowDown,
        Key::KEY_UP => PhysicalKey::ArrowUp,
        Key::KEY_LEFTCTRL => PhysicalKey::ControlLeft,
        Key::KEY_LEFTSHIFT => PhysicalKey::ShiftLeft,
        Key::KEY_LEFTALT => PhysicalKey::AltLeft,
        Key::KEY_LEFTMETA => PhysicalKey::SuperLeft,
        Key::KEY_RIGHTCTRL => PhysicalKey::ControlRight,
        Key::KEY_RIGHTSHIFT => PhysicalKey::ShiftRight,
        Key::KEY_RIGHTALT => PhysicalKey::AltRight,
        Key::KEY_RIGHTMETA => PhysicalKey::SuperRight,
        Key::KEY_PLAYPAUSE => PhysicalKey::MediaPlayPause,
        Key::KEY_NEXTSONG => PhysicalKey::MediaNextTrack,
        Key::KEY_PREVIOUSSONG => PhysicalKey::MediaPrevTrack,
        Key::KEY_VOLUMEUP => PhysicalKey::MediaVolumeUp,
        Key::KEY_VOLUMEDOWN => PhysicalKey::MediaVolumeDown,
        Key::KEY_MUTE => PhysicalKey::MediaMute,
        Key::KEY_COMPOSE => PhysicalKey::Compose,
        other => PhysicalKey::Other(other.code() as u32),
    }
}

/// Linux kernel's `<linux/input-event-codes.h>` defines `BTN_MISC` (0x100) and
/// `BTN_JOYSTICK` (0x120) as range boundaries, not individual buttons — the `evdev`
/// crate's `Key` enum only exposes the named keys inside that range (`BTN_0`..`BTN_9`,
/// `BTN_LEFT`..`BTN_TASK`), not the boundary constants themselves, so the range is
/// reproduced here as raw codes.
const BTN_MISC: u16 = 0x100;
const BTN_JOYSTICK: u16 = 0x120;

pub fn map_button(key: Key) -> Option<PointerButton> {
    match key {
        Key::BTN_LEFT => Some(PointerButton::Left),
        Key::BTN_RIGHT => Some(PointerButton::Right),
        Key::BTN_MIDDLE => Some(PointerButton::Middle),
        Key::BTN_SIDE => Some(PointerButton::Back),
        Key::BTN_EXTRA => Some(PointerButton::Forward),
        other if (BTN_MISC..BTN_JOYSTICK).contains(&other.code()) => {
            Some(PointerButton::Other(other.code()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letter_and_digit_keys_map_to_their_named_variants() {
        assert_eq!(map_key(Key::KEY_A), PhysicalKey::KeyA);
        assert_eq!(map_key(Key::KEY_Z), PhysicalKey::KeyZ);
        assert_eq!(map_key(Key::KEY_0), PhysicalKey::Digit0);
        assert_eq!(map_key(Key::KEY_9), PhysicalKey::Digit9);
    }

    #[test]
    fn an_unmapped_key_falls_back_to_other_carrying_its_raw_code() {
        // KEY_KATAKANA has no named PhysicalKey variant; must not silently map onto an
        // unrelated key, and must not panic.
        let mapped = map_key(Key::KEY_KATAKANA);
        assert_eq!(mapped, PhysicalKey::Other(Key::KEY_KATAKANA.code() as u32));
    }

    #[test]
    fn mouse_buttons_map_to_their_named_pointer_button_variants() {
        assert_eq!(map_button(Key::BTN_LEFT), Some(PointerButton::Left));
        assert_eq!(map_button(Key::BTN_RIGHT), Some(PointerButton::Right));
        assert_eq!(map_button(Key::BTN_MIDDLE), Some(PointerButton::Middle));
        assert_eq!(map_button(Key::BTN_SIDE), Some(PointerButton::Back));
        assert_eq!(map_button(Key::BTN_EXTRA), Some(PointerButton::Forward));
    }

    #[test]
    fn misc_button_codes_in_range_map_to_other_with_the_raw_code_preserved() {
        // BTN_0 (0x100) is the first code in the BTN_MISC..BTN_JOYSTICK range and has
        // no named PointerButton variant — must come back as `Other` carrying the
        // exact evdev code, not a truncated or reinterpreted one (this range used to
        // be computed from `Key::BTN_MISC`/`Key::BTN_JOYSTICK`, constants that do not
        // actually exist on `evdev::Key` — a bug only a real Linux compile surfaced).
        assert_eq!(
            map_button(Key::BTN_0),
            Some(PointerButton::Other(Key::BTN_0.code()))
        );
        assert_eq!(Key::BTN_0.code(), 0x100);
    }

    #[test]
    fn a_letter_key_is_not_mistaken_for_a_pointer_button() {
        assert_eq!(map_button(Key::KEY_A), None);
    }
}
