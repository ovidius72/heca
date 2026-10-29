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
//!
//! The keycap drawing lives in [`keycap`], and where a letter lands over a target — plus the
//! universal [`paint_hint_label`] painter called for every widget — in [`placement`].

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::color::Color;
use crate::component::{paint_child, Base, Component, PaintCx};
use crate::reactive::{Signal, SignalGet};
use crate::style::{Direction, Length};

mod keycap;
mod placement;

pub use keycap::{keycap_size, keycap_size_nf, paint_keycap, paint_keycap_nf, KeyCap, KeycapVariant};
pub use placement::{HintPlacement, HintStyle};
pub(crate) use placement::paint_hint_label;

/// A transparent wrapper that carries a hint letter on behalf of a **region** — a group of widgets,
/// or something that is not a widget you can put a builder on.
///
/// It no longer *draws* the letter: [`paint_child`] does that for
/// every widget carrying one, so a widget can be pickable on its own and this is a decorator you
/// reach for when there is nothing to hang the declaration on.
pub struct KeyHint {
    base: Base,
}

#[heca_grid_ui_macros::props]
impl KeyHint {
    // `on_hint` is **not here any more**. It is
    // [`HintExt::on_hint`](crate::builders::HintExt::on_hint), on every widget — so a
    // `KeyHint::new(row).on_hint(…)` call still reads exactly the same, and a widget that can carry
    // the declaration itself no longer has to be wrapped to say what a pick does to it.
    //
    // Having it here put the two facts about a target on two different nodes: a row named itself
    // and the wrapper around it carried the pick, while a mounted dock named itself outside and
    // declared the pick within. Which one was on top depended on how the tree was built, so the
    // code matching a letter to an action had to search both up and down — and searching one way
    // only is why sidebar letters kept failing with no error.

    /// Wrap `child`. Bind the hint text with [`hint`](KeyHint::hint).
    pub fn new(child: impl crate::builders::IntoComponent) -> Self {
        Self::wrap(child.into_component())
    }

    fn wrap(child: crate::WidgetModel) -> Self {
        let mut base = Base::new();
        // Hug the child so the wrapper's bounds match it (overlay positions off them).
        base.style.layout.width = Length::Auto;
        base.style.layout.height = Length::Auto;
        // …and transparent to layout as well, or a child sized as a share resolves it against
        // this wrapper and quietly becomes its content size instead.
        crate::component::wrap_transparently(&mut base, child.as_ref());
        // Column direction so the single child stretches to the wrapper's full width
        // (cross-axis, default `Align::Stretch`). This keeps the wrapper transparent
        // to a stretching parent: a wide list row fills its column instead of
        // shrinking to content width, while a hugged square target is unaffected.
        base.style.layout.direction = Direction::Column;
        base.children.push(child);
        Self { base }
    }

    /// Bind the **host-owned** hint signal. The app sets `Some(letter)` when a
    /// pick/jump mode opens (from key **or** RPC) and clears it on exit.
    #[heca_grid_ui_macros::host_only("bound to a live host signal, which static data cannot drive")]
    pub fn hint(mut self, hint: Signal<Option<String>>) -> Self {
        self.base.hint_label = hint;
        self
    }

    /// The hint signal (e.g. to set/clear it directly).
    ///
    /// It **is** [`Base::hint_label`], not a second signal beside it: the universal picker offers a
    /// letter through the base slot and a host mode (pane-select, swap) sets the same one, so the
    /// two cannot show different letters over one target.
    pub fn hint_signal(&self) -> Signal<Option<String>> {
        self.base.hint_label
    }

    // ── The four cap knobs ────────────────────────────────────────────────────────────────
    //
    // These are the **wrapper's spelling** of capabilities that live on every widget
    // (`HintExt::hint_placement` and friends): the slot they write, `Base::hint_style`, has
    // always been universal, and so has the way to set it. They delegate
    // rather than repeat the assignment, so there is one writer per field and the two spellings
    // cannot drift — the same arrangement `Tooltip` has with `TooltipExt::tooltip`.
    //
    // Kept because the wrapper itself is kept: for a region that is not a widget you can put a
    // builder on.

    /// Where the keycap sits over the target (default [`HintPlacement::TopCenter`]).
    #[heca_grid_ui_macros::prop]
    pub fn placement(self, placement: HintPlacement) -> Self {
        crate::builders::HintExt::hint_placement(self, placement)
    }

    /// Explicit keycap font size in logical px (overrides the font-derived size).
    #[heca_grid_ui_macros::prop]
    pub fn size(self, px: f32) -> Self {
        crate::builders::HintExt::hint_size(self, px)
    }

    /// Override the keycap color (default: theme `accent`). The glow follows it too.
    #[heca_grid_ui_macros::prop]
    pub fn color(self, c: Color) -> Self {
        crate::builders::HintExt::hint_color(self, c)
    }

    /// Nudge the keycap down by `px` logical pixels after placement (positive = down).
    /// Use it to drop a `TopCenter` cap from a tall target's top edge onto its header
    /// row (e.g. align with a workspace dock's title).
    #[heca_grid_ui_macros::prop]
    pub fn offset_y(self, px: f64) -> Self {
        crate::builders::HintExt::hint_offset_y(self, px)
    }
}

impl Component for KeyHint {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Just the wrapped, still-interactive child. **The letter is not drawn here** — `paint_child`
    /// draws it for every widget that carries one, this wrapper included, so being pickable stopped
    /// depending on being wrapped.
    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }
    }
}

impl LayoutExt for KeyHint {}
impl StyleExt for KeyHint {}
impl Parent for KeyHint {}
