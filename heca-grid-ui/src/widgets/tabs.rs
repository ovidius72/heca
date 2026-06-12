//! [`Tabs`] — a horizontal segmented selector with an **animated underline**
//! that slides to the active tab. A change widget: selecting a tab emits
//! `Action::value("tab-change", SignalData::Usize(index))`.
//!
//! It lays its own segments out from monospace metrics (no child components),
//! hit-tests pointer presses by x, and moves selection with Left/Right while
//! focused. Reuses [`Base::disabled`](crate::component::Base) and the
//! focus-visible ring; the underline animates via [`Component::tick`].

use crate::action::{Action, SignalData};
use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Glow, TextAlign};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Horizontal padding inside each tab.
const TAB_PAD_H: f64 = 14.0;
/// Gap between tabs.
const TAB_GAP: f64 = 6.0;
/// Underline thickness.
const UNDERLINE_H: f64 = 2.0;
/// Extra vertical room around the text (for the underline).
const V_PAD: f32 = 12.0;
/// Seconds for the underline to slide between tabs.
const ANIM_DURATION: f32 = 0.12;
/// Underline glow radius (px).
const GLOW_RADIUS: f32 = 12.0;
/// Underline glow intensity.
const GLOW_INTENSITY: f32 = 0.12;

/// A segmented tab selector.
pub struct Tabs {
    base: Base,
    labels: Vec<String>,
    selected: Signal<usize>,
    hovered: Option<usize>,
    /// Animated underline position/size (relative to bounds origin).
    ind_x: f64,
    ind_w: f64,
    on_change: Option<Box<dyn Fn(Action)>>,
}

impl Tabs {
    /// New tabs from `labels`; the first tab is selected.
    pub fn new(labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let labels: Vec<String> = labels.into_iter().map(Into::into).collect();
        let base = Base::new();
        let mut tabs = Self {
            base,
            labels,
            selected: signal(0),
            hovered: None,
            ind_x: 0.0,
            ind_w: 0.0,
            on_change: None,
        };
        tabs.remeasure();
        let (x, w) = tabs.segment(0).unwrap_or((0.0, 0.0));
        tabs.ind_x = x;
        tabs.ind_w = w;
        tabs
    }

    /// Explicit font size — overrides the inherited theme font.
    pub fn font_size(mut self, fs: f32) -> Self {
        self.base.style.font_size = fs;
        self.base.font = fs;
        self.remeasure();
        // Re-anchor the active indicator to the re-measured segments.
        let i = self.selected.get_untracked();
        let (x, w) = self.segment(i).unwrap_or((0.0, 0.0));
        self.ind_x = x;
        self.ind_w = w;
        self
    }

    /// Select an initial tab (clamped to the tab count).
    pub fn selected(mut self, index: usize) -> Self {
        let i = index.min(self.labels.len().saturating_sub(1));
        self.selected.set(i);
        let (x, w) = self.segment(i).unwrap_or((0.0, 0.0));
        self.ind_x = x;
        self.ind_w = w;
        self
    }

    /// Set the change handler. Receives `Action::value("tab-change",
    /// SignalData::Usize(index))` when the active tab changes.
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

    fn advance(&self) -> f64 {
        (self.base.font * MONO_ADVANCE_RATIO) as f64
    }

    /// Per-tab horizontal padding + inter-tab gap, scaled by the size variant.
    fn tab_pad_h(&self) -> f64 {
        TAB_PAD_H * self.base.size_scale() as f64
    }
    fn tab_gap(&self) -> f64 {
        TAB_GAP * self.base.size_scale() as f64
    }

    /// `(start_x, width)` of tab `i` relative to the bounds origin.
    fn segment(&self, i: usize) -> Option<(f64, f64)> {
        if i >= self.labels.len() {
            return None;
        }
        let mut x = 0.0;
        for (j, label) in self.labels.iter().enumerate() {
            let w = label.chars().count() as f64 * self.advance() + 2.0 * self.tab_pad_h();
            if j == i {
                return Some((x, w));
            }
            x += w + self.tab_gap();
        }
        None
    }

    fn total_width(&self) -> f64 {
        let mut x = 0.0;
        for label in &self.labels {
            let w = label.chars().count() as f64 * self.advance() + 2.0 * self.tab_pad_h();
            x += w + self.tab_gap();
        }
        (x - self.tab_gap()).max(0.0)
    }

    /// Index of the tab under relative x, if any.
    fn tab_at(&self, rel_x: f64) -> Option<usize> {
        for i in 0..self.labels.len() {
            if let Some((sx, sw)) = self.segment(i)
                && rel_x >= sx
                && rel_x < sx + sw
            {
                return Some(i);
            }
        }
        None
    }

    fn select(&mut self, i: usize) {
        if i == self.selected.get_untracked() || i >= self.labels.len() {
            return;
        }
        self.selected.set(i);
        if let Some(f) = &self.on_change {
            f(Action::value("tab-change", SignalData::Usize(i)));
        }
    }
}

impl Component for Tabs {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn focusable(&self) -> bool {
        !self.base.disabled.get_untracked()
    }

    /// Strip width + height track the resolved font.
    fn remeasure(&mut self) {
        self.base.style.width = Length::Px(self.total_width() as f32);
        self.base.style.height =
            Length::Px(self.base.font * MONO_LINE_RATIO + 2.0 * V_PAD * self.base.size_scale());
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let (accent, glow_c, muted, foreground) = {
            let t = cx.theme();
            (t.accent, t.glow, t.muted, t.foreground)
        };
        let b = self.base.bounds;
        let fs = self.base.font;
        let selected = self.selected.get_untracked();

        // Tab labels.
        for (i, label) in self.labels.iter().enumerate() {
            let Some((sx, sw)) = self.segment(i) else {
                continue;
            };
            let rect = Rectangle::new(
                Point::new(b.loc.x + sx, b.loc.y),
                Size::new(sw, b.size.h - UNDERLINE_H),
            );
            let color = if i == selected {
                accent
            } else if self.hovered == Some(i) {
                foreground
            } else {
                muted
            };
            cx.text(rect, label, color, fs, TextAlign::Center, i == selected);
        }

        // Animated underline under the active tab.
        let underline = Rectangle::new(
            Point::new(b.loc.x + self.ind_x, b.loc.y + b.size.h - UNDERLINE_H),
            Size::new(self.ind_w, UNDERLINE_H),
        );
        let glow = (!disabled).then_some(Glow {
            color: glow_c,
            radius: GLOW_RADIUS,
            intensity: GLOW_INTENSITY,
        });
        cx.rect(underline, accent, None, 0.0, glow);

        if disabled {
            cx.dim(b, 0.0);
        }
        if !disabled && self.base.focus_visible.get_untracked() && cx.theme().show_focus_border {
            cx.corner_brackets(b, accent);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::PointerMoved { pos } => {
                let rel = pos.x - self.base.bounds.loc.x;
                let hit = if self.base.bounds.contains(*pos) {
                    self.tab_at(rel)
                } else {
                    None
                };
                if self.hovered != hit {
                    self.hovered = hit;
                }
                Handled::No
            }
            Event::PointerPressed { pos } if self.base.bounds.contains(*pos) => {
                if let Some(i) = self.tab_at(pos.x - self.base.bounds.loc.x) {
                    self.select(i);
                }
                Handled::Yes
            }
            Event::Key {
                key: GridKey::ArrowLeft,
                pressed: true,
            } => {
                self.select(self.selected.get_untracked().saturating_sub(1));
                Handled::Yes
            }
            Event::Key {
                key: GridKey::ArrowRight,
                pressed: true,
            } => {
                let next = (self.selected.get_untracked() + 1).min(self.labels.len() - 1);
                self.select(next);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let Some((tx, tw)) = self.segment(self.selected.get_untracked()) else {
            return false;
        };
        let done = (self.ind_x - tx).abs() < 0.5 && (self.ind_w - tw).abs() < 0.5;
        if done {
            self.ind_x = tx;
            self.ind_w = tw;
            return false;
        }
        let t = (dt / ANIM_DURATION).min(1.0) as f64;
        self.ind_x += (tx - self.ind_x) * t;
        self.ind_w += (tw - self.ind_w) * t;
        // Damage just our own rect so the underline slide doesn't force a full redraw.
        self.base.mark_needs_paint();
        true
    }
}

impl LayoutExt for Tabs {}
