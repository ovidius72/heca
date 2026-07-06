//! [`Select`] — a single-select dropdown. The first consumer of the overlay
//! layer: while open, its option list paints in the scene's **overlay layer**
//! (on top of everything) and it reports [`overlay_active`](Component::overlay_active)
//! so the host routes input to it first — letting it capture clicks on rows that
//! fall outside its own layout bounds.
//!
//! Self-contained like [`Tabs`](super::Tabs): it lays its trigger + rows out from
//! its own metrics (no child components). Selecting emits
//! `Action::value("select-change", SignalData::Usize(index))`. Honors
//! [`Base::disabled`](crate::component::Base); keyboard: ↑/↓ move the highlight,
//! Enter/Space open/commit, Esc closes.

use crate::action::{Action, SignalData};
use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Border, Glow, TextAlign};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::Cell;

/// Default trigger width (logical px).
const DEFAULT_WIDTH: f32 = 200.0;
/// Inner horizontal padding.
const PAD_H: f64 = 12.0;
/// Inner vertical padding (trigger).
const PAD_V: f64 = 8.0;
/// Height of each option row in the open list.
/// Option-row height as a multiple of the (resolved) font — so rows grow with
/// the font instead of clipping. ≈30px at the 15px base font.
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
/// Horizontal room reserved for the down-chevron (+ gap) when sizing the width.
const CHEVRON_W: f64 = 22.0;

/// A single-select dropdown.
pub struct Select {
    base: Base,
    options: Vec<String>,
    /// Selected index, exposed reactively via [`state`](Select::state).
    selected: Signal<usize>,
    /// Whether the dropdown is open.
    open: bool,
    /// Highlighted row while open (keyboard/hover cursor).
    highlight: usize,
    /// Index of the first visible row when the list scrolls.
    scroll: usize,
    /// Rows shown at once while open — capped to what fits in the viewport.
    vis_rows: usize,
    /// Whether the open list flips *above* the trigger (no room below).
    open_up: bool,
    /// Last-seen viewport height (set during paint), used to flip/cap the list.
    viewport_h: Cell<f64>,
    on_change: Option<Box<dyn Fn(Action)>>,
}

impl Select {
    /// A new select over `options`; the first option is selected.
    pub fn new(options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let options: Vec<String> = options.into_iter().map(Into::into).collect();
        let mut base = Base::new();
        base.style.width = Length::Px(DEFAULT_WIDTH);
        base.style.height = Length::Px(base.font * MONO_LINE_RATIO + 2.0 * PAD_V as f32);
        Self {
            base,
            options,
            selected: signal(0),
            open: false,
            highlight: 0,
            scroll: 0,
            vis_rows: MAX_VISIBLE,
            open_up: false,
            viewport_h: Cell::new(f64::MAX),
            on_change: None,
        }
    }

    /// Explicit font size — overrides the inherited theme font.
    pub fn font_size(mut self, fs: f32) -> Self {
        self.base.style.font_size = fs;
        self.base.font = fs;
        self.remeasure();
        self
    }

    /// Select an initial option (clamped to the option count).
    pub fn selected(self, index: usize) -> Self {
        let i = index.min(self.options.len().saturating_sub(1));
        self.selected.set(i);
        self
    }

    /// Set the change handler. Receives `Action::value("select-change",
    /// SignalData::Usize(index))` when the selection changes.
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

    /// The currently selected option's label.
    pub fn selected_label(&self) -> &str {
        self.options
            .get(self.selected.get_untracked())
            .map(String::as_str)
            .unwrap_or("")
    }

    /// Option-row height, derived from the resolved font.
    fn row_h(&self) -> f64 {
        self.base.font as f64 * ROW_H_RATIO
    }

    /// Padding / chevron width scaled by the size variant (the font already is), so
    /// the control and its panel grow/shrink as a unit. Used by both measure + paint
    /// + hit-test so they stay in agreement.
    fn pad_h(&self) -> f64 {
        PAD_H * self.base.size_scale() as f64
    }
    fn pad_v(&self) -> f64 {
        PAD_V * self.base.size_scale() as f64
    }
    fn chevron_w(&self) -> f64 {
        CHEVRON_W * self.base.size_scale() as f64
    }

    /// Panel height for the current visible-row count.
    fn panel_h(&self) -> f64 {
        2.0 * PANEL_PAD + self.vis_rows as f64 * self.row_h()
    }

    /// Y of the panel top — below the trigger normally, above it when flipped up.
    fn panel_top(&self) -> f64 {
        let b = self.base.bounds;
        if self.open_up {
            b.loc.y - PANEL_GAP - self.panel_h()
        } else {
            b.loc.y + b.size.h + PANEL_GAP
        }
    }

    /// Number of rows shown at once (set when the list opens, capped to fit).
    fn visible_count(&self) -> usize {
        self.vis_rows
    }

    /// Whether the list is longer than the visible window (needs a scrollbar).
    fn scrollable(&self) -> bool {
        self.options.len() > self.vis_rows
    }

    /// Largest valid `scroll` offset.
    fn max_scroll(&self) -> usize {
        self.options.len().saturating_sub(self.vis_rows)
    }

    /// The bounding rect of the open option list (panel).
    fn panel_rect(&self) -> Rectangle {
        let b = self.base.bounds;
        Rectangle::new(
            Point::new(b.loc.x, self.panel_top()),
            Size::new(b.size.w, self.panel_h()),
        )
    }

    /// The rect of the `slot`-th *visible* row (0-based from the top of the list).
    fn slot_rect(&self, slot: usize) -> Rectangle {
        let b = self.base.bounds;
        let row_h = self.row_h();
        Rectangle::new(
            Point::new(b.loc.x, self.panel_top() + PANEL_PAD + slot as f64 * row_h),
            Size::new(b.size.w, row_h),
        )
    }

    /// Index of the option row under `pos`, if any (only meaningful while open).
    fn row_at(&self, pos: Point) -> Option<usize> {
        (0..self.visible_count())
            .find(|&slot| self.slot_rect(slot).contains(pos))
            .map(|slot| self.scroll + slot)
    }

    /// Scroll so the highlighted row is within the visible window.
    fn scroll_into_view(&mut self) {
        if self.highlight < self.scroll {
            self.scroll = self.highlight;
        } else if self.highlight >= self.scroll + self.vis_rows {
            self.scroll = self.highlight + 1 - self.vis_rows;
        }
    }

    /// Open the list. Picks a direction (below the trigger, or flipped above when
    /// there's no room) and caps the visible rows to what fits in the viewport;
    /// longer lists scroll inside the panel.
    fn open_list(&mut self) {
        self.open = true;
        self.highlight = self.selected.get_untracked();

        let vp = self.viewport_h.get();
        let b = self.base.bounds;
        let space_below = (vp - (b.loc.y + b.size.h) - 2.0 * PANEL_GAP).max(0.0);
        let space_above = (b.loc.y - 2.0 * PANEL_GAP).max(0.0);
        let row_h = self.row_h();
        let rows_in = |space: f64| ((space - 2.0 * PANEL_PAD) / row_h).floor().max(0.0) as usize;
        let want = self.options.len().min(MAX_VISIBLE);
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
    }

    fn commit(&mut self, i: usize) {
        if i < self.options.len() {
            self.selected.set(i);
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

    fn focusable(&self) -> bool {
        !self.base.disabled.get_untracked()
    }

    fn overlay_active(&self) -> bool {
        self.open && !self.base.disabled.get_untracked()
    }

    /// Trigger height + width track the resolved font: the width adapts to the
    /// widest option (plus padding, chevron and scrollbar room) so it's snug, not
    /// a fixed block, and text never overflows.
    fn remeasure(&mut self) {
        let fs = self.base.font;
        self.base.style.height = Length::Px(fs * MONO_LINE_RATIO + 2.0 * self.pad_v() as f32);
        let longest = self
            .options
            .iter()
            .map(|s| s.chars().count())
            .max()
            .unwrap_or(0) as f32;
        let text_w = longest * fs * MONO_ADVANCE_RATIO;
        let chrome = (2.0 * self.pad_h() + self.chevron_w() + SCROLLBAR_W) as f32;
        self.base.style.width = Length::Px(text_w + chrome);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        // Remember the viewport so the next `open_list` can flip/cap the panel.
        self.viewport_h.set(cx.viewport().h);
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
        let fs = self.base.font;

        // Trigger box: border firms muted → accent when focused/open. Radius +
        // border width come from the theme so global settings scale them.
        let p = if active { 1.0 } else { 0.0 };
        let rest_border = ia.control_rest_border as f32;
        let border_a = rest_border + (255.0 - rest_border) * p;
        let border = Border {
            color: muted.lerp(accent, p).with_alpha(border_a.round() as u8),
            width: bw,
        };
        cx.rect(b, surface, Some(border), radius, None);

        // Selected label (left), inset by padding.
        let pad_h = self.pad_h();
        let text_rect = Rectangle::new(
            Point::new(b.loc.x + pad_h, b.loc.y),
            Size::new((b.size.w - 2.0 * pad_h - 12.0).max(0.0), b.size.h),
        );
        cx.text(
            text_rect,
            self.selected_label(),
            foreground,
            fs,
            TextAlign::Start,
            false,
        );

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
        if !disabled && self.base.focus_visible.get_untracked() && cx.theme().colors.show_focus_border {
            cx.corner_brackets(b, accent);
        }

        // Open option list — painted in the overlay layer (on top of everything).
        if self.open && !disabled {
            cx.with_overlay(|cx| {
                let panel = self.panel_rect();
                let glow = Some(Glow {
                    color: glow_c,
                    radius: GLOW_RADIUS,
                    intensity: GLOW_INTENSITY,
                });
                cx.rect(
                    panel,
                    surface,
                    Some(Border {
                        color: accent,
                        width: bw,
                    }),
                    radius,
                    glow,
                );
                let selected = self.selected.get_untracked();
                let scrollbar = self.scrollable();
                // Render only the visible window of rows (no clipping needed).
                for slot in 0..self.visible_count() {
                    let i = self.scroll + slot;
                    let row = self.slot_rect(slot);
                    if i == self.highlight {
                        cx.rect(row, accent.with_alpha(cx.theme().colors.interaction.hilite), None, 2.0, None);
                    }
                    // Leave room for the scrollbar on the right when present.
                    let pad_h = self.pad_h();
                    let right_pad = if scrollbar {
                        pad_h + SCROLLBAR_W
                    } else {
                        pad_h
                    };
                    let row_text = Rectangle::new(
                        Point::new(row.loc.x + pad_h, row.loc.y),
                        Size::new((row.size.w - pad_h - right_pad).max(0.0), row.size.h),
                    );
                    let color = if i == selected { accent } else { foreground };
                    cx.text(
                        row_text,
                        &self.options[i],
                        color,
                        fs,
                        TextAlign::Start,
                        i == selected,
                    );
                }
                // Scrollbar: a thumb sized/positioned by the visible window.
                if scrollbar {
                    let n = self.options.len() as f64;
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

    fn event(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::PointerMoved { pos } => {
                if self.open
                    && let Some(i) = self.row_at(*pos)
                {
                    self.highlight = i;
                }
                Handled::No
            }
            Event::PointerPressed { pos } => {
                if self.open {
                    if let Some(i) = self.row_at(*pos) {
                        self.commit(i);
                    }
                    // Any press while open closes it (option, trigger, or outside).
                    self.open = false;
                    Handled::Yes
                } else if self.base.bounds.contains(*pos) {
                    self.open_list();
                    Handled::Yes
                } else {
                    Handled::No
                }
            }
            Event::Scroll { delta } if self.open => {
                let max = self.max_scroll() as f32;
                self.scroll = (self.scroll as f32 + delta).clamp(0.0, max).round() as usize;
                Handled::Yes
            }
            Event::Key { key, pressed: true } => match key {
                GridKey::Escape if self.open => {
                    self.open = false;
                    Handled::Yes
                }
                GridKey::Enter | GridKey::Space => {
                    if self.open {
                        self.commit(self.highlight);
                        self.open = false;
                    } else {
                        self.open_list();
                    }
                    Handled::Yes
                }
                GridKey::ArrowDown => {
                    if self.open {
                        self.highlight = (self.highlight + 1).min(self.options.len() - 1);
                        self.scroll_into_view();
                    } else {
                        self.open_list();
                    }
                    Handled::Yes
                }
                GridKey::ArrowUp if self.open => {
                    self.highlight = self.highlight.saturating_sub(1);
                    self.scroll_into_view();
                    Handled::Yes
                }
                _ => Handled::No,
            },
            _ => Handled::No,
        }
    }

    fn on_blur(&mut self) {
        self.open = false;
        self.base.focused.set(false);
        self.base.focus_visible.set(false);
    }
}

impl LayoutExt for Select {}
