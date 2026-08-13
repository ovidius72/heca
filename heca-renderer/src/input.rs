//! **winit → heca events**: the one place a platform key becomes something a widget understands.
//!
//! `heca-grid-ui` is renderer-agnostic and never sees `winit`, so somebody has to bridge the two.
//! That somebody was every surface: the heca app mapped its normalised chord to a
//! [`GridKey`](heca_grid_ui::GridKey) in `app/registry.rs`, and the showcase mapped
//! `winit::NamedKey` to one in its own file — the same twelve keys written twice, in two shapes,
//! free to drift apart with nothing to notice.
//!
//! The list itself now belongs to the key (`GridKey::from_name`); this module is the small part
//! that is genuinely about winit: which *name* a `NamedKey` is, and how to build a whole
//! [`KeyPress`] out of a `KeyEvent`.

use heca_grid_ui::{GridKey, KeyPress, Modifiers};
use winit::keyboard::{Key, NamedKey};

/// The canonical name of a winit key, or `None` for one heca has no vocabulary for (a function
/// key, a modifier on its own, a dead key).
///
/// Deliberately a *name* rather than a `GridKey` directly: the mapping from a name to a key is the
/// list every surface used to keep, and it lives on [`GridKey::from_name`] so config text, a
/// platform event and an RPC string all resolve the same way.
pub fn key_name(key: &Key) -> Option<String> {
    Some(match key {
        Key::Named(NamedKey::Tab) => "Tab".into(),
        Key::Named(NamedKey::Enter) => "Enter".into(),
        Key::Named(NamedKey::Space) => "Space".into(),
        Key::Named(NamedKey::Escape) => "Escape".into(),
        Key::Named(NamedKey::Backspace) => "Backspace".into(),
        Key::Named(NamedKey::Delete) => "Delete".into(),
        Key::Named(NamedKey::ArrowLeft) => "ArrowLeft".into(),
        Key::Named(NamedKey::ArrowRight) => "ArrowRight".into(),
        Key::Named(NamedKey::ArrowUp) => "ArrowUp".into(),
        Key::Named(NamedKey::ArrowDown) => "ArrowDown".into(),
        Key::Named(NamedKey::Home) => "Home".into(),
        Key::Named(NamedKey::End) => "End".into(),
        Key::Character(s) => s.to_string(),
        _ => return None,
    })
}

/// The [`GridKey`] a winit key means.
pub fn grid_key(key: &Key) -> Option<GridKey> {
    GridKey::from_name(&key_name(key)?)
}

/// **A whole key press**, ready for [`Keymap::deliver_press`](heca_grid_ui::Keymap::deliver_press):
/// the key, the text the platform says it committed, and the modifiers held.
///
/// The text is passed through untouched — it is the platform's answer, and the funnel decides
/// whether it counts as typing.
pub fn key_press(event: &winit::event::KeyEvent, mods: Modifiers) -> Option<KeyPress> {
    Some(KeyPress {
        key: grid_key(&event.logical_key)?,
        text: event.text.as_ref().map(|t| t.to_string()),
        mods,
    })
}
