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
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
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

    /// The bounding rect of the open option list (panel).
    fn panel_rect(&self) -> Rectangle {
        let b = self.base.bounds;
        let h = 2.0 * PANEL_PAD + self.options.len() as f64 * ROW_H;
        Rectangle::new(Point::new(b.loc.x, self.panel_top()), Size::new(b.size.w, h))
    }

    /// The rect of option row `i` within the open panel.
    fn row_rect(&self, i: usize) -> Rectangle {
        let b = self.base.bounds;
        Rectangle::new(
            Point::new(b.loc.x, self.panel_top() + PANEL_PAD + i as f64 * ROW_H),
            Size::new(b.size.w, ROW_H),
        )
    }

    /// Index of the option row under `pos`, if any (only meaningful while open).
    fn row_at(&self, pos: Point) -> Option<usize> {
        (0..self.options.len()).find(|&i| self.row_rect(i).contains(pos))
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
                cx.rect(panel, surface, Some(Border { color: accent, width: 1.5 }), RADIUS, glow);
                let selected = self.selected.get_untracked();
                for (i, opt) in self.options.iter().enumerate() {
                    let row = self.row_rect(i);
                    if i == self.highlight {
                        cx.rect(row, accent.with_alpha(HILITE_ALPHA), None, 2.0, None);
                    }
                    let row_text = Rectangle::new(
                        Point::new(row.loc.x + PAD_H, row.loc.y),
                        Size::new((row.size.w - 2.0 * PAD_H).max(0.0), row.size.h),
                    );
                    let color = if i == selected { accent } else { foreground };
                    cx.text(row_text, opt, color, fs, TextAlign::Start, i == selected);
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
                if self.open && let Some(i) = self.row_at(*pos) {
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
                    self.open = true;
                    self.highlight = self.selected.get_untracked();
                    Handled::Yes
                } else {
                    Handled::No
                }
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
                        self.open = true;
                        self.highlight = self.selected.get_untracked();
                    }
                    Handled::Yes
                }
                GridKey::ArrowDown => {
                    if self.open {
                        self.highlight = (self.highlight + 1).min(self.options.len() - 1);
                    } else {
                        self.open = true;
                        self.highlight = self.selected.get_untracked();
                    }
                    Handled::Yes
                }
                GridKey::ArrowUp if self.open => {
                    self.highlight = self.highlight.saturating_sub(1);
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
