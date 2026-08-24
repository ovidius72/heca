//! App-level keyboard normalization helpers.
//!
//! These functions keep event-to-keybinding and event-to-terminal translation
//! logic out of `main.rs`.

use crate::keymap::KeyCombo;
use heca_core::backend::{BackendKeyCode, BackendKeyEvent, BackendModifiers};
use winit::keyboard::{Key, ModifiersState, NamedKey, PhysicalKey};

/// Build a normalized key combo from the current key event and modifiers.
pub(crate) fn build_event_combo(
    logical_key: &Key,
    physical_key: &PhysicalKey,
    key_text: &str,
    modifiers: ModifiersState,
) -> KeyCombo {
    KeyCombo {
        key: normalize_key_text(
            logical_key,
            key_text,
            modifiers.shift_key(),
            modifiers.control_key(),
            physical_key,
        ),
        ctrl: modifiers.control_key(),
        shift: modifiers.shift_key(),
        alt: modifiers.alt_key(),
        super_: modifiers.super_key(),
    }
}

/// Check if the current event matches the configured prefix combo.
pub(crate) fn is_prefix_match(
    event_combo: &KeyCombo,
    prefix_combo: &KeyCombo,
    logical_key: &Key,
    key_text: &str,
) -> bool {
    event_combo_matches(event_combo, prefix_combo)
        || (prefix_combo.ctrl
            && prefix_combo.key.len() == 1
            && prefix_combo
                .key
                .chars()
                .next()
                .expect("single-char prefix key")
                .is_ascii_lowercase()
            && {
                let expected_ctrl = (prefix_combo.key.as_bytes()[0] - b'a' + 1) as char;
                key_text == String::from(expected_ctrl)
                    || matches!(logical_key, Key::Character(c) if c.starts_with(expected_ctrl))
            })
}

/// Check if a key event's combo matches a configured KeyCombo.
/// Case-insensitive for alphabetic keys; exact otherwise.
pub(crate) fn event_combo_matches(event: &KeyCombo, configured: &KeyCombo) -> bool {
    if event.ctrl != configured.ctrl
        || event.shift != configured.shift
        || event.alt != configured.alt
        || event.super_ != configured.super_
    {
        return false;
    }

    if event.key.len() == 1 && configured.key.len() == 1 {
        let e = event.key.chars().next().expect("single-char event key");
        let c = configured
            .key
            .chars()
            .next()
            .expect("single-char configured key");
        e.eq_ignore_ascii_case(&c)
    } else {
        event.key.eq_ignore_ascii_case(&configured.key)
    }
}

/// Normalize a winit key event into a config-compatible key string.
/// Named keys become their canonical name (Enter, Tab, ArrowLeft, etc.).
/// Character keys are lowercased so 'Q' from Shift+q matches config 'q'.
/// When `shift` is true, shifted symbols are mapped back to their unshifted
/// base key so that config "Shift+=" matches the event from Shift+Equal.
pub(crate) fn normalize_key_text(
    logical_key: &Key,
    key_text: &str,
    shift: bool,
    ctrl: bool,
    physical_key: &PhysicalKey,
) -> String {
    if ctrl && let PhysicalKey::Code(code) = physical_key {
        let mapped = match code {
            winit::keyboard::KeyCode::BracketLeft => "[",
            winit::keyboard::KeyCode::BracketRight => "]",
            winit::keyboard::KeyCode::Semicolon => ";",
            winit::keyboard::KeyCode::Quote => "'",
            winit::keyboard::KeyCode::Comma => ",",
            winit::keyboard::KeyCode::Period => ".",
            winit::keyboard::KeyCode::Slash => "/",
            winit::keyboard::KeyCode::Backslash => "\\",
            winit::keyboard::KeyCode::Minus => "-",
            winit::keyboard::KeyCode::Equal => "=",
            winit::keyboard::KeyCode::Backquote => "`",
            winit::keyboard::KeyCode::Digit0 => "0",
            winit::keyboard::KeyCode::Digit1 => "1",
            winit::keyboard::KeyCode::Digit2 => "2",
            winit::keyboard::KeyCode::Digit3 => "3",
            winit::keyboard::KeyCode::Digit4 => "4",
            winit::keyboard::KeyCode::Digit5 => "5",
            winit::keyboard::KeyCode::Digit6 => "6",
            winit::keyboard::KeyCode::Digit7 => "7",
            winit::keyboard::KeyCode::Digit8 => "8",
            winit::keyboard::KeyCode::Digit9 => "9",
            _ => "",
        };
        if !mapped.is_empty() {
            return mapped.to_string();
        }
    }

    let mut key = match logical_key {
        Key::Named(NamedKey::Escape) if ctrl => {
            if let PhysicalKey::Code(code) = physical_key {
                let mapped = match code {
                    winit::keyboard::KeyCode::BracketLeft => "[",
                    winit::keyboard::KeyCode::BracketRight => "]",
                    _ => "",
                };
                if !mapped.is_empty() {
                    return mapped.to_string();
                }
            }
            return "Escape".to_string();
        }
        Key::Named(n) => return format!("{:?}", n),
        Key::Character(c) => c.to_lowercase().to_string(),
        _ if !key_text.is_empty() => key_text.to_lowercase(),
        _ => return String::new(),
    };

    if shift {
        key = match key.as_str() {
            "+" => "=".to_string(),
            "_" => "-".to_string(),
            "{" => "[".to_string(),
            "}" => "]".to_string(),
            "|" => "\\".to_string(),
            ":" => ";".to_string(),
            "\"" => "'".to_string(),
            "<" => ",".to_string(),
            ">" => ".".to_string(),
            "?" => "/".to_string(),
            "!" => "1".to_string(),
            "@" => "2".to_string(),
            "#" => "3".to_string(),
            "$" => "4".to_string(),
            "%" => "5".to_string(),
            "^" => "6".to_string(),
            "&" => "7".to_string(),
            "*" => "8".to_string(),
            "(" => "9".to_string(),
            ")" => "0".to_string(),
            "~" => "`".to_string(),
            _ => key,
        };
    }

    key
}

/// Resolve a typed letter candidate from key text or physical key fallback.
pub(crate) fn typed_candidate_char(
    key_text: &str,
    physical_key: &PhysicalKey,
    shift: bool,
) -> Option<char> {
    // **Case is significant.** One picker hands out 52 letters — `a`-`z` then `A`-`Z` — so folding
    // everything to lowercase makes the capitals unreachable: the letter is drawn on screen and
    // pressing it matches nothing. It only stayed hidden while pickers never had more than 26
    // targets at once.
    key_text
        .chars()
        .next()
        .or_else(|| match physical_key {
            // The physical fallback (macOS `Ctrl+letter` gives a control char, so `key_text` is
            // empty) names a key, not a character, so the shift state decides its case.
            PhysicalKey::Code(code) => {
                let s = format!("{:?}", code);
                s.strip_prefix("Key").and_then(|n| n.chars().next()).map(|c| {
                    if shift {
                        c.to_ascii_uppercase()
                    } else {
                        c.to_ascii_lowercase()
                    }
                })
            }
            _ => None,
        })
}

/// **Is this key press just a modifier being held?**
///
/// Asked in two unrelated places for two unrelated reasons — a letter picker must not treat Shift
/// as "a wrong key" and cancel itself, and the terminal must not snap to the live bottom when you
/// merely reach for a modifier. Two copies of one question is a missing function, so this is it.
///
/// Read off the **logical** key: a modifier is a named key, and naming physical codes would have to
/// list left and right variants of everything and would still miss whatever a layout calls its own.
pub(crate) fn is_modifier_key(logical_key: &Key) -> bool {
    matches!(
        logical_key,
        Key::Named(
            NamedKey::Shift
                | NamedKey::Control
                | NamedKey::Alt
                | NamedKey::Super
                | NamedKey::Hyper
                | NamedKey::Meta
                | NamedKey::CapsLock
        )
    )
}

/// Convert a configured prefix combo to the literal bytes that should be
/// forwarded on double-prefix.
pub(crate) fn prefix_combo_to_literal_input(prefix_combo: &KeyCombo) -> Vec<u8> {
    if prefix_combo.alt || prefix_combo.super_ {
        return Vec::new();
    }

    if prefix_combo.ctrl {
        return match prefix_combo.key.as_str() {
            "[" => vec![0x1b],
            "\\" => vec![0x1c],
            "]" => vec![0x1d],
            _ if prefix_combo.key.len() == 1 => {
                let c = prefix_combo.key.as_bytes()[0];
                if c.is_ascii_lowercase() {
                    vec![c - b'a' + 1]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };
    }

    if prefix_combo.shift || prefix_combo.key.len() != 1 {
        return match prefix_combo.key.as_str() {
            "enter" => vec![b'\r'],
            "tab" => vec![b'\t'],
            "escape" => vec![0x1b],
            "space" => vec![b' '],
            _ => Vec::new(),
        };
    }

    prefix_combo.key.as_bytes().to_vec()
}

/// Convert a winit key event to terminal input bytes.
pub(crate) fn winit_key_to_terminal_input(key: &Key, text: &str, ctrl: bool) -> Vec<u8> {
    if ctrl && text.len() == 1 {
        let c = text.as_bytes()[0];
        if c.is_ascii_lowercase() {
            return vec![c - b'a' + 1];
        }
    }

    match key {
        Key::Named(NamedKey::Enter) => vec![b'\r'],
        Key::Named(NamedKey::Backspace) => vec![0x7f],
        Key::Named(NamedKey::Tab) => vec![b'\t'],
        Key::Named(NamedKey::Escape) => vec![0x1b],
        Key::Named(NamedKey::ArrowUp) => b"\x1b[A".to_vec(),
        Key::Named(NamedKey::ArrowDown) => b"\x1b[B".to_vec(),
        Key::Named(NamedKey::ArrowRight) => b"\x1b[C".to_vec(),
        Key::Named(NamedKey::ArrowLeft) => b"\x1b[D".to_vec(),
        Key::Named(NamedKey::Home) => b"\x1b[H".to_vec(),
        Key::Named(NamedKey::End) => b"\x1b[F".to_vec(),
        Key::Named(NamedKey::PageUp) => b"\x1b[5~".to_vec(),
        Key::Named(NamedKey::PageDown) => b"\x1b[6~".to_vec(),
        Key::Named(NamedKey::Delete) => b"\x1b[3~".to_vec(),
        Key::Named(NamedKey::Space) => vec![b' '],
        Key::Character(c) => c.as_bytes().to_vec(),
        _ => vec![],
    }
}

/// Convert a winit key event into a structured backend key event.
pub(crate) fn winit_key_to_backend_event(
    key: &Key,
    modifiers: ModifiersState,
) -> Option<BackendKeyEvent> {
    let code = match key {
        Key::Character(text) => text.chars().next().map(BackendKeyCode::Char)?,
        Key::Named(NamedKey::Enter) => BackendKeyCode::Enter,
        Key::Named(NamedKey::Backspace) => BackendKeyCode::Backspace,
        Key::Named(NamedKey::Tab) => BackendKeyCode::Tab,
        Key::Named(NamedKey::Escape) => BackendKeyCode::Escape,
        Key::Named(NamedKey::ArrowLeft) => BackendKeyCode::LeftArrow,
        Key::Named(NamedKey::ArrowRight) => BackendKeyCode::RightArrow,
        Key::Named(NamedKey::ArrowUp) => BackendKeyCode::UpArrow,
        Key::Named(NamedKey::ArrowDown) => BackendKeyCode::DownArrow,
        Key::Named(NamedKey::Home) => BackendKeyCode::Home,
        Key::Named(NamedKey::End) => BackendKeyCode::End,
        Key::Named(NamedKey::PageUp) => BackendKeyCode::PageUp,
        Key::Named(NamedKey::PageDown) => BackendKeyCode::PageDown,
        Key::Named(NamedKey::Insert) => BackendKeyCode::Insert,
        Key::Named(NamedKey::Delete) => BackendKeyCode::Delete,
        Key::Named(NamedKey::F1) => BackendKeyCode::Function(1),
        Key::Named(NamedKey::F2) => BackendKeyCode::Function(2),
        Key::Named(NamedKey::F3) => BackendKeyCode::Function(3),
        Key::Named(NamedKey::F4) => BackendKeyCode::Function(4),
        Key::Named(NamedKey::F5) => BackendKeyCode::Function(5),
        Key::Named(NamedKey::F6) => BackendKeyCode::Function(6),
        Key::Named(NamedKey::F7) => BackendKeyCode::Function(7),
        Key::Named(NamedKey::F8) => BackendKeyCode::Function(8),
        Key::Named(NamedKey::F9) => BackendKeyCode::Function(9),
        Key::Named(NamedKey::F10) => BackendKeyCode::Function(10),
        Key::Named(NamedKey::F11) => BackendKeyCode::Function(11),
        Key::Named(NamedKey::F12) => BackendKeyCode::Function(12),
        Key::Named(NamedKey::F13) => BackendKeyCode::Function(13),
        Key::Named(NamedKey::F14) => BackendKeyCode::Function(14),
        Key::Named(NamedKey::F15) => BackendKeyCode::Function(15),
        Key::Named(NamedKey::F16) => BackendKeyCode::Function(16),
        Key::Named(NamedKey::F17) => BackendKeyCode::Function(17),
        Key::Named(NamedKey::F18) => BackendKeyCode::Function(18),
        Key::Named(NamedKey::F19) => BackendKeyCode::Function(19),
        Key::Named(NamedKey::F20) => BackendKeyCode::Function(20),
        Key::Named(NamedKey::F21) => BackendKeyCode::Function(21),
        Key::Named(NamedKey::F22) => BackendKeyCode::Function(22),
        Key::Named(NamedKey::F23) => BackendKeyCode::Function(23),
        Key::Named(NamedKey::F24) => BackendKeyCode::Function(24),
        _ => return None,
    };

    Some(BackendKeyEvent {
        code,
        modifiers: BackendModifiers {
            ctrl: modifiers.control_key(),
            shift: modifiers.shift_key(),
            alt: modifiers.alt_key(),
            super_: modifiers.super_key(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{prefix_combo_to_literal_input, typed_candidate_char, winit_key_to_backend_event};
    use crate::keymap::KeyCombo;
    use heca_core::backend::BackendKeyCode;
    use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

    /// **The typed text wins, and its case is kept.** A picker hands out `a`-`z` then `A`-`Z`, so
    /// folding to lowercase makes every capital unreachable — drawn on screen, matching nothing.
    #[test]
    fn typed_candidate_char_prefers_key_text_and_keeps_its_case() {
        assert_eq!(
            typed_candidate_char("Q", &PhysicalKey::Code(KeyCode::KeyA), true),
            Some('Q')
        );
        assert_eq!(
            typed_candidate_char("q", &PhysicalKey::Code(KeyCode::KeyA), false),
            Some('q')
        );
    }

    /// The physical fallback (macOS `Ctrl+letter` empties `key_text`) names a KEY, not a character,
    /// so the shift state decides its case.
    #[test]
    fn typed_candidate_char_falls_back_to_physical_key() {
        assert_eq!(
            typed_candidate_char("", &PhysicalKey::Code(KeyCode::KeyZ), false),
            Some('z')
        );
        assert_eq!(
            typed_candidate_char("", &PhysicalKey::Code(KeyCode::KeyZ), true),
            Some('Z')
        );
    }

    /// A modifier being held is not a key a picker should answer — it is part of pressing the
    /// letter that follows.
    #[test]
    fn a_modifier_press_is_not_a_candidate_key() {
        use super::is_modifier_key;
        use winit::keyboard::{Key, NamedKey};
        assert!(is_modifier_key(&Key::Named(NamedKey::Shift)));
        assert!(is_modifier_key(&Key::Named(NamedKey::Control)));
        assert!(!is_modifier_key(&Key::Named(NamedKey::Escape)));
        assert!(!is_modifier_key(&Key::Character("a".into())));
    }

    #[test]
    fn prefix_combo_to_literal_input_uses_configured_ctrl_prefix() {
        assert_eq!(
            prefix_combo_to_literal_input(&KeyCombo::parse("Ctrl+a")),
            vec![0x01]
        );
        assert_eq!(
            prefix_combo_to_literal_input(&KeyCombo::parse("Ctrl+[")),
            vec![0x1b]
        );
    }

    #[test]
    fn backend_key_event_maps_named_and_character_keys() {
        let char_event = winit_key_to_backend_event(
            &winit::keyboard::Key::Character("x".into()),
            ModifiersState::empty(),
        )
        .expect("character key should map");
        assert_eq!(char_event.code, BackendKeyCode::Char('x'));

        let enter_event = winit_key_to_backend_event(
            &winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter),
            ModifiersState::empty(),
        )
        .expect("enter key should map");
        assert_eq!(enter_event.code, BackendKeyCode::Enter);
    }
}
