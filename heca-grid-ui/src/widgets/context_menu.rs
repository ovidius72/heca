//! **Menus, split by what each part actually knows.**
//!
//! | | |
//! |---|---|
//! | [`MenuItem`] | one row: label, icon, enabled, danger, `on_click` |
//! | [`Menu`] | a titled list of items. **Content only** — it knows nothing about triggers, anchors or keys |
//! | [`ContextMenu`] | contains a `Menu` and shows it on right-click or the host's `open_context_menu`, anchored at the cursor or under the widget |
//! | `MenuBar` | *not built yet*: contains `Menu`s and shows them as a strip, on a click of a title or its own keybinding |
//!
//! The split is at the joint a menu bar proves is real: a `MenuBar` shows the **same `Menu` value**
//! as a strip, opening on a click of its title with its own keyboard convention. The trigger, the
//! anchor and the shortcut belong to whatever contains the menu; the menu is content, and stays
//! reusable across every surface that shows one. A submenu is the same shape again — a `MenuItem`
//! holding a `Menu`.
//!
//! The pointer counterpart to the keyboard pick flows: a floating list of
//! [`MenuEntry`]s opened at a point (typically the right-click cursor). It uses the
//! same input-capturing overlay contract as [`CommandPalette`](super::CommandPalette):
//! `overlay_active` + `focusable` only while open, content
//! **drawn + hit-tested manually** on the overlay layer.
//!
//! Open/close and the anchor point are **host-owned** [`Signal`]s, so right-click
//! detection (which lives at the app/winit level — grid-ui pointer events carry no
//! button) stays out of the widget: the host sets the anchor to the cursor and flips
//! `open`. Each entry carries a label, an optional [`Glyph`] icon, an optional
//! shortcut hint, a `danger` flag (destructive actions, e.g. Close/Delete), an
//! `enabled` flag, and an `on_select` callback fired when chosen. Navigation carries
//! **no hardcoded keys**: as a vertical list the widget responds to the semantic
//! [`Event::Widget`] intents `MenuUp`/`MenuDown`/`Activate`/`Dismiss`, which the host
//! resolves from the configurable `[keys.widgets]` bindings. Raw [`Event::Key`] is only a
//! quick-pick letter that activates its entry directly. (App wiring: `widget-keys-config`.)

use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx, WidgetIntent};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Glow, TextAlign, TextStyle};
use crate::widgets::key_hint::{keycap_size, paint_keycap, KeycapVariant};
use crate::widgets::{paint_panel_chrome, place_at_point, Glyph, PanelChrome, PanelElevation};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::Cell;

/// One row in a [`ContextMenu`]: what it says, and what it does.
///
/// ```
/// use heca_grid_ui::prelude::*;
/// use heca_grid_ui::widgets::{ContextMenu, Glyph, MenuItem};
///
/// let menu = ContextMenu::new()
///     .child(MenuItem::new("Save as…").icon(Glyph::File).on_click(|| {}))
///     .child(MenuItem::new("Revert").enabled(false).on_click(|| {}));
/// ```
///
/// Read top to bottom: the label first, the behaviour after — which is the order a menu is written
/// in and the order it is read on screen.
#[derive(Clone)]
pub struct MenuItem {
    label: String,
    icon: Option<Glyph>,
    key: Option<char>,
    shortcut: Option<String>,
    danger: bool,
    enabled: bool,
    on_select: std::rc::Rc<dyn Fn()>,
}

/// The name this row had when only a host built menus, kept so existing callers still compile.
pub type MenuEntry = MenuItem;

#[heca_grid_ui_macros::props]
impl MenuItem {
    /// A row labelled `label` that does nothing yet — give it behaviour with
    /// [`on_click`](MenuItem::on_click).
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            key: None,
            shortcut: None,
            danger: false,
            enabled: true,
            on_select: std::rc::Rc::new(|| {}),
        }
    }

    /// What this row does when chosen. Replaces whatever was there.
    ///
    /// A **plain closure**: this library knows nothing about actions, and an item that took an
    /// action id would tie every menu in it to one host's dispatch. Firing a catalogued action
    /// inside the closure is the author's choice — and the way to stay reachable from the command
    /// palette and RPC, which a closure alone is not.
    #[heca_grid_ui_macros::host_only("a closure — behaviour crosses a description as an Intent")]
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_select = std::rc::Rc::new(f);
        self
    }

    /// An optional leading icon.
    #[heca_grid_ui_macros::prop]
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// A **quick-pick key** rendered as a [`KeyHint`](super::KeyHint)-style keycap on
    /// the right; pressing it (case-insensitive) activates the entry immediately.
    #[heca_grid_ui_macros::host_only("unsupported argument type (char)")]
    pub fn key(mut self, key: char) -> Self {
        self.key = Some(key);
        self
    }

    /// An optional textual shortcut hint (e.g. `"prefix+x"`), drawn left of the
    /// quick-pick keycap. Informational only — not pressable inside the menu.
    #[heca_grid_ui_macros::prop]
    pub fn shortcut(mut self, hint: impl Into<String>) -> Self {
        self.shortcut = Some(hint.into());
        self
    }

    /// Mark this entry as **destructive** — its label renders in the `danger` hue.
    #[heca_grid_ui_macros::prop]
    pub fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }

    /// Enable/disable the entry. A disabled entry is dimmed and cannot be selected.
    #[heca_grid_ui_macros::prop]
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
/// Inset of the quick-pick keycap from the row's right edge. The keycap chip itself
/// is drawn by the shared [`paint_keycap`] primitive (sized via [`keycap_size`]) — the
/// menu never re-derives the chip metrics.
const KEYCAP_INSET: f64 = 8.0;
/// Quick-pick keycap font as a fraction of the menu font — a compact chip, smaller than the
/// row label (mirrors the sub-font scale the `Tag` chip uses).
const KEYCAP_FONT_SCALE: f32 = 0.72;

/// A cursor-anchored action menu.
pub struct ContextMenu {
    base: Base,
    entries: Vec<MenuItem>,
    selected: usize,
    open: Signal<bool>,
    /// Host-owned anchor (top-left preferred position; clamped to the viewport).
    anchor: Signal<Point>,
    /// When true the panel is **centered on the anchor** (anchor = desired center) instead of
    /// placed down-right of it (anchor = top-left). Used by keyboard/RPC-opened menus so they
    /// stay centered on screen rather than offset to the bottom-right of the window center.
    centered: bool,
    viewport: Cell<Size>,
    /// Panel rect cached at paint, so overlay damage targets just the menu.
    panel: Cell<Rectangle>,
    /// Fired when the menu is **dismissed** (Esc / outside-click) — not when an entry is
    /// selected. The host points this at its overlay-close path (e.g. emit `CloseOverlay`),
    /// mirroring [`Dialog::on_dismiss`](super::Dialog).
    on_dismiss: Option<Box<dyn Fn()>>,
    /// Fired after an entry ran, whatever it was — the host's hook for taking the layer down.
    after_select: Option<Box<dyn Fn()>>,
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
            centered: false,
            viewport: Cell::new(Size::new(f64::MAX, f64::MAX)),
            panel: Cell::new(Rectangle::from_size(Size::new(0.0, 0.0))),
            on_dismiss: None,
            after_select: None,
        }
    }

    /// Add an entry.
    #[heca_grid_ui_macros::host_only("a composed value, not a scalar — built from `children`")]
    pub fn entry(mut self, e: MenuItem) -> Self {
        self.entries.push(e);
        self
    }

    /// Add an entry — the name that reads right when the items are written as content.
    ///
    /// The same call as [`entry`](ContextMenu::entry). An inherent method, so it wins over
    /// [`Parent::child`](crate::builders::Parent::child) for a menu: a menu's rows are drawn from
    /// data rather than laid out as child widgets, which is why this widget is one of the few that
    /// does not take `impl Component` here.
    pub fn child(self, item: MenuItem) -> Self {
        self.entry(item)
    }

    /// Open or close the panel.
    #[heca_grid_ui_macros::prop]
    pub fn open(self, open: bool) -> Self {
        self.open.set(open);
        self
    }

    /// The preferred top-left position, clamped into the viewport at paint.
    #[heca_grid_ui_macros::prop]
    pub fn anchor(self, at: Point) -> Self {
        self.anchor.set(at);
        self
    }

    /// Centre the panel on the anchor (anchor = desired centre) instead of placing its top-left
    /// there — for a trigger with no pointer position.
    #[heca_grid_ui_macros::prop]
    pub fn centered(mut self, on: bool) -> Self {
        self.centered = on;
        self
    }

    /// The rows' labels, in order — what a caller (or a test) can read back off a built panel
    /// without reaching into its entries.
    pub fn entry_labels(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.label.clone()).collect()
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

    /// Compact keycap font (a fraction of the menu font) — shared by sizing + glyph so the chip
    /// and its letter stay in proportion.
    fn keycap_font(&self) -> f32 {
        self.base.font * KEYCAP_FONT_SCALE
    }

    /// Size of a quick-pick keycap for `key`, via the shared [`keycap_size`] metric (single
    /// source with [`KeyHint`](super::KeyHint)) at the compact keycap font.
    fn keycap_size(&self, key: char) -> Size {
        keycap_size(self.keycap_font(), &key.to_string())
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
            if let Some(f) = &self.after_select {
                f();
            }
        }
        self.close();
    }

    /// Called **after an entry ran**, whatever the entry was.
    ///
    /// The host's seam for taking the panel's layer down. It is here rather than in each item
    /// because an item is a plain closure that knows nothing about layers — and requiring every
    /// author to close the menu they opened is a rule that gets forgotten exactly once per menu.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn after_select(mut self, f: impl Fn() + 'static) -> Self {
        self.after_select = Some(Box::new(f));
        self
    }

    /// Set the callback fired when the menu is **dismissed** (Esc / outside-click). The host
    /// wires this to its overlay-close path (mirrors [`Dialog::on_dismiss`](super::Dialog)).
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
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
            if let Some(k) = e.key {
                w += SHORTCUT_GAP + self.keycap_size(k).w + KEYCAP_INSET;
            }
            content_w = content_w.max(w);
        }
        let panel_w = (content_w + 2.0 * PAD).clamp(MIN_W, MAX_W);
        let panel_h = 2.0 * PAD + self.entries.len().max(1) as f64 * row_h;

        // Placement (down-right of the cursor, flip up-left, clamp — or centered on
        // the anchor for keyboard/RPC-opened menus) lives in the shared overlay
        // placement authority so it is not re-derived per widget.
        place_at_point(
            self.anchor.get(),
            Size::new(panel_w, panel_h),
            vp,
            ANCHOR_INSET,
            self.centered,
        )
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
        // NB: the panel's own surface fill + corner radius are read by the shared
        // `paint_panel_chrome`, so they are deliberately not pulled out here.
        let (accent, glow_c, foreground, muted, danger, ctrl_radius) = {
            let t = cx.theme();
            (
                t.colors.accent,
                t.colors.glow,
                t.colors.foreground,
                t.colors.muted,
                t.colors.danger,
                t.colors.control_radius(),
            )
        };
        let font = self.base.font;
        let panel = self.layout();
        self.panel.set(panel);

        cx.with_overlay(|cx| {
            // Panel — the SHARED overlay panel chrome (drop shadow + theme surface
            // fill + bracket reticle), so a menu reads as the same surface as every
            // other overlay panel. On top of it the menu keeps its own identity: an
            // accent edge and the glow it shares with the CommandPalette (the glow
            // radius still scales with the theme `glow_size`). No scrim — context
            // menus dismiss on outside-click rather than darkening the whole view.
            let panel_border = cx.border(accent.with_alpha(cx.theme().colors.interaction.panel_border));
            paint_panel_chrome(
                cx,
                panel,
                PanelChrome {
                    border: panel_border,
                    glow: Some(Glow { color: glow_c, radius: 12.0, intensity: 0.3 }),
                    elevation: PanelElevation::Panel,
                },
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
                cx.text(lbl_rect, &e.label, text_color, font, TextAlign::Start, TextStyle::REGULAR);

                // Quick-pick keycap (rightmost) — the shared bordered keycap primitive
                // (never hand-drawn); pressing the key activates the entry.
                let mut right_edge = row.loc.x + row.size.w - KEYCAP_INSET;
                if let Some(k) = e.key {
                    let ks = self.keycap_size(k);
                    let cap = Rectangle::new(
                        Point::new(right_edge - ks.w, row.loc.y + (row.size.h - ks.h) / 2.0),
                        ks,
                    );
                    let cap_color = if e.enabled { accent } else { muted };
                    paint_keycap(cx, cap, &k.to_string(), self.keycap_font(), Some(cap_color), KeycapVariant::Bordered);
                    right_edge = cap.loc.x - SHORTCUT_GAP;
                }
                // Textual shortcut hint, right-aligned left of the keycap.
                if let Some(s) = &e.shortcut {
                    let srect = Rectangle::new(
                        Point::new(row.loc.x, row.loc.y),
                        Size::new((right_edge - row.loc.x).max(0.0), row.size.h),
                    );
                    cx.text(srect, s, muted.with_alpha(cx.theme().colors.interaction.menu_shortcut), font, TextAlign::End, TextStyle::REGULAR);
                }
            }
        });
    }

    /// Owns its walk. Its entries are drawn from `MenuEntry` values rather than mounted as
    /// children, and while open it captures input over its panel.
    fn routes_own_subtree(&self) -> bool {
        true
    }

    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        match ev {
            // Nav is host-resolved from the configurable `[keys.widgets]` bindings and
            // arrives as a semantic `WidgetIntent` — the menu carries NO hardcoded nav keys.
            // A vertical list: it uses `MenuUp`/`MenuDown` (not the horizontal `Item*`).
            Event::Widget(intent) => match intent {
                WidgetIntent::Dismiss => {
                    self.fire_dismiss();
                    Handled::Yes
                }
                WidgetIntent::Activate => {
                    self.run_selected();
                    Handled::Yes
                }
                WidgetIntent::MenuDown => {
                    self.select_next();
                    Handled::Yes
                }
                WidgetIntent::MenuUp => {
                    self.select_prev();
                    Handled::Yes
                }
                _ => Handled::No,
            },
            // Raw keys are only quick-pick letters: a letter activates its entry
            // directly (case-insensitive).
            Event::Key { key: GridKey::Char(c), pressed: true } => {
                if let Some(i) = self
                    .entries
                    .iter()
                    .position(|e| e.enabled && e.key.is_some_and(|k| k.eq_ignore_ascii_case(c)))
                {
                    self.selected = i;
                    self.run_selected();
                    Handled::Yes
                } else {
                    // Not a quick-pick letter — report unhandled so the host can resolve it as a
                    // `WidgetIntent` (nav/activate). Modal capture is the HOST's job (the overlay
                    // branch swallows), not a `Handled::Yes` hardcoded here.
                    Handled::No
                }
            }
            // Other raw keys (arrows, Enter, Tab) are NOT swallowed: report unhandled so the host
            // offers the resolved `WidgetIntent`. The host owns modal capture.
            Event::Key { pressed: true, .. } => Handled::No,
            // The menu's rows are drawn from data, not from child widgets, so it hit-tests its
            // own row rects — but only inside a panel the router already decided the pointer is
            // over (`hit_bounds`). A move that is not over this menu never gets here.
            Event::PointerMove(p) => {
                let panel = self.layout();
                for i in 0..self.entries.len() {
                    if self.entries[i].enabled && self.row_rect(panel, i).contains(p.pos) {
                        self.selected = i;
                        break;
                    }
                }
                Handled::Yes
            }
            Event::PointerDown(p) => {
                let panel = self.layout();
                let mut ran = false;
                for i in 0..self.entries.len() {
                    if self.entries[i].enabled && self.row_rect(panel, i).contains(p.pos) {
                        self.selected = i;
                        self.run_selected();
                        ran = true;
                        break;
                    }
                }
                // A press inside the panel that matched no entry — a disabled row, the padding
                // between rows — does nothing at all. Dismissal is what a press *outside* means,
                // and that arrives as `PointerDownOutside`.
                let _ = ran;
                Handled::Yes
            }
            // The press that landed somewhere else — the whole of "click away to close", with no
            // geometry of this menu's own and nothing to keep in step with the panel placement.
            Event::PointerDownOutside(_) => {
                self.fire_dismiss();
                Handled::No
            }
            // Swallow all other input while open.
            _ => Handled::Yes,
        }
    }

    /// The menu's **input** surface is the panel it draws, not the layout box it was placed in —
    /// the rows live there. Closed, it takes nothing.
    fn hit_bounds(&self) -> Option<Rectangle> {
        if self.is_open() {
            Some(self.panel.get())
        } else {
            None
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
            .entry(MenuItem::new("Rename").on_click(move || r.set(r.get() + 1)).key('r'))
            .open(true)
            .on_dismiss(move || d.set(d.get() + 1));

        // WidgetIntent::Dismiss (host-resolved from `dismiss`) → dismiss (fires
        // callback, closes), no entry run.
        crate::component::dispatch(&mut m, &Event::Widget(WidgetIntent::Dismiss));
        assert_eq!(dismissed.get(), 1, "dismiss fired on_dismiss");
        assert_eq!(ran.get(), 0);
        assert!(!m.is_open(), "closed after dismiss");

        // Reopen; a quick-key selection runs the entry and does NOT fire dismiss.
        m.open.set(true);
        crate::component::dispatch(&mut m, &Event::Key { key: GridKey::Char('r'), pressed: true });
        assert_eq!(ran.get(), 1, "entry ran on quick-key");
        assert_eq!(dismissed.get(), 1, "a selection is not a dismissal");
    }

    /// Regression (context-menu-bug, 2026-07-13→14): keyboard navigation of an open menu did
    /// nothing — the highlight would not move for `Ctrl+j`/`Ctrl+k` or the arrows. The whole
    /// path was untested end to end, which is why it broke silently and why nobody could tell
    /// when it started working again.
    ///
    /// This is the widget half: given the semantic intent the host resolves from
    /// `[keys.widgets]`, the selection MOVES (and disabled entries are skipped). The host half
    /// — that the default config actually binds those keys to those intents — is asserted in
    /// `heca`'s `build_widget_keymap` test.
    #[test]
    fn menu_intents_move_the_highlight_and_skip_disabled_entries() {
        let noop = || {};
        let mut m = ContextMenu::new()
            .entry(MenuItem::new("First").on_click(noop))
            .entry(MenuItem::new("Disabled").on_click(noop).enabled(false))
            .entry(MenuItem::new("Last").on_click(noop))
            .open(true);

        assert_eq!(m.selected, 0, "starts on the first entry");

        // Down: skips the disabled middle entry.
        assert_eq!(crate::component::dispatch(&mut m, &Event::Widget(WidgetIntent::MenuDown)), Handled::Yes);
        assert_eq!(m.selected, 2, "MenuDown moved past the disabled entry");

        // Down again at the end: stays put (no wrap, no panic).
        crate::component::dispatch(&mut m, &Event::Widget(WidgetIntent::MenuDown));
        assert_eq!(m.selected, 2, "no wrap past the last enabled entry");

        // Up: back to the first, skipping the disabled entry again.
        assert_eq!(crate::component::dispatch(&mut m, &Event::Widget(WidgetIntent::MenuUp)), Handled::Yes);
        assert_eq!(m.selected, 0, "MenuUp moved back past the disabled entry");

        // Up at the top: stays put.
        crate::component::dispatch(&mut m, &Event::Widget(WidgetIntent::MenuUp));
        assert_eq!(m.selected, 0);

        // A raw arrow is NOT swallowed: the widget reports it unhandled so the host can resolve
        // it into a `WidgetIntent` and re-deliver. Swallowing it here is what would break nav.
        assert_eq!(
            crate::component::dispatch(&mut m, &Event::Key {
                key: GridKey::ArrowDown,
                pressed: true
            }),
            Handled::No,
            "raw keys stay unhandled so the host can map them to intents",
        );
    }

    /// `centered: true` places the **panel center** on the anchor (so a keyboard-opened menu is
    /// centered on screen), not the top-left. `centered: false` keeps the down-right cursor
    /// placement. Both still clamp to the viewport.
    #[test]
    fn centered_places_panel_center_on_anchor() {
        let vp = Size::new(2000.0, 2000.0);
        let anchor = Point::new(1000.0, 1000.0);

        // Centered: panel center == anchor (no clamping at this viewport/anchor).
        let m = ContextMenu::new()
            .anchor(anchor)
            .centered(true)
            .entry(MenuItem::new("Split").on_click(|| {}).key('s'))
            .entry(MenuItem::new("Close").on_click(|| {}).key('c'))
            .open(true);
        m.viewport.set(vp);
        let p = m.layout();
        assert!((p.loc.x + p.size.w / 2.0 - anchor.x).abs() < 1e-9, "centered x center != anchor");
        assert!((p.loc.y + p.size.h / 2.0 - anchor.y).abs() < 1e-9, "centered y center != anchor");

        // Non-centered (cursor mode): top-left is down-right of the anchor by ANCHOR_INSET.
        let m2 = ContextMenu::new()
            .anchor(anchor)
            .centered(false)
            .entry(MenuItem::new("Split").on_click(|| {}).key('s'))
            .entry(MenuItem::new("Close").on_click(|| {}).key('c'))
            .open(true);
        m2.viewport.set(vp);
        let p2 = m2.layout();
        assert!((p2.loc.x - (anchor.x + ANCHOR_INSET)).abs() < 1e-9, "cursor x != anchor+inset");
        assert!((p2.loc.y - (anchor.y + ANCHOR_INSET)).abs() < 1e-9, "cursor y != anchor+inset");
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Menu — the content
// ══════════════════════════════════════════════════════════════════════════════

/// **A titled list of [`MenuItem`]s.** Content, and nothing else: it does not know what opens it,
/// where it appears, or which key summons it — those belong to whatever *presents* it.
///
/// Build it where the data is, so its items simply capture what they act on:
///
/// ```
/// use heca_grid_ui::prelude::*;
/// use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};
///
/// let id = 7u64;
/// let menu = Menu::new("Pane", "What you can do with this pane")
///     .child(MenuItem::new("Rename").on_click(move || { let _ = id; }))
///     .child(MenuItem::new("Close").danger(true).on_click(move || { let _ = id; }));
///
/// // Presented as a context menu on a row — right-click, or the keyboard action:
/// let row = Row::new().child(Label::new("nvim")).context_menu(menu);
/// ```
///
/// There is no row identity to declare, no path string, no menu id and no registry: the closure
/// captured `id` in the loop that was already drawing that row.
#[derive(Clone)]
pub struct Menu {
    title: String,
    description: String,
    name: Option<String>,
    items: Vec<MenuItem>,
}

impl Menu {
    /// A menu titled `title`, described by `description`.
    ///
    /// The title heads the panel a [`ContextMenu`] shows — and would be the strip label in a menu
    /// bar, which is why it lives on the content rather than on a presenter. The description is
    /// what makes a menu self-documenting instead of needing a declaration somewhere else.
    pub fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            name: None,
            items: Vec::new(),
        }
    }

    /// Add a row.
    pub fn child(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    /// **Optional. Give this menu a name so other components can add rows to it.**
    ///
    /// The single thing left of the contribution design: a named menu is one a host can offer to
    /// everything mounted before it is shown, so a plugin's "Open in container" can appear on a row
    /// it does not own. A menu without a name is closed, and needs nothing.
    ///
    /// Opaque here — this library neither parses it nor knows who answers it.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// The menu's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// What this menu is for, in a sentence.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The name other components may add rows to, if this menu has one.
    pub fn declared_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The rows' labels, in order — what a caller (or a test) reads back without reaching into the
    /// items.
    pub fn item_labels(&self) -> Vec<String> {
        self.items.iter().map(|i| i.label.clone()).collect()
    }

    /// Append rows a host collected from elsewhere (contributions to a [named](Menu::name) menu).
    pub fn extend(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Turn this content into the panel that shows it — what a **host** does when it presents a
    /// menu into a layer. A caller building a menu never needs it.
    pub fn into_panel(self) -> ContextMenu {
        let mut panel = ContextMenu::new();
        for item in self.items {
            panel = panel.entry(item);
        }
        panel
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  ContextMenu — the presenter
// ══════════════════════════════════════════════════════════════════════════════

/// Where a menu appears — chosen by **what triggered it**, never by an author.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuAnchor {
    /// At a point: the panel's top-left goes there, clamped into the viewport. The pointer trigger.
    At(Point),
    /// Under a widget: the panel hangs off its bottom edge, flipping above and clamping like every
    /// other anchored panel — so the row the menu is *about* stays visible while you read it. The
    /// keyboard trigger.
    Under(Rectangle),
}

impl MenuAnchor {
    /// Apply this anchor to a panel and open it.
    pub fn open(self, panel: ContextMenu) -> ContextMenu {
        match self {
            Self::At(p) => panel.anchor(p).centered(false).open(true),
            Self::Under(b) => panel
                .anchor(Point::new(b.loc.x, b.loc.y + b.size.h))
                .centered(false)
                .open(true),
        }
    }
}
