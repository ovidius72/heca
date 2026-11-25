//! **Typing into a terminal**, whoever owns it: the text and keys a terminal heard while it held the
//! keyboard reach its process by the terminal's id.
//!
//! The terminal works out nothing here — it says what arrived
//! ([`TerminalInput::Text`](crate::chrome::panes::terminal::TerminalInput::Text),
//! [`TerminalInput::Key`](crate::chrome::panes::terminal::TerminalInput::Key)) and this hands it to the
//! process. A key is encoded by the emulator (`process_key_event`), which knows what the program asked for
//! (application cursor keys, kitty keyboard, …); this only names the key in the backend's terms.

use crate::app_state::AppState;
use crate::chrome::panes::terminal::TerminalId;
use heca_core::backend::{BackendKeyCode, BackendKeyEvent, BackendModifiers, PaneBackend};
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

/// **What typing does, whatever was typed**: the terminal's process hears it through `send` and the
/// view snaps to the live bottom — typing is how you say you want to see where you are. Nothing
/// happens for a terminal that no longer exists.
///
/// **A frame only when the view moved.** What a key changes on screen is the program's answer, and
/// that arrives as output, which asks for its own frame. Drawing on the key itself painted the same
/// picture again before the echo existed — and that frame took one of the two places queued to the
/// screen, so the frame with the character waited behind it.
fn type_into(state: &mut AppState, terminal: TerminalId, send: impl FnOnce(&mut dyn PaneBackend)) {
    if let Some(backend) = state.client.backends.get_mut_by_id(terminal)
        && typed(backend, send)
    {
        state.needs_redraw = true;
    }
}

/// [`type_into`] on the terminal itself: the view snaps to the bottom and the process hears `send`.
/// Says whether the view moved — the only thing typing changes on screen by itself. A key sent to
/// the focused terminal goes through here too, so typed text and a pressed key follow one rule.
pub(crate) fn typed(
    backend: &mut dyn PaneBackend,
    send: impl FnOnce(&mut dyn PaneBackend),
) -> bool {
    let was_back_in_history = !backend.at_bottom();
    backend.scroll_to_bottom();
    send(backend);
    was_back_in_history
}

/// Typed text reaches the process.
pub(super) fn on_text(state: &mut AppState, terminal: TerminalId, text: &str) {
    type_into(state, terminal, |backend| {
        backend.process_input(text.as_bytes());
    });
}

/// Text to paste reaches the process the way the clipboard does — bracketed when the program asked.
pub(super) fn on_paste(state: &mut AppState, terminal: TerminalId, text: &str) {
    type_into(state, terminal, |backend| backend.paste(text));
}

/// A key that is not text reaches the process. A lone modifier is not typing: it never reaches
/// here, because the tree only delivers a key.
pub(super) fn on_key(
    state: &mut AppState,
    terminal: TerminalId,
    key: GridKey,
    modifiers: Modifiers,
) {
    type_into(state, terminal, |backend| {
        backend.process_key_event(&key_event(key, modifiers));
    });
}

#[cfg(test)]
mod tests;
