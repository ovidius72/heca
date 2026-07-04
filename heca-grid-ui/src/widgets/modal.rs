//! [`Modal`] — a centered **confirm / alert dialog** over a dimming scrim.
//!
//! Unlike a layout container, a modal must paint **centered on the viewport** and
//! **capture all input** while open. It therefore uses the same overlay contract
//! as [`Select`](super::Select): it reports [`overlay_active`](Component::overlay_active)
//! and is `focusable` while open, so the host routes pointer/key events to it
//! first (see [`FocusManager`](crate::focus::FocusManager)). Its content is **drawn +
//! hit-tested manually** (title, message, and a row of **N action buttons**) on the
//! scene's overlay layer — there is no child subtree to relocate.
//!
//! **Keyboard-driven.** One button is focused (a focus ring); the host moves focus
//! with Tab/Shift+Tab/arrows (see [`focus_next`](Modal::focus_next) /
//! [`focus_prev`](Modal::focus_prev)), activates the focused one with Enter/Space
//! ([`activate_focused`](Modal::activate_focused)), and each button may carry a
//! **letter shortcut** ([`ModalButton::shortcut`]) shown as `Label (x)` and fired by
//! [`activate_shortcut`](Modal::activate_shortcut). Initial focus is the **cancel**
//! button (safe default for destructive dialogs).
//!
//! Open/close is a host-owned [`Signal<bool>`](crate::reactive::Signal). Dismissal
//! paths: a button, **Esc** (= the cancel button), and a click on the **scrim**. Set
//! [`dismissible(false)`](Modal::dismissible) for a **forced-decision** dialog —
//! Esc/scrim are swallowed and only the buttons close it.

use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Border, Glow, Shadow, TextAlign};
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
/// Gap between adjacent action buttons.
const BTN_GAP: f64 = 10.0;
/// Panel width bounds: a minimum, and a fraction of the viewport as the cap.
const PANEL_MIN_W: f64 = 240.0;
const PANEL_MAX_W_FRAC: f64 = 0.6;
/// Multiplier applied to the config-driven `colors.shadow.blur` token to size the
/// modal's drop-shadow falloff. The base token (~8px) is tuned for small
/// elevated surfaces; the modal panel is large and needs a wider, softer halo,
/// so we scale it up. Config drives the *base*; this multiplier adapts it to the
/// modal's visual scale.
const SHADOW_BLUR_MULT: f32 = 4.0;
/// Downward offset that lifts the dialog off the scrim.
const SHADOW_DROP: f32 = 12.0;

/// One action button in a [`Modal`]'s button row. Domain-neutral: the label + an
/// optional letter shortcut + a callback the host fires on activation.
pub struct ModalButton {
    label: String,
    shortcut: Option<char>,
    /// Tint with the danger hue (destructive action) instead of the accent.
    danger: bool,
    /// The cancel button: Esc activates it and it takes initial focus.
    is_cancel: bool,
    on_activate: Box<dyn Fn()>,
}

impl ModalButton {
    /// A new button with `label`; `on_activate` fires when it is chosen (then the
    /// dialog closes).
    pub fn new(label: impl Into<String>, on_activate: impl Fn() + 'static) -> Self {
        Self {
            label: label.into(),
            shortcut: None,
            danger: false,
            is_cancel: false,
            on_activate: Box::new(on_activate),
        }
    }

    /// A letter shortcut, shown as `Label (x)` and fired by
    /// [`Modal::activate_shortcut`]. Not baked in — the host decides the letters.
    pub fn shortcut(mut self, c: char) -> Self {
        self.shortcut = Some(c);
        self
    }

    /// Tint with the danger hue (destructive primary action).
    pub fn danger(mut self, on: bool) -> Self {
        self.danger = on;
        self
    }

    /// Mark as the **cancel** button: Esc / scrim-click activate it, and it receives
    /// initial focus (safe default for a destructive dialog).
    pub fn cancel(mut self) -> Self {
        self.is_cancel = true;
        self
    }

    /// The label as shown, including the `(x)` shortcut hint when set.
    fn display(&self) -> String {
        match self.shortcut {
            Some(c) => format!("{} ({})", self.label, c),
            None => self.label.clone(),
        }
    }
}

/// Computed rects for one layout pass (in viewport space).
struct Rects {
    panel: Rectangle,
    title: Rectangle,
    message: Rectangle,
    /// One rect per button, in `buttons` order (laid out left→right, right-aligned).
    buttons: Vec<Rectangle>,
}

/// A centered confirm/alert dialog with a row of action buttons.
pub struct Modal {
    base: Base,
    open: Signal<bool>,
    title: String,
    message: String,
    buttons: Vec<ModalButton>,
    /// Index of the `.confirm(...)` (primary) button, so `.danger(true)` can tint it.
    primary_idx: Option<usize>,
    /// When `false`, Esc / scrim clicks are swallowed but don't close — the user
    /// must pick a button (a forced-decision dialog). Default `true`.
    dismissible: bool,
    /// The focused button index (moved by the host / arrows; Enter/Space activate it).
    focused: usize,
    /// Hovered button index (pointer).
    hovered: Option<usize>,
    /// Last-seen viewport, cached during paint so `event` can lay out identically.
    viewport: Cell<Size>,
}

impl Modal {
    /// A new (closed) dialog with `title` and a one-line `message`. Add buttons with
    /// [`button`](Modal::button) (or the [`confirm`](Modal::confirm) /
    /// [`cancel`](Modal::cancel) convenience).
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            base: Base::new(),
            open: signal(false),
            title: title.into(),
            message: message.into(),
            buttons: Vec::new(),
            primary_idx: None,
            dismissible: true,
            focused: 0,
            hovered: None,
            viewport: Cell::new(Size::new(f64::INFINITY, f64::INFINITY)),
        }
    }

    /// Append an action button (general form — N buttons, per-button shortcuts).
    pub fn button(mut self, button: ModalButton) -> Self {
        self.buttons.push(button);
        self
    }

    /// Convenience: the **primary** (confirm) button. `.danger(true)` tints it.
    pub fn confirm(mut self, label: impl Into<String>, f: impl Fn() + 'static) -> Self {
        self.primary_idx = Some(self.buttons.len());
        self.buttons.push(ModalButton::new(label, f));
        self
    }

    /// Convenience: a **cancel** button (Esc / scrim also activate it).
    pub fn cancel(mut self, label: impl Into<String>, f: impl Fn() + 'static) -> Self {
        self.buttons.push(ModalButton::new(label, f).cancel());
        self
    }

    /// Tint the primary (confirm) button with the danger hue.
    pub fn danger(mut self, on: bool) -> Self {
        if let Some(i) = self.primary_idx {
            self.buttons[i].danger = on;
        }
        self
    }

    /// `false` to force an explicit choice — only the buttons close it. Pair with a
    /// cancel button so there's always a non-destructive way out.
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }

    /// Set the initial open state (and reset focus to the cancel button).
    pub fn open(mut self, open: bool) -> Self {
        self.open.set(open);
        if open {
            self.focused = self.initial_focus();
        }
        self
    }

    /// The open-state signal — the host binds this to show/hide the dialog.
    pub fn open_signal(&self) -> Signal<bool> {
        self.open
    }

    /// Initial focus: the cancel button if any (safe default), else the first.
    fn initial_focus(&self) -> usize {
        self.buttons
            .iter()
            .position(|b| b.is_cancel)
            .unwrap_or(0)
    }

    fn is_open(&self) -> bool {
        self.open.get_untracked()
    }

    // ── Host-driven keyboard actions (the host supplies modifier awareness) ──────

    /// Move focus to the next button (wraps). No-op with no buttons.
    pub fn focus_next(&mut self) {
        if !self.buttons.is_empty() {
            self.focused = (self.focused + 1) % self.buttons.len();
        }
    }

    /// Move focus to the previous button (wraps). No-op with no buttons.
    pub fn focus_prev(&mut self) {
        if !self.buttons.is_empty() {
            self.focused = (self.focused + self.buttons.len() - 1) % self.buttons.len();
        }
    }

    /// Activate the focused button (fires its callback + closes).
    pub fn activate_focused(&mut self) {
        self.activate(self.focused);
    }

    /// Activate the button whose letter shortcut equals `c` (case-insensitive).
    /// Returns `true` if one matched.
    pub fn activate_shortcut(&mut self, c: char) -> bool {
        if let Some(i) = self
            .buttons
            .iter()
            .position(|b| b.shortcut.is_some_and(|s| s.eq_ignore_ascii_case(&c)))
        {
            self.activate(i);
            true
        } else {
            false
        }
    }

    /// Activate the cancel button (Esc / scrim). Falls back to just closing when
    /// there is no explicit cancel button but the dialog is dismissible.
    pub fn request_cancel(&mut self) {
        if let Some(i) = self.buttons.iter().position(|b| b.is_cancel) {
            self.activate(i);
        } else if self.dismissible {
            self.close();
        }
    }

    fn activate(&mut self, idx: usize) {
        if let Some(b) = self.buttons.get(idx) {
            (b.on_activate)();
        }
        self.close();
    }

    fn close(&mut self) {
        self.open.set(false);
        self.hovered = None;
    }

    fn text_w(&self, s: &str) -> f64 {
        s.chars().count() as f64 * (self.base.font * MONO_ADVANCE_RATIO) as f64
    }

    fn line_h(&self) -> f64 {
        (self.base.font * MONO_LINE_RATIO) as f64
    }

    fn btn_w(&self, b: &ModalButton) -> f64 {
        self.text_w(&b.display()) + 2.0 * BTN_PAD_X
    }

    /// Lay out the panel + content centered in the cached viewport.
    fn rects(&self) -> Rects {
        let vp = self.viewport.get();
        let line = self.line_h();
        let widths: Vec<f64> = self.buttons.iter().map(|b| self.btn_w(b)).collect();
        let actions_w = widths.iter().sum::<f64>()
            + (widths.len().saturating_sub(1)) as f64 * BTN_GAP;

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
        // Right-aligned button row along the panel bottom, laid out left→right.
        let by = py + panel_h - PAD - BTN_H;
        let mut x = px + panel_w - PAD - actions_w;
        let mut buttons = Vec::with_capacity(widths.len());
        for w in &widths {
            buttons.push(Rectangle::new(Point::new(x, by), Size::new(*w, BTN_H)));
            x += w + BTN_GAP;
        }
        Rects { panel, title, message, buttons }
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
        let (background, surface, accent, danger, foreground, muted, shadow, shadow_blur, ctrl_radius, radius, on_accent, on_danger) = {
            let t = cx.theme();
            (t.colors.background, t.colors.surface, t.colors.accent, t.colors.danger, t.colors.foreground, t.colors.muted, t.shadow_color(), t.colors.shadow.blur, t.colors.control_radius(), t.colors.border_radius, t.colors.on(t.colors.accent), t.colors.on(t.colors.danger))
        };
        let r = self.rects();

        cx.with_overlay(|cx| {
            // Scrim over the whole viewport.
            let vp = self.viewport.get();
            let scrim = if vp.w.is_finite() {
                Rectangle::new(Point::new(0.0, 0.0), vp)
            } else {
                r.panel
            };
            cx.rect(scrim, background.with_alpha(SCRIM_ALPHA), None, 0.0, None);

            // Lift the dialog off the scrim with a soft drop shadow (drawn behind
            // the panel). Independent of the glow/border tokens, so the modal stays
            // identifiable even at border_width == 0.
            cx.drop_shadow(r.panel, radius, Shadow { color: shadow, radius: shadow_blur * SHADOW_BLUR_MULT, dx: 0.0, dy: SHADOW_DROP });

            // Panel: surface fill + the shared Pane/DockFrame corner-bracket reticle.
            cx.rect(r.panel, surface, None, radius, None);
            cx.bracket_frame(r.panel);
            cx.text(r.title, &self.title, foreground, self.base.font, TextAlign::Start, true);
            cx.text(r.message, &self.message, muted, self.base.font, TextAlign::Start, false);

            // Buttons. Cancel/ghost = bordered; primary/danger = toned fill. The
            // focused button gets an accent focus ring + glow (keyboard cue).
            for (i, (b, rect)) in self.buttons.iter().zip(&r.buttons).enumerate() {
                let hov = self.hovered == Some(i);
                let is_focused = self.focused == i;
                let tone = if b.danger { danger } else { accent };
                if b.is_cancel {
                    // Ghost button: border + subtle hover fill; dark glyph on hover.
                    cx.rect(
                        *rect,
                        foreground.with_alpha(if hov { 26 } else { 0 }),
                        cx.border(muted.with_alpha(180)),
                        ctrl_radius,
                        None,
                    );
                    cx.text(*rect, &b.display(), foreground, self.base.font, TextAlign::Center, false);
                } else {
                    cx.rect(
                        *rect,
                        tone.with_alpha(if hov { 235 } else { 200 }),
                        None,
                        ctrl_radius,
                        (hov).then_some(Glow { color: tone, radius: 8.0, intensity: 0.3 }),
                    );
                    let on_tone = if b.danger { on_danger } else { on_accent };
                    cx.text(*rect, &b.display(), on_tone, self.base.font, TextAlign::Center, true);
                }
                // Focus ring (accent border + soft glow) over whichever is focused.
                if is_focused {
                    cx.rect(
                        *rect,
                        accent.with_alpha(0),
                        Some(Border { color: accent, width: 2.0 }),
                        ctrl_radius,
                        Some(Glow { color: accent, radius: 6.0, intensity: 0.35 }),
                    );
                }
            }
        });
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        let r = self.rects();
        let button_at = |pos: &Point| r.buttons.iter().position(|rect| rect.contains(*pos));
        match ev {
            Event::PointerMoved { pos } => {
                let h = button_at(pos);
                if self.hovered != h {
                    self.hovered = h;
                }
                Handled::Yes
            }
            Event::PointerPressed { pos } => {
                if let Some(i) = button_at(pos) {
                    self.activate(i);
                } else if !r.panel.contains(*pos) && self.dismissible {
                    // Click on the scrim dismisses (= cancel) unless non-dismissible.
                    self.request_cancel();
                }
                // Always swallow input while open (modal).
                Handled::Yes
            }
            Event::Key { key: GridKey::Escape, pressed: true } => {
                if self.dismissible {
                    self.request_cancel();
                }
                Handled::Yes
            }
            Event::Key { key: GridKey::Enter | GridKey::Space, pressed: true } => {
                self.activate_focused();
                Handled::Yes
            }
            Event::Key { key: GridKey::Tab | GridKey::ArrowRight, pressed: true } => {
                self.focus_next();
                Handled::Yes
            }
            Event::Key { key: GridKey::ArrowLeft, pressed: true } => {
                self.focus_prev();
                Handled::Yes
            }
            Event::Key { key: GridKey::Char(c), pressed: true } => {
                self.activate_shortcut(*c);
                Handled::Yes
            }
            // Swallow every other key while the dialog owns input.
            Event::Key { .. } => Handled::Yes,
            // A modal blocks the content behind it — swallow scroll too, so the
            // page doesn't scroll underneath the scrim.
            Event::Scroll { .. } => Handled::Yes,
            _ => Handled::No,
        }
    }
}

impl LayoutExt for Modal {}
