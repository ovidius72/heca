//! App-level keyboard normalization helpers.
//!
//! These functions keep event-to-keybinding and event-to-terminal translation
//! logic out of `main.rs`.

use crate::keymap::KeyCombo;
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
pub(crate) fn typed_candidate_char(key_text: &str, physical_key: &PhysicalKey) -> Option<char> {
    key_text
        .chars()
        .next()
        .or_else(|| match physical_key {
            PhysicalKey::Code(code) => {
                let s = format!("{:?}", code);
                s.strip_prefix("Key").and_then(|n| n.chars().next())
            }
            _ => None,
        })
        .map(|c| c.to_ascii_lowercase())
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

#[cfg(test)]
mod tests {
    use super::{prefix_combo_to_literal_input, typed_candidate_char};
    use crate::keymap::KeyCombo;
    use winit::keyboard::{KeyCode, PhysicalKey};

    #[test]
    fn typed_candidate_char_prefers_key_text() {
        assert_eq!(
            typed_candidate_char("Q", &PhysicalKey::Code(KeyCode::KeyA)),
            Some('q')
        );
    }

    #[test]
    fn typed_candidate_char_falls_back_to_physical_key() {
        assert_eq!(
            typed_candidate_char("", &PhysicalKey::Code(KeyCode::KeyZ)),
            Some('z')
        );
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
}
