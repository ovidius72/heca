//! [`NfIcon`] — a single glyph from the embedded **Nerd Font**.
//!
//! The second icon set, and it exists because the first one cannot do this job: Phosphor
//! ([`Icon`](super::Icon)) is the app's pictogram library and has **no keyboard glyphs at all** —
//! no shift, control, option, command, escape or tab key. Drawing those as plain text in the UI face
//! is not a fallback either: the embedded UI font (Geist Mono) has `⇧ ↑ ↓ ← → ⏎ ␣ ⇥ ⌫ ⌦` but **not**
//! `⌃ ⌥ ⌘ ⎋ ⇞ ⇟`, so half a keyboard shortcut renders as tofu. Both facts were measured against the
//! embedded fonts' `cmap`, not assumed.
//!
//! So a keycap ([`paint_keycap_nf`](super::paint_keycap_nf)) and any future accelerator chip draw
//! from here. The font is the one already embedded for the terminal — a real Nerd Font, shipped with
//! the binary, so a glyph looks identical on every platform and survives the user configuring a
//! different terminal font (the family is addressed by name, and it is registered whatever the
//! config says).
//!
//! **Single-layer**, unlike the duotone [`Icon`](super::Icon): a Nerd Font glyph is one codepoint,
//! not a `:before`/`:after` pair.

use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::{Signal, SignalGet, signal};
use crate::style::Length;

/// A curated set of Nerd Font glyphs, by name — **the keyboard set**, which is what the app needs
/// from this font today.
///
/// Every codepoint below is **verified present** in the embedded font (`NfGlyph::ALL` is checked
/// against its `cmap` by a renderer test, so a glyph that would render as tofu fails the build
/// rather than the eye). What a test cannot check is that a *name* matches its picture: Nerd Font
/// glyph names are not in the font (its glyphs are named `uniF0636`), and the five
/// `nf-md-apple_keyboard_*` codepoints below are read off the Material Design block's alphabetical
/// order (caps, command, control, option, shift — five consecutive slots). **Confirm them visually
/// in the showcase glyph gallery** — the same caveat [`Glyph::CaretLeft`](super::Glyph::CaretLeft)
/// carries, and a one-character fix here if any is off by one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, heca_grid_ui_macros::PropName)]
pub enum NfGlyph {
    /// `nf-md-apple_keyboard_shift` — the ⇧ key.
    Shift,
    /// `nf-md-apple_keyboard_control` — the ⌃ key.
    Control,
    /// `nf-md-apple_keyboard_option` — the ⌥ / Alt key.
    Option,
    /// `nf-md-apple_keyboard_command` — the ⌘ key.
    Command,
    /// `nf-md-apple_keyboard_caps` — the caps-lock key.
    CapsLock,
    /// `nf-md-keyboard_return` — Enter / Return.
    Enter,
    /// `nf-md-keyboard_esc` — Escape.
    Escape,
    /// `nf-md-keyboard_tab` — Tab.
    Tab,
    /// `nf-md-keyboard_space` — the space bar.
    Space,
    /// `nf-md-backspace` — Backspace.
    Backspace,
    /// `nf-md-arrow_up` — the ↑ key.
    ArrowUp,
    /// `nf-md-arrow_down` — the ↓ key.
    ArrowDown,
    /// `nf-md-arrow_left` — the ← key.
    ArrowLeft,
    /// `nf-md-arrow_right` — the → key.
    ArrowRight,
}

impl NfGlyph {
    /// Every glyph, for a gallery (the showcase renders this to confirm the names) and for the
    /// coverage test that checks each one against the embedded font.
    pub const ALL: &'static [NfGlyph] = &[
        NfGlyph::Shift,
        NfGlyph::Control,
        NfGlyph::Option,
        NfGlyph::Command,
        NfGlyph::CapsLock,
        NfGlyph::Enter,
        NfGlyph::Escape,
        NfGlyph::Tab,
        NfGlyph::Space,
        NfGlyph::Backspace,
        NfGlyph::ArrowUp,
        NfGlyph::ArrowDown,
        NfGlyph::ArrowLeft,
        NfGlyph::ArrowRight,
    ];

    /// The glyph's codepoint in the embedded Nerd Font.
    pub fn codepoint(self) -> u32 {
        match self {
            // Material Design block, alphabetical run: caps, command, control, option, shift.
            NfGlyph::CapsLock => 0xF0632,
            NfGlyph::Command => 0xF0633,
            NfGlyph::Control => 0xF0634,
            NfGlyph::Option => 0xF0635,
            NfGlyph::Shift => 0xF0636,
            NfGlyph::Enter => 0xF0311,
            NfGlyph::Tab => 0xF0312,
            NfGlyph::Escape => 0xF12B7,
            NfGlyph::Space => 0xF12B8,
            NfGlyph::Backspace => 0xF006E,
            NfGlyph::ArrowUp => 0xF005D,
            NfGlyph::ArrowDown => 0xF0045,
            NfGlyph::ArrowLeft => 0xF004D,
            NfGlyph::ArrowRight => 0xF0054,
        }
    }

    /// The glyph as a `char`, for a caller drawing it directly through
    /// [`PaintCx::nf_icon`](crate::component::PaintCx::nf_icon).
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one")]
    pub fn char(self) -> Option<char> {
        char::from_u32(self.codepoint())
    }

    /// The glyph's name, for a gallery that has to say which one it is showing.
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one")]
    pub fn name(self) -> &'static str {
        match self {
            NfGlyph::Shift => "Shift",
            NfGlyph::Control => "Control",
            NfGlyph::Option => "Option",
            NfGlyph::Command => "Command",
            NfGlyph::CapsLock => "CapsLock",
            NfGlyph::Enter => "Enter",
            NfGlyph::Escape => "Escape",
            NfGlyph::Tab => "Tab",
            NfGlyph::Space => "Space",
            NfGlyph::Backspace => "Backspace",
            NfGlyph::ArrowUp => "ArrowUp",
            NfGlyph::ArrowDown => "ArrowDown",
            NfGlyph::ArrowLeft => "ArrowLeft",
            NfGlyph::ArrowRight => "ArrowRight",
        }
    }
}

/// A single Nerd Font glyph. Sizes to a square of the resolved font size (or an explicit
/// [`size`](NfIcon::size)); colors come from the theme unless overridden — the same contract as
/// [`Icon`](super::Icon), minus the duotone second layer.
pub struct NfIcon {
    base: Base,
    glyph: Signal<NfGlyph>,
    seen_glyph: NfGlyph,
    /// Explicit glyph size (px); otherwise the inherited font size.
    size: Option<f32>,
    color: Option<Color>,
}

#[heca_grid_ui_macros::props]
impl NfIcon {
    /// A new icon for a named [`NfGlyph`].
    pub fn new(glyph: NfGlyph) -> Self {
        let mut icon = Self {
            base: Base::new(),
            glyph: signal(glyph),
            seen_glyph: glyph,
            size: None,
            color: None,
        };
        icon.remeasure();
        icon
    }

    /// Handle to the glyph signal so hosts can update it live.
    pub fn glyph_signal(&self) -> Signal<NfGlyph> {
        self.glyph
    }

    /// Explicit glyph size in logical px (overrides the inherited font size).
    #[heca_grid_ui_macros::prop]
    pub fn size(mut self, px: f32) -> Self {
        self.size = Some(px);
        self.remeasure();
        self
    }

    /// Glyph color (default: the enclosing control's content color, else the theme foreground).
    #[heca_grid_ui_macros::prop]
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    fn glyph_size(&self) -> f32 {
        match self.size {
            Some(px) => px * self.base.style.layout.size.font_scale(),
            None => self.base.font,
        }
    }
}

impl Component for NfIcon {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Lay out as a square of the glyph size (font-driven unless explicit).
    fn remeasure(&mut self) {
        let s = self.glyph_size();
        self.base.style.layout.width = Length::Px(s);
        self.base.style.layout.height = Length::Px(s);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let size = self.glyph_size();
        let color = self
            .color
            .or_else(|| cx.content_color())
            .unwrap_or_else(|| cx.theme().colors.foreground);
        if let Some(c) = self.glyph.get_untracked().char() {
            cx.nf_icon(self.base.bounds, &c.to_string(), color, size);
        }
    }

    fn tick(&mut self, _dt: f32) -> bool {
        let next = self.glyph.get_untracked();
        if next != self.seen_glyph {
            self.seen_glyph = next;
            self.base.mark_needs_paint();
        }
        false
    }
}

/// **An icon places and sizes itself like any other widget.**
///
/// It had no builder trait at all, so it could only be positioned by whatever held it — which was
/// survivable while a grid placed its children, and stopped being so the moment placement became
/// the child's own property, as CSS has it.
impl LayoutExt for NfIcon {}

#[cfg(test)]
mod tests {
    use super::NfGlyph;

    /// `ALL` must list every variant exactly once, and no two glyphs may share a codepoint — the
    /// hand-kept list the compiler cannot check. (That the codepoints *exist in the font* is checked
    /// where the font lives: `heca-renderer`.)
    #[test]
    fn all_nf_glyphs_have_unique_codepoints() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for &g in NfGlyph::ALL {
            assert!(
                seen.insert(g.codepoint()),
                "duplicate codepoint {:#07x} for {g:?}",
                g.codepoint(),
            );
            assert!(g.char().is_some(), "{g:?} is not a valid codepoint");
        }
        assert_eq!(seen.len(), NfGlyph::ALL.len());
    }
}
