//! [`Item`] — a generic horizontal row: an optional **leading** slot, a label,
//! and an optional **trailing** slot, with hover / selected / activate states.
//! It is the shared building block for menu/dropdown options and sidebar rows
//! (your "list item" with left/right slots).
//!
//! Slots are arbitrary [`Component`]s (an icon widget, a [`Badge`](super::Badge)
//! kbd-hint, a [`StatusDot`](super::StatusDot), a `>` [`Label`](super::Label),
//! …), so `Item` is icon-agnostic — it lays the slots out and draws the row
//! chrome + label, the slots draw themselves. Set [`on_activate`](Item::on_activate)
//! to make it clickable/keyboard-operable (focusable); leave it unset for a
//! display-only row.

use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::effects::Flash;
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Glow, TextAlign};
use crate::style::{Direction, Justify, Length};
use crate::widgets::Flex;
use heca_core::layout::{Point, Rectangle, Size};

/// Row height (logical px).
const ROW_H: f32 = 38.0;
/// Horizontal inner padding.
const PAD_H: f64 = 14.0;
/// Gap between a slot and the label.
const GAP: f64 = 10.0;
/// Label font size.
const FONT_SIZE: f32 = 15.0;
/// Width of the left accent bar shown when selected.
const SEL_BAR_W: f64 = 3.0;
/// Selected-row fill alpha.
const SEL_FILL_ALPHA: u8 = 30;
/// Hover-row fill alpha.
const HOVER_FILL_ALPHA: u8 = 16;

/// Index of the leading / trailing slot within `base.children`.
const LEADING: usize = 0;
const TRAILING: usize = 1;

/// A generic list row with leading/trailing slots and a label.
pub struct Item {
    base: Base,
    label: Signal<String>,
    /// Active/selected styling (accent bar + tinted bg + accent label).
    selected: Signal<bool>,
    /// Render the label in the muted color (e.g. a section header).
    muted: bool,
    hovered: Signal<bool>,
    flash: Flash,
    on_activate: Option<Box<dyn Fn()>>,
}

/// A zero-size placeholder used for an empty slot.
fn spacer() -> Flex {
    Flex::row().width(Length::Px(0.0)).height(Length::Px(0.0))
}

impl Item {
    /// A new row showing `label`, with empty slots.
    pub fn new(label: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.direction = Direction::Row;
        base.style.justify = Justify::SpaceBetween; // leading left, trailing right
        base.style.padding = PAD_H as f32;
        base.style.height = Length::Px(ROW_H);
        base.style.font_size = FONT_SIZE;
        // children[LEADING], children[TRAILING] — replaced by the slot builders.
        base.children.push(Box::new(spacer()));
        base.children.push(Box::new(spacer()));
        Self {
            base,
            label: signal(label.into()),
            selected: signal(false),
            muted: false,
            hovered: signal(false),
            flash: Flash::new(),
            on_activate: None,
        }
    }

    /// Set the leading (left) slot — any component (icon, dot, badge…).
    pub fn leading(mut self, c: impl Component + 'static) -> Self {
        self.base.children[LEADING] = Box::new(c);
        self
    }

    /// Set the trailing (right) slot — any component (kbd hint, `>`, badge…).
    pub fn trailing(mut self, c: impl Component + 'static) -> Self {
        self.base.children[TRAILING] = Box::new(c);
        self
    }

    /// Set the initial selected/active state.
    pub fn selected(self, selected: bool) -> Self {
        self.selected.set(selected);
        self
    }

    /// Render the label muted (section-header style).
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// Make the row clickable/keyboard-activatable (also makes it focusable).
    pub fn on_activate(mut self, f: impl Fn() + 'static) -> Self {
        self.on_activate = Some(Box::new(f));
        self
    }

    /// The selected-state signal — bind UI to it reactively.
    pub fn state(&self) -> Signal<bool> {
        self.selected
    }

    /// The label text signal (set it to update reactively).
    pub fn label_signal(&self) -> Signal<String> {
        self.label
    }

    fn interactive(&self) -> bool {
        self.on_activate.is_some()
    }

    fn contains(&self, p: Point) -> bool {
        self.base.bounds.contains(p)
    }

    fn activate(&mut self) {
        self.flash.trigger();
        if let Some(f) = &self.on_activate {
            f();
        }
    }

    /// The label rect: the gap between the leading and trailing slots.
    fn label_rect(&self) -> Rectangle {
        let b = self.base.bounds;
        let lead = self.base.children[LEADING].base().bounds;
        let trail = self.base.children[TRAILING].base().bounds;
        let lead_w = lead.size.w;
        let trail_w = trail.size.w;
        let x0 = lead.loc.x + lead_w + if lead_w > 0.1 { GAP } else { 0.0 };
        let x1 = trail.loc.x - if trail_w > 0.1 { GAP } else { 0.0 };
        Rectangle::new(Point::new(x0, b.loc.y), Size::new((x1 - x0).max(0.0), b.size.h))
    }
}

impl Component for Item {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn focusable(&self) -> bool {
        self.interactive() && !self.base.disabled.get_untracked()
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let selected = self.selected.get_untracked();
        let (accent, glow_c, foreground, muted_c) = {
            let t = cx.theme();
            (t.accent, t.glow, t.foreground, t.muted)
        };
        let b = self.base.bounds;

        // Row background: tinted when selected, faint on hover.
        if selected {
            cx.rect(b, accent.with_alpha(SEL_FILL_ALPHA), None, 0.0, None);
            // Left accent bar with a glow.
            cx.rect(
                Rectangle::new(b.loc, Size::new(SEL_BAR_W, b.size.h)),
                accent,
                None,
                0.0,
                Some(Glow {
                    color: glow_c,
                    radius: 10.0,
                    intensity: 0.14,
                }),
            );
        } else if self.hovered.get_untracked() {
            cx.rect(b, foreground.with_alpha(HOVER_FILL_ALPHA), None, 0.0, None);
        }

        // Label (state-driven color).
        let color = if selected {
            accent
        } else if self.muted {
            muted_c
        } else {
            foreground
        };
        cx.text(
            self.label_rect(),
            &self.label.get_untracked(),
            color,
            self.base.style.font_size,
            TextAlign::Start,
            selected,
        );

        // Slots (leading + trailing) draw themselves.
        for child in &self.base.children {
            child.paint(cx);
        }

        if self.interactive() && !disabled {
            cx.flash(b, self.flash.amount() * 0.5, 0.0);
        }
        if disabled {
            cx.dim(b, 0.0);
        }
        if self.interactive()
            && !disabled
            && self.base.focus_visible.get_untracked()
            && cx.theme().show_focus_border
        {
            cx.corner_brackets(b, accent);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if !self.interactive() || self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::PointerMoved { pos } => {
                let inside = self.contains(*pos);
                if self.hovered.get_untracked() != inside {
                    self.hovered.set(inside);
                }
                Handled::No
            }
            Event::PointerPressed { pos } if self.contains(*pos) => {
                self.activate();
                Handled::Yes
            }
            Event::Key {
                key: GridKey::Enter | GridKey::Space,
                pressed: true,
            } => {
                self.activate();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = self.flash.tick(dt);
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }
}

impl LayoutExt for Item {}
