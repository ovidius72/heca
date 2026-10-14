//! Keys that go **into the window tree**, and which surfaces get to send them there.
//!
//! The tree delivers a key to whatever holds focus and bubbles it back up, so a focused field, a
//! docked terminal or a plugin's control is typed into by the rule that already reaches every other
//! widget. This module is the host's half: *when* a key is handed to the tree, and *how* — one
//! function, so the layer branch and the dock branch cannot feed a widget differently.

use crate::app::input::FocusedSurface;
use crate::app_state::AppState;
use crate::keymap::{DIRECT_LAYER, KeyCombo, KeymapRegistry};
use heca_grid_ui::Handled;

impl FocusedSurface {
    /// **Does a key nobody claimed go into the tree?** A layer and a dock hold the keyboard, so what
    /// their keymaps leave over belongs to whatever inside them holds focus. The panes never do:
    /// their keys belong to the program in the pane, and a header button the user happened to click
    /// must not be able to take typing from it.
    pub(crate) fn delivers_to_tree(&self) -> bool {
        match self {
            Self::Layer { .. } | Self::Dock { .. } => true,
            Self::Panes => false,
        }
    }
}

/// **Does the window hold the keyboard?** Whatever holds it in there — a dock, an overlay, anything
/// placed later — a key goes to it and bubbles up, like any key. Not while the panes have it: they
/// are not in the window's keyboard yet, and their keys go to the pane's program.
pub(crate) fn window_holds_keyboard(state: &AppState) -> bool {
    crate::app::input::focused_surface(state).delivers_to_tree()
}

/// The press a combo and its text make, in the tree's terms. `None` when the key has no tree form.
fn press_of(combo: &KeyCombo, key_text: &str) -> Option<heca_grid_ui::KeyPress> {
    // The already-normalized combo, which carries the macOS physical-key fallback for
    // `Ctrl+letter`, unlike the raw logical key — so vim-style `Ctrl+h/j/k/l` resolve correctly.
    let (key, mods) = crate::app::registry::combo_to_grid(combo)?;
    Some(heca_grid_ui::KeyPress {
        key,
        text: Some(key_text.to_string()),
        mods,
    })
}

/// Give one key press to the window tree: the committed text, the key, then the intents the key
/// resolves to — in that order, written once inside `Keymap::deliver_press`, so every surface feeds
/// a widget identically.
///
/// `None` when the key has no tree form at all (a lone modifier, say), so the caller can tell
/// "nothing to deliver" from "delivered and nobody took it".
///
/// **A key the app binds is the app's before any terminal's**, wherever the terminal is placed. A
/// terminal sends every key it is given to its program, so a terminal in a dock or a popup would
/// otherwise eat `Shift+PageUp` or `Cmd+V` — which a pane's terminal never sees, because the app's
/// keys are answered before a pane's program is. Such a key is not delivered, and the caller's
/// keymap answers it.
pub(crate) fn deliver_press_to_tree(
    state: &mut AppState,
    app_keys: &KeymapRegistry,
    combo: &KeyCombo,
    key_text: &str,
) -> Option<Handled> {
    let press = press_of(combo, key_text)?;
    if the_app_answers_first(state.window_keyboard_terminal().is_some(), app_keys, combo) {
        return Some(Handled::No);
    }
    // The keymap and the tree are different fields, so both are borrowed as they are: no copy.
    Some(state.widget_keymap.deliver_press(&press, |ev| {
        heca_grid_ui::dispatch(&mut state.window_root, ev)
    }))
}

/// Whether `combo` is the app's to answer before the tree: it is bound app-wide and a terminal
/// holds the keyboard.
fn the_app_answers_first(
    terminal_holds_keyboard: bool,
    app_keys: &KeymapRegistry,
    combo: &KeyCombo,
) -> bool {
    terminal_holds_keyboard && app_keys.resolve(DIRECT_LAYER, combo).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dock() -> FocusedSurface {
        FocusedSurface::Dock {
            mount: "workspaces".into(),
            kind: None,
        }
    }

    /// The rule that keeps typing in a pane: whatever a clicked header button, a toolbar or any
    /// other tree widget holds, the panes' keys go to the pane's program.
    #[test]
    fn the_panes_never_hand_keys_to_the_tree() {
        assert!(!FocusedSurface::Panes.delivers_to_tree());
    }

    #[test]
    fn a_dock_and_a_layer_hand_the_keys_they_do_not_claim_to_the_tree() {
        assert!(dock().delivers_to_tree());
        assert!(FocusedSurface::Layer { name: None }.delivers_to_tree());
    }

    /// **A terminal in a dock or a popup is the same terminal as a pane's**: the keys the app binds
    /// — paging its history, pasting — are the app's before the program in it hears them. Every
    /// other key, and every key while no terminal holds the keyboard, goes to the tree.
    #[test]
    fn the_app_keys_reach_the_app_before_a_terminal_holding_the_keyboard() {
        let keys = crate::app::registry::build_keymaps(
            &heca_config::theme::Config::default(),
            &mut crate::app::conflicts::Conflicts::default(),
        );
        for key in ["Shift+PageUp", "Shift+Up", "Super+v"] {
            let combo = KeyCombo::parse(key);
            assert!(the_app_answers_first(true, &keys.flat, &combo), "{key}");
            assert!(
                !the_app_answers_first(false, &keys.flat, &combo),
                "{key}: no terminal"
            );
        }
        for key in ["PageUp", "a", "Escape"] {
            assert!(
                !the_app_answers_first(true, &keys.flat, &KeyCombo::parse(key)),
                "{key}"
            );
        }
    }
}
