//! [`Select`] — a single-select dropdown whose options are [`Choice`] children.
//!
//! The first consumer of the overlay layer: while open, its option list paints in the scene's
//! **overlay layer** (on top of everything) and it reports
//! [`overlay_active`](Component::overlay_active) so the host routes input to it first — letting it
//! capture clicks on rows that fall outside its own layout bounds.
//!
//! # Composed, not hand-drawn
//! The options are **child components** ([`Choice`]), so an option can be anything — a word, an
//! icon + a label, a two-line row with a [`Badge`](super::Badge). `Select` owns only the *chrome*:
//! the trigger box, the chevron, the panel, the keyboard cursor, the scrollbar. Each row draws
//! itself.
//!
//! The trigger shows the chosen option **itself**, icon and all. Closed, that option *is* the
//! trigger's content (it is placed there). Open, it has moved into the list — a component is laid
//! out in exactly one place — so the trigger draws a second *image* of its content with
//! [`PaintCx::with_translate`], translated from the panel back into the trigger. The row's chrome is
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
//! Hit-testing goes through [`choice_at`], which reads the rows' real bounds — never row
//! arithmetic — so what the user clicks and what they see can never disagree, whatever the rows
//! contain and however tall they are.
//!
//! Selecting emits `Action::value("select-change", SignalData::Usize(index))`. Honors
//! [`Base::disabled`](crate::component::Base).
//!
//! **Open-list navigation is host-configured, not hardcoded** (`widget-keys-config`),
//! shared with [`ContextMenu`](super::ContextMenu)/[`CommandPalette`](super::CommandPalette):
//! while open the widget is an overlay and responds to the semantic [`Event::Widget`] intents
//! `MenuUp`/`MenuDown` (move the cursor), `Activate` (commit), `Dismiss` (close) — the host
//! resolves the configurable `[keys.widgets]` keys (defaults ↑/`Ctrl+k`, ↓/`Ctrl+j`, Enter,
//! Esc) into it. Only the **closed** trigger keeps raw activation keys (Enter / Space / ↓ open
//! the list), delivered to the focused widget like a button.

use crate::action::{Action, SignalData};
use crate::builders::LayoutExt;
use crate::component::{
    shift_subtree, Base, Component, Event, GridKey, Handled, PaintCx, WidgetIntent,
};
use crate::font::MONO_LINE_RATIO;
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Border, Glow};
use crate::style::{Align, Direction, Length};
use crate::widgets::choice::{self, choice_at, Choice};
use crate::widgets::overlay::{
    paint_panel_chrome, place_anchored_on, AnchorSide, PanelChrome, PanelElevation,
};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::Cell;

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

/// A single-select dropdown over [`Choice`] options.
///
/// ```ignore
/// // Sugar — plain text options (each becomes a `Choice::labeled(text, text)`).
/// Select::new(["LOW", "MEDIUM", "HIGH"]).selected(1).on_change(|a| …)
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
    /// Selected-state signal of each option row, captured when the [`Choice`] is added (the rows
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

#[heca_grid_ui_macros::props]
impl Select {
    /// An empty select — add options with [`option`](Select::option).
    pub fn empty() -> Self {
        let mut base = Base::new();
        base.focusable = true; // keyboard-focusable when enabled (Component::focusable)
        // One control = one Tab stop: focus never descends into the options.
        base.focus_barrier = true;
        // The options stack vertically. This is the flow taffy *measures* them in; `place_options`
        // then moves them into the overlay panel.
        base.style.layout.direction = Direction::Column;
        // Hug the widest option instead of stretching to fill a column parent (the width is
        // `Auto`, and the default cross-axis alignment is `Stretch`).
        base.style.layout.align_self = Some(Align::Start);
        let mut select = Self {
            base,
            option_states: Vec::new(),
            selected: signal(0),
            open: false,
            highlight: 0,
            scroll: 0,
            vis_rows: MAX_VISIBLE,
            open_up: false,
            natural_h: Vec::new(),
            viewport: Cell::new(Size::new(f64::INFINITY, f64::MAX)),
            on_change: None,
        };
        select.remeasure();
        select
    }

    /// A new select over plain-text `options`; the first is selected.
    ///
    /// Sugar: each string becomes a [`Choice::labeled`] child whose **value is the text itself**.
    /// It builds exactly the tree [`option`](Select::option) would — there is one option model, not
    /// a "simple mode".
    pub fn new(options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut select = Self::empty();
        for option in options {
            let text: String = option.into();
            select = select.option(Choice::labeled(text.clone(), text));
        }
        select
    }

    /// Append a composed option.
    ///
    /// Typed to [`Choice`] on purpose: the control keeps the row's selected-state
    /// [`Signal`](Choice::state) so it can drive the selection in place, and a `Box<dyn Component>`
    /// would have thrown that away. It is also the contract — the rows of a select **are** options.
    #[heca_grid_ui_macros::host_only("a composed value, not a scalar — built from `children`")]
    pub fn option(mut self, choice: Choice) -> Self {
        self.option_states.push(choice.state());
        self.base.children.push(Box::new(choice));
        self.sync_option_states();
        self
    }

    /// Explicit font size — overrides the inherited theme font.
    #[heca_grid_ui_macros::prop]
    pub fn font_size(mut self, fs: f32) -> Self {
        self.base.style.visual.font_size = fs;
        self.base.font = fs;
        self.remeasure();
        self
    }

    /// Select an initial option (clamped to the option count). Call it **after** the options.
    #[heca_grid_ui_macros::prop]
    pub fn selected(self, index: usize) -> Self {
        let i = index.min(self.count().saturating_sub(1));
        self.selected.set(i);
        self.sync_option_states();
        self
    }

    /// Set the change handler. Receives `Action::value("select-change",
    /// SignalData::Usize(index))` when the selection changes.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_change(mut self, f: impl Fn(Action) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// The selected-index signal — bind UI to it reactively.
    pub fn state(&self) -> Signal<usize> {
        self.selected
    }

    /// The currently selected index (untracked read).
    pub fn index(&self) -> usize {
        self.selected.get_untracked()
    }

    /// The selected option **as text** — its [accessible name](Component::text_summary), computed
    /// from the option's content: for `Choice::labeled` it is the label; for an option composed of
    /// an `Icon` + `Label("HIGH")` it is `"HIGH"`. Empty when the option has no text at all (an
    /// icon-only option). Use it to report the current value; the trigger itself renders the option,
    /// not this.
    pub fn selected_label(&self) -> String {
        self.base
            .children
            .get(self.selected.get_untracked())
            .and_then(|c| c.text_summary())
            .unwrap_or_default()
    }

    /// Number of options.
    fn count(&self) -> usize {
        self.base.children.len()
    }

    /// Push the selection down into the options: while the list is **open**, exactly one row carries
    /// the selected state, and it tints its own content (accent) through the [`Choice`].
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

    /// The measured height of row `i` (fallback: font-derived, before the first layout).
    fn row_h(&self, i: usize) -> f64 {
        self.natural_h
            .get(i)
            .copied()
            .filter(|h| *h > 0.0)
            .unwrap_or(self.base.font as f64 * ROW_H_RATIO)
    }

    /// The tallest row — what the fit/flip decision reserves per row, so a mixed-height list is
    /// capped conservatively and no row is ever clipped by the viewport edge.
    fn max_row_h(&self) -> f64 {
        (0..self.count())
            .map(|i| self.row_h(i))
            .fold(0.0_f64, f64::max)
            .max(self.base.font as f64 * ROW_H_RATIO)
    }

    /// The visible window of options: `[first, last)`.
    fn window(&self) -> (usize, usize) {
        let lo = self.scroll.min(self.count());
        (lo, (lo + self.vis_rows).min(self.count()))
    }

    /// Padding / chevron width scaled by the size variant (the font already is), so the control and
    /// its panel grow/shrink as a unit.
    fn pad_h(&self) -> f64 {
        PAD_H * self.base.size_scale() as f64
    }
    fn pad_v(&self) -> f64 {
        PAD_V * self.base.size_scale() as f64
    }
    /// Room reserved on the right of the rows: the trigger's chevron, and the panel's scrollbar
    /// lane. Applied as a margin on the rows, so it widens the control (taffy sizes it to the
    /// widest option *plus* this) and keeps the option content clear of both.
    fn gutter(&self) -> f64 {
        CHEVRON_W * self.base.size_scale() as f64 + SCROLLBAR_W
    }

    /// Panel height for the current visible window: the rows it actually holds.
    fn panel_h(&self) -> f64 {
        let (lo, hi) = self.window();
        2.0 * PANEL_PAD + (lo..hi).map(|i| self.row_h(i)).sum::<f64>()
    }

    /// The bounding rect of the open option list (panel).
    ///
    /// Placement goes through the **shared overlay placement authority**
    /// ([`place_anchored_on`]): the panel hangs off the trigger rect, on the side
    /// [`open_list`](Select::open_list) already chose. The side is passed
    /// explicitly rather than re-decided, because the flip and the visible-row
    /// count are computed **together** in `open_list` (the panel's height depends
    /// on the side) — letting the placement re-decide could disagree with the row
    /// count. Width tracks the trigger, so the control and its list stay one unit.
    ///
    /// **This must stay a pure function of `bounds` + side + row heights.** It is
    /// the *same* rect [`place_options`](Select::place_options) lays the rows into
    /// and [`paint`](Component::paint) draws the panel at, but those run at
    /// different moments (layout vs paint). Anything time-varying here — notably
    /// clamping against a viewport cached during paint — makes the two calls
    /// disagree, and the rows visibly detach from their panel. The list is kept
    /// on-screen by capping the visible row *count* in `open_list`, not by moving
    /// the panel, so an infinite viewport is passed deliberately.
    fn panel_rect(&self) -> Rectangle {
        let b = self.base.bounds;
        let side = if self.open_up {
            AnchorSide::Above
        } else {
            AnchorSide::Below
        };
        place_anchored_on(
            b,
            Size::new(b.size.w, self.panel_h()),
            Size::new(f64::INFINITY, f64::INFINITY),
            PANEL_GAP,
            side,
        )
    }

    /// Whether the list is longer than the visible window (needs a scrollbar).
    fn scrollable(&self) -> bool {
        self.count() > self.vis_rows
    }

    /// Largest valid `scroll` offset.
    fn max_scroll(&self) -> usize {
        self.count().saturating_sub(self.vis_rows)
    }

    /// **Place the option rows.** The engine laid them out in the trigger's flow and measured them
    /// there; this moves each visible row into its slot in the overlay panel by baking the offset
    /// into its bounds (see the module docs). Rows outside the visible window — and every row while
    /// the list is closed — are collapsed to zero size: they are not drawn, so no stale rect is
    /// left behind to swallow a click.
    ///
    /// Called from every path that can move a row (open, scroll, keyboard cursor, close) and after
    /// each layout pass, and it is idempotent: each row is placed at an **absolute** target, so
    /// re-running it never compounds an offset.
    fn place_options(&mut self) {
        let mut moved = false;
        // **While the list is closed the chosen option is an echo, not a target.** It stands inside
        // the trigger so the control shows real content — an icon, a badge, whatever the option
        // composes — but it is not something you can pick: the trigger is the control. Left
        // hittable it hovered on its own, and its pill stops at the chevron gutter, so the text lit
        // up and the caret beside it did not (F003/P096/T483).
        let echo = !self.open;
        for child in self.base.children.iter_mut() {
            child.base_mut().pointer_transparent = echo;
        }
        for (i, target) in self.option_targets().into_iter().enumerate() {
            let Some(target) = target else {
                // Not shown: collapse it, so no stale rect is left behind to swallow a click.
                let bounds = &mut self.base.children[i].base_mut().bounds;
                moved |= bounds.size != Size::new(0.0, 0.0);
                bounds.size = Size::new(0.0, 0.0);
                continue;
            };
            let child = self.base.children[i].as_mut();
            let dy = target.loc.y - child.base().bounds.loc.y;
            if dy != 0.0 {
                shift_subtree(child, 0.0, dy);
                moved = true;
            }
            // The row's *box* is the full width of what it sits in — so its pill, its hover tint
            // and its hit rect reach the edges — while its *content* stays where the engine put it:
            // inside the margins, clear of the chevron and the scrollbar lane. (Only `y` is ever
            // shifted; the content's `x` is already right, which is why the margins carry the
            // insets.)
            let bounds = &mut child.base_mut().bounds;
            moved |= *bounds != target;
            *bounds = target;
        }
        if moved {
            self.base.mark_needs_paint();
        }
    }

    /// The chosen option's slot **inside the trigger** — where it stands while the list is closed,
    /// and where its content is echoed while the list is open. Its width stops at the
    /// [gutter](Select::gutter), so the content never runs under the chevron.
    fn trigger_slot(&self) -> Rectangle {
        let b = self.base.bounds;
        let h = self.row_h(self.selected.get_untracked());
        Rectangle::new(
            Point::new(b.loc.x, b.loc.y + (b.size.h - h) / 2.0),
            Size::new((b.size.w - self.gutter()).max(0.0), h),
        )
    }

    /// Where each option goes this frame: `None` ⇒ not shown (collapse it).
    ///
    /// Open, the visible window stacks down the panel. **Closed, the chosen option is placed inside
    /// the trigger** — it is a real component, so the trigger shows its *content* (an icon, a badge,
    /// whatever it composes), not merely its text. Everything else is collapsed.
    fn option_targets(&self) -> Vec<Option<Rectangle>> {
        let mut targets = vec![None; self.count()];
        if !self.open {
            let i = self.selected.get_untracked();
            let slot = self.trigger_slot();
            if let Some(target) = targets.get_mut(i) {
                *target = Some(slot);
            }
            return targets;
        }
        let panel = self.panel_rect();
        let (lo, hi) = self.window();
        let mut y = panel.loc.y + PANEL_PAD;
        for (i, slot) in targets.iter_mut().enumerate().take(hi).skip(lo) {
            let h = self.row_h(i);
            *slot = Some(Rectangle::new(
                Point::new(panel.loc.x, y),
                Size::new(panel.size.w, h),
            ));
            y += h;
        }
        targets
    }

    /// Scroll so the cursor row is within the visible window.
    fn scroll_into_view(&mut self) {
        if self.highlight < self.scroll {
            self.scroll = self.highlight;
        } else if self.highlight >= self.scroll + self.vis_rows {
            self.scroll = self.highlight + 1 - self.vis_rows;
        }
    }

    /// Open the list. Picks a direction (below the trigger, or flipped above when there's no room)
    /// and caps the visible rows to what fits in the viewport; longer lists scroll inside the panel.
    fn open_list(&mut self) {
        if self.count() == 0 {
            return;
        }
        self.open = true;
        // **Focus is not set here.** A list opens either from a click — which focused this widget
        // through the host's `FocusManager`, clearing whoever held it — or from a key, which this
        // widget only received because it was focused already. Setting the flag directly instead
        // made a *second* widget claim focus without releasing the first, and `wants_visible`
        // defaults to that flag: every enclosing `ScrollRegion` then kept scrolling to a select
        // that had been opened once and never blurred, so a click anywhere on the page jumped it to
        // the same spot (Antonio, 2026-08-10).
        self.highlight = self.selected.get_untracked();

        let vp = self.viewport.get().h;
        let b = self.base.bounds;
        let space_below = (vp - (b.loc.y + b.size.h) - 2.0 * PANEL_GAP).max(0.0);
        let space_above = (b.loc.y - 2.0 * PANEL_GAP).max(0.0);
        let row_h = self.max_row_h();
        let rows_in = |space: f64| ((space - 2.0 * PANEL_PAD) / row_h).floor().max(0.0) as usize;
        let want = self.count().min(MAX_VISIBLE);
        let fit_below = rows_in(space_below);
        let fit_above = rows_in(space_above);

        if fit_below >= want {
            self.open_up = false;
            self.vis_rows = want;
        } else if fit_above > fit_below {
            self.open_up = true;
            self.vis_rows = want.min(fit_above).max(1);
        } else {
            self.open_up = false;
            self.vis_rows = want.min(fit_below).max(1);
        }

        self.scroll = self.highlight.min(self.max_scroll());
        self.sync_option_states();
        self.place_options();
    }

    /// Close the list: the chosen option goes back to standing in the trigger, the rest collapse.
    fn close(&mut self) {
        self.open = false;
        self.sync_option_states();
        self.place_options();
    }

    fn commit(&mut self, i: usize) {
        if i < self.count() {
            self.selected.set(i);
            self.sync_option_states();
            if let Some(f) = &self.on_change {
                f(Action::value("select-change", SignalData::Usize(i)));
            }
        }
    }
}

impl Component for Select {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn overlay_active(&self) -> bool {
        self.open && !self.base.disabled.get_untracked()
    }

    /// An open dropdown occludes its **panel rect** (the option list drawn above
    /// the page) — a host must not synthesize a page-level action (e.g. open a
    /// context menu) on a point the panel covers. Points outside the panel are
    /// not occluded (an outside click is the light-dismiss).
    fn overlay_occludes(&self, pos: Point) -> bool {
        self.overlay_active() && self.panel_rect().contains(pos)
    }

    /// The trigger's height tracks the resolved font. Its **width is the widest option** — the
    /// engine measures the rows (they hug their content) and sizes this node to them, so the
    /// control is snug around real content, icons included, instead of a char-count estimate.
    ///
    /// The rows carry the control's horizontal insets as margins: `left` aligns their content with
    /// the trigger's label (an option's own padding is smaller than the trigger's), `right` is the
    /// [gutter](Select::gutter) for the chevron and the scrollbar. Being margins, they widen the
    /// control *and* keep the option content out of them — which is exactly what a padding on this
    /// node could not do, since it would inset the rows themselves.
    fn remeasure(&mut self) {
        let fs = self.base.font;
        // Tall enough for a line of text, and for the tallest option — the trigger has to be able
        // to stand *any* of them in it (a two-line option, an icon row), and sizing to the tallest
        // rather than to the chosen one keeps the control from resizing as the selection moves. The
        // rows' measured heights come from the previous layout pass, so a first frame with content
        // taller than one line settles on the next one; for text options the two agree exactly.
        let line_h = fs * MONO_LINE_RATIO + 2.0 * self.pad_v() as f32;
        let tallest = self.natural_h.iter().copied().fold(0.0_f64, f64::max) as f32;
        self.base.style.layout.height = Length::Px(line_h.max(tallest));
        self.base.style.layout.width = if self.base.children.is_empty() {
            Length::Px(EMPTY_W * self.base.size_scale())
        } else {
            Length::Auto
        };

        let inset = (PAD_H as f32 - choice::BASE_PAD) * self.base.size_scale();
        let gutter = self.gutter() as f32;
        for child in self.base.children.iter_mut() {
            let style = &mut child.base_mut().style.layout;
            style.margin_left = Some(inset.into());
            style.margin_right = Some(gutter.into());
        }
    }

    /// Layout just reset every row to its natural position and size: re-measure the rows from it,
    /// then place them into the panel again (see [`place_options`](Select::place_options)).
    fn on_layout(&mut self) {
        self.natural_h = self
            .base
            .children
            .iter()
            .map(|c| c.base().bounds.size.h)
            .collect();
        self.sync_option_states();
        self.place_options();
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        // Remember the viewport so the next `open_list` can flip/cap the panel.
        self.viewport.set(cx.viewport());
        let disabled = self.base.disabled.get_untracked();
        let active = self.open || self.base.focused.get_untracked();
        let (surface, accent, glow_c, muted, foreground, radius, bw, ia) = {
            let t = cx.theme();
            (
                t.colors.surface,
                t.colors.accent,
                t.colors.glow,
                t.colors.muted,
                t.colors.foreground,
                t.colors.control_radius(),
                t.colors.border_width,
                t.colors.interaction,
            )
        };
        let b = self.base.bounds;

        // Trigger box: border firms muted → accent when focused/open. Radius +
        // border width come from the theme so global settings scale them. A faint
        // theme rest glow (`interaction.control_rest_glow`) gives the trigger the
        // shared neon identity at rest; `glow_size` scales it (T011).
        let p = if active { 1.0 } else { 0.0 };
        let rest_border = ia.control_rest_border as f32;
        let border_a = rest_border + (255.0 - rest_border) * p;
        let border = Border {
            color: muted.lerp(accent, p).with_alpha(border_a.round() as u8),
            width: bw,
        };
        let glow = if disabled { None } else { cx.rest_glow(GLOW_RADIUS) };
        cx.rect(b, surface, Some(border), radius, glow);

        // **One control, one highlight.** The whole trigger lights under the pointer — the chosen
        // option's text and the chevron together — in the same interaction token an `Item` and a
        // `Choice` use, so a control and a list row read as the same family. It used to be the
        // echoed option that hovered, and its pill stops at the chevron gutter, which is why the
        // caret stayed dark while the text lit up (F003/P096/T483).
        if !disabled && !self.open && self.base.hovered() {
            cx.rect(b, foreground.with_alpha(ia.row_hover_fill), None, radius, None);
        }

        // What the trigger shows: the **chosen option**, content and all — its icon, its badge,
        // whatever it composes — never just words about it.
        //
        // Closed, that option *is* the trigger's content: `place_options` stood the child in the
        // trigger slot, so it simply paints there (in the plain content color — it is not styled as
        // "selected" while it stands in the trigger; see `sync_option_states`).
        //
        // Open, the option has moved into the list — a component is laid out in exactly one place —
        // so the trigger draws a second **image** of its content, translated from where it now
        // lives back into the slot. Only the content is echoed, not the row's chrome: the trigger's
        // own box is its chrome, and a selection pill inside it would be a box in a box.
        if let Some(chosen) = self.base.children.get(self.selected.get_untracked()) {
            if self.open {
                let row = chosen.base().bounds;
                let slot = self.trigger_slot();
                let (dx, dy) = (slot.loc.x - row.loc.x, slot.loc.y - row.loc.y);
                cx.with_translate(dx, dy, |cx| {
                    cx.with_content_color(foreground, |cx| {
                        for content in &chosen.base().children {
                            content.paint(cx);
                        }
                    });
                });
            } else {
                chosen.paint(cx);
            }
        }

        // Down-chevron on the right, built from stacked rects (a small triangle).
        let chev_color = muted.lerp(accent, p);
        let cw = 9.0_f64;
        let cx0 = b.loc.x + b.size.w - self.pad_h() - cw;
        let cy0 = b.loc.y + b.size.h / 2.0 - 2.0;
        for k in 0..3 {
            let inset = k as f64 * (cw / 2.0) / 3.0;
            cx.rect(
                Rectangle::new(
                    Point::new(cx0 + inset, cy0 + k as f64 * 2.0),
                    Size::new(cw - 2.0 * inset, 1.5),
                ),
                chev_color,
                None,
                0.0,
                None,
            );
        }

        if disabled {
            cx.dim(b, radius);
        }
        if !disabled && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            let ring = cx.theme().colors.effective_focus_ring();
            cx.focus_ring(b, ring, radius);
        }

        // Open option list — painted in the overlay layer (on top of everything).
        if self.open && !disabled {
            cx.with_overlay(|cx| {
                let panel = self.panel_rect();
                // The panel's presentation comes from the SHARED overlay panel
                // chrome (drop shadow + theme surface fill + bracket reticle), so a
                // dropdown reads as the same surface as every other overlay panel.
                // The dropdown keeps its own identity on top: the accent edge and
                // the neon halo that mark it as an open control.
                paint_panel_chrome(
                    cx,
                    panel,
                    PanelChrome {
                        border: Some(Border {
                            color: accent,
                            width: bw,
                        }),
                        glow: Some(Glow {
                            color: glow_c,
                            radius: GLOW_RADIUS,
                            intensity: GLOW_INTENSITY,
                        }),
                        elevation: PanelElevation::Panel,
                    },
                );

                // Only the visible window is drawn — its rows were placed there, and their bounds
                // say so. The cursor is the *container's* chrome, painted over the row's bounds;
                // the row's own chosen/hover state is the `Choice`'s, and it tints its content.
                let (lo, hi) = self.window();
                for i in lo..hi {
                    let child = self.base.children[i].as_ref();
                    if i == self.highlight {
                        cx.rect(
                            child.base().bounds,
                            accent.with_alpha(ia.hilite),
                            None,
                            radius,
                            None,
                        );
                    }
                    crate::component::paint_child(child, cx);
                }

                // Scrollbar: a thumb sized/positioned by the visible window.
                if self.scrollable() {
                    let n = self.count() as f64;
                    let track_h = panel.size.h - 2.0 * PANEL_PAD;
                    let thumb_h = (track_h * self.vis_rows as f64 / n).max(12.0);
                    let frac = self.scroll as f64 / self.max_scroll() as f64;
                    let track_x = panel.loc.x + panel.size.w - SCROLLBAR_W - 2.0;
                    let thumb_y = panel.loc.y + PANEL_PAD + (track_h - thumb_h) * frac;
                    cx.rect(
                        Rectangle::new(
                            Point::new(track_x, thumb_y),
                            Size::new(SCROLLBAR_W, thumb_h),
                        ),
                        accent,
                        None,
                        (SCROLLBAR_W / 2.0) as f32,
                        None,
                    );
                }
            });
        }
    }

    /// The select's **input** surface: the trigger, plus the list it is showing.
    fn hit_bounds(&self) -> Option<Rectangle> {
        let trigger = self.base.bounds;
        if !self.open {
            return Some(trigger);
        }
        let panel = self.panel_rect();
        let x0 = trigger.loc.x.min(panel.loc.x);
        let y0 = trigger.loc.y.min(panel.loc.y);
        let x1 = (trigger.loc.x + trigger.size.w).max(panel.loc.x + panel.size.w);
        let y1 = (trigger.loc.y + trigger.size.h).max(panel.loc.y + panel.size.h);
        Some(Rectangle::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0)))
    }

    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            // The open panel is opaque by construction now: it is what the router hit-tested, so
            // a move that reaches this widget is over the trigger or the list, and the widgets the
            // panel covers are not on the path at all — they cannot light up behind it.
            Event::PointerMove(p) => {
                if self.open
                    && let Some(i) = self.row_at(p.pos)
                {
                    self.highlight = i;
                }
                Handled::No
            }
            Event::PointerDown(p) => {
                if self.open {
                    if let Some(i) = self.row_at(p.pos) {
                        self.commit(i);
                    }
                    // A press on the option or the trigger closes it; one outside arrives as
                    // `PointerDownOutside` below.
                    self.close();
                    Handled::Yes
                } else {
                    self.open_list();
                    Handled::Yes
                }
            }
            Event::PointerDownOutside(_) if self.open => {
                self.close();
                Handled::No
            }
            Event::Scroll(p) if self.open => {
                let max = self.max_scroll() as f32;
                self.scroll = (self.scroll as f32 + p.delta_y).clamp(0.0, max).round() as usize;
                self.place_options();
                Handled::Yes
            }
            // Open-list navigation: the widget is an overlay while open, so the host sends the
            // configurable `[keys.widgets]` keys as a semantic `WidgetIntent` (shared with the
            // context menu / command palette). A vertical list: `MenuUp`/`MenuDown` (not the
            // horizontal `Item*`). No literal nav keys live here.
            Event::Widget(intent) if self.open => match intent {
                WidgetIntent::MenuUp => {
                    self.highlight = self.highlight.saturating_sub(1);
                    self.scroll_into_view();
                    self.place_options();
                    Handled::Yes
                }
                WidgetIntent::MenuDown => {
                    self.highlight = (self.highlight + 1).min(self.count().saturating_sub(1));
                    self.scroll_into_view();
                    self.place_options();
                    Handled::Yes
                }
                WidgetIntent::Activate => {
                    self.commit(self.highlight);
                    self.close();
                    Handled::Yes
                }
                WidgetIntent::Dismiss => {
                    self.close();
                    Handled::Yes
                }
                _ => Handled::No,
            },
            // Closed + focused (not yet an overlay): raw activation keys open the list, like a
            // button's Enter/Space. The open list is driven by `WidgetIntent` above, not raw keys.
            Event::Key {
                key: GridKey::Enter | GridKey::Space | GridKey::ArrowDown,
                pressed: true,
            } if !self.open => {
                self.open_list();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn on_blur(&mut self) {
        self.close();
        self.base.focused.set(false);
        self.base.focus_visible.set(false);
    }
}

impl Select {
    /// The option under `pos`, resolved from the visible rows' **real bounds** — never from row
    /// arithmetic, so a click can only ever land on the row the user sees there.
    fn row_at(&self, pos: Point) -> Option<usize> {
        let (lo, hi) = self.window();
        choice_at(&self.base.children[lo..hi], pos).map(|slot| lo + slot)
    }
}

impl LayoutExt for Select {}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3-option select with a laid-out trigger rect and a known viewport.
    fn select_at(trigger: Rectangle, viewport: Size) -> Select {
        let mut s = Select::new(["low", "medium", "high"]);
        s.base.bounds = trigger;
        s.viewport.set(viewport);
        s
    }

    /// With room below, the panel hangs under the trigger: left-aligned to it,
    /// one `PANEL_GAP` down, and as wide as the trigger.
    #[test]
    fn open_panel_sits_below_the_trigger_when_there_is_room() {
        let trigger = Rectangle::new(Point::new(40.0, 100.0), Size::new(120.0, 30.0));
        let mut s = select_at(trigger, Size::new(800.0, 600.0));
        s.open_list();
        assert!(!s.open_up, "room below → no flip");
        let p = s.panel_rect();
        assert_eq!(p.loc.x, 40.0, "left-aligned to the trigger");
        assert_eq!(p.loc.y, 100.0 + 30.0 + PANEL_GAP, "gap below the trigger");
        assert_eq!(p.size.w, 120.0, "panel width tracks the trigger");
    }

    /// Near the viewport bottom the list flips **above** the trigger, and the
    /// placement honours that decision (it does not re-derive the side).
    #[test]
    fn open_panel_flips_above_near_the_viewport_bottom() {
        let trigger = Rectangle::new(Point::new(40.0, 560.0), Size::new(120.0, 30.0));
        let mut s = select_at(trigger, Size::new(800.0, 600.0));
        s.open_list();
        assert!(s.open_up, "no room below → flipped up");
        let p = s.panel_rect();
        assert_eq!(
            p.loc.y,
            560.0 - PANEL_GAP - p.size.h,
            "panel bottom sits a gap above the trigger top"
        );
        assert!(p.loc.y >= 0.0, "still on-screen");
    }

    /// **Regression guard.** The panel rect must be a pure function of the trigger
    /// bounds + side + row heights — it is computed once when the rows are placed
    /// (layout) and again when the panel is drawn (paint), and those run at
    /// different moments. Making it depend on anything time-varying (a viewport
    /// cached during paint, used to clamp) makes the two disagree and the rows
    /// visibly detach from the panel — which is exactly what happened when the
    /// list flipped **above** the trigger and a tall panel's negative `y` got
    /// clamped to the viewport top on one of the two calls.
    #[test]
    fn open_panel_rect_is_independent_of_the_cached_viewport() {
        let trigger = Rectangle::new(Point::new(760.0, 560.0), Size::new(120.0, 30.0));
        let mut s = select_at(trigger, Size::new(800.0, 600.0));
        s.open_list();
        let before = s.panel_rect();
        // A different viewport (resize, or the value paint caches vs. what layout
        // saw) must not move the panel out from under its rows.
        s.viewport.set(Size::new(400.0, 200.0));
        assert_eq!(s.panel_rect(), before, "panel rect must not track the viewport");
        s.viewport.set(Size::new(f64::INFINITY, f64::INFINITY));
        assert_eq!(s.panel_rect(), before, "…nor an unset one");
    }

    /// Flipped above, the panel's geometry matches the original hand-rolled
    /// formula exactly: bottom edge one gap above the trigger top, no clamping.
    #[test]
    fn flipped_panel_is_not_clamped_even_when_it_overflows_the_top() {
        // A trigger high on screen with a tall list: the panel legitimately starts
        // at a negative y. Clamping it here is what detached the rows.
        let trigger = Rectangle::new(Point::new(40.0, 60.0), Size::new(120.0, 30.0));
        let mut s = select_at(trigger, Size::new(800.0, 600.0));
        s.open_up = true;
        s.open = true;
        let p = s.panel_rect();
        assert_eq!(p.loc.y, 60.0 - PANEL_GAP - p.size.h, "exact flip-above formula");
    }

    /// Closed, the panel is not consulted: the widget reports no overlay and the
    /// chosen option stands in the trigger.
    #[test]
    fn closed_select_is_not_an_overlay() {
        let trigger = Rectangle::new(Point::new(40.0, 100.0), Size::new(120.0, 30.0));
        let s = select_at(trigger, Size::new(800.0, 600.0));
        assert!(!s.overlay_active());
        assert!(!s.overlay_occludes(Point::new(45.0, 140.0)));
    }
}
