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
//! The trigger shows the chosen option **itself**, icon and all. The option lives in the list — a
//! component is laid out in exactly one place — so the trigger draws an *image* of its content with
//! [`PaintCx::with_translate`](crate::component::PaintCx::with_translate), translated from the list
//! into the trigger. The row's chrome is not echoed: the trigger's own box is its chrome.
//!
//! # Where the rows live
//! The options are the children of a [`ScrollRegion`](super::ScrollRegion) — the control's `list`
//! part — inside a light [`Overlay`](super::Overlay) that stays in the control's own flow
//! ([`in_flow`](super::Overlay::in_flow)). That is where the control's width comes from: it is as
//! wide as its widest option. The overlay hangs the list from the trigger once layout is final
//! ([`anchored_to`](super::Overlay::anchored_to)) — below, flipped above when there is no room —
//! draws its frame on the overlay layer, and leaves the keyboard with the control. The list shows
//! [`visible_children`](super::ScrollRegion::visible_children) rows and scrolls the rest itself;
//! closed, the overlay keeps its box in the flow.
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
/// The trigger's resting halo radius.
// Per-widget glow, intended: this widget's own measure, still scaled by `glow_size`.
// See docs/widgets.md → "Each widget's own glow and border".
const GLOW_RADIUS: f32 = 16.0;
/// **The open list's halo, as multipliers of the theme's glow** — at the default theme the 16 px,
/// 0.1 halo it has always had (the theme's glow is 10 px at 0.55). Per-widget glow, intended.
const LIST_GLOW: crate::widgets::PanelGlow = crate::widgets::PanelGlow::new(1.6, 0.1 / 0.55);
/// Max option rows shown at once; longer lists scroll with a scrollbar.
const MAX_VISIBLE: usize = 6;
/// Scrollbar track width (logical px) — the lane the rows leave clear on their right.
const SCROLLBAR_W: f64 = 4.0;
/// The open list's name among the control's parts.
const LIST_AREA: &str = "list";
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
    /// are `crate::WidgetModel` afterwards, so their type — and this signal — is out of reach).
    /// Keeping the signals is what lets the control flip a row's selection **in place**, with no
    /// rebuild.
    option_states: Vec<Signal<bool>>,
    /// Selected index, exposed reactively via [`state`](Select::state).
    selected: Signal<usize>,
    /// Whether the dropdown is open — what the list's layer follows.
    open: Signal<bool>,
    /// Cursor-state signal of each option row, captured like `option_states`: the control moves one
    /// cursor over its options, and the option under it draws it.
    option_cursors: Vec<Signal<bool>>,
    /// The cursor row while open (keyboard/hover).
    highlight: usize,
    /// Whether the keyboard moved the cursor last — only then does the list scroll to it (what the
    /// pointer is on is already in view).
    by_keyboard: Signal<bool>,
    /// Rows shown at once while open — capped to what fits in the viewport. The list's
    /// [`visible_children`](crate::widgets::ScrollRegion::visible_children).
    vis_rows: Signal<usize>,
    /// **Where the trigger is on screen**, handed to the list's layer once layout is final, so
    /// the list hangs from it — below, flipped above when there is no room — wherever the control
    /// was scrolled to.
    trigger: Signal<heca_core::layout::Rectangle>,
    /// Each row's **measured** height, captured post-layout (`on_layout`) while the bounds are
    /// still at their natural, engine-computed values. The panel's geometry is derived from these,
    /// so rows of different heights (a two-line option) stack correctly instead of being forced
    /// onto an arithmetic grid.
    natural_h: Vec<f64>,
    on_change: Option<Box<dyn Fn(Action)>>,
}

impl Select {
    /// The open list — a [`ScrollRegion`](crate::widgets::ScrollRegion) holding the options as a
    /// column the engine lays out — found by name.
    fn list(&self) -> Option<&Base> {
        crate::component::area(self, LIST_AREA).map(|c| c.base())
    }

    fn list_mut(&mut self) -> Option<&mut crate::WidgetModel> {
        crate::component::area_slot(&mut self.base, LIST_AREA)
    }

    /// Whether the list is open.
    fn is_open(&self) -> bool {
        self.open.get_untracked()
    }

    /// The option rows, in order.
    fn rows(&self) -> &[crate::WidgetModel] {
        self.list().map_or(&[], |list| list.children.as_slice())
    }

    /// Number of options.
    fn count(&self) -> usize {
        self.rows().len()
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
        let set = |signal: &Signal<bool>, want: bool| {
            if signal.get_untracked() != want {
                signal.set(want);
            }
        };
        let open = self.is_open();
        for (i, state) in self.option_states.iter().enumerate() {
            set(state, open && i == selected);
        }
        for (i, cursor) in self.option_cursors.iter().enumerate() {
            set(cursor, open && i == self.highlight);
        }
    }
}

impl LayoutExt for Select {}
