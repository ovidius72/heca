//! [`Item`] — a generic horizontal row: an optional **leading** slot, a label,
//! and an optional **trailing** slot, with hover / selected / activate states.
//! It is the shared building block for menu/dropdown options and sidebar rows
//! (your "list item" with left/right slots).
//!
//! Slots are arbitrary [`Component`]s (an icon widget, a [`Badge`](super::Badge)
//! kbd-hint, a [`StatusDot`](super::StatusDot), a `>` [`Label`],
//! …), so `Item` is icon-agnostic — it lays the slots out and draws the row
//! chrome + label, the slots draw themselves. Set [`on_click`](crate::builders::EventExt::on_click)
//! to make it clickable/keyboard-operable (focusable); leave it unset for a
//! display-only row.

use crate::builders::LayoutExt;
use crate::component::{Base, Component, PaintCx};
use crate::effects::Flash;
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Glow, TextAlign};
use crate::style::{Align, Direction, Justify, Space, Spacing};
use crate::widgets::{Flex, Label};
use heca_core::layout::{Point, Rectangle, Size};

/// Inner padding at the sides — where a row's content starts.
const PAD_X: Spacing = Spacing::Md;
/// Inner padding above and below. A row has no fixed height: it is its content plus this, as a
/// CSS box is, so it follows the font and a taller slot.
const PAD_Y: Spacing = Spacing::Sm;
/// **Where a row's content starts**, from its own left edge. A part placed beside rows — a
/// [`DockFrame`](super::DockFrame)'s leading part or a title row of its caller's — starts here
/// too, so its left edge lines up with every row's content.
pub(crate) const ROW_INSET: Spacing = PAD_X;
/// **A row's padding above and below.** A title row written by a [`DockFrame`](super::DockFrame)'s
/// caller takes it too, so it is as tall as a Dock's own title row (an `Item`) for the same content.
pub(crate) const ROW_PAD_Y: Spacing = PAD_Y;
/// Gap between a slot and the label.
const GAP: Spacing = Spacing::Sm;
/// Width of the left accent bar shown when active.
const ACTIVE_BAR_W: f64 = 3.0;
/// Active left bar height as a fraction of the row (centered, not full height).
const ACTIVE_BAR_FRAC: f64 = 0.65;
/// The active bar's halo, as shares of the theme's glow radius and strength.
const BAR_GLOW_RADIUS_SCALE: f32 = 0.8;
const BAR_GLOW_STRENGTH_SCALE: f32 = 0.3;
/// Padding the optional slot border adds around the slot content.
const SLOT_BORDER_PAD_X: Spacing = Spacing::Sm;
const SLOT_BORDER_PAD_Y: Spacing = Spacing::Xs;

/// Inset of the active/hover selection pill from the row edges, so its corners
/// never contend with a rounded container's corners.
const SEL_INSET: Spacing = Spacing::Xs;

/// Size of the [`ActiveMarker::Check`] pip (logical px).
const CHECK_SIZE: f64 = 10.0;

/// Index of the leading slot / the label / the trailing slot within `base.children`.
///
/// The label is a real [`Label`] child (not text drawn by `Item`), so the row is composed like
/// every other widget: the engine lays the three out, each paints itself, and the label can be
/// swapped or extended without touching `Item::paint`.
const LEADING: usize = 0;
const LABEL: usize = 1;
const TRAILING: usize = 2;

/// How an [`Item`]'s active state is indicated. Set per context; the row carries
/// the `active` bool, the marker decides how it's shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
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
///
/// **Content is composed**: the label is a [`Label`] child and the slots are arbitrary components,
/// so the row draws only its own chrome (selection pill, marker, hover/press, focus ring) and lets
/// the engine lay the content out. The row's **state color** (active → accent, muted → muted) is
/// published once per paint via [`PaintCx::with_content_color`] and inherited by the label; its
/// **bold-when-active** weight rides the label's own weight signal.
pub struct Item {
    base: Base,
    /// The label child's text signal (the child owns the text; this is the handle callers get).
    label: Signal<String>,
    /// The label child's weight signal — set in `tick` when `active` changes, because weight
    /// (unlike color) is not part of the inherited paint context.
    label_weight: Signal<crate::style::FontWeight>,
    /// Last `active` value seen by `tick`, so the weight signal is written only on a real change.
    seen_active: bool,
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
    flash: Flash,
}

#[heca_grid_ui_macros::props]
impl Item {
    /// A new row showing `label`, with empty slots.
    pub fn new(label: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.layout.direction = Direction::Row;
        base.style.layout.justify = Justify::Start; // the growing label pushes the trailing slot right
        base.style.layout.align = Align::Center; // center slots vertically (kbd hint, dot)
        base.style.layout.padding = PAD_Y.into();
        base.style.layout.padding_x = Some(PAD_X.into());
        base.style.layout.gap = GAP.into();
        // One control = one Tab stop: focus never descends into the composed content.
        base.focus_barrier = true;

        // children[LEADING], children[LABEL], children[TRAILING]. The slots are placeholders until
        // a builder replaces them; the label is a real child that grows to take the middle, so the
        // text starts hard left after the leading slot and the trailing slot sits at the far edge.
        let label = Label::new(label).align(TextAlign::Start).grow(1.0);
        let text = label.text_signal();
        let label_weight = label.font_weight_signal();
        // An empty slot is out of the layout, as an absent element is: otherwise the row's gap is
        // spent on nothing and the label of a row without a leading part starts one gap further
        // right than every other row's content.
        let empty_slot = || {
            let mut slot = Flex::empty();
            slot.base_mut().set_hidden(true);
            crate::WidgetModel::new(slot)
        };
        base.children.push(empty_slot());
        base.children.push(crate::WidgetModel::new(label));
        base.children.push(empty_slot());

        Self {
            base,
            label: text,
            label_weight,
            seen_active: false,
            active: signal(false),
            marker: ActiveMarker::None,
            muted: false,
            leading_border: false,
            trailing_border: false,
            flash: Flash::new(),
        }
    }

    /// Explicit label font size — overrides the inherited theme font.
    ///
    /// Applied to the label **child** as well: an explicit `font_size` pins one node only (unlike
    /// the size *variant*, which the layout pass inherits down the tree), so the row and its text
    /// would otherwise disagree.
    #[heca_grid_ui_macros::prop]
    pub fn font_size(mut self, fs: f32) -> Self {
        self.base.style.visual.font_size = fs;
        self.base.font = fs;
        let label = self.base.children[LABEL].base_mut();
        label.style.visual.font_size = fs;
        label.font = fs;
        self.remeasure();
        self
    }

    /// Set the leading (left) slot — any component (icon, dot, badge…).
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn leading(mut self, c: impl crate::builders::IntoComponent) -> Self {
        self.base.children[LEADING] = c.into_component();
        self
    }

    /// Set the trailing (right) slot — any component (kbd hint, `>`, badge…).
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn trailing(mut self, c: impl crate::builders::IntoComponent) -> Self {
        self.base.children[TRAILING] = c.into_component();
        self
    }

    /// Set the active state — the clicked-and-stays current item (tinted bg +
    /// accent label + optional indicator).
    #[heca_grid_ui_macros::prop]
    pub fn active(self, active: bool) -> Self {
        self.active.set(active);
        self
    }

    /// Set how the active state is visually indicated. Defaults to
    /// [`ActiveMarker::None`] (tinted bg + accent label only).
    #[heca_grid_ui_macros::prop]
    pub fn marker(mut self, marker: ActiveMarker) -> Self {
        self.marker = marker;
        self
    }

    /// Render the label muted (section-header style).
    #[heca_grid_ui_macros::prop]
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// Draw a rounded border (chip frame) around the leading slot.
    #[heca_grid_ui_macros::prop]
    pub fn leading_bordered(mut self, bordered: bool) -> Self {
        self.leading_border = bordered;
        self
    }

    /// Draw a rounded border (chip frame) around the trailing slot — e.g. a
    /// keymap hint like `⌘P`.
    #[heca_grid_ui_macros::prop]
    pub fn trailing_bordered(mut self, bordered: bool) -> Self {
        self.trailing_border = bordered;
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
        self.base.activatable
    }
}

impl Component for Item {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let active = self.active.get_untracked();
        let (accent, glow_c, foreground, muted_c, border_c, ctrl_radius, bw) = {
            let t = cx.theme();
            (
                cx.accent(),
                t.colors.glow,
                t.colors.foreground,
                t.colors.muted,
                t.colors.border,
                t.colors.control_radius(),
                t.colors.border_width,
            )
        };
        let b = self.base.bounds;
        let font = self.base.font;
        let step = |s: Spacing| Space::from(s).resolve(font) as f64;

        // Row background: tinted when active, faint on hover. Drawn as an *inset* selection pill
        // (not full-bleed) so its rounded corners never contend with a rounded container's corners
        // at any radius — a full-bleed fill in a heavily-rounded Pane leaves notches at the corners.
        // Clamped to this item's own padding so the pill never crops its content
        // (`Base::highlight_rect`).
        let sel = self.base.highlight_rect(step(SEL_INSET));
        let sel_radius = ctrl_radius.min((sel.size.h / 2.0) as f32);
        if active {
            cx.rect(
                sel,
                accent.with_alpha(cx.theme().colors.interaction.row_active_fill),
                None,
                sel_radius,
                None,
            );
        } else if self.base.hovered() {
            cx.rect(
                sel,
                foreground.with_alpha(cx.theme().colors.interaction.row_hover_fill),
                None,
                sel_radius,
                None,
            );
        }

        // Active indicator — depends on marker.
        if active {
            match self.marker {
                ActiveMarker::Bar => {
                    // Vivid left bar — centered, ~65% of row height.
                    let bar_h = b.size.h * ACTIVE_BAR_FRAC;
                    let bar_y = b.loc.y + (b.size.h - bar_h) / 2.0;
                    cx.rect(
                        Rectangle::new(Point::new(b.loc.x, bar_y), Size::new(ACTIVE_BAR_W, bar_h)),
                        accent,
                        None,
                        (ACTIVE_BAR_W / 2.0) as f32,
                        Some(Glow {
                            color: glow_c,
                            radius: cx.theme().colors.active_glow_radius * BAR_GLOW_RADIUS_SCALE,
                            intensity: cx.theme().colors.active_glow_strength
                                * BAR_GLOW_STRENGTH_SCALE,
                        }),
                    );
                }
                ActiveMarker::Check => {
                    // Small accent pip centered in the left gutter.
                    let pip_x = b.loc.x + step(PAD_X) / 2.0 - CHECK_SIZE / 2.0;
                    let pip_y = b.loc.y + (b.size.h - CHECK_SIZE) / 2.0;
                    cx.rect(
                        Rectangle::new(Point::new(pip_x, pip_y), Size::new(CHECK_SIZE, CHECK_SIZE)),
                        accent,
                        None,
                        (CHECK_SIZE / 2.0) as f32,
                        None,
                    );
                }
                ActiveMarker::None => {}
            }
        }

        // The row's state color. It is not applied to anything here: it is *published*, and the
        // composed content (the label, and any unstyled icon in a slot) inherits it — the same
        // mechanism a Button uses for its hover/disabled content. Bold-when-active rides the
        // label's own signal instead (weight is not part of the inherited context) — see `tick`.
        let content_color = if active {
            accent
        } else if self.muted {
            muted_c
        } else {
            foreground
        };

        // Optional chip border around a present slot (drawn behind its content).
        let (border_x, border_y) = (step(SLOT_BORDER_PAD_X), step(SLOT_BORDER_PAD_Y));
        let slot_border = |cx: &mut PaintCx, slot: Rectangle| {
            if slot.size.w <= 0.1 {
                return;
            }
            let frame = Rectangle::new(
                Point::new(slot.loc.x - border_x, slot.loc.y - border_y),
                Size::new(slot.size.w + 2.0 * border_x, slot.size.h + 2.0 * border_y),
            );
            cx.rect(
                frame,
                crate::color::Color::TRANSPARENT,
                Some(crate::scene::Border {
                    color: border_c,
                    width: bw,
                }),
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

        // The composed content (leading slot, label, trailing slot) draws itself, under the row's
        // state color.
        cx.with_content_color(content_color, |cx| {
            for child in &self.base.children {
                crate::component::paint_child(child.as_ref(), cx);
            }
        });

        if self.interactive() && !disabled {
            cx.flash(b, self.flash.amount() * 0.5, 0.0);
        }
        if disabled {
            cx.dim(b, 0.0);
        }
        if self.interactive()
            && !disabled
            && self.base.shows_focus_ring()
            && cx.theme().colors.show_focus_border
        {
            let ring = cx.theme().colors.effective_focus_ring();
            cx.focus_ring(b, ring, ctrl_radius);
        }
    }

    /// Used: the press flash, shown only when this control is the one that used the event.
    fn show_press(&mut self) {
        self.flash.trigger();
    }

    fn is_control(&self) -> bool {
        true
    }

    fn tick(&mut self, dt: f32) -> bool {
        // The active row bolds its label. Color is inherited at paint time, but weight is not part
        // of the paint context, so the row drives the label's own weight signal — written only on a
        // real change, so a stable row never marks itself dirty.
        let active = self.active.get_untracked();
        if active != self.seen_active {
            self.seen_active = active;
            self.label_weight.set(if active {
                crate::style::FontWeight::Bold
            } else {
                crate::style::FontWeight::Regular
            });
        }

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
