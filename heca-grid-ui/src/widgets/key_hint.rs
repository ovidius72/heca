//! [`KeyHint`] — a **generic keyboard-hint overlay** for any actionable component.
//!
//! Many chrome interactions are *keyboard-centric picks*: a prefix (move / swap /
//! focus-select, a jump mode, a command palette) lights up **letters over the
//! things you can act on**, and you press the letter to choose one. That overlay
//! is the same everywhere — a glowing keycap drawn on top of a target — so it
//! lives here as a reusable wrapper rather than being baked into any one widget
//! (rail icon cells, content-area panes, list rows, palette items all reuse it).
//!
//! `KeyHint` wraps a single child and is **transparent**: it isn't focusable and
//! it routes events/ticks straight through (the default [`Component`] container
//! behaviour), so the wrapped widget stays clickable/focusable exactly as before.
//! It only *adds paint*: while its host-owned [`Signal<Option<String>>`] is
//! `Some`, it stamps that text as a keycap over the child; when `None`, it paints
//! nothing extra. Because the signal is written by the **host**, mouse, keyboard,
//! and RPC all drive it identically (the chrome plan's read-via-signals /
//! write-via-actions rule) — matching `heca`'s existing `candidates` flow.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{paint_child, Base, Component, PaintCx};
use crate::reactive::{signal, Signal, SignalGet};
use crate::scene::{Glow, TextAlign};
use crate::style::{Direction, Length};
use heca_core::layout::{Point, Rectangle, Size};

/// Where the keycap sits over the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HintPlacement {
    /// Centered along the top edge — for compact square targets (rail icon cells).
    #[default]
    TopCenter,
    /// Centered over the whole target — for large targets (content-area panes).
    Center,
    /// Vertically centered, pinned to the **right edge** — for wide list rows
    /// (sidebar pane/column cards) where a right-aligned keycap keeps the row's
    /// label readable.
    CenterRight,
}

/// Keycap font size as a fraction of the wrapped component's resolved font.
const HINT_FONT_MUL: f32 = 1.05;
/// Horizontal / vertical padding inside the keycap, in fractions of the hint font.
const PAD_X_FRAC: f32 = 0.42;
const PAD_Y_FRAC: f32 = 0.22;
/// Inset of a `TopCenter` keycap from the target's top edge (logical px).
const TOP_INSET: f64 = 2.0;
/// Inset of a `CenterRight` keycap from the target's right edge (logical px).
const RIGHT_INSET: f64 = 6.0;
/// Per-glyph advance estimate (fraction of font) for sizing the keycap to its text.
const GLYPH_ADVANCE_FRAC: f32 = 0.62;
/// Keycap fill alpha — slightly translucent so it reads as an overlay, not a
/// solid bright block over the target.
const KEYCAP_ALPHA: u8 = 200;
/// Keycap glow intensity (scaled by the theme `glow_size`) — soft, not blazing.
const KEYCAP_GLOW: f32 = 0.45;

/// A transparent wrapper that overlays a glowing key letter on its child while a
/// host-driven pick/jump hint is active.
pub struct KeyHint {
    base: Base,
    /// Host-owned hint text: `Some(s)` overlays the keycap; `None` hides it.
    hint: Signal<Option<String>>,
    placement: HintPlacement,
    /// Explicit keycap font size (px); otherwise derived from the resolved font.
    size: Option<f32>,
}

impl KeyHint {
    /// Wrap `child`. Bind the hint text with [`hint`](KeyHint::hint).
    pub fn new(child: impl Component + 'static) -> Self {
        let mut base = Base::new();
        // Hug the child so the wrapper's bounds match it (overlay positions off them).
        base.style.width = Length::Auto;
        base.style.height = Length::Auto;
        // Column direction so the single child stretches to the wrapper's full width
        // (cross-axis, default `Align::Stretch`). This keeps the wrapper transparent
        // to a stretching parent: a wide list row fills its column instead of
        // shrinking to content width, while a hugged square target is unaffected.
        base.style.direction = Direction::Column;
        base.children.push(Box::new(child));
        Self { base, hint: signal(None), placement: HintPlacement::default(), size: None }
    }

    /// Bind the **host-owned** hint signal. The app sets `Some(letter)` when a
    /// pick/jump mode opens (from key **or** RPC) and clears it on exit.
    pub fn hint(mut self, hint: Signal<Option<String>>) -> Self {
        self.hint = hint;
        self
    }

    /// The hint signal (e.g. to set/clear it directly).
    pub fn hint_signal(&self) -> Signal<Option<String>> {
        self.hint
    }

    /// Where the keycap sits over the target (default [`HintPlacement::TopCenter`]).
    pub fn placement(mut self, placement: HintPlacement) -> Self {
        self.placement = placement;
        self
    }

    /// Explicit keycap font size in logical px (overrides the font-derived size).
    pub fn size(mut self, px: f32) -> Self {
        self.size = Some(px);
        self
    }

    fn hint_font(&self) -> f32 {
        self.size.unwrap_or(self.base.font * HINT_FONT_MUL)
    }

    /// The keycap rect for `text` within the target `b`, per placement.
    fn keycap_rect(&self, b: Rectangle, text: &str) -> Rectangle {
        let font = self.hint_font() as f64;
        let pad_x = (self.hint_font() * PAD_X_FRAC) as f64;
        let pad_y = (self.hint_font() * PAD_Y_FRAC) as f64;
        let glyphs = text.chars().count().max(1) as f64;
        let w = (glyphs * (self.hint_font() * GLYPH_ADVANCE_FRAC) as f64 + 2.0 * pad_x)
            .max(font + 2.0 * pad_y);
        let h = font + 2.0 * pad_y;
        let (x, y) = match self.placement {
            HintPlacement::TopCenter => {
                (b.loc.x + (b.size.w - w) / 2.0, b.loc.y + TOP_INSET)
            }
            HintPlacement::Center => {
                (b.loc.x + (b.size.w - w) / 2.0, b.loc.y + (b.size.h - h) / 2.0)
            }
            HintPlacement::CenterRight => {
                (b.loc.x + b.size.w - w - RIGHT_INSET, b.loc.y + (b.size.h - h) / 2.0)
            }
        };
        Rectangle::new(Point::new(x, y), Size::new(w, h))
    }
}

impl Component for KeyHint {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        // The wrapped, still-interactive child first.
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }

        // Keycap overlay — only while the host has set a hint.
        let Some(text) = self.hint.get_untracked() else { return };
        if text.is_empty() {
            return;
        }
        let (accent, glow_c, background, ctrl_radius) = {
            let t = cx.theme();
            (t.accent, t.glow, t.background, t.control_radius())
        };
        let cap = self.keycap_rect(self.base.bounds, &text);
        let radius = ctrl_radius.min((cap.size.h / 2.0) as f32);
        // Softly-glowing, slightly translucent accent keycap; dark bold glyph on
        // top for contrast on dark.
        cx.rect(
            cap,
            accent.with_alpha(KEYCAP_ALPHA),
            None,
            radius,
            Some(Glow { color: glow_c, radius: 6.0, intensity: KEYCAP_GLOW }),
        );
        cx.text(cap, &text, background, self.hint_font(), TextAlign::Center, true);
    }
}

impl LayoutExt for KeyHint {}
impl StyleExt for KeyHint {}
impl Parent for KeyHint {}
