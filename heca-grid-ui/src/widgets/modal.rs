//! [`Modal`] — a centered **confirm / alert dialog** over a dimming scrim.
//!
//! Unlike a layout container, a modal must paint **centered on the viewport** and
//! **capture all input** while open. It therefore uses the same overlay contract
//! as [`Select`](super::Select): it reports [`overlay_active`](Component::overlay_active)
//! and is `focusable` while open, so the host routes pointer/key events to it
//! first (see [`FocusManager`](crate::focus::FocusManager)). Like `Select`'s
//! dropdown, its content is **drawn + hit-tested manually** (title, message, and
//! one or two action buttons) on the scene's overlay layer — there is no child
//! subtree to relocate.
//!
//! Open/close is a host-owned [`Signal<bool>`](crate::reactive::Signal) so mouse,
//! keyboard, and RPC all drive it. Dismissal paths: the **Confirm**/**Cancel**
//! buttons, **Esc** (= cancel), and a click on the **scrim** (= cancel). Each
//! fires the matching callback and closes. Set [`dismissible(false)`](Modal::dismissible)
//! for a **forced-decision** dialog — Esc/scrim are swallowed and only the buttons
//! close it.

use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Border, Glow, TextAlign};
use std::cell::Cell;
use heca_core::layout::{Point, Rectangle, Size};

/// Scrim (backdrop) alpha over the rest of the UI.
const SCRIM_ALPHA: u8 = 150;
/// Panel inner padding.
const PAD: f64 = 18.0;
/// Gap between title, message, and the action row.
const GAP: f64 = 14.0;
/// Action button height (logical px) and horizontal text padding.
const BTN_H: f64 = 30.0;
const BTN_PAD_X: f64 = 16.0;
/// Gap between the two action buttons.
const BTN_GAP: f64 = 10.0;
/// Panel width bounds: a minimum, and a fraction of the viewport as the cap.
const PANEL_MIN_W: f64 = 240.0;
const PANEL_MAX_W_FRAC: f64 = 0.6;

/// Computed rects for one layout pass (in viewport space).
struct Rects {
    panel: Rectangle,
    title: Rectangle,
    message: Rectangle,
    cancel: Option<Rectangle>,
    confirm: Rectangle,
}

/// A centered confirm/alert dialog.
pub struct Modal {
    base: Base,
    open: Signal<bool>,
    title: String,
    message: String,
    confirm_label: String,
    cancel_label: Option<String>,
    /// Tint the confirm button with the danger hue (destructive action).
    danger: bool,
    /// When `false`, Esc / scrim clicks are swallowed but don't close — the user
    /// must pick a button (a forced-decision dialog). Default `true`.
    dismissible: bool,
    on_confirm: Option<Box<dyn Fn()>>,
    on_cancel: Option<Box<dyn Fn()>>,
    /// Hovered action: `Some(false)` = cancel, `Some(true)` = confirm.
    hovered: Option<bool>,
    /// Last-seen viewport, cached during paint so `event` can lay out identically.
    viewport: Cell<Size>,
}

impl Modal {
    /// A new (closed) dialog with `title` and a one-line `message`. The confirm
    /// button defaults to `OK`.
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            base: Base::new(),
            open: signal(false),
            title: title.into(),
            message: message.into(),
            confirm_label: "OK".to_string(),
            cancel_label: None,
            danger: false,
            dismissible: true,
            on_confirm: None,
            on_cancel: None,
            hovered: None,
            viewport: Cell::new(Size::new(f64::MAX, f64::MAX)),
        }
    }

    /// Set the confirm button label + callback (fires then closes).
    pub fn confirm(mut self, label: impl Into<String>, f: impl Fn() + 'static) -> Self {
        self.confirm_label = label.into();
        self.on_confirm = Some(Box::new(f));
        self
    }

    /// Add a cancel button with `label` + callback (Esc / scrim also cancel).
    pub fn cancel(mut self, label: impl Into<String>, f: impl Fn() + 'static) -> Self {
        self.cancel_label = Some(label.into());
        self.on_cancel = Some(Box::new(f));
        self
    }

    /// Tint the confirm button with the danger hue (for destructive actions).
    pub fn danger(mut self, on: bool) -> Self {
        self.danger = on;
        self
    }

    /// Whether Esc / a scrim click dismiss the dialog (default `true`). Set
    /// `false` to force an explicit choice — only the buttons close it. Pair with
    /// a `.cancel(...)` so there's always a non-destructive way out.
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }

    /// Set the initial open state.
    pub fn open(self, open: bool) -> Self {
        self.open.set(open);
        self
    }

    /// The open-state signal — the host binds this to show/hide the dialog.
    pub fn open_signal(&self) -> Signal<bool> {
        self.open
    }

    fn is_open(&self) -> bool {
        self.open.get_untracked()
    }

    fn text_w(&self, s: &str) -> f64 {
        s.chars().count() as f64 * (self.base.font * MONO_ADVANCE_RATIO) as f64
    }

    fn line_h(&self) -> f64 {
        (self.base.font * MONO_LINE_RATIO) as f64
    }

    fn btn_w(&self, label: &str) -> f64 {
        self.text_w(label) + 2.0 * BTN_PAD_X
    }

    /// Lay out the panel + content centered in the cached viewport.
    fn rects(&self) -> Rects {
        let vp = self.viewport.get();
        let line = self.line_h();
        let confirm_w = self.btn_w(&self.confirm_label);
        let cancel_w = self.cancel_label.as_ref().map(|l| self.btn_w(l));
        let actions_w = confirm_w + cancel_w.map(|w| w + BTN_GAP).unwrap_or(0.0);

        // Panel width hugs the widest of title/message/actions, within bounds.
        let content_w = self
            .text_w(&self.title)
            .max(self.text_w(&self.message))
            .max(actions_w);
        let cap = if vp.w.is_finite() { vp.w * PANEL_MAX_W_FRAC } else { f64::MAX };
        let panel_w = (content_w + 2.0 * PAD).clamp(PANEL_MIN_W, cap.max(PANEL_MIN_W));
        let panel_h = PAD + line + GAP + line + GAP + BTN_H + PAD;

        let (vw, vh) = if vp.w.is_finite() { (vp.w, vp.h) } else { (panel_w, panel_h) };
        let px = (vw - panel_w) / 2.0;
        let py = (vh - panel_h) / 2.0;
        let panel = Rectangle::new(Point::new(px, py), Size::new(panel_w, panel_h));

        let title = Rectangle::new(Point::new(px + PAD, py + PAD), Size::new(panel_w - 2.0 * PAD, line));
        let message = Rectangle::new(
            Point::new(px + PAD, title.loc.y + line + GAP),
            Size::new(panel_w - 2.0 * PAD, line),
        );
        // Action row, right-aligned along the panel bottom.
        let by = py + panel_h - PAD - BTN_H;
        let confirm_x = px + panel_w - PAD - confirm_w;
        let confirm = Rectangle::new(Point::new(confirm_x, by), Size::new(confirm_w, BTN_H));
        let cancel = cancel_w.map(|w| {
            Rectangle::new(Point::new(confirm_x - BTN_GAP - w, by), Size::new(w, BTN_H))
        });
        Rects { panel, title, message, cancel, confirm }
    }

    /// Fire confirm + close.
    fn do_confirm(&mut self) {
        if let Some(f) = &self.on_confirm {
            f();
        }
        self.close();
    }

    /// Fire cancel (if any) + close.
    fn do_cancel(&mut self) {
        if let Some(f) = &self.on_cancel {
            f();
        }
        self.close();
    }

    fn close(&mut self) {
        self.open.set(false);
        self.hovered = None;
    }
}

impl Component for Modal {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Focusable only while open, so the overlay scan routes input here.
    fn focusable(&self) -> bool {
        self.is_open()
    }

    fn overlay_active(&self) -> bool {
        self.is_open()
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() || !self.is_open() {
            return;
        }
        self.viewport.set(cx.viewport());
        let (background, surface, accent, danger, foreground, muted, ctrl_radius, radius) = {
            let t = cx.theme();
            (t.background, t.surface, t.accent, t.danger, t.foreground, t.muted, t.control_radius(), t.radius)
        };
        let r = self.rects();
        let confirm_tone = if self.danger { danger } else { accent };

        cx.with_overlay(|cx| {
            // Scrim over the whole viewport.
            let vp = self.viewport.get();
            let scrim = if vp.w.is_finite() {
                Rectangle::new(Point::new(0.0, 0.0), vp)
            } else {
                r.panel
            };
            cx.rect(scrim, background.with_alpha(SCRIM_ALPHA), None, 0.0, None);

            // Panel: surface fill + the shared Pane/DockFrame corner-bracket
            // reticle frame (matches the linked GridCN modal — no plain border).
            cx.rect(r.panel, surface, None, radius, None);
            cx.bracket_frame(r.panel, Some(surface));
            cx.text(r.title, &self.title, foreground, self.base.font, TextAlign::Start, true);
            cx.text(r.message, &self.message, muted, self.base.font, TextAlign::Start, false);

            // Cancel (ghost) + Confirm (toned) buttons.
            if let Some(cr) = r.cancel {
                let hov = self.hovered == Some(false);
                cx.rect(
                    cr,
                    foreground.with_alpha(if hov { 26 } else { 0 }),
                    Some(Border { color: muted.with_alpha(180), width: 1.0 }),
                    ctrl_radius,
                    None,
                );
                cx.text(cr, self.cancel_label.as_deref().unwrap_or(""), foreground, self.base.font, TextAlign::Center, false);
            }
            let hov = self.hovered == Some(true);
            cx.rect(
                r.confirm,
                confirm_tone.with_alpha(if hov { 235 } else { 200 }),
                None,
                ctrl_radius,
                (hov).then_some(Glow { color: confirm_tone, radius: 8.0, intensity: 0.3 }),
            );
            cx.text(r.confirm, &self.confirm_label, background, self.base.font, TextAlign::Center, true);
        });
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        let r = self.rects();
        match ev {
            Event::PointerMoved { pos } => {
                let h = if r.cancel.is_some_and(|cr| cr.contains(*pos)) {
                    Some(false)
                } else if r.confirm.contains(*pos) {
                    Some(true)
                } else {
                    None
                };
                if self.hovered != h {
                    self.hovered = h;
                }
                Handled::Yes
            }
            Event::PointerPressed { pos } => {
                if r.confirm.contains(*pos) {
                    self.do_confirm();
                } else if r.cancel.is_some_and(|cr| cr.contains(*pos)) {
                    self.do_cancel();
                } else if !r.panel.contains(*pos) && self.dismissible {
                    // Click on the scrim dismisses (= cancel) unless non-dismissible.
                    self.do_cancel();
                }
                // Always swallow input while open (modal).
                Handled::Yes
            }
            Event::Key { key: GridKey::Escape, pressed: true } => {
                if self.dismissible {
                    self.do_cancel();
                }
                Handled::Yes
            }
            Event::Key { key: GridKey::Enter, pressed: true } => {
                self.do_confirm();
                Handled::Yes
            }
            // Swallow every other key while the dialog owns input.
            Event::Key { .. } => Handled::Yes,
            _ => Handled::No,
        }
    }
}

impl LayoutExt for Modal {}
