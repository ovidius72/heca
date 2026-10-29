use super::{Menu, MenuAnchor};
use super::{ANCHOR_INSET, KEYCAP_FONT_SCALE, KEYCAP_INSET, MAX_W, MIN_W, PAD, SHORTCUT_GAP};
use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{
    Base, Component, Event, GridKey, Handled, PaintCx, WidgetIntent, paint_child, shift_subtree,
};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Glow, TextAlign, TextStyle};
use crate::style::{Direction, Length};
use crate::widgets::key_hint::{KeycapVariant, keycap_size, paint_keycap};
use crate::widgets::{PanelChrome, PanelElevation, paint_panel_chrome, place_at_point};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::Cell;
use std::rc::Rc;

/// **A named presenter for one [`Menu`], shown on right-click or the host's `open_context_menu`.**
///
/// It is also the panel: it realizes the menu's rows into its own children, lets the layout engine
/// size them, places itself at the anchor the trigger chose, and paints and hit-tests the result.
/// There is no separate panel type.
///
/// ```
/// use heca_grid_ui::prelude::*;
/// use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};
///
/// let ctx = ContextMenu::new("pane-menu").child(
///     Menu::new("Pane", "What you can do with this pane")
///         .child(MenuItem::new().label("Rename").on_click(|| {})),
/// );
///
/// // Declared on a widget — or shown from a handler with `ctx.show(ev)`.
/// let row = Row::new().context_menu(ctx);
/// ```
///
/// The name is an **id**, not a title: it is what something else refers to this menu by. The title
/// and the description live on the [`Menu`].
pub struct ContextMenu {
    base: Base,
    name: String,
    menu: Menu,
    selected: usize,
    open: Signal<bool>,
    /// Anchor (top-left preferred position; clamped to the viewport).
    anchor: Signal<Point>,
    /// When true the panel is **centered on the anchor** instead of placed down-right of it.
    centered: bool,
    viewport: Cell<Size>,
    /// Panel rect cached at layout/paint, so overlay damage targets just the menu.
    panel: Cell<Rectangle>,
    /// Fired when the menu is **dismissed** (Esc / outside-click) — not when an entry is selected.
    on_dismiss: Option<Rc<dyn Fn()>>,
    /// Fired after an entry ran, whatever it was — the host's hook for taking the layer down.
    after_select: Option<Rc<dyn Fn()>>,
}

/// Cloning a menu gives you **another declaration of the same menu**, not a second view of one
/// panel: the content is shared (it is all `Rc` and plain data) while the widget state — bounds,
/// children, open, selection — starts fresh. That is what lets one `ContextMenu` value be declared
/// on every row of a list.
impl Clone for ContextMenu {
    fn clone(&self) -> Self {
        let open = signal(false);
        Self {
            // `Base::new()` here — not `panel_base()` — cost a released-looking bug: a cloned menu
            // lost `Direction::Column` and fell back to the `Row` default, so **every declared
            // menu laid out horizontally** while `open_dropdown`, which never clones, stayed
            // vertical. Two menus, same widget, different shape. A `Clone` that rebuilds a `Base`
            // must rebuild the widget's box with it.
            base: Self::panel_base(open),
            name: self.name.clone(),
            menu: self.menu.clone(),
            selected: 0,
            open,
            anchor: signal(self.anchor.get_untracked()),
            centered: self.centered,
            viewport: Cell::new(self.viewport.get()),
            panel: Cell::new(Rectangle::from_size(Size::new(0.0, 0.0))),
            on_dismiss: self.on_dismiss.clone(),
            after_select: self.after_select.clone(),
        }
    }
}

impl ContextMenu {
    /// A menu identified by `name`, with no content yet — add it with [`child`](ContextMenu::child).
    ///
    /// `name` is an **id**: what another component refers to this menu by. It is not shown; the
    /// title a panel displays comes from the [`Menu`].
    pub fn new(name: impl Into<String>) -> Self {
        let open = signal(false);
        Self {
            base: Self::panel_base(open),
            name: name.into(),
            menu: Menu::default(),
            selected: 0,
            open,
            anchor: signal(Point::new(0.0, 0.0)),
            centered: false,
            viewport: Cell::new(Size::new(f64::MAX, f64::MAX)),
            panel: Cell::new(Rectangle::from_size(Size::new(0.0, 0.0))),
            on_dismiss: None,
            after_select: None,
        }
    }

    /// **The panel's own box** — a padded column with a width floor and ceiling.
    ///
    /// One definition, used by [`new`](ContextMenu::new) **and** by `Clone`: the rows are children,
    /// so everything else about the panel's size is the layout engine's answer, but the box those
    /// children stack in is this widget's and must survive being cloned.
    ///
    /// **Open is focused.** The menu follows `open` into [`Base::focused`]
    /// ([`Base::follow_focus_modal`]), which is the whole of how it hears the keyboard: keys and
    /// the intents they resolve to go to the focus owner and bubble, so an open menu is on the
    /// path and a closed one is not. It replaces a declaration that it took raw keys plus a
    /// catch-all that swallowed everything else while open — the pair that ate every quick-pick
    /// letter.
    fn panel_base(open: Signal<bool>) -> Base {
        let mut base = Base::new();
        base.style.layout.direction = Direction::Column;
        base.style.layout.padding = (PAD).into();
        base.style.layout.min_width = Some(Length::Px(MIN_W));
        base.style.layout.max_width = Some(Length::Px(MAX_W));
        base.follow_focus_modal(open);
        // **A menu locks because it is a menu.** It demands a choice, so nothing behind it is
        // reachable while it is up — even though its panel is small. Without that, a prefix
        // sequence deliberately falling through the menu's key path reached the app and ran.
        base.lock = true;
        base
    }

    /// The one [`Menu`] this presenter shows. Calling it again replaces the menu.
    ///
    /// One menu per context menu, deliberately: a panel showing two lists would make its entries
    /// mean different targets in the same place.
    pub fn child(mut self, menu: Menu) -> Self {
        self.menu = menu;
        self
    }

    /// This menu's id.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The menu content, for a host that merges contributions into a [named](Menu::name) menu
    /// before showing it.
    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    /// Replace the content — the host's seam for a merged menu.
    pub fn set_menu(&mut self, menu: Menu) {
        self.menu = menu;
        if self.is_open() {
            self.realize();
        }
    }

    /// **Show this menu, anchored from the event that asked for it.**
    ///
    /// The escape hatch for a trigger you invent — a left click, a long press:
    ///
    /// ```ignore
    /// Button::new("More").on_click(move |ev| ctx.show(ev));
    /// ```
    ///
    /// The anchor is never the author's: a pointer event gives the cursor, a keyboard event gives
    /// the target widget's bounds. An event with neither shows nothing rather than guessing a
    /// position — there is no sensible default for "somewhere".
    pub fn show(&self, ev: &Event) {
        if let Some(anchor) = MenuAnchor::from_event(ev) {
            crate::menu::present(self.clone(), anchor, None);
        }
    }

    /// Open or close the panel. Opening **realizes the rows** — the builders run here.
    #[heca_grid_ui_macros::prop]
    pub fn default_open(mut self, open: bool) -> Self {
        self.open.set(open);
        if open {
            self.realize();
        }
        self
    }

    /// The preferred top-left position, clamped into the viewport at layout.
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

    /// Build the rows into real children. Runs each composed row's builder, so it happens **at
    /// open**, not when the menu was written.
    fn realize(&mut self) {
        self.assign_quick_picks();
        let font = self.base.font;
        let rows: Vec<crate::WidgetModel> =
            self.menu.items.iter().map(|i| i.build_row(font)).collect();
        self.base.children = rows;
        self.selected = self.enabled_from(0, true).unwrap_or(0);
    }

    /// **Give every enabled row a quick-pick letter it does not already have.**
    ///
    /// The menu does this, not its author: hosts were each writing `let mut letters = 'a'..='z'`
    /// beside their own loop, so a menu built in one place had keycaps and the same menu built in
    /// another had none. A letter an author set explicitly is kept and never handed out twice; a
    /// disabled row gets none, since it cannot be picked.
    ///
    /// Runs at open, **after** any contributed rows have been merged in — so a plugin's row is
    /// lettered too, rather than being the one row you cannot reach from the keyboard.
    fn assign_quick_picks(&mut self) {
        let taken: Vec<char> = self
            .menu
            .items
            .iter()
            .filter_map(|i| i.key.map(|k| k.to_ascii_lowercase()))
            .collect();
        let mut free = ('a'..='z').filter(|c| !taken.contains(c));
        for item in &mut self.menu.items {
            if item.enabled && item.key.is_none() {
                item.key = free.next();
            }
        }
    }

    /// The quick-pick letter of each row, in order — what a test reads back to prove the widget
    /// assigned them rather than its caller.
    pub fn quick_pick_keys(&self) -> Vec<Option<char>> {
        self.menu.items.iter().map(|i| i.key).collect()
    }

    /// The rows' labels, in order — what a caller (or a test) can read back off a built panel.
    pub fn entry_labels(&self) -> Vec<Option<String>> {
        self.menu.item_labels()
    }

    /// The open-state signal — the host flips it on dismiss.
    pub fn open_signal(&self) -> Signal<bool> {
        self.open
    }

    /// The anchor signal — the host sets it before opening.
    pub fn anchor_signal(&self) -> Signal<Point> {
        self.anchor
    }

    fn is_open(&self) -> bool {
        self.open.get_untracked()
    }

    /// Compact keycap font (a fraction of the menu font) — shared by sizing + glyph so the chip
    /// and its letter stay in proportion.
    fn keycap_font(&self) -> f32 {
        self.base.font * KEYCAP_FONT_SCALE
    }

    /// Index of the first enabled entry at or after `from`, wrapping search forward
    /// then backward; `None` if every entry is disabled.
    fn enabled_from(&self, from: usize, forward: bool) -> Option<usize> {
        let n = self.menu.items.len();
        if n == 0 {
            return None;
        }
        let mut i = from.min(n - 1);
        for _ in 0..n {
            if self.menu.items[i].enabled {
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
        let n = self.menu.items.len();
        if n == 0 {
            return;
        }
        if let Some(i) = self.enabled_from((self.selected + 1).min(n - 1), true) {
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
        if let Some(e) = self.menu.items.get(self.selected)
            && e.enabled
        {
            // **The menu comes down first, then the entry runs.** Not cosmetic: while the panel is
            // up the host is in its overlay domain, and an action dispatched from there is refused
            // by the interaction policy — `[heca] interaction: blocked intent`. Every menu closes
            // before it acts, so the entry lands in the state the user is actually returning to.
            if let Some(f) = &self.after_select {
                f();
            }
            (e.on_select)();
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
        self.after_select = Some(Rc::new(f));
        self
    }

    /// Set the callback fired when the menu is **dismissed** (Esc / outside-click). The host
    /// wires this to its overlay-close path (mirrors [`Dialog::on_dismiss`](crate::widgets::Dialog)).
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_dismiss(mut self, f: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
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

    /// The laid-out rect of row `idx` — its child's bounds, because the rows are real children.
    fn row_rect(&self, idx: usize) -> Rectangle {
        self.base
            .children
            .get(idx)
            .map(|c| c.base().bounds)
            .unwrap_or_else(|| Rectangle::from_size(Size::new(0.0, 0.0)))
    }

    /// Place the finished panel at its anchor by baking the offset into its bounds — the same
    /// subtree-shift `Overlay` / `Select` / `ScrollRegion` use. Idempotent: the target is absolute,
    /// so re-running never compounds.
    ///
    /// This is the **only** geometry this widget owns. Row sizes, the panel's width and its height
    /// all come from the layout engine, because the rows are real children.
    fn place_panel(&mut self) {
        let current = self.base.bounds;
        if current.size == Size::new(0.0, 0.0) {
            return; // Not laid out yet — nothing to place.
        }
        // The viewport the layout pass was given, not the one the last paint cached: placement
        // happens here, and reading it a pass later is what made the menu appear at the raw anchor
        // and then jump once it had been clamped.
        let target = place_at_point(
            self.anchor.get_untracked(),
            current.size,
            self.base.viewport,
            ANCHOR_INSET,
            self.centered,
        );
        self.panel.set(target);
        let dx = target.loc.x - current.loc.x;
        let dy = target.loc.y - current.loc.y;
        if dx != 0.0 || dy != 0.0 {
            shift_subtree(self, dx, dy);
            self.base.mark_needs_paint();
        }
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

    /// On screen only while open: closed, its rows are still children, and gone to every walk.
    fn is_shown(&self) -> bool {
        self.base.visible.get_untracked() && !self.base.style.layout.hidden && self.is_open()
    }

    /// Layout just placed the panel wherever its parent put it; move it to the anchor the trigger
    /// chose (idempotent — see `place_panel`).
    fn on_layout(&mut self) {
        self.place_panel();
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
                cx.accent(),
                t.colors.glow,
                t.colors.foreground,
                t.colors.muted,
                t.colors.danger,
                t.colors.control_radius(),
            )
        };
        let font = self.base.font;
        let panel = self.base.bounds;
        self.panel.set(panel);

        cx.with_overlay(|cx| {
            // Panel — the SHARED overlay panel chrome (drop shadow + theme surface
            // fill + bracket reticle), so a menu reads as the same surface as every
            // other overlay panel. On top of it the menu keeps its own identity: an
            // accent edge and the glow it shares with the CommandPalette. No scrim —
            // context menus dismiss on outside-click rather than darkening the view.
            let panel_border =
                cx.border(accent.with_alpha(cx.theme().colors.interaction.panel_border));
            paint_panel_chrome(
                cx,
                panel,
                PanelChrome {
                    border: panel_border,
                    glow: Some(Glow {
                        color: glow_c,
                        radius: 12.0,
                        intensity: 0.3,
                    }),
                    elevation: PanelElevation::Panel,
                },
            );

            for (i, e) in self.menu.items.iter().enumerate() {
                let row = self.row_rect(i);
                let is_sel = i == self.selected && e.enabled;
                if is_sel {
                    let row_border = cx
                        .border(accent.with_alpha(cx.theme().colors.interaction.panel_row_border));
                    cx.rect(
                        row,
                        accent.with_alpha(cx.theme().colors.interaction.panel_row_fill),
                        row_border,
                        ctrl_radius,
                        None,
                    );
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

                // The row's colour is decided **here**, at paint, from the theme and the row's
                // state, and published to the subtree as the inherited content colour. That is
                // what lets a composed row — an `Icon` and a `Label` an author wrote — read as
                // danger, or dim when disabled, without knowing anything about menus.
                let base_color: Color = if e.danger { danger } else { foreground };
                let text_color = if !e.enabled {
                    muted.with_alpha(cx.theme().colors.interaction.menu_shortcut_dim)
                } else if is_sel {
                    base_color
                } else {
                    muted.lerp(base_color, 0.75)
                };

                if let Some(child) = self.base.children.get(i) {
                    cx.with_content_color(text_color, |cx| paint_child(child.as_ref(), cx));
                }

                // Quick-pick keycap (rightmost) and the shortcut hint left of it. These are the
                // menu's own affordances rather than the row's content, so they are painted into
                // the space the row reserved for them (`MenuItem::trailing_width`) and a composed
                // row never has to lay them out.
                let mut right_edge = row.loc.x + row.size.w - KEYCAP_INSET;
                if let Some(k) = e.key {
                    let ks = keycap_size(self.keycap_font(), &k.to_string());
                    let cap = Rectangle::new(
                        Point::new(right_edge - ks.w, row.loc.y + (row.size.h - ks.h) / 2.0),
                        ks,
                    );
                    let cap_color = if e.enabled { accent } else { muted };
                    paint_keycap(
                        cx,
                        cap,
                        &k.to_string(),
                        self.keycap_font(),
                        Some(cap_color),
                        KeycapVariant::Bordered,
                    );
                    right_edge = cap.loc.x - SHORTCUT_GAP;
                }
                if let Some(s) = &e.shortcut {
                    let srect = Rectangle::new(
                        Point::new(row.loc.x, row.loc.y),
                        Size::new((right_edge - row.loc.x).max(0.0), row.size.h),
                    );
                    cx.text(
                        srect,
                        s,
                        muted.with_alpha(cx.theme().colors.interaction.menu_shortcut),
                        font,
                        TextAlign::End,
                        TextStyle::REGULAR,
                    );
                }
            }
        });
    }

    /// **A layer is not scrolled into view.** `Base::focused` says this widget holds the keyboard
    /// while it is open, and `wants_visible` defaults to exactly that — so an enclosing
    /// `ScrollRegion` would scroll the page to wherever this widget's layout node happens to sit,
    /// every frame it is open. A layer draws over the page; the page does not come to it.
    fn wants_visible(&self) -> bool {
        false
    }

    /// The menu's **input** surface is the panel — its own laid-out bounds, since the rows are
    /// children. Closed, it takes nothing.
    fn hit_bounds(&self) -> Option<Rectangle> {
        if self.is_open() {
            Some(self.base.bounds)
        } else {
            None
        }
    }

    /// The menu paints on the overlay layer, so overlay damage targets the panel rect.
    fn damage_bounds(&self) -> Rectangle {
        crate::widgets::tooltip::damage(&self.base)
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
            Event::Key {
                key: GridKey::Char(c),
                pressed: true,
            } => {
                if let Some(i) = self
                    .menu
                    .items
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
            // Rows are children now, so their rects come from the layout engine — but the menu
            // still resolves the pointer itself, because a row is a *choice* rather than a widget
            // that answers its own clicks.
            Event::PointerMove(p) => {
                for i in 0..self.menu.items.len() {
                    if self.menu.items[i].enabled && self.row_rect(i).contains(p.pos) {
                        self.selected = i;
                        break;
                    }
                }
                Handled::Yes
            }
            Event::PointerDown(p) => {
                for i in 0..self.menu.items.len() {
                    if self.menu.items[i].enabled && self.row_rect(i).contains(p.pos) {
                        self.selected = i;
                        self.run_selected();
                        break;
                    }
                }
                // A press inside the panel that matched no entry — a disabled row, the padding
                // between rows — does nothing at all. Dismissal is what a press *outside* means,
                // and that arrives as `PointerDownOutside`.
                Handled::Yes
            }
            // The press that landed somewhere else — the whole of "click away to close", with no
            // geometry of this menu's own and nothing to keep in step with the panel placement.
            Event::PointerDownOutside(_) => {
                self.fire_dismiss();
                Handled::No
            }
            // **Claim what you act on, and nothing else.** This arm used to be
            // `_ => Handled::Yes` — "swallow all other input while open" — three arms below the
            // comment saying modal capture is the host's job. It ate `Event::TextInput`, and
            // because a host offers text before it resolves the key, every quick-pick letter did
            // nothing at all while 1230 tests passed.
            _ => Handled::No,
        }
    }
}

impl LayoutExt for ContextMenu {}
