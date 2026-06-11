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
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Glow, TextAlign};
use crate::style::{Align, Direction, Justify, Length};
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
/// Width of the left accent bar shown when active.
const ACTIVE_BAR_W: f64 = 3.0;
/// Active left bar height as a fraction of the row (centered, not full height).
const ACTIVE_BAR_FRAC: f64 = 0.65;
/// Active-row fill alpha.
const ACTIVE_FILL_ALPHA: u8 = 30;
/// Hover-row fill alpha.
const HOVER_FILL_ALPHA: u8 = 16;
/// Padding the optional slot border adds around the slot content.
const SLOT_BORDER_PAD_X: f64 = 7.0;
const SLOT_BORDER_PAD_Y: f64 = 4.0;

/// Inset of the active/hover selection pill from the row edges, so its corners
/// never contend with a rounded container's corners.
const SEL_INSET: f64 = 4.0;

/// Size of the [`ActiveMarker::Check`] pip (logical px).
const CHECK_SIZE: f64 = 10.0;

/// Index of the leading / trailing slot within `base.children`.
const LEADING: usize = 0;
const TRAILING: usize = 1;

/// How an [`Item`]'s active state is indicated. Set per context; the row carries
/// the `active` bool, the marker decides how it's shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveMarker {
    /// No marker — only the tinted bg + accent label (default; plain rows /
    /// dropdown options without a bar).
    #[default]
    None,
    /// Vivid left bar — the sidebar/nav current item.
    Bar,
    /// A pip in the left gutter — a selected menu/dropdown option (works for
    /// multi-select, where several rows are active at once).
    Check,
}

/// A generic list row with leading/trailing slots and a label.
pub struct Item {
    base: Base,
    label: Signal<String>,
    /// Active (clicked-and-stays current item): tinted bg + accent label, plus
    /// an optional indicator controlled by [`ActiveMarker`].
    active: Signal<bool>,
    /// How the active state is visually indicated — context-dependent.
    marker: ActiveMarker,
    /// Render the label in the muted color (e.g. a section header).
    muted: bool,
    /// Draw a rounded border around the leading / trailing slot (chip style).
    leading_border: bool,
    trailing_border: bool,
    hovered: Signal<bool>,
    flash: Flash,
    on_activate: Option<Box<dyn Fn()>>,
}

impl Item {
    /// A new row showing `label`, with empty slots.
    pub fn new(label: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.direction = Direction::Row;
        base.style.justify = Justify::SpaceBetween; // leading left, trailing right
        base.style.align = Align::Center; // center slots vertically (kbd hint, dot)
        base.style.padding = PAD_H as f32;
        base.style.height = Length::Px(ROW_H);
        // children[LEADING], children[TRAILING] — replaced by the slot builders.
        base.children.push(Box::new(Flex::empty()));
        base.children.push(Box::new(Flex::empty()));
        Self {
            base,
            label: signal(label.into()),
            active: signal(false),
            marker: ActiveMarker::None,
            muted: false,
            leading_border: false,
            trailing_border: false,
            hovered: signal(false),
            flash: Flash::new(),
            on_activate: None,
        }
    }

    /// Explicit label font size — overrides the inherited theme font.
    pub fn font_size(mut self, fs: f32) -> Self {
        self.base.style.font_size = fs;
        self.base.font = fs;
        self.remeasure();
        self
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

    /// Set the active state — the clicked-and-stays current item (tinted bg +
    /// accent label + optional indicator).
    pub fn active(self, active: bool) -> Self {
        self.active.set(active);
        self
    }

    /// Set how the active state is visually indicated. Defaults to
    /// [`ActiveMarker::None`] (tinted bg + accent label only).
    pub fn marker(mut self, marker: ActiveMarker) -> Self {
        self.marker = marker;
        self
    }

    /// Render the label muted (section-header style).
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// Draw a rounded border (chip frame) around the leading slot.
    pub fn leading_bordered(mut self, bordered: bool) -> Self {
        self.leading_border = bordered;
        self
    }

    /// Draw a rounded border (chip frame) around the trailing slot — e.g. a
    /// keymap hint like `⌘P`.
    pub fn trailing_bordered(mut self, bordered: bool) -> Self {
        self.trailing_border = bordered;
        self
    }

    /// Make the row clickable/keyboard-activatable (also makes it focusable).
    pub fn on_activate(mut self, f: impl Fn() + 'static) -> Self {
        self.on_activate = Some(Box::new(f));
        self
    }

    /// The active-state signal — bind UI to it reactively.
    pub fn state(&self) -> Signal<bool> {
        self.active
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
        Rectangle::new(
            Point::new(x0, b.loc.y),
            Size::new((x1 - x0).max(0.0), b.size.h),
        )
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

    /// Row height scales with the resolved font (keeps the default 38px at 15px).
    fn remeasure(&mut self) {
        self.base.style.height = Length::Px(self.base.font * (ROW_H / FONT_SIZE));
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let active = self.active.get_untracked();
        let (accent, glow_c, foreground, muted_c, border_c, ctrl_radius, bw) = {
            let t = cx.theme();
            (t.accent, t.glow, t.foreground, t.muted, t.border, t.control_radius(), t.border_width)
        };
        let b = self.base.bounds;

        // Row background: tinted when active, faint on hover. Drawn as an *inset*
        // selection pill (not full-bleed) so its rounded corners never contend
        // with a rounded container's corners at any radius — a full-bleed fill in
        // a heavily-rounded Pane leaves notches at the corners.
        let sel = Rectangle::new(
            Point::new(b.loc.x + SEL_INSET, b.loc.y + SEL_INSET),
            Size::new(
                (b.size.w - 2.0 * SEL_INSET).max(0.0),
                (b.size.h - 2.0 * SEL_INSET).max(0.0),
            ),
        );
        let sel_radius = ctrl_radius.min((sel.size.h / 2.0) as f32);
        if active {
            cx.rect(sel, accent.with_alpha(ACTIVE_FILL_ALPHA), None, sel_radius, None);
        } else if self.hovered.get_untracked() {
            cx.rect(sel, foreground.with_alpha(HOVER_FILL_ALPHA), None, sel_radius, None);
        }

        // Active indicator — depends on marker.
        if active {
            match self.marker {
                ActiveMarker::Bar => {
                    // Vivid left bar — centered, ~65% of row height.
                    let bar_h = b.size.h * ACTIVE_BAR_FRAC;
                    let bar_y = b.loc.y + (b.size.h - bar_h) / 2.0;
                    cx.rect(
                        Rectangle::new(
                            Point::new(b.loc.x, bar_y),
                            Size::new(ACTIVE_BAR_W, bar_h),
                        ),
                        accent,
                        None,
                        (ACTIVE_BAR_W / 2.0) as f32,
                        Some(Glow {
                            color: glow_c,
                            radius: 8.0,
                            intensity: 0.16,
                        }),
                    );
                }
                ActiveMarker::Check => {
                    // Small accent pip centered in the left gutter.
                    let pip_x = b.loc.x + PAD_H / 2.0 - CHECK_SIZE / 2.0;
                    let pip_y = b.loc.y + (b.size.h - CHECK_SIZE) / 2.0;
                    cx.rect(
                        Rectangle::new(
                            Point::new(pip_x, pip_y),
                            Size::new(CHECK_SIZE, CHECK_SIZE),
                        ),
                        accent,
                        None,
                        (CHECK_SIZE / 2.0) as f32,
                        None,
                    );
                }
                ActiveMarker::None => {}
            }
        }

        // Label (state-driven color).
        let color = if active {
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
            self.base.font,
            TextAlign::Start,
            active,
        );

        // Optional chip border around a present slot (drawn behind its content).
        let slot_border = |cx: &mut PaintCx, slot: Rectangle| {
            if slot.size.w <= 0.1 {
                return;
            }
            let frame = Rectangle::new(
                Point::new(
                    slot.loc.x - SLOT_BORDER_PAD_X,
                    slot.loc.y - SLOT_BORDER_PAD_Y,
                ),
                Size::new(
                    slot.size.w + 2.0 * SLOT_BORDER_PAD_X,
                    slot.size.h + 2.0 * SLOT_BORDER_PAD_Y,
                ),
            );
            cx.rect(
                frame,
                crate::color::Color::TRANSPARENT,
                Some(crate::scene::Border { color: border_c, width: bw }),
                ctrl_radius,
                None,
            );
        };
        if self.leading_border {
            slot_border(cx, self.base.children[LEADING].base().bounds);
        }
        if self.trailing_border {
            slot_border(cx, self.base.children[TRAILING].base().bounds);
        }

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
        // Damage our own rect (which contains our slots) so a press flash or an
        // animating child doesn't force a full redraw.
        if animating {
            self.base.mark_needs_paint();
        }
        animating
    }
}

impl LayoutExt for Item {}
