//! [`Choice`] — a **selectable container that carries a value and composes arbitrary content**.
//!
//! It is the option primitive: one `Choice` = one alternative the user can pick. [`Select`] mounts
//! them as its dropdown rows and [`Tabs`] as its segments (and, later, `ContextMenu` /
//! `CommandPalette` as their entries) — so the *look* of an option is written once, here, and its
//! *content* is whatever the caller composes.
//!
//! # `Choice` vs [`Item`](super::Item)
//! Both are selectable, both compose their content, both are a single Tab stop. They differ in
//! shape and purpose:
//!
//! | | [`Item`](super::Item) | [`Choice`] |
//! |---|---|---|
//! | Shape | a **row**: leading slot · label · trailing slot, fixed row height | **any content**, hugging it |
//! | Carries | a label | a **value** — the thing chosen |
//! | Use for | sidebar / menu lists that look like rows | choosing among **alternatives** (`Select`, `Tabs`) |
//!
//! Rule of thumb: if you're building a list of rows, use `Item`; if the user is **picking one of
//! several**, use `Choice`.
//!
//! # Content is children
//! `Choice` paints only its own chrome — the selected pill, the hover tint, the press flash, the
//! focus ring, all from the [`Theme`](crate::theme::Theme) — and lets the layout engine place its
//! children, which paint themselves. It publishes its **state color** via
//! [`PaintCx::with_content_color`], so an unstyled [`Label`](super::Label)/[`Icon`](super::Icon)
//! inside tints with the selection and hover automatically, while a child with its own color (a
//! `Badge::danger`) keeps it.

use crate::builders::{LayoutExt, Parent};
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::effects::Flash;
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::style::{Align, Direction, Justify, Length};
use crate::widgets::Label;
use heca_core::layout::Point;

/// Reference inner padding (logical px) at [`WidgetSize::Large`](crate::style::WidgetSize); smaller
/// size variants scale it down via `pad_scale`, so an option in a compact `Select` is tighter than
/// one in a roomy tab strip without either caller computing pixels.
pub(crate) const BASE_PAD: f32 = 8.0;
/// Gap between composed content items (icon ↔ label …), as a fraction of the resolved font — the
/// same half-character rhythm [`Button`](super::Button) uses, so an icon+label option and an
/// icon+label button read identically.
const GAP_RATIO: f32 = 0.5;

/// A selectable option: **a value plus composed content**.
///
/// Construct it with a value and give it content; the widget owns selection chrome and input
/// (click / `Enter` / `Space`), the caller owns what it looks like inside.
///
/// ```ignore
/// // Sugar: a plain text option.
/// Choice::labeled("high", "HIGH")
///
/// // Composed: any tree, any depth. The Icon + Label inherit the option's state color.
/// Choice::new("high")
///     .child(Flex::row().gap(6.0)
///         .child(Icon::new(Glyph::Warning))
///         .child(Label::new("HIGH")))
///     .selected(true)
/// ```
///
/// The **value** is what the choice *means* (`"high"`), independent of what it *shows* (an icon and
/// the word `HIGH`). Its consumers report the chosen option by index; the host maps that index back
/// to this value — which is what lets a declarative author receive `{"value": "high"}` instead of an
/// opaque `1`.
pub struct Choice {
    base: Base,
    /// The value this option stands for. A plain `String` on purpose: `heca-grid-ui` never depends
    /// on the app (or its `PropValue`), so the widget stores the value in the one shape both sides
    /// can speak. The host converts to/from richer types at its boundary.
    value: String,
    /// Selected (chosen) — a tinted pill + accent content.
    selected: Signal<bool>,
    hovered: Signal<bool>,
    flash: Flash,
    on_activate: Option<Box<dyn Fn()>>,
}

#[heca_grid_ui_macros::props]
impl Choice {
    /// A new option standing for `value`, with **no content** — compose it with
    /// [`child`](Parent::child).
    pub fn new(value: impl Into<String>) -> Self {
        let mut base = Base::new();
        // Content is laid out as a centered row; padding/gap derive from the size variant in
        // `remeasure`, and the variant itself cascades to the content during layout.
        base.style.layout.direction = Direction::Row;
        base.style.layout.align = Align::Center;
        base.style.layout.justify = Justify::Start;
        // One option = one Tab stop: focus never descends into the composed content.
        base.focus_barrier = true;
        let mut choice = Self {
            base,
            value: value.into(),
            selected: signal(false),
            hovered: signal(false),
            flash: Flash::new(),
            on_activate: None,
        };
        choice.remeasure();
        choice
    }

    /// Sugar for the common case: an option standing for `value`, showing `label`.
    ///
    /// Builds exactly the child a caller would compose by hand (`Choice::new(v).child(Label::new(l))`)
    /// — there is one content model, not a "simple mode".
    pub fn labeled(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(value).child(Label::new(label))
    }

    /// The value this option stands for.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Set the selected (chosen) state.
    #[heca_grid_ui_macros::prop]
    pub fn selected(self, selected: bool) -> Self {
        self.selected.set(selected);
        self
    }

    /// The selected-state signal — bind it so a container can flip the selection **in place**,
    /// without rebuilding the tree.
    pub fn state(&self) -> Signal<bool> {
        self.selected
    }

    /// The hover-state signal.
    pub fn hovered(&self) -> Signal<bool> {
        self.hovered
    }

    /// Make the option activatable (click / `Enter` / `Space`), which also makes it focusable.
    ///
    /// A container (`Select`, `Tabs`) wires this to record the pick; a standalone `Choice` can use
    /// it directly.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_activate(mut self, f: impl Fn() + 'static) -> Self {
        self.on_activate = Some(Box::new(f));
        self.base.focusable = true; // interactive options are focusable (Component::focusable)
        self
    }

    fn interactive(&self) -> bool {
        self.on_activate.is_some()
    }

    fn activate(&mut self) {
        self.flash.trigger();
        if let Some(f) = &self.on_activate {
            f();
        }
    }
}

impl Component for Choice {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// The option **hugs its content**: `Auto` in both axes, so the engine measures whatever tree it
    /// holds (a label, an icon + label, a two-line column). Only padding and gap are its own, and
    /// both derive from the resolved font + size variant, so the whole affordance scales together.
    fn remeasure(&mut self) {
        let pad = BASE_PAD * self.base.size_scale();
        self.base.style.layout.padding = pad;
        self.base.style.layout.gap = self.base.font * GAP_RATIO;
        self.base.style.layout.width = Length::Auto;
        self.base.style.layout.height = Length::Auto;
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let selected = self.selected.get_untracked();
        let (accent, foreground, radius, ia) = {
            let t = cx.theme();
            (
                t.colors.accent,
                t.colors.foreground,
                t.colors.control_radius(),
                t.colors.interaction,
            )
        };
        let b = self.base.bounds;

        // Chrome: a tinted pill when chosen, a faint one on hover. Same interaction tokens as
        // `Item`, so an option and a list row read as the same family.
        if selected {
            cx.rect(b, accent.with_alpha(ia.row_active_fill), None, radius, None);
        } else if self.hovered.get_untracked() {
            cx.rect(b, foreground.with_alpha(ia.row_hover_fill), None, radius, None);
        }

        // The composed content paints itself, under the option's state color: chosen → accent,
        // otherwise the foreground. Unstyled `Label`/`Icon` children inherit it (a child with its
        // own color keeps that), which is what makes an icon+label option tint as one thing.
        let content_color = if selected { accent } else { foreground };
        cx.with_content_color(content_color, |cx| {
            for child in &self.base.children {
                child.paint(cx);
            }
        });

        if self.interactive() && !disabled {
            cx.flash(b, self.flash.amount() * 0.5, radius);
        }
        if disabled {
            cx.dim(b, radius);
        }
        if self.interactive()
            && !disabled
            && self.base.shows_focus_ring()
            && cx.theme().colors.show_focus_border
        {
            let ring = cx.theme().colors.effective_focus_ring();
            cx.focus_ring(b, ring, radius);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if !self.interactive() || self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::PointerMoved { pos } => {
                let inside = self.base.bounds.contains(*pos);
                if self.hovered.get_untracked() != inside {
                    self.hovered.set(inside);
                }
                Handled::No
            }
            Event::PointerPressed { pos } if self.base.bounds.contains(*pos) => {
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
        // Composed content animates itself (e.g. a Spinner child).
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        if animating {
            self.base.mark_needs_paint();
        }
        animating
    }
}

impl LayoutExt for Choice {}
/// Content is children: `.child(..)` appends any component (the [`labeled`](Choice::labeled) sugar
/// builds the same child).
impl Parent for Choice {}

/// Hit-test helper used by containers that lay their options out themselves (a `Select` panel):
/// the index of the option whose **bounds** contain `pos`.
///
/// Containers must resolve picks from the children's real bounds — never from row arithmetic — so
/// that what is drawn and what is clickable can never disagree.
pub fn choice_at(children: &[Box<dyn Component>], pos: Point) -> Option<usize> {
    children
        .iter()
        .position(|c| c.base().bounds.contains(pos) && c.base().visible.get_untracked())
}
