//! [`Row`] — a **focusable, clickable container** for arbitrary composed content.
//!
//! [`Item`](super::Item) is the fixed leading/label/trailing row; `Row` is its
//! open cousin: it provides the same interactive chrome — hover tint, active
//! (selected) state with an [`ActiveMarker`], press flash, focus ring, and
//! `on_activate` (mouse + Enter/Space) — but holds **any** children (a
//! [`Grid`](super::Grid) of [`Label`](super::Label)/[`Icon`](super::Icon)/
//! [`Badge`](super::Badge), a multi-line card, …). Use it for rich Dock rows
//! that need to be clicked and selected, where `Item`'s slots aren't enough.
//!
//! An optional persistent background (via [`StyleExt`]) is painted *under* the
//! interactive hover/active overlay — e.g. a state tint that stays while the
//! selection highlight layers on top.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::color::Color;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::effects::{Attention, Flash};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Border, Glow};
use crate::style::{Align, Direction};
use crate::widgets::ActiveMarker;
use heca_core::layout::{Point, Rectangle, Size};

/// Inset of the active/hover selection pill from the row edges, so its rounded
/// corners never contend with a rounded container's corners.
const SEL_INSET: f64 = 3.0;
/// Number of flashes a `needs attention` pulse plays.
const ATTENTION_PULSES: u32 = 4;
/// Peak glow radius (logical px) of the attention pulse border.
const ATTENTION_GLOW_RADIUS: f32 = 12.0;
/// Rest-glow spread radius (px) — a filled row's share of the theme rest halo.
const REST_GLOW_RADIUS: f32 = 10.0;
/// Width of the left accent bar shown when active.
const BAR_W: f64 = 3.0;
/// Active left bar height as a fraction of the row (centered, not full height).
const BAR_FRAC: f64 = 0.65;
/// Size of the [`ActiveMarker::Check`] pip (logical px).
const CHECK_SIZE: f64 = 10.0;
/// Left gutter the check pip centers in.
const CHECK_GUTTER: f64 = 14.0;

/// A focusable, selectable container holding arbitrary child content.
pub struct Row {
    base: Base,
    /// Active (selected/current) state: tinted bg + optional marker.
    active: Signal<bool>,
    /// Sidebar-nav cursor state: a hollow outline, distinct from `active`.
    nav: Signal<bool>,
    marker: ActiveMarker,
    hovered: Signal<bool>,
    flash: Flash,
    on_activate: Option<Box<dyn Fn()>>,
    /// Override for the hover/active highlight color. Defaults to the row's own
    /// background (a stronger tint of the same hue), else the theme accent.
    highlight: Option<Color>,
    /// "Needs attention" pulse + the host-owned request signal that fires it.
    attention: Attention,
    attention_req: Option<Signal<bool>>,
    /// Color of the attention pulse (default: theme warning).
    attention_color: Option<Color>,
}

impl Row {
    /// A new (horizontal) row. Add content with `.child(...)`; make it
    /// clickable/selectable with [`on_activate`](Row::on_activate).
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.direction = Direction::Row;
        base.style.align = Align::Center;
        Self {
            base,
            active: signal(false),
            nav: signal(false),
            marker: ActiveMarker::Bar,
            hovered: signal(false),
            flash: Flash::new(),
            on_activate: None,
            highlight: None,
            attention: Attention::new(),
            attention_req: None,
            attention_color: None,
        }
    }

    /// Override the hover/active highlight color. By default the highlight derives
    /// from the row's background — a stronger tint of the **same hue** — so a
    /// state-tinted row highlights in its own color (not the accent); rows with no
    /// background fall back to the theme accent.
    pub fn highlight(mut self, c: Color) -> Self {
        self.highlight = Some(c);
        self
    }

    /// Make the row clickable/keyboard-activatable (also makes it focusable).
    pub fn on_activate(mut self, f: impl Fn() + 'static) -> Self {
        self.on_activate = Some(Box::new(f));
        self.base.focusable = true; // interactive rows are focusable (Component::focusable)
        self
    }

    /// Bind a **host-owned** "needs attention" request signal. When the host sets
    /// it `true`, the row flashes [`ATTENTION_PULSES`] times (and the signal is
    /// consumed back to `false`). The matching **sound** is the host's job — it
    /// plays its beep when it sets this signal (grid-ui stays audio-free).
    pub fn attention(mut self, req: Signal<bool>) -> Self {
        self.attention_req = Some(req);
        self
    }

    /// Color of the attention pulse (default: the theme `warning` hue).
    pub fn attention_color(mut self, c: Color) -> Self {
        self.attention_color = Some(c);
        self
    }

    /// Set the active (selected) state.
    pub fn active(self, active: bool) -> Self {
        self.active.set(active);
        self
    }

    /// How the active state is indicated (default [`ActiveMarker::Bar`]).
    pub fn marker(mut self, marker: ActiveMarker) -> Self {
        self.marker = marker;
        self
    }

    /// The active-state signal — bind UI to it reactively.
    pub fn state(&self) -> Signal<bool> {
        self.active
    }

    /// Set the sidebar-nav **cursor** state — a hollow outline shown distinctly
    /// from the filled `active` pill (e.g. the workspaces sidebar highlights the
    /// nav cursor while the real focused pane keeps its pill).
    pub fn nav_selected(self, on: bool) -> Self {
        self.nav.set(on);
        self
    }

    /// The nav-cursor signal — bind UI to it reactively.
    pub fn nav_state(&self) -> Signal<bool> {
        self.nav
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

impl Component for Row {
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
        let (accent, glow_c, foreground, ctrl_radius, sel_border_w, ia) = {
            let t = cx.theme();
            (t.colors.accent, t.colors.glow, t.colors.foreground, t.colors.control_radius(), t.focus_border_width, t.colors.interaction)
        };
        let b = self.base.bounds;

        // Persistent background (e.g. a state tint) under the interactive overlay.
        // A FILLED row (a list/pane card) without an explicit `.glow(..)` falls
        // back to the theme rest glow (`PaintCx::rest_glow`), so cards honor the
        // `glow_size` setting at rest; an unfilled row stays surface-less and flat.
        let s = &self.base.style;
        if let (Some(fill), None) = (s.fill, s.glow) {
            cx.rect(b, fill, s.border, s.radius, cx.rest_glow(REST_GLOW_RADIUS));
        } else {
            cx.paint_base(&self.base);
        }

        // Selection pill: tinted when active, faint on hover. Inset so its rounded
        // corners never contend with a rounded container's corners.
        let sel = Rectangle::new(
            Point::new(b.loc.x + SEL_INSET, b.loc.y + SEL_INSET),
            Size::new(
                (b.size.w - 2.0 * SEL_INSET).max(0.0),
                (b.size.h - 2.0 * SEL_INSET).max(0.0),
            ),
        );
        let sel_radius = ctrl_radius.min((sel.size.h / 2.0) as f32);
        // Without an explicit override, derive the highlight from the row's own fill
        // so state-tinted rows stay in-family. With an explicit `highlight`, use the
        // lighter Item-style accent wash instead of a heavy same-hue tint.
        let highlight_base = self.highlight.or(self.base.style.fill);
        if active {
            let fill = if let Some(highlight) = self.highlight {
                highlight.with_alpha(ia.row_active_fill)
            } else {
                highlight_base
                    .map(|h| h.with_alpha(ia.row_active_tint))
                    .unwrap_or(accent.with_alpha(ia.row_active_fill))
            };
            // A crisp same-hue border is the clearest "selected" cue — a tinted
            // fill alone is hard to tell apart from the row's background.
            let edge = highlight_base.unwrap_or(accent).with_alpha(ia.row_active_border);
            cx.rect(
                sel,
                fill,
                Some(Border { color: edge, width: sel_border_w }),
                sel_radius,
                None,
            );
        } else if self.hovered.get_untracked() {
            let c = if let Some(highlight) = self.highlight {
                highlight.with_alpha(ia.row_hover_fill)
            } else {
                highlight_base
                    .map(|h| h.with_alpha(ia.row_hover_tint))
                    .unwrap_or(foreground.with_alpha(ia.row_hover_fill))
            };
            cx.rect(sel, c, None, sel_radius, None);
        }

        // Nav-cursor outline: a **distinct-colored** border (theme foreground, not the
        // accent the `active` pill uses) marking the sidebar j/k/arrow cursor. Drawn
        // ALWAYS when nav — even on the active row — so the cursor stays visible when
        // it coincides with the active pill (a plain accent outline would vanish into
        // the pill).
        if self.nav.get_untracked() {
            cx.rect(
                sel,
                accent.with_alpha(0),
                Some(Border {
                    color: accent.with_alpha(ia.nav_outline),
                    width: (sel_border_w * 1.5).max(2.0),
                }),
                sel_radius,
                Some(Glow { color: glow_c, radius: 8.0, intensity: 0.28 }),
            );
        }

        // Active indicator.
        if active {
            match self.marker {
                ActiveMarker::Bar => {
                    let bar_h = b.size.h * BAR_FRAC;
                    let bar_y = b.loc.y + (b.size.h - bar_h) / 2.0;
                    // The bar (and its glow) follow the highlight hue too.
                    let bar_c = highlight_base.map(|h| h.with_alpha(255)).unwrap_or(accent);
                    let bar_glow = highlight_base.map(|h| h.with_alpha(255)).unwrap_or(glow_c);
                    cx.rect(
                        Rectangle::new(Point::new(b.loc.x, bar_y), Size::new(BAR_W, bar_h)),
                        bar_c,
                        None,
                        (BAR_W / 2.0) as f32,
                        Some(Glow { color: bar_glow, radius: 8.0, intensity: 0.16 }),
                    );
                }
                ActiveMarker::Check => {
                    let pip_x = b.loc.x + CHECK_GUTTER / 2.0 - CHECK_SIZE / 2.0;
                    let pip_y = b.loc.y + (b.size.h - CHECK_SIZE) / 2.0;
                    cx.rect(
                        Rectangle::new(Point::new(pip_x, pip_y), Size::new(CHECK_SIZE, CHECK_SIZE)),
                        highlight_base.map(|h| h.with_alpha(255)).unwrap_or(accent),
                        None,
                        (CHECK_SIZE / 2.0) as f32,
                        None,
                    );
                }
                ActiveMarker::None => {}
            }
        }

        // Content.
        for child in &self.base.children {
            child.paint(cx);
        }

        if self.interactive() && !disabled {
            cx.flash(b, self.flash.amount() * 0.5, 0.0);
        }
        if disabled {
            cx.dim(b, self.base.style.radius);
        }
        if self.interactive()
            && !disabled
            && self.base.shows_focus_ring()
            && cx.theme().colors.show_focus_border
        {
            let ring = cx.theme().colors.effective_focus_ring();
            cx.focus_ring(b, ring, ctrl_radius);
        }

        // "Needs attention" pulse — a glowing colored border over the row,
        // intensity following the current pulse. Painted last so it reads over
        // the content/selection.
        let attn = self.attention.amount();
        if attn > 0.0 {
            let c = self.attention_color.unwrap_or_else(|| cx.theme().colors.warning);
            let radius = ctrl_radius.min((b.size.h / 2.0) as f32);
            cx.rect(
                b,
                c.with_alpha((40.0 * attn) as u8),
                Some(Border { color: c.with_alpha((235.0 * attn) as u8), width: sel_border_w }),
                radius,
                Some(Glow { color: c, radius: ATTENTION_GLOW_RADIUS, intensity: attn }),
            );
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
            Event::Key { key: GridKey::Enter | GridKey::Space, pressed: true } => {
                self.activate();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        // Consume a host attention request (rising edge): fire the pulse + reset
        // the signal so each `true` triggers exactly one sequence.
        if let Some(req) = self.attention_req
            && req.get_untracked()
        {
            self.attention.trigger(ATTENTION_PULSES);
            req.set(false);
        }
        let mut animating = self.flash.tick(dt);
        animating |= self.attention.tick(dt);
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        // Damage our own rect (which contains our content) so a press flash, the
        // attention pulse, or an animating child doesn't force a full redraw.
        if animating {
            self.base.mark_needs_paint();
        }
        animating
    }
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExt for Row {}
impl StyleExt for Row {}
impl Parent for Row {}
