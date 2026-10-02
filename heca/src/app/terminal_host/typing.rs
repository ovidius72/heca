//! **Typing into a terminal**, whoever owns it: the text and keys a terminal heard while it held the
//! keyboard reach its process by the terminal's id.
//!
//! The terminal works out nothing here — it says what arrived ([`TerminalInput::Text`],
//! [`TerminalInput::Key`]) and this hands it to the process. A key is encoded by the emulator
//! (`process_key_event`), which knows what the program asked for (application cursor keys, kitty
//! keyboard, …); this only names the key in the backend's terms.

use crate::app_state::AppState;
use crate::chrome::terminal::TerminalId;
use heca_core::backend::{BackendKeyCode, BackendKeyEvent, BackendModifiers};
use heca_grid_ui::{GridKey, Modifiers};

/// The backend's name for a key the tree delivered.
fn key_code(key: GridKey) -> BackendKeyCode {
    match key {
        GridKey::Char(c) => BackendKeyCode::Char(c),
        GridKey::Space => BackendKeyCode::Char(' '),
        GridKey::Enter => BackendKeyCode::Enter,
        GridKey::Tab => BackendKeyCode::Tab,
        GridKey::Escape => BackendKeyCode::Escape,
        GridKey::Backspace => BackendKeyCode::Backspace,
        GridKey::Delete => BackendKeyCode::Delete,
        GridKey::ArrowLeft => BackendKeyCode::LeftArrow,
        GridKey::ArrowRight => BackendKeyCode::RightArrow,
        GridKey::ArrowUp => BackendKeyCode::UpArrow,
        GridKey::ArrowDown => BackendKeyCode::DownArrow,
        GridKey::Home => BackendKeyCode::Home,
        GridKey::End => BackendKeyCode::End,
        GridKey::PageUp => BackendKeyCode::PageUp,
        GridKey::PageDown => BackendKeyCode::PageDown,
        GridKey::Insert => BackendKeyCode::Insert,
        GridKey::Function(n) => BackendKeyCode::Function(n),
    }
}

/// A key and what was held, as the backend takes it.
pub(super) fn key_event(key: GridKey, modifiers: Modifiers) -> BackendKeyEvent {
    BackendKeyEvent {
        code: key_code(key),
        modifiers: BackendModifiers {
            ctrl: modifiers.ctrl,
            shift: modifiers.shift,
            alt: modifiers.alt,
            super_: modifiers.meta,
        },
    }
}

/// Typed text reaches the process, and the view snaps to the live bottom — typing is how you say
/// you want to see where you are.
pub(super) fn on_text(state: &mut AppState, terminal: TerminalId, text: &str) {
    if let Some(backend) = state.backends.get_mut_by_id(terminal) {
        backend.scroll_to_bottom();
        backend.process_input(text.as_bytes());
        state.needs_redraw = true;
    }
}

/// A key that is not text reaches the process, snapping to the live bottom like typing does. A
/// lone modifier is not typing: it never reaches here, because the tree only delivers a key.
pub(super) fn on_key(
    state: &mut AppState,
    terminal: TerminalId,
    key: GridKey,
    modifiers: Modifiers,
) {
    if let Some(backend) = state.backends.get_mut_by_id(terminal) {
        backend.scroll_to_bottom();
        backend.process_key_event(&key_event(key, modifiers));
        state.needs_redraw = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_named_in_the_backends_terms_with_what_was_held() {
        let ctrl = Modifiers {
            ctrl: true,
            ..Default::default()
        };
        let event = key_event(GridKey::Char('c'), ctrl);
        assert_eq!(event.code, BackendKeyCode::Char('c'));
        assert!(event.modifiers.ctrl && !event.modifiers.shift && !event.modifiers.alt);
    }

    #[test]
    fn the_command_key_is_the_backends_super() {
        let meta = Modifiers {
            meta: true,
            ..Default::default()
        };
        assert!(key_event(GridKey::Enter, meta).modifiers.super_);
    }

    #[test]
    fn every_key_the_tree_can_deliver_has_a_backend_name() {
        assert_eq!(key_code(GridKey::Space), BackendKeyCode::Char(' '));
        assert_eq!(key_code(GridKey::PageUp), BackendKeyCode::PageUp);
        assert_eq!(key_code(GridKey::Function(5)), BackendKeyCode::Function(5));
        assert_eq!(key_code(GridKey::ArrowLeft), BackendKeyCode::LeftArrow);
    }
}
