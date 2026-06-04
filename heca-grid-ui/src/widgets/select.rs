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
use crate::font::MONO_LINE_RATIO;
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Border, Glow, TextAlign};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Default trigger width (logical px).
const DEFAULT_WIDTH: f32 = 200.0;
/// Trigger/row font size.
const FONT_SIZE: f32 = 14.0;
/// Inner horizontal padding.
const PAD_H: f64 = 12.0;
/// Inner vertical padding (trigger).
const PAD_V: f64 = 8.0;
/// Height of each option row in the open list.
const ROW_H: f64 = 30.0;
/// Padding around the option list inside the panel.
const PANEL_PAD: f64 = 4.0;
/// Gap between the trigger and the panel.
const PANEL_GAP: f64 = 4.0;
/// Corner radius for trigger + panel.
const RADIUS: f32 = 4.0;
/// Border alpha at rest; firms to solid when focused/open.
const REST_BORDER_ALPHA: f32 = 150.0;
/// Highlighted-row fill alpha.
const HILITE_ALPHA: u8 = 48;
/// Panel glow.
const GLOW_RADIUS: f32 = 16.0;
const GLOW_INTENSITY: f32 = 0.1;
/// Max option rows shown at once; longer lists scroll with a scrollbar.
const MAX_VISIBLE: usize = 6;
/// Scrollbar track width (logical px).
const SCROLLBAR_W: f64 = 4.0;

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
    on_change: Option<Box<dyn Fn(Action)>>,
}

impl Select {
    /// A new select over `options`; the first option is selected.
    pub fn new(options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let options: Vec<String> = options.into_iter().map(Into::into).collect();
        let mut base = Base::new();
        base.style.font_size = FONT_SIZE;
        base.style.width = Length::Px(DEFAULT_WIDTH);
        base.style.height = Length::Px(FONT_SIZE * MONO_LINE_RATIO + 2.0 * PAD_V as f32);
        Self {
            base,
            options,
            selected: signal(0),
            open: false,
            highlight: 0,
            scroll: 0,
            on_change: None,
        }
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

    fn panel_top(&self) -> f64 {
        self.base.bounds.loc.y + self.base.bounds.size.h + PANEL_GAP
    }

    /// Number of rows shown at once (capped by [`MAX_VISIBLE`]).
    fn visible_count(&self) -> usize {
        self.options.len().min(MAX_VISIBLE)
    }

    /// Whether the list is longer than the visible window (needs a scrollbar).
    fn scrollable(&self) -> bool {
        self.options.len() > MAX_VISIBLE
    }

    /// Largest valid `scroll` offset.
    fn max_scroll(&self) -> usize {
        self.options.len().saturating_sub(MAX_VISIBLE)
    }

    /// The bounding rect of the open option list (panel).
    fn panel_rect(&self) -> Rectangle {
        let b = self.base.bounds;
        let h = 2.0 * PANEL_PAD + self.visible_count() as f64 * ROW_H;
        Rectangle::new(
            Point::new(b.loc.x, self.panel_top()),
            Size::new(b.size.w, h),
        )
    }

    /// The rect of the `slot`-th *visible* row (0-based from the top of the list).
    fn slot_rect(&self, slot: usize) -> Rectangle {
        let b = self.base.bounds;
        Rectangle::new(
            Point::new(b.loc.x, self.panel_top() + PANEL_PAD + slot as f64 * ROW_H),
            Size::new(b.size.w, ROW_H),
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
        } else if self.highlight >= self.scroll + MAX_VISIBLE {
            self.scroll = self.highlight + 1 - MAX_VISIBLE;
        }
    }

    /// Open the list, highlighting (and scrolling to) the current selection.
    fn open_list(&mut self) {
        self.open = true;
        self.highlight = self.selected.get_untracked();
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

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let active = self.open || self.base.focused.get_untracked();
        let (surface, accent, glow_c, muted, foreground) = {
            let t = cx.theme();
            (t.surface, t.accent, t.glow, t.muted, t.foreground)
        };
        let b = self.base.bounds;
        let fs = self.base.style.font_size;

        // Trigger box: border firms muted → accent when focused/open.
        let p = if active { 1.0 } else { 0.0 };
        let border_a = REST_BORDER_ALPHA + (255.0 - REST_BORDER_ALPHA) * p;
        let border = Border {
            color: muted.lerp(accent, p).with_alpha(border_a.round() as u8),
            width: 1.5,
        };
        cx.rect(b, surface, Some(border), RADIUS, None);

        // Selected label (left), inset by padding.
        let text_rect = Rectangle::new(
            Point::new(b.loc.x + PAD_H, b.loc.y),
            Size::new((b.size.w - 2.0 * PAD_H - 12.0).max(0.0), b.size.h),
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
        let cx0 = b.loc.x + b.size.w - PAD_H - cw;
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
            cx.dim(b, RADIUS);
        }
        if !disabled && self.base.focus_visible.get_untracked() && cx.theme().show_focus_border {
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
                        width: 1.5,
                    }),
                    RADIUS,
                    glow,
                );
                let selected = self.selected.get_untracked();
                let scrollbar = self.scrollable();
                // Render only the visible window of rows (no clipping needed).
                for slot in 0..self.visible_count() {
                    let i = self.scroll + slot;
                    let row = self.slot_rect(slot);
                    if i == self.highlight {
                        cx.rect(row, accent.with_alpha(HILITE_ALPHA), None, 2.0, None);
                    }
                    // Leave room for the scrollbar on the right when present.
                    let right_pad = if scrollbar {
                        PAD_H + SCROLLBAR_W
                    } else {
                        PAD_H
                    };
                    let row_text = Rectangle::new(
                        Point::new(row.loc.x + PAD_H, row.loc.y),
                        Size::new((row.size.w - PAD_H - right_pad).max(0.0), row.size.h),
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
                    let thumb_h = (track_h * MAX_VISIBLE as f64 / n).max(12.0);
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
