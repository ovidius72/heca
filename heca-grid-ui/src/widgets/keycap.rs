//! [`Keycap`] — **one key, drawn as a chip**: `Keycap::new("K")`.
//!
//! A shortcut is a row of them — `Flex::row().child([Keycap::new(NfGlyph::Command),
//! Keycap::new("K")])` — laid out by the engine like any other children, so nothing measures or
//! positions a chip by hand. The chip itself (its size for the text, the two looks, the glyph's
//! contrast with its fill) is [`KeyCap`]'s, the same drawing the picker's letters use, so a key in a
//! palette row and a letter over a pane are one look.
//!
//! ```
//! use heca_grid_ui::prelude::*;
//! use heca_grid_ui::widgets::{Flex, Keycap, KeycapVariant, NfGlyph};
//!
//! let shortcut = Flex::row()
//!     .gap(Spacing::Xs)
//!     .child([Keycap::new(NfGlyph::Command), Keycap::new("K")]);
//! let quiet = Keycap::new("Esc").variant(KeycapVariant::Bordered);
//! ```
//!
//! Described, as a plugin writes it: `{ "kind": "Keycap", "text": "K", "props": { "variant":
//! "bordered" } }`.

use crate::builders::LayoutExt;
use crate::component::{Base, Component, PaintCx};
use crate::style::Length;
use crate::widgets::{KeyCap, KeycapVariant, NfGlyph};

/// **One key, drawn as a chip** — see the module docs.
pub struct Keycap {
    base: Base,
    cap: KeyCap,
    variant: KeycapVariant,
    /// The chip's hue — `None` is the accent.
    tone: crate::reactive::Signal<Option<crate::Tone>>,
}

#[heca_grid_ui_macros::props]
impl Keycap {
    /// A key: its text (`"K"`, `"Esc"`, `"F5"`) or a Nerd Font key glyph (`NfGlyph::Command`).
    pub fn new(cap: impl Into<KeyCap>) -> Self {
        let mut base = Base::new();
        // A chip keeps its size: in a row of keys squeezed for room, the text beside it gives way.
        base.style.layout.flex_shrink = Some(0.0);
        Self {
            base,
            cap: cap.into(),
            variant: KeycapVariant::default(),
            tone: crate::reactive::signal(None),
        }
    }

    /// **The chip's look** — [`Filled`](KeycapVariant::Filled) (the default: a solid glowing chip,
    /// legible over anything) or [`Bordered`](KeycapVariant::Bordered) (a quiet outlined chip for a
    /// key shown on a panel of its own, a palette row or a menu).
    #[heca_grid_ui_macros::prop]
    pub fn variant(mut self, variant: KeycapVariant) -> Self {
        self.variant = variant;
        self
    }

    /// **The chip's hue, by meaning** — the accent when unset; `Tone::Muted` for a key that cannot
    /// be used right now (a disabled menu entry's letter). Fixed, or one that changes, followed in
    /// place:
    ///
    /// ```ignore
    /// Keycap::new("r").variant(KeycapVariant::Bordered).tone(move || (!enabled.get()).then_some(Tone::Muted))
    /// ```
    #[heca_grid_ui_macros::prop]
    pub fn tone(self, tone: impl crate::builders::IntoToneValue) -> Self {
        tone.into_tone_value().show_in(self.tone);
        self
    }

    /// The key this chip shows.
    pub fn cap(&self) -> &KeyCap {
        &self.cap
    }
}

impl From<&str> for KeyCap {
    fn from(text: &str) -> Self {
        KeyCap::Text(text.to_string())
    }
}

impl From<String> for KeyCap {
    fn from(text: String) -> Self {
        KeyCap::Text(text)
    }
}

impl From<NfGlyph> for KeyCap {
    fn from(glyph: NfGlyph) -> Self {
        KeyCap::Nf(glyph)
    }
}

impl Component for Keycap {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// The chip's size for its key at the font it inherits — so a size variant or a font change
    /// resizes it with nothing else written.
    fn remeasure(&mut self) {
        let size = self.cap.size(self.base.font);
        self.base.style.layout.width = Length::Px(size.w as f32);
        self.base.style.layout.height = Length::Px(size.h as f32);
    }

    fn paint(&self, cx: &mut PaintCx) {
        let color = crate::reactive::SignalGet::get_untracked(&self.tone)
            .map(|tone| tone.resolve(cx.theme()));
        self.cap
            .paint(cx, self.base.bounds, self.base.font, color, self.variant);
    }

    fn text_summary(&self) -> Option<String> {
        match &self.cap {
            KeyCap::Text(text) => Some(text.clone()),
            KeyCap::Nf(_) => None,
        }
    }
}

impl LayoutExt for Keycap {}
