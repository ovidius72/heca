//! [`Tabs`] — a horizontal segmented selector with an **animated underline** that slides to the
//! active tab. A change widget: selecting a tab emits
//! `Action::value("tab-change", SignalData::Usize(index))`.
//!
//! # Composed, not hand-drawn
//! The segments are **[`Choice`] children** — the same option primitive [`Select`](super::Select)
//! mounts as its dropdown rows — so a tab can be anything: a word, an icon + a label, a label with a
//! count [`Badge`](super::Badge). `Tabs` owns only the strip's own chrome: the sliding underline and
//! the focus ring. Each tab draws itself and tints its own content.
//!
//! The underline slides between the **selected child's real bounds**. It follows whatever the tab
//! actually is — no monospace metrics, no segment arithmetic — so it is correct for a tab that
//! holds an icon or a badge, which a char-count could never have measured.
//!
//! **Keyboard nav is host-configured, not hardcoded** (`widget-keys-config`): as a **horizontal**
//! selector it moves selection on the semantic [`Event::Widget`] intents `ItemPrevious`/`ItemNext`
//! (left/right). The host resolves the configurable `item_previous`/`item_next` keys into it
//! (defaults ←/`Ctrl+h` → previous, →/`Ctrl+l` → next). Reuses
//! [`Base::disabled`](crate::component::Base) and the focus-visible ring; the strip is **one Tab
//! stop** ([`Base::focus_barrier`](crate::component::Base)), and the underline animates via
//! [`Component::tick`].

use crate::action::{Action, SignalData};
use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, Handled, PaintCx, WidgetIntent};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::Glow;
use crate::style::{Align, Length};
use crate::widgets::choice::{choice_at, Choice};
use heca_core::layout::{Point, Rectangle, Size};

/// Gap between tabs.
const TAB_GAP: f32 = 6.0;
/// Underline thickness.
const UNDERLINE_H: f64 = 2.0;
/// Gap between a tab's content and its underline. Reserved as a bottom margin on the tabs, so the
/// strip measures to "the tallest tab + the underline band" without either the widget or the caller
/// computing a height.
const UNDERLINE_GAP: f32 = 2.0;
/// Seconds for the underline to slide between tabs.
const ANIM_DURATION: f32 = 0.12;
/// Underline glow radius (px).
const GLOW_RADIUS: f32 = 12.0;
/// Underline glow intensity.
const GLOW_INTENSITY: f32 = 0.12;

/// A segmented tab selector over [`Choice`] tabs.
///
/// ```ignore
/// // Sugar — plain text tabs (each becomes a `Choice::labeled(text, text)`).
/// Tabs::new(["FILES", "SEARCH"]).selected(1).on_change(|a| …)
///
/// // Composed — a tab is a value plus any content.
/// Tabs::empty()
///     .tab(Choice::new("files").child(Icon::new(Glyph::Folder)).child(Label::new("FILES")))
///     .tab(Choice::new("issues").child(Label::new("ISSUES")).child(Badge::new("3")))
/// ```
pub struct Tabs {
    base: Base,
    /// Selected-state signal of each tab, captured when the [`Choice`] is added (afterwards the tabs
    /// are `Box<dyn Component>` and their type — with this signal — is out of reach). Keeping them
    /// is what lets the strip flip the selection **in place**, with no rebuild.
    tab_states: Vec<Signal<bool>>,
    /// Hover-state signal of each tab, same story: the strip resolves the hover from the tabs'
    /// bounds and pushes it down, so a tab never has to hit-test itself.
    hover_states: Vec<Signal<bool>>,
    selected: Signal<usize>,
    /// Animated underline (x + width, relative to the strip's origin so it survives a scroll).
    /// `None` until the first layout has given the tabs their bounds — then it snaps to the
    /// selected one instead of sliding in from nowhere.
    underline: Option<(f64, f64)>,
    on_change: Option<Box<dyn Fn(Action)>>,
}

#[heca_grid_ui_macros::props]
impl Tabs {
    /// An empty tab strip — add tabs with [`tab`](Tabs::tab).
    pub fn empty() -> Self {
        let mut base = Base::new();
        base.focusable = true; // keyboard-focusable when enabled (Component::focusable)
        // One control = one Tab stop: focus never descends into the tabs.
        base.focus_barrier = true;
        base.style.layout.gap = (TAB_GAP).into();
        // Hug the tabs instead of stretching to fill a column parent (the width is `Auto`, and the
        // default cross-axis alignment is `Stretch`).
        base.style.layout.align_self = Some(Align::Start);
        let mut tabs = Self {
            base,
            tab_states: Vec::new(),
            hover_states: Vec::new(),
            selected: signal(0),
            underline: None,
            on_change: None,
        };
        tabs.remeasure();
        tabs
    }

    /// New tabs from plain-text `labels`; the first is selected.
    ///
    /// Sugar: each string becomes a [`Choice::labeled`] child whose **value is the text itself**. It
    /// builds exactly the tree [`tab`](Tabs::tab) would — there is one tab model, not a "simple
    /// mode".
    pub fn new(labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut tabs = Self::empty();
        for label in labels {
            let text: String = label.into();
            tabs = tabs.tab(Choice::labeled(text.clone(), text));
        }
        tabs
    }

    /// Append a composed tab.
    ///
    /// Typed to [`Choice`] on purpose — the strip keeps the tab's selected/hover
    /// [signals](Choice::state) so it can drive them in place, and a `Box<dyn Component>` would have
    /// thrown them away. It is also the contract: the segments of a tab strip **are** options.
    #[heca_grid_ui_macros::host_only("a composed value, not a scalar — built from `children`")]
    pub fn tab(mut self, choice: Choice) -> Self {
        self.tab_states.push(choice.state());
        self.hover_states.push(choice.hovered());
        self.base.children.push(Box::new(choice));
        self.sync_tab_states();
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

    /// Select an initial tab (clamped to the tab count). Call it **after** the tabs.
    #[heca_grid_ui_macros::prop]
    pub fn selected(self, index: usize) -> Self {
        let i = index.min(self.count().saturating_sub(1));
        self.selected.set(i);
        self.sync_tab_states();
        self
    }

    /// Set the change handler. Receives `Action::value("tab-change",
    /// SignalData::Usize(index))` when the active tab changes.
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

    /// The selected tab's text — its [accessible name](Component::text_summary), computed from the
    /// tab's content. Empty when the tab has no text (an icon-only tab).
    pub fn selected_label(&self) -> String {
        self.base
            .children
            .get(self.selected.get_untracked())
            .and_then(|c| c.text_summary())
            .unwrap_or_default()
    }

    /// Number of tabs.
    fn count(&self) -> usize {
        self.base.children.len()
    }

    /// Push the selection down into the tabs: exactly one carries the selected state, and it tints
    /// its own content (accent) through the [`Choice`]. Idempotent — it writes only on a real
    /// change, so every path that could have moved the selection can just call it.
    fn sync_tab_states(&self) {
        let selected = self.selected.get_untracked();
        for (i, state) in self.tab_states.iter().enumerate() {
            let want = i == selected;
            if state.get_untracked() != want {
                state.set(want);
            }
        }
    }

    /// The underline's target for the selected tab: `(x relative to the strip, width)` — read
    /// straight off that tab's **bounds**, so the indicator matches whatever the tab is.
    fn underline_target(&self) -> Option<(f64, f64)> {
        let tab = self.base.children.get(self.selected.get_untracked())?;
        let b = tab.base().bounds;
        (b.size.w > 0.0).then_some((b.loc.x - self.base.bounds.loc.x, b.size.w))
    }

    /// The tab under `pos`, resolved from the tabs' **real bounds** — never from segment arithmetic,
    /// so a click can only ever land on the tab the user sees there.
    fn tab_at(&self, pos: Point) -> Option<usize> {
        choice_at(&self.base.children, pos)
    }

    fn select(&mut self, i: usize) {
        if i == self.selected.get_untracked() || i >= self.count() {
            return;
        }
        self.selected.set(i);
        self.sync_tab_states();
        if let Some(f) = &self.on_change {
            f(Action::value("tab-change", SignalData::Usize(i)));
        }
    }
}

impl Tabs {
    /// Light exactly one tab (or none).
    fn set_hover(&self, hit: Option<usize>) {
        for (i, hovered) in self.hover_states.iter().enumerate() {
            let want = hit == Some(i);
            if hovered.get_untracked() != want {
                hovered.set(want);
            }
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

    /// The strip **hugs its tabs** in both axes: the engine measures them (each [`Choice`] hugs its
    /// own content, icons and badges included), so there is no width to compute here and no
    /// char-count estimate to get wrong.
    ///
    /// The underline band is reserved as a **bottom margin on the tabs**, which makes the strip
    /// measure to "the tallest tab + the band" — a padding on the strip could not do that without
    /// also insetting the tabs, and the underline has to sit *under* them.
    fn remeasure(&mut self) {
        self.base.style.layout.width = Length::Auto;
        self.base.style.layout.height = Length::Auto;
        self.base.style.layout.gap = (TAB_GAP * self.base.size_scale()).into();
        let band = (UNDERLINE_H as f32 + UNDERLINE_GAP) * self.base.size_scale();
        for child in self.base.children.iter_mut() {
            child.base_mut().style.layout.margin_bottom = Some(band.into());
        }
    }

    /// The tabs have just been (re)laid out — snap the underline to the selected one if it has never
    /// been placed (a first frame should not animate in from the origin).
    fn on_layout(&mut self) {
        if self.underline.is_none() {
            self.underline = self.underline_target();
        }
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let (accent, glow_c) = {
            let t = cx.theme();
            (cx.accent(), t.colors.glow)
        };
        let b = self.base.bounds;

        // The tabs paint themselves: their pill, their hover tint, and their content in the state
        // color the `Choice` publishes (accent when selected). The strip adds only its own chrome.
        for tab in &self.base.children {
            crate::component::paint_child(tab.as_ref(), cx);
        }

        // Animated underline under the active tab — sized and placed from that tab's real bounds.
        if let Some((x, w)) = self.underline {
            let underline = Rectangle::new(
                Point::new(b.loc.x + x, b.loc.y + b.size.h - UNDERLINE_H),
                Size::new(w, UNDERLINE_H),
            );
            let glow = (!disabled).then_some(Glow {
                color: glow_c,
                radius: GLOW_RADIUS,
                intensity: GLOW_INTENSITY,
            });
            cx.rect(underline, accent, None, 0.0, glow);
        }

        if disabled {
            cx.dim(b, 0.0);
        }
        if !disabled && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            let ring = cx.theme().colors.effective_focus_ring();
            let r = cx.theme().colors.control_radius();
            cx.focus_ring(b, ring, r);
        }
    }

    /// Capture, not bubble: the strip is **one Tab stop and one click target**, so a tab is picked
    /// by `tab_at` here rather than by its `Choice` child consuming the press on its own.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            // Per-**tab** hover: the tabs are rects this widget draws, not children, so
            // `Base::hovered` (true anywhere over the strip) cannot say which one. The move only
            // arrives when the pointer is over the strip, and a leave clears every tab.
            Event::PointerMove(p) => {
                self.set_hover(self.tab_at(p.pos));
                Handled::No
            }
            Event::PointerLeave(_) => {
                self.set_hover(None);
                Handled::No
            }
            Event::PointerDown(_) => Handled::Yes,
            Event::Click(p) => {
                if let Some(i) = self.tab_at(p.pos) {
                    self.select(i);
                }
                Handled::Yes
            }
            // Host-resolved navigation: a **horizontal** selector, so it moves on the
            // `ItemPrevious`/`ItemNext` intents (left/right — ←/→, `Ctrl+h`/`Ctrl+l`), not the
            // vertical `Menu*`. No literal keys live here; the host maps `[keys.widgets]` to this.
            Event::Widget(WidgetIntent::ItemPrevious) => {
                self.select(self.selected.get_untracked().saturating_sub(1));
                Handled::Yes
            }
            Event::Widget(WidgetIntent::ItemNext) => {
                let next = (self.selected.get_untracked() + 1).min(self.count().saturating_sub(1));
                self.select(next);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        // The tabs animate themselves (a Spinner or a flashing Badge inside one).
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }

        let (Some((tx, tw)), Some((x, w))) = (self.underline_target(), self.underline) else {
            return animating;
        };
        if (x - tx).abs() < 0.5 && (w - tw).abs() < 0.5 {
            self.underline = Some((tx, tw));
            return animating;
        }
        let t = (dt / ANIM_DURATION).min(1.0) as f64;
        self.underline = Some((x + (tx - x) * t, w + (tw - w) * t));
        // Damage just our own rect so the underline slide doesn't force a full redraw.
        self.base.mark_needs_paint();
        true
    }
}

impl LayoutExt for Tabs {}
