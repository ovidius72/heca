//! [`Select`] — a single-select dropdown whose options are [`Choice`](crate::widgets::Choice) children.
//!
//! The first consumer of the overlay layer: while open, its option list paints in the scene's
//! **overlay layer** (on top of everything) and it reports
//! [`overlay_active`](crate::component::Component::overlay_active) so the host routes input to it first — letting it
//! capture clicks on rows that fall outside its own layout bounds.
//!
//! # Composed, not hand-drawn
//! The options are **child components** ([`Choice`](crate::widgets::Choice)), so an option can be anything — a word, an
//! icon + a label, a two-line row with a [`Badge`](super::Badge). `Select` owns only the *chrome*:
//! the trigger box, the chevron, the panel, the keyboard cursor, the scrollbar. Each row draws
//! itself.
//!
//! The trigger shows the chosen option **itself**, icon and all. Closed, that option *is* the
//! trigger's content (it is placed there). Open, it has moved into the list — a component is laid
//! out in exactly one place — so the trigger draws a second *image* of its content with
//! [`PaintCx::with_translate`](crate::component::PaintCx::with_translate), translated from the panel back into the trigger. The row's chrome is
//! not echoed: the trigger's own box is its chrome.
//!
//! # Where the rows live (the one subtle thing in this widget)
//! The dropdown panel is an **overlay**: it is drawn outside the control's layout box, and it may
//! flip above the trigger. The layout engine cannot place it there — a taffy node sits where its
//! parent's flow puts it. So the rows are laid out in the trigger's flow (where taffy *measures*
//! them: each `Choice` hugs its content) and `Select` then **places** them, baking the offset from
//! that flow into their bounds — the same trick [`ScrollRegion`](super::ScrollRegion) uses to bake
//! `-scroll_offset` into its children.
//!
//! The invariant that falls out of it, and the reason it is done this way:
//!
//! > **bounds === what is drawn === what is clickable.**
//!
//! Hit-testing goes through [`choice_at`](crate::widgets::choice::choice_at), which reads the rows' real bounds — never row
//! arithmetic — so what the user clicks and what they see can never disagree, whatever the rows
//! contain and however tall they are.
//!
//! Selecting emits `Action::value("select-change", SignalData::Usize(index))`. Honors
//! [`Base::disabled`](crate::component::Base).
//!
//! **Open-list navigation is host-configured, not hardcoded** (`widget-keys-config`),
//! shared with [`ContextMenu`](super::ContextMenu)/[`CommandPalette`](super::CommandPalette):
//! while open the widget is an overlay and responds to the semantic [`Event::Widget`](crate::component::Event::Widget) intents
//! `MenuUp`/`MenuDown` (move the cursor), `Activate` (commit), `Dismiss` (close) — the host
//! resolves the configurable `[keys.widgets]` keys (defaults ↑/`Ctrl+k`, ↓/`Ctrl+j`, Enter,
//! Esc) into it. Only the **closed** trigger keeps raw activation keys (Enter / Space / ↓ open
//! the list), delivered to the focused widget like a button.

use crate::action::Action;
use crate::builders::LayoutExt;
use crate::component::Base;
use crate::reactive::{Signal, SignalGet, SignalUpdate};
use heca_core::layout::Size;
use std::cell::Cell;

mod builders;
mod component;

/// Inner horizontal padding of the trigger — the inset of its label. The option rows are margined
/// so their content lands on this same inset, so the label doesn't jump when the list opens.
const PAD_H: f64 = 12.0;
/// Inner vertical padding (trigger).
const PAD_V: f64 = 8.0;
/// Fallback option-row height as a multiple of the resolved font, used only before the first
/// layout pass has measured the rows (they are `Choice` children and hug their own content).
const ROW_H_RATIO: f64 = 2.0;
/// Padding around the option list inside the panel.
const PANEL_PAD: f64 = 4.0;
/// Gap between the trigger and the panel.
const PANEL_GAP: f64 = 4.0;
/// Panel glow.
const GLOW_RADIUS: f32 = 16.0;
const GLOW_INTENSITY: f32 = 0.1;
/// Max option rows shown at once; longer lists scroll with a scrollbar.
const MAX_VISIBLE: usize = 6;
/// Scrollbar track width (logical px).
const SCROLLBAR_W: f64 = 4.0;
/// Horizontal room reserved for the down-chevron (+ gap).
const CHEVRON_W: f64 = 22.0;
/// Trigger width when there are no options to hug (a select is sized by its widest option, and an
/// empty one would otherwise measure to nothing).
const EMPTY_W: f32 = 120.0;

/// A single-select dropdown over [`Choice`](crate::widgets::Choice) options.
///
/// ```ignore
/// // Sugar — plain text options (each becomes a `Choice::labeled(text, text)`).
/// Select::new(["LOW", "MEDIUM", "HIGH"]).selected(1).on_change(|a| { /* … */ })
///
/// // Composed — an option is a value plus any content.
/// Select::empty()
///     .option(Choice::new("low").child(Flex::row()
///         .child(Icon::new(Glyph::Circle))
///         .child(Label::new("LOW"))))
///     .option(Choice::new("high").child(Flex::row()
///         .child(Icon::new(Glyph::Warning))
///         .child(Label::new("HIGH"))))
/// ```
pub struct Select {
    base: Base,
    /// Selected-state signal of each option row, captured when the [`Choice`](crate::widgets::Choice) is added (the rows
    /// are `Box<dyn Component>` afterwards, so their type — and this signal — is out of reach).
    /// Keeping the signals is what lets the control flip a row's selection **in place**, with no
    /// rebuild.
    option_states: Vec<Signal<bool>>,
    /// Selected index, exposed reactively via [`state`](Select::state).
    selected: Signal<usize>,
    /// Whether the dropdown is open.
    open: bool,
    /// The cursor row while open (keyboard/hover). This is the *container's* state, not the
    /// option's: it is drawn as chrome over the row's bounds, so an option never has to know it is
    /// being pointed at.
    highlight: usize,
    /// Index of the first visible row when the list scrolls.
    scroll: usize,
    /// Rows shown at once while open — capped to what fits in the viewport.
    vis_rows: usize,
    /// Whether the open list flips *above* the trigger (no room below).
    open_up: bool,
    /// Each row's **measured** height, captured post-layout (`on_layout`) while the bounds are
    /// still at their natural, engine-computed values. The panel's geometry is derived from these,
    /// so rows of different heights (a two-line option) stack correctly instead of being forced
    /// onto an arithmetic grid.
    natural_h: Vec<f64>,
    /// Last-seen viewport (set during paint), used to flip/cap the list and to
    /// clamp the panel on-screen via the shared placement authority.
    viewport: Cell<Size>,
    on_change: Option<Box<dyn Fn(Action)>>,
}

impl Select {
    /// Number of options.
    fn count(&self) -> usize {
        self.base.children.len()
    }

    /// Push the selection down into the options: while the list is **open**, exactly one row carries
    /// the selected state, and it tints its own content (accent) through the [`Choice`](crate::widgets::Choice).
    ///
    /// While it is **closed** no option is "selected" in that sense: the chosen one is standing in
    /// the trigger, where the trigger's own box is the chrome — a selection pill drawn *inside* it
    /// would be a box in a box. So it renders in the plain content color, exactly as the trigger's
    /// label always has.
    ///
    /// Cheap and idempotent — it writes only on a real change, so every path that could have moved
    /// the selection (or opened/closed the list) can just call it.
    fn sync_option_states(&self) {
        let selected = self.selected.get_untracked();
        for (i, state) in self.option_states.iter().enumerate() {
            let want = self.open && i == selected;
            if state.get_untracked() != want {
                state.set(want);
            }
        }
    }
}

impl LayoutExt for Select {}
