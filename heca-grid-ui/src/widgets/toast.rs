//! [`Toast`] — a compact **notification card**: a bracket-framed surface with a
//! severity-colored leading [`Icon`](super::Icon), a strong title, optional small
//! body text, an optional inline **action** button, and an optional **×** dismiss
//! affordance.
//!
//! This is a **presentation widget only** — it holds *no* queue, timer, or global
//! state. Lifecycle (when a toast appears, how long it lives, auto-dismiss policy,
//! deduplication, sound) is the **host application's** job; the widget merely
//! *renders* one notification and *reports interactions* back via callbacks
//! ([`on_click`](Toast::on_click), [`on_action`](Toast::on_action),
//! [`on_dismiss`](Toast::on_dismiss)). The host removes it from its own store.
//!
//! Because it is an ordinary in-tree component (not overlay-drawn), it is equally
//! usable **inline** — e.g. as a notification row in a sidebar — as it will be
//! inside the overlay stack manager built on top of it. Severity maps to theme
//! tokens (`accent`/`success`/`warning`/`danger`), never baked-in literals.

use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::effects::Flash;
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::SignalGet;
use crate::scene::{Border, Glow, TextAlign, TextStyle};
use crate::style::Length;
use crate::widgets::Glyph;
use heca_core::layout::{Point, Rectangle, Size};

/// Inner padding.
const PAD: f64 = 13.0;
/// Gap between title, body, and the action row.
const GAP: f64 = 6.0;
/// Gap between the leading icon and the text column.
const ICON_GAP: f64 = 10.0;
/// Leading-icon size as a multiple of the resolved font (sits on the title line).
const ICON_SCALE: f32 = 1.1;
/// Body text multiplier relative to the title (which uses the resolved font).
const BODY_SCALE: f32 = 0.9;
/// Action button height (logical px) and horizontal text padding.
const ACTION_H: f64 = 24.0;
const ACTION_PAD_X: f64 = 12.0;
/// Rest-glow spread radius (px) — the card's share of the theme rest halo. A
/// touch wider than the small controls (12): the toast is a card-sized surface
/// and a tight halo read visibly weaker beside them (user-reported).
const GLOW_RADIUS: f32 = 14.0;
/// The × dismiss hit-square edge as a multiple of the resolved font.
const DISMISS_SCALE: f32 = 1.4;
/// Default card width.
const DEFAULT_WIDTH: f32 = 320.0;

/// Severity of a [`Toast`], mapped to theme tokens at paint time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastSeverity {
    /// Informational (accent).
    #[default]
    Info,
    /// Success (positive).
    Success,
    /// Warning.
    Warning,
    /// Danger (error).
    Danger,
}

impl ToastSeverity {
    /// The default leading glyph for this severity (overridable via [`Toast::icon`]).
    fn default_glyph(self) -> Glyph {
        match self {
            ToastSeverity::Info => Glyph::Info,
            ToastSeverity::Success => Glyph::Check,
            ToastSeverity::Warning => Glyph::Warning,
            ToastSeverity::Danger => Glyph::WarningCircle,
        }
    }
}

/// Which sub-region the pointer is over (drives hover highlight + click routing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    None,
    Body,
    Action,
    Dismiss,
}

/// Computed sub-rects for one layout pass (in the card's own coordinate space).
struct Rects {
    icon: Option<Rectangle>,
    title: Rectangle,
    body: Option<Rectangle>,
    action: Option<Rectangle>,
    dismiss: Option<Rectangle>,
}

/// A compact notification card. Presentation only — the host owns lifecycle.
pub struct Toast {
    base: Base,
    severity: ToastSeverity,
    /// Leading glyph; defaults to the severity glyph, `None` hides the icon.
    icon: Option<Glyph>,
    show_icon: bool,
    title: String,
    body: Option<String>,
    action_label: Option<String>,
    /// Whether the × dismiss affordance is shown (default `true`).
    dismissible: bool,
    on_click: Option<Box<dyn Fn()>>,
    on_action: Option<Box<dyn Fn()>>,
    on_dismiss: Option<Box<dyn Fn()>>,
    /// Sub-region currently hovered (for highlight).
    hovered: Region,
    flash: Flash,
    /// Which sub-region the active press flash belongs to (so it's drawn over
    /// just that rect, not the whole card).
    flash_region: Region,
}

impl Toast {
    /// A new info toast showing `title`. Add body text with [`body`](Toast::body),
    /// an action with [`action`](Toast::action), severity via the convenience
    /// constructors, and wire dismissal with [`on_dismiss`](Toast::on_dismiss).
    pub fn new(title: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.layout.width = Length::Px(DEFAULT_WIDTH);
        let mut toast = Self {
            base,
            severity: ToastSeverity::Info,
            icon: None,
            show_icon: true,
            title: title.into(),
            body: None,
            action_label: None,
            dismissible: true,
            on_click: None,
            on_action: None,
            on_dismiss: None,
            hovered: Region::None,
            flash: Flash::new(),
            flash_region: Region::None,
        };
        toast.remeasure();
        toast
    }

    /// Convenience constructors, one per severity.
    pub fn info(title: impl Into<String>) -> Self {
        Self::new(title)
    }
    pub fn success(title: impl Into<String>) -> Self {
        Self::new(title).severity(ToastSeverity::Success)
    }
    pub fn warning(title: impl Into<String>) -> Self {
        Self::new(title).severity(ToastSeverity::Warning)
    }
    pub fn danger(title: impl Into<String>) -> Self {
        Self::new(title).severity(ToastSeverity::Danger)
    }

    /// Set the severity (hue + default leading glyph).
    pub fn severity(mut self, severity: ToastSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Override the leading glyph (default: the severity glyph).
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.icon = Some(glyph);
        self.show_icon = true;
        self
    }

    /// Hide the leading icon entirely.
    pub fn no_icon(mut self) -> Self {
        self.show_icon = false;
        self.remeasure();
        self
    }

    /// Set the small body text (a second line under the title).
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self.remeasure();
        self
    }

    /// Add an inline action button with `label` + callback.
    pub fn action(mut self, label: impl Into<String>, f: impl Fn() + 'static) -> Self {
        self.action_label = Some(label.into());
        self.on_action = Some(Box::new(f));
        self.remeasure();
        self
    }

    /// Whether the × dismiss affordance is shown (default `true`).
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }

    /// Make the whole card clickable (fires before any dismiss/action hit-test miss).
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self.base.focusable = true; // a clickable toast is focusable (Component::focusable)
        self
    }

    /// Set the callback fired when the × is clicked. The host removes the toast.
    pub fn on_dismiss(mut self, f: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Box::new(f));
        self
    }

    fn line_h(&self, scale: f32) -> f64 {
        (self.base.font * scale) as f64 * MONO_LINE_RATIO as f64
    }

    fn icon_size(&self) -> f64 {
        (self.base.font * ICON_SCALE) as f64
    }

    fn dismiss_size(&self) -> f64 {
        (self.base.font * DISMISS_SCALE) as f64
    }

    fn text_w(&self, s: &str, scale: f32) -> f64 {
        s.chars().count() as f64 * (self.base.font * scale * MONO_ADVANCE_RATIO) as f64
    }

    /// Lay the card's content out within the resolved [`bounds`](Base::bounds).
    fn rects(&self) -> Rects {
        let b = self.base.bounds;
        let title_h = self.line_h(1.0);
        let has_icon = self.show_icon;
        let has_dismiss = self.dismissible;

        // The × sits in the top-right corner; the leading icon at the top-left.
        let dsz = self.dismiss_size();
        let dismiss = has_dismiss.then(|| {
            Rectangle::new(
                Point::new(b.loc.x + b.size.w - PAD - dsz, b.loc.y + PAD),
                Size::new(dsz, dsz),
            )
        });
        let isz = self.icon_size();
        let icon = has_icon.then(|| {
            Rectangle::new(
                // Vertically centered on the title line.
                Point::new(b.loc.x + PAD, b.loc.y + PAD + (title_h - isz) / 2.0),
                Size::new(isz, isz),
            )
        });

        // Text column: right of the icon, left of the × gutter.
        let text_x = b.loc.x + PAD + if has_icon { isz + ICON_GAP } else { 0.0 };
        let right = b.loc.x + b.size.w - PAD - if has_dismiss { dsz + ICON_GAP } else { 0.0 };
        let text_w = (right - text_x).max(0.0);

        let title = Rectangle::new(Point::new(text_x, b.loc.y + PAD), Size::new(text_w, title_h));
        let mut y = b.loc.y + PAD + title_h;
        let body = self.body.as_ref().map(|_| {
            let h = self.line_h(BODY_SCALE);
            let r = Rectangle::new(Point::new(text_x, y + GAP), Size::new(text_w, h));
            y = r.loc.y + h;
            r
        });
        let action = self.action_label.as_ref().map(|l| {
            let w = self.text_w(l, 1.0) + 2.0 * ACTION_PAD_X;
            Rectangle::new(Point::new(text_x, y + GAP), Size::new(w, ACTION_H))
        });

        Rects { icon, title, body, action, dismiss }
    }

    /// Which sub-region a point falls in.
    fn region_at(&self, r: &Rects, pos: Point) -> Region {
        if r.dismiss.is_some_and(|d| d.contains(pos)) {
            Region::Dismiss
        } else if r.action.is_some_and(|a| a.contains(pos)) {
            Region::Action
        } else if self.base.bounds.contains(pos) {
            Region::Body
        } else {
            Region::None
        }
    }
}

impl Component for Toast {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Focusable when the card itself is clickable (Enter/Space activates it).
    /// Height = padding + title + optional body + optional action, from the font.
    fn remeasure(&mut self) {
        let mut h = self.line_h(1.0);
        if self.body.is_some() {
            h += GAP + self.line_h(BODY_SCALE);
        }
        if self.action_label.is_some() {
            h += GAP + ACTION_H;
        }
        // The leading icon never exceeds the title line, so it doesn't grow height.
        self.base.style.layout.height = Length::Px((2.0 * PAD + h) as f32);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let (surface, foreground, muted, radius, card_radius, toast_tint) = {
            let t = cx.theme();
            (t.colors.surface, t.colors.foreground, t.colors.muted, t.colors.control_radius(), t.colors.border_radius, t.colors.interaction.toast_tint)
        };
        let tone = match self.severity {
            ToastSeverity::Info => cx.theme().colors.accent,
            ToastSeverity::Success => cx.theme().colors.success,
            ToastSeverity::Warning => cx.theme().colors.warning,
            ToastSeverity::Danger => cx.theme().colors.danger,
        };
        let b = self.base.bounds;
        let r = self.rects();
        let title_fs = self.base.font;
        let body_fs = self.base.font * BODY_SCALE;

        // Surface: severity-tinted fill + the shared Pane/DockFrame corner-bracket
        // reticle frame (GridCN fidelity — same as the Modal panel, #79). The theme
        // rest glow (`PaintCx::rest_glow`) keeps toasts scaling with `glow_size`
        // (T011) — in the THEME glow color, not the severity tone: the bracket
        // frame is always accent, and a danger-red halo under a blue frame blends
        // to a muddy purple fringe (user-reported).
        let glow = cx.rest_glow(GLOW_RADIUS);
        cx.rect(b, surface.lerp(tone, toast_tint as f32 / 255.0), None, card_radius, glow);
        cx.bracket_frame(b);

        // Leading severity icon (single-layer, toned).
        if let (Some(ir), Some(ch)) = (r.icon, self.icon.unwrap_or(self.severity.default_glyph()).primary_char()) {
            cx.icon(ir, &ch.to_string(), tone, ir.size.h as f32);
        }

        // Title (strong, severity-toned) then optional body (muted).
        cx.text(r.title, &self.title, tone, title_fs, TextAlign::Start, TextStyle::BOLD);
        if let (Some(br), Some(body)) = (r.body, &self.body) {
            cx.text(br, body, foreground.lerp(muted, 0.2), body_fs, TextAlign::Start, TextStyle::REGULAR);
        }

        // Action button (toned ghost; brighter on hover).
        if let (Some(ar), Some(label)) = (r.action, &self.action_label) {
            let hov = self.hovered == Region::Action;
            cx.rect(
                ar,
                tone.with_alpha(if hov { 40 } else { 22 }),
                Some(Border { color: tone.with_alpha(if hov { 220 } else { 150 }), width: 1.0 }),
                radius,
                hov.then_some(Glow { color: tone, radius: 7.0, intensity: 0.2 }),
            );
            cx.text(ar, label, tone, title_fs, TextAlign::Center, TextStyle::REGULAR);
        }

        // × dismiss affordance (muted; foreground on hover).
        if let Some(dr) = r.dismiss {
            let hov = self.hovered == Region::Dismiss;
            if let Some(ch) = Glyph::Close.primary_char() {
                cx.icon(dr, &ch.to_string(), if hov { foreground } else { muted }, self.base.font);
            }
        }

        // Press flash localized to the pressed sub-region, so pressing the action
        // (e.g. "Retry") or the × doesn't light up the whole card. A whole-card
        // press (body `on_click` / keyboard) flashes the full card.
        if self.flash.amount() > 0.0 {
            let (frect, frad) = match self.flash_region {
                Region::Action => (r.action, radius),
                Region::Dismiss => (r.dismiss, radius),
                _ => (Some(b), card_radius),
            };
            if let Some(fr) = frect {
                cx.flash(fr, self.flash.amount() * 0.4, frad);
            }
        }
        // Focus ring when clickable + focused (theme-aware shift of the toast tone).
        if self.focusable() && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            let ring = cx.theme().colors.focus_ring_tone(tone);
            cx.focus_ring(b, ring, card_radius);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        let r = self.rects();
        match ev {
            Event::PointerMoved { pos } => {
                let region = self.region_at(&r, *pos);
                if self.hovered != region {
                    self.hovered = region;
                }
                // Don't consume moves — siblings still need hover tracking.
                Handled::No
            }
            Event::PointerPressed { pos } => match self.region_at(&r, *pos) {
                Region::Dismiss => {
                    self.flash_region = Region::Dismiss;
                    self.flash.trigger();
                    if let Some(f) = &self.on_dismiss {
                        f();
                    }
                    Handled::Yes
                }
                Region::Action => {
                    self.flash_region = Region::Action;
                    self.flash.trigger();
                    if let Some(f) = &self.on_action {
                        f();
                    }
                    Handled::Yes
                }
                Region::Body if self.on_click.is_some() => {
                    self.flash_region = Region::Body;
                    self.flash.trigger();
                    if let Some(f) = &self.on_click {
                        f();
                    }
                    Handled::Yes
                }
                _ => Handled::No,
            },
            Event::Key { key: GridKey::Enter | GridKey::Space, pressed: true } if self.focusable() => {
                self.flash_region = Region::Body;
                self.flash.trigger();
                if let Some(f) = &self.on_click {
                    f();
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let animating = self.flash.tick(dt);
        // When used inline (in-tree), damage just our own rect on a press flash. In a
        // `ToastStack` the toast paints on the overlay layer and the stack owns the
        // damage region, so this mark is simply unobserved there.
        if animating {
            self.base.mark_needs_paint();
        }
        animating
    }
}

impl LayoutExt for Toast {}
