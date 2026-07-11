//! [`Keymap`] — the host-owned, config-populated map from a key chord to the semantic
//! [`WidgetIntent`]s it triggers.
//!
//! This is the single place key→intent resolution lives. A **host** (the heca app, the
//! showcase, a future detached-pane window) builds **one** `Keymap` — the app from the
//! `[keys.widgets]` config table, a config-less host from [`Keymap::with_defaults`] — stores
//! it, and rebuilds it on reload. On each key it resolves the chord to the intent(s) and
//! delivers `Event::Widget(intent)` to the focused/overlay widget (see [`Keymap::dispatch`]).
//!
//! The keymap is **owned by the host, not a global**: `heca-grid-ui` is single-threaded
//! (`Rc`/`floem_reactive`), so each window/host owns its own `Keymap` — no shared static that
//! a second window or a detached pane would race or leave uninitialised.
//!
//! A chord may map to **several** intents in a defined order (e.g. `Ctrl+h` →
//! `EditDeleteBack` then `ItemPrevious`); [`dispatch`](Keymap::dispatch) delivers them in
//! order and stops at the first the focused widget consumes, so the overload disambiguates by
//! focus. Text-edit intents are bound first so a focused `Input` wins (field-first).

use crate::component::{Event, GridKey, Handled, Modifiers, WidgetIntent};
use std::collections::HashMap;

/// A key + modifier chord (renderer-agnostic). Letter keys are stored lowercase so
/// `Ctrl+h` and `Ctrl+H` resolve the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    /// The (normalised) key.
    pub key: GridKey,
    /// Held modifiers.
    pub mods: Modifiers,
}

impl KeyChord {
    /// A chord from a key + modifiers (letters normalised to lowercase).
    pub fn new(key: GridKey, mods: Modifiers) -> Self {
        let key = match key {
            GridKey::Char(c) => GridKey::Char(c.to_ascii_lowercase()),
            other => other,
        };
        Self { key, mods }
    }
}

/// Host-owned map: a key chord → the [`WidgetIntent`]s it triggers, in delivery order.
#[derive(Clone, Debug, Default)]
pub struct Keymap {
    map: HashMap<KeyChord, Vec<WidgetIntent>>,
}

impl Keymap {
    /// An empty keymap — bind into it from config, or start from [`with_defaults`](Keymap::with_defaults).
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind `intent` to a chord. Appended, so a chord can trigger several intents; the
    /// **first bound is tried first** (bind edit intents before nav so a focused field wins).
    pub fn bind(&mut self, key: GridKey, mods: Modifiers, intent: WidgetIntent) {
        self.map
            .entry(KeyChord::new(key, mods))
            .or_default()
            .push(intent);
    }

    /// The intents a key + modifiers resolves to (in delivery order); empty if unbound.
    pub fn resolve(&self, key: GridKey, mods: Modifiers) -> &[WidgetIntent] {
        self.map
            .get(&KeyChord::new(key, mods))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Deliver a key to the focused widget **field-first**, then as semantic intents:
    ///
    /// 1. the raw `Event::Key` — so a focused widget's own keys (an `Input`'s typing / caret
    ///    motion / Backspace, a menu's quick-pick letter) are never stolen by a semantic intent
    ///    bound to the same chord (e.g. `←` moves an Input's caret rather than navigating);
    /// 2. then each `Event::Widget(intent)` the chord resolves to, in order (edit intents before
    ///    the nav overload), stopping at the first the widget consumes.
    ///
    /// Returns whether anything consumed the key. `deliver` is the host's target — a focused
    /// component, or an overlay's root (which forwards field-first to its own focused child).
    pub fn dispatch(
        &self,
        key: GridKey,
        mods: Modifiers,
        mut deliver: impl FnMut(&Event) -> Handled,
    ) -> Handled {
        if deliver(&Event::Key { key, pressed: true }) == Handled::Yes {
            return Handled::Yes;
        }
        for intent in self.resolve(key, mods) {
            if deliver(&Event::Widget(*intent)) == Handled::Yes {
                return Handled::Yes;
            }
        }
        Handled::No
    }

    /// The built-in, vim-friendly default bindings — for a host with no config file (the
    /// showcase). The heca app builds its own from `[keys.widgets]` instead.
    ///
    /// Horizontal `Item*` = ←/→, `Ctrl+h`/`Ctrl+l`, Tab/Shift+Tab. Vertical `Menu*` = ↑/↓,
    /// `Ctrl+k`/`Ctrl+j`. Shared `Activate` = Enter, `Dismiss` = Esc. Edits = `Ctrl+h`
    /// (delete back, bound before its `ItemPrevious` overload), `Ctrl+u`, `Ctrl+a`/`Cmd+a`.
    pub fn with_defaults() -> Self {
        use GridKey::*;
        use WidgetIntent::*;
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        let meta = Modifiers { meta: true, ..Default::default() };
        let none = Modifiers::default();

        let mut km = Keymap::new();
        // Text edits first — so a focused Input consumes `Ctrl+h` as delete before its
        // `ItemPrevious` overload is offered.
        km.bind(Char('h'), ctrl, EditDeleteBack);
        km.bind(Char('u'), ctrl, EditDeleteToLineStart);
        km.bind(Char('a'), ctrl, EditSelectAll);
        km.bind(Char('a'), meta, EditSelectAll);
        // Horizontal navigation (left/right). Tab / Shift+Tab are NOT here — they are the
        // universal, always-on focus-traversal primitive handled by the `FocusManager` (and
        // trapped inside a modal `Dialog`), not a rebindable `[keys.widgets]` binding.
        km.bind(ArrowLeft, none, ItemPrevious);
        km.bind(ArrowRight, none, ItemNext);
        km.bind(Char('h'), ctrl, ItemPrevious);
        km.bind(Char('l'), ctrl, ItemNext);
        // Vertical navigation (up/down).
        km.bind(ArrowUp, none, MenuUp);
        km.bind(ArrowDown, none, MenuDown);
        km.bind(Char('k'), ctrl, MenuUp);
        km.bind(Char('j'), ctrl, MenuDown);
        // Shared.
        km.bind(Enter, none, Activate);
        km.bind(Escape, none, Dismiss);
        km
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bindings_resolve_by_axis() {
        let km = Keymap::with_defaults();
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        let none = Modifiers::default();
        // Horizontal = item_*, vertical = menu_*.
        assert_eq!(km.resolve(GridKey::ArrowRight, none), &[WidgetIntent::ItemNext]);
        assert_eq!(km.resolve(GridKey::Char('l'), ctrl), &[WidgetIntent::ItemNext]);
        assert_eq!(km.resolve(GridKey::ArrowDown, none), &[WidgetIntent::MenuDown]);
        assert_eq!(km.resolve(GridKey::Char('j'), ctrl), &[WidgetIntent::MenuDown]);
        assert_eq!(km.resolve(GridKey::Char('k'), ctrl), &[WidgetIntent::MenuUp]);
    }

    #[test]
    fn ctrl_h_offers_edit_before_nav() {
        // The overload: Ctrl+h is delete-back first (a focused Input wins), then ItemPrevious.
        let km = Keymap::with_defaults();
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        assert_eq!(
            km.resolve(GridKey::Char('h'), ctrl),
            &[WidgetIntent::EditDeleteBack, WidgetIntent::ItemPrevious],
        );
    }

    #[test]
    fn letter_case_is_normalised() {
        let mut km = Keymap::new();
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        km.bind(GridKey::Char('H'), ctrl, WidgetIntent::ItemPrevious);
        assert_eq!(km.resolve(GridKey::Char('h'), ctrl), &[WidgetIntent::ItemPrevious]);
    }

    #[test]
    fn dispatch_is_field_first_then_intents() {
        use std::cell::RefCell;
        let km = Keymap::with_defaults();
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        // A widget that only consumes ItemPrevious (like Tabs): the raw key is offered first
        // (ignored), then the edit intent (ignored), then the nav intent (consumed).
        let seen = RefCell::new(Vec::new());
        let handled = km.dispatch(GridKey::Char('h'), ctrl, |ev| {
            seen.borrow_mut().push(*ev);
            if *ev == Event::Widget(WidgetIntent::ItemPrevious) {
                Handled::Yes
            } else {
                Handled::No
            }
        });
        assert_eq!(handled, Handled::Yes);
        assert_eq!(
            *seen.borrow(),
            vec![
                Event::Key { key: GridKey::Char('h'), pressed: true },
                Event::Widget(WidgetIntent::EditDeleteBack),
                Event::Widget(WidgetIntent::ItemPrevious),
            ],
            "raw key first (field-first), then edit intent, then nav consumed",
        );
    }

    #[test]
    fn dispatch_field_first_lets_the_widget_keep_its_raw_key() {
        // A focused Input consumes the raw `←` for caret motion, so the `ItemPrevious` bound to
        // it is never offered — navigation does not steal the field's own key.
        let km = Keymap::with_defaults();
        let none = Modifiers::default();
        let mut offered_intent = false;
        let handled = km.dispatch(GridKey::ArrowLeft, none, |ev| match ev {
            Event::Key { .. } => Handled::Yes, // the field consumes the raw key
            Event::Widget(_) => {
                offered_intent = true;
                Handled::No
            }
            _ => Handled::No,
        });
        assert_eq!(handled, Handled::Yes);
        assert!(!offered_intent, "the raw key was consumed, so no intent was offered");
    }
}
