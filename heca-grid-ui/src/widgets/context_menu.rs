//! [`ContextMenu`] — a cursor-anchored action menu (overlay).
//!
//! The pointer counterpart to the keyboard pick flows: a floating list of
//! [`MenuEntry`]s opened at a point (typically the right-click cursor). It uses the
//! same input-capturing overlay contract as [`CommandPalette`](super::CommandPalette)
//! / [`Modal`](super::Modal): `overlay_active` + `focusable` only while open, content
//! **drawn + hit-tested manually** on the overlay layer.
//!
//! Open/close and the anchor point are **host-owned** [`Signal`]s, so right-click
//! detection (which lives at the app/winit level — grid-ui pointer events carry no
//! button) stays out of the widget: the host sets the anchor to the cursor and flips
//! `open`. Each entry carries a label, an optional [`Glyph`] icon, an optional
//! shortcut hint, a `danger` flag (destructive actions, e.g. Close/Delete), an
//! `enabled` flag, and an `on_select` callback fired when chosen. Navigation is built
//! in (↑/↓, Enter, Esc) and also exposed as intents so a host can bind its own keys.

use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Glow, TextAlign};
use crate::widgets::Glyph;
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::Cell;

/// One entry in a [`ContextMenu`].
pub struct MenuEntry {
    label: String,
    icon: Option<Glyph>,
    key: Option<char>,
    shortcut: Option<String>,
    danger: bool,
    enabled: bool,
    on_select: Box<dyn Fn()>,
}

impl MenuEntry {
    /// An entry with `label` that runs `on_select` when chosen.
    pub fn new(label: impl Into<String>, on_select: impl Fn() + 'static) -> Self {
        Self {
            label: label.into(),
            icon: None,
            key: None,
            shortcut: None,
            danger: false,
            enabled: true,
            on_select: Box::new(on_select),
        }
    }

    /// An optional leading icon.
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// A **quick-pick key** rendered as a [`KeyHint`](super::KeyHint)-style keycap on
    /// the right; pressing it (case-insensitive) activates the entry immediately.
    pub fn key(mut self, key: char) -> Self {
        self.key = Some(key);
        self
    }

    /// An optional textual shortcut hint (e.g. `"prefix+x"`), drawn left of the
    /// quick-pick keycap. Informational only — not pressable inside the menu.
    pub fn shortcut(mut self, hint: impl Into<String>) -> Self {
        self.shortcut = Some(hint.into());
        self
    }

    /// Mark this entry as **destructive** — its label renders in the `danger` hue.
    pub fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }

    /// Enable/disable the entry. A disabled entry is dimmed and cannot be selected.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Panel inner padding (around the list).
const PAD: f64 = 4.0;
/// Vertical / horizontal padding inside each row.
const ROW_PAD_Y: f64 = 6.0;
const ROW_PAD_X: f64 = 12.0;
/// Gap between an icon and the label.
const ICON_GAP: f64 = 10.0;
/// Minimum gap between the label and a right-aligned shortcut.
const SHORTCUT_GAP: f64 = 28.0;
/// Content-width clamp for the panel.
const MIN_W: f64 = 160.0;
const MAX_W: f64 = 380.0;
/// Inset of the anchor from the cursor so the menu doesn't sit directly under it.
const ANCHOR_INSET: f64 = 2.0;
/// Quick-pick keycap metrics (mirror [`KeyHint`](super::KeyHint)).
const KEYCAP_PAD_X: f64 = 0.42; // fraction of font
const KEYCAP_PAD_Y: f64 = 0.20;
const GLYPH_ADV_FRAC: f64 = 0.62; // per-glyph advance estimate, fraction of font
/// Inset of the keycap from the row's right edge.
const KEYCAP_INSET: f64 = 8.0;

/// A cursor-anchored action menu.
pub struct ContextMenu {
    base: Base,
    entries: Vec<MenuEntry>,
    selected: usize,
    open: Signal<bool>,
    /// Host-owned anchor (top-left preferred position; clamped to the viewport).
    anchor: Signal<Point>,
    viewport: Cell<Size>,
    /// Panel rect cached at paint, so overlay damage targets just the menu.
    panel: Cell<Rectangle>,
    /// Fired when the menu is **dismissed** (Esc / outside-click) — not when an entry is
    /// selected. The host points this at its overlay-close path (e.g. emit `CloseOverlay`),
    /// mirroring [`Dialog::on_dismiss`](super::Dialog).
    on_dismiss: Option<Box<dyn Fn()>>,
}

impl ContextMenu {
    /// A new, empty (closed) menu.
    pub fn new() -> Self {
        Self {
            base: Base::new(),
            entries: Vec::new(),
            selected: 0,
            open: signal(false),
            anchor: signal(Point::new(0.0, 0.0)),
            viewport: Cell::new(Size::new(f64::MAX, f64::MAX)),
            panel: Cell::new(Rectangle::from_size(Size::new(0.0, 0.0))),
            on_dismiss: None,
        }
    }

    /// Add an entry.
    pub fn entry(mut self, e: MenuEntry) -> Self {
        self.entries.push(e);
        self
    }

    /// Set the initial open state.
    pub fn open(self, open: bool) -> Self {
        self.open.set(open);
        self
    }

    /// Set the initial anchor (top-left preferred position).
    pub fn anchor(self, at: Point) -> Self {
        self.anchor.set(at);
        self
    }

    /// The open-state signal — the host flips it on right-click / dismiss.
    pub fn open_signal(&self) -> Signal<bool> {
        self.open
    }

    /// The anchor signal — the host sets it to the cursor before opening.
    pub fn anchor_signal(&self) -> Signal<Point> {
        self.anchor
    }

    fn is_open(&self) -> bool {
        self.open.get_untracked()
    }

    fn line_h(&self) -> f64 {
        (self.base.font * MONO_LINE_RATIO) as f64
    }

    fn row_h(&self) -> f64 {
        self.line_h() + 2.0 * ROW_PAD_Y
    }

    /// Size of a single-char quick-pick keycap at the current font.
    fn keycap_size(&self) -> Size {
        let font = self.base.font as f64;
        Size::new(
            font * GLYPH_ADV_FRAC + 2.0 * font * KEYCAP_PAD_X,
            font + 2.0 * font * KEYCAP_PAD_Y,
        )
    }

    /// Index of the first enabled entry at or after `from`, wrapping search forward
    /// then backward; `None` if every entry is disabled.
    fn enabled_from(&self, from: usize, forward: bool) -> Option<usize> {
        let n = self.entries.len();
        if n == 0 {
            return None;
        }
        let mut i = from.min(n - 1);
        for _ in 0..n {
            if self.entries[i].enabled {
                return Some(i);
            }
            i = if forward {
                (i + 1) % n
            } else {
                (i + n - 1) % n
            };
        }
        None
    }

    /// Move the selection to the next enabled entry (↓).
    pub fn select_next(&mut self) {
        if let Some(i) = self.enabled_from((self.selected + 1).min(self.entries.len().saturating_sub(1)), true)
        {
            // Skip back onto `selected` only if it was the sole enabled entry.
            self.selected = if i == self.selected {
                self.enabled_from(self.selected, true).unwrap_or(i)
            } else {
                i
            };
        }
    }

    /// Move the selection to the previous enabled entry (↑).
    pub fn select_prev(&mut self) {
        let prev = self.selected.saturating_sub(1);
        if let Some(i) = self.enabled_from(prev, false) {
            self.selected = i;
        }
    }

    /// Run the selected entry (if enabled) and close.
    pub fn run_selected(&mut self) {
        if let Some(e) = self.entries.get(self.selected)
            && e.enabled
        {
            (e.on_select)();
        }
        self.close();
    }

    /// Set the callback fired when the menu is **dismissed** (Esc / outside-click). The host
    /// wires this to its overlay-close path (mirrors [`Dialog::on_dismiss`](super::Dialog)).
    pub fn on_dismiss(mut self, f: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Box::new(f));
        self
    }

    /// Dismiss (Esc / outside-click): fire [`on_dismiss`](Self::on_dismiss), then close. Distinct
    /// from [`run_selected`](Self::run_selected), which is a *choice*, not a dismissal.
    fn fire_dismiss(&mut self) {
        if let Some(f) = &self.on_dismiss {
            f();
        }
        self.close();
    }

    fn close(&mut self) {
        self.open.set(false);
        self.selected = self.enabled_from(0, true).unwrap_or(0);
    }

    /// Panel rect for the current viewport + anchor + entry count. The anchor is the
    /// preferred top-left; the panel is clamped (and flipped) to stay on-screen.
    fn layout(&self) -> Rectangle {
        let vp = self.viewport.get();
        let font = self.base.font as f64;
        let adv = (self.base.font * MONO_ADVANCE_RATIO) as f64;
        let row_h = self.row_h();

        // Content width = widest row.
        let keycap_w = self.keycap_size().w;
        let mut content_w: f64 = 0.0;
        for e in &self.entries {
            let mut w = 2.0 * ROW_PAD_X;
            if e.icon.is_some() {
                w += font * 1.05 + ICON_GAP;
            }
            w += e.label.chars().count() as f64 * adv;
            if let Some(s) = &e.shortcut {
                w += SHORTCUT_GAP + s.chars().count() as f64 * adv;
            }
            if e.key.is_some() {
                w += SHORTCUT_GAP + keycap_w + KEYCAP_INSET;
            }
            content_w = content_w.max(w);
        }
        let panel_w = (content_w + 2.0 * PAD).clamp(MIN_W, MAX_W);
        let panel_h = 2.0 * PAD + self.entries.len().max(1) as f64 * row_h;

        let a = self.anchor.get();
        let (vw, vh) = if vp.w.is_finite() { (vp.w, vp.h) } else { (panel_w, panel_h) };
        // Prefer down-right of the anchor; flip/clamp to keep the panel on-screen.
        let mut x = a.x + ANCHOR_INSET;
        if x + panel_w > vw {
            x = (a.x - panel_w - ANCHOR_INSET).max(0.0);
        }
        x = x.clamp(0.0, (vw - panel_w).max(0.0));
        let mut y = a.y + ANCHOR_INSET;
        if y + panel_h > vh {
            y = (a.y - panel_h - ANCHOR_INSET).max(0.0);
        }
        y = y.clamp(0.0, (vh - panel_h).max(0.0));

        Rectangle::new(Point::new(x, y), Size::new(panel_w, panel_h))
    }

    fn row_rect(&self, panel: Rectangle, idx: usize) -> Rectangle {
        let row_h = self.row_h();
        Rectangle::new(
            Point::new(panel.loc.x + PAD, panel.loc.y + PAD + idx as f64 * row_h),
            Size::new(panel.size.w - 2.0 * PAD, row_h),
        )
    }
}

impl Default for ContextMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ContextMenu {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

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
        let (background, surface, accent, glow_c, foreground, muted, danger, ctrl_radius, radius) = {
            let t = cx.theme();
            (
                t.colors.background,
                t.colors.surface,
                t.colors.accent,
                t.colors.glow,
                t.colors.foreground,
                t.colors.muted,
                t.colors.danger,
                t.colors.control_radius(),
                t.colors.border_radius,
            )
        };
        let font = self.base.font;
        let panel = self.layout();
        self.panel.set(panel);

        cx.with_overlay(|cx| {
            // Panel — a glowing accent-bordered surface (no scrim: context menus
            // dismiss on outside-click rather than darkening the whole view).
            let panel_border = cx.border(accent.with_alpha(cx.theme().colors.interaction.panel_border));
            cx.rect(
                panel,
                surface,
                panel_border,
                radius,
                // Match the CommandPalette panel glow so the two overlays read as
                // one family (radius still scales with the theme `glow_size`).
                Some(Glow { color: glow_c, radius: 12.0, intensity: 0.3 }),
            );

            for (i, e) in self.entries.iter().enumerate() {
                let row = self.row_rect(panel, i);
                let is_sel = i == self.selected && e.enabled;
                if is_sel {
                    let row_border = cx.border(accent.with_alpha(cx.theme().colors.interaction.panel_row_border));
                    cx.rect(row, accent.with_alpha(cx.theme().colors.interaction.panel_row_fill), row_border, ctrl_radius, None);
                    cx.rect(
                        Rectangle::new(
                            Point::new(row.loc.x, row.loc.y + row.size.h * 0.2),
                            Size::new(2.5, row.size.h * 0.6),
                        ),
                        accent,
                        None,
                        1.0,
                        None,
                    );
                }
                // Base text color: danger entries read red; disabled ones dim.
                let base_color: Color = if e.danger { danger } else { foreground };
                let text_color = if !e.enabled {
                    muted.with_alpha(cx.theme().colors.interaction.menu_shortcut_dim)
                } else if is_sel {
                    base_color
                } else {
                    muted.lerp(base_color, 0.75)
                };

                let mut text_x = row.loc.x + ROW_PAD_X;
                if let Some(g) = e.icon
                    && let Some(ch) = g.primary_char()
                {
                    let isz = font * 1.05;
                    let irect =
                        Rectangle::new(Point::new(text_x, row.loc.y), Size::new(isz as f64, row.size.h));
                    cx.icon(irect, &ch.to_string(), text_color, isz);
                    text_x += font as f64 * 1.05 + ICON_GAP;
                }
                let lbl_rect =
                    Rectangle::new(Point::new(text_x, row.loc.y), Size::new(row.size.w, row.size.h));
                cx.text(lbl_rect, &e.label, text_color, font, TextAlign::Start, false);

                // Quick-pick keycap (rightmost) — KeyHint-style accent cap with a dark
                // bold glyph; pressing the key activates the entry.
                let mut right_edge = row.loc.x + row.size.w - KEYCAP_INSET;
                if let Some(k) = e.key {
                    let ks = self.keycap_size();
                    let cap = Rectangle::new(
                        Point::new(right_edge - ks.w, row.loc.y + (row.size.h - ks.h) / 2.0),
                        ks,
                    );
                    let cap_radius = ctrl_radius.min((ks.h / 2.0) as f32);
                    let cap_color = if e.enabled { accent } else { muted };
                    cx.rect(
                        cap,
                        cap_color.with_alpha(cx.theme().colors.interaction.keycap),
                        None,
                        cap_radius,
                        Some(Glow { color: glow_c, radius: 5.0, intensity: 0.4 }),
                    );
                    cx.text(cap, &k.to_string(), background, font, TextAlign::Center, true);
                    right_edge = cap.loc.x - SHORTCUT_GAP;
                }
                // Textual shortcut hint, right-aligned left of the keycap.
                if let Some(s) = &e.shortcut {
                    let srect = Rectangle::new(
                        Point::new(row.loc.x, row.loc.y),
                        Size::new((right_edge - row.loc.x).max(0.0), row.size.h),
                    );
                    cx.text(srect, s, muted.with_alpha(cx.theme().colors.interaction.menu_shortcut), font, TextAlign::End, false);
                }
            }
        });
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        match ev {
            Event::Key { key, pressed: true } => {
                match key {
                    GridKey::Escape => self.fire_dismiss(),
                    GridKey::Enter => self.run_selected(),
                    GridKey::ArrowDown => self.select_next(),
                    GridKey::ArrowUp => self.select_prev(),
                    // A quick-pick key activates its entry directly (case-insensitive).
                    GridKey::Char(c) => {
                        if let Some(i) = self
                            .entries
                            .iter()
                            .position(|e| e.enabled && e.key.is_some_and(|k| k.eq_ignore_ascii_case(c)))
                        {
                            self.selected = i;
                            self.run_selected();
                        }
                    }
                    _ => {}
                }
                Handled::Yes
            }
            Event::PointerMoved { pos } => {
                let panel = self.layout();
                for i in 0..self.entries.len() {
                    if self.entries[i].enabled && self.row_rect(panel, i).contains(*pos) {
                        self.selected = i;
                        break;
                    }
                }
                Handled::Yes
            }
            Event::PointerPressed { pos } => {
                let panel = self.layout();
                let mut ran = false;
                for i in 0..self.entries.len() {
                    if self.entries[i].enabled && self.row_rect(panel, i).contains(*pos) {
                        self.selected = i;
                        self.run_selected();
                        ran = true;
                        break;
                    }
                }
                // A click outside the panel (or on a disabled row) dismisses.
                if !ran && !panel.contains(*pos) {
                    self.fire_dismiss();
                }
                Handled::Yes
            }
            // Swallow all other input while open.
            _ => Handled::Yes,
        }
    }

    /// The menu paints on the overlay layer, away from its layout `bounds`, so
    /// overlay damage targets the cached panel rect.
    fn damage_bounds(&self) -> Rectangle {
        if self.is_open() {
            self.panel.get()
        } else {
            self.base.bounds
        }
    }
}

impl LayoutExt for ContextMenu {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Event, GridKey};
    use std::cell::Cell;
    use std::rc::Rc;

    /// Esc and outside-click fire `on_dismiss` (the host's overlay-close path); selecting an
    /// entry runs it and closes but is **not** a dismissal.
    #[test]
    fn dismiss_fires_on_esc_and_outside_click_not_on_select() {
        let dismissed = Rc::new(Cell::new(0u32));
        let ran = Rc::new(Cell::new(0u32));
        let (d, r) = (dismissed.clone(), ran.clone());
        let mut m = ContextMenu::new()
            .entry(MenuEntry::new("Rename", move || r.set(r.get() + 1)).key('r'))
            .open(true)
            .on_dismiss(move || d.set(d.get() + 1));

        // Esc → dismiss (fires callback, closes), no entry run.
        m.event(&Event::Key { key: GridKey::Escape, pressed: true });
        assert_eq!(dismissed.get(), 1, "Esc fired on_dismiss");
        assert_eq!(ran.get(), 0);
        assert!(!m.is_open(), "closed after dismiss");

        // Reopen; a quick-key selection runs the entry and does NOT fire dismiss.
        m.open.set(true);
        m.event(&Event::Key { key: GridKey::Char('r'), pressed: true });
        assert_eq!(ran.get(), 1, "entry ran on quick-key");
        assert_eq!(dismissed.get(), 1, "a selection is not a dismissal");
    }
}
