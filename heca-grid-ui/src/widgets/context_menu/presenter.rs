use super::{ANCHOR_INSET, MAX_W, MENU_GLOW, MIN_W, PAD, ROWS};
use super::{Menu, MenuAnchor};
use crate::builders::{LayoutExt, Parent, PlaceExt};
use crate::component::{Base, Component, Event, GridKey, Handled, WidgetIntent};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::style::Fill;
use crate::widgets::{Flex, Overlay};
use heca_core::layout::{Point, Rectangle};
use std::rc::Rc;

/// **A named presenter for one [`Menu`], shown on right-click or the host's `open_context_menu`.**
///
/// It is also the panel: it realizes the menu's rows as real [`Row`](crate::widgets::Row)s in a
/// light [`Overlay`] at the anchor the trigger chose — the overlay places the panel, draws its frame
/// on the overlay layer and keeps it inside the window; the rows are sized by the layout engine and
/// mark the chosen entry themselves. There is no separate panel type.
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
    /// The entry the cursor is on — what Enter runs, and what its row lights.
    selected: Signal<usize>,
    open: Signal<bool>,
    /// Anchor (top-left preferred position; clamped to the viewport).
    anchor: Signal<Point>,
    /// When true the panel is **centered on the anchor** instead of placed down-right of it.
    centered: bool,
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
            base: Self::menu_base(open),
            name: self.name.clone(),
            menu: self.menu.clone(),
            selected: signal(0),
            open,
            anchor: signal(self.anchor.get_untracked()),
            centered: self.centered,
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
            base: Self::menu_base(open),
            name: name.into(),
            menu: Menu::default(),
            selected: signal(0),
            open,
            anchor: signal(Point::new(0.0, 0.0)),
            centered: false,
            on_dismiss: None,
            after_select: None,
        }
    }

    /// **The menu's own box** — one definition, used by [`new`](ContextMenu::new) **and** by
    /// `Clone`, so a cloned declaration is the same widget.
    ///
    /// **Open is focused.** The menu follows `open` into [`Base::focused`]
    /// ([`Base::follow_focus_modal`]), which is the whole of how it hears the keyboard: keys and
    /// the intents they resolve to go to the focus owner and bubble, so an open menu is on the
    /// path and a closed one is not. Its layer leaves the keyboard here (`keyboard(false)`).
    fn menu_base(open: Signal<bool>) -> Base {
        let mut base = Base::new();
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

    /// **Build the panel**: the rows, as real children, in a light layer at the anchor. Runs each
    /// composed row's builder, so it happens **at open**, not when the menu was written.
    ///
    /// The layer is an [`Overlay`]: it places the panel at the anchor (`at_point_to`, clamped into
    /// the window), draws the menu's frame — the accent edge and its halo — on the overlay layer,
    /// and leaves the keyboard with the menu. The rows mark the chosen entry themselves
    /// ([`CursorLook::Pill`](crate::widgets::CursorLook::Pill)).
    fn realize(&mut self) {
        self.assign_quick_picks();
        let font = self.base.font;
        let selected = self.selected;
        let rows: Vec<crate::WidgetModel> = self
            .menu
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let chosen = move || selected.get() == i;
                crate::WidgetModel::new(item.build_row(font, chosen))
            })
            .collect();
        let column = Flex::column()
            .padding(PAD)
            .min_width(MIN_W)
            .max_width(MAX_W)
            .area(ROWS)
            .child(rows);
        let layer = Overlay::new()
            .blocking(false)
            .keyboard(false)
            .at_point_to(self.anchor, ANCHOR_INSET, self.centered)
            .panel_edge(Fill::theme("accent").alpha("panel_border"))
            .panel_glow(MENU_GLOW)
            .open(self.open)
            // The layer names its panel; the column inside keeps the menu's own name.
            .panel(Flex::column().child(column));
        self.base.children = vec![crate::WidgetModel::new(layer)];
        self.selected.set(self.enabled_from(0, true).unwrap_or(0));
    }

    /// The name the column of rows carries — what finds the panel inside a tree.
    pub const PANEL: &'static str = ROWS;

    /// **The rows, in order** — real children, laid out by the engine. Empty until the menu opens.
    pub fn rows(&self) -> &[crate::WidgetModel] {
        crate::component::area(self, ROWS).map_or(&[], |column| column.base().children.as_slice())
    }

    /// **Where the panel is on screen** — the box the rows stack in. Empty before the menu opens.
    pub fn panel_bounds(&self) -> Rectangle {
        crate::component::area(self, ROWS).map_or_else(
            || Rectangle::from_size(heca_core::layout::Size::new(0.0, 0.0)),
            |column| column.base().bounds,
        )
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

    fn selected(&self) -> usize {
        self.selected.get_untracked()
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
        let at = self.selected();
        if let Some(i) = self.enabled_from((at + 1).min(n - 1), true) {
            self.selected.set(if i == at {
                self.enabled_from(at, true).unwrap_or(i)
            } else {
                i
            });
        }
    }

    /// Move the selection to the previous enabled entry (↑).
    pub fn select_prev(&mut self) {
        let prev = self.selected().saturating_sub(1);
        if let Some(i) = self.enabled_from(prev, false) {
            self.selected.set(i);
        }
    }

    /// Run the selected entry (if enabled) and close.
    pub fn run_selected(&mut self) {
        if let Some(e) = self.menu.items.get(self.selected())
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
        self.selected.set(self.enabled_from(0, true).unwrap_or(0));
    }

    /// **The enabled entry under the pointer**, asked of the framework rather than hit-tested here:
    /// hover is marked along the target's ancestor chain before an event is delivered, so the row
    /// answering `hovered` is the row the pointer is on.
    fn pointed_entry(&self) -> Option<usize> {
        self.rows()
            .iter()
            .position(|row| row.base().hovered())
            .filter(|&i| self.menu.items.get(i).is_some_and(|item| item.enabled))
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

    /// **A layer is not scrolled into view.** `Base::focused` says this widget holds the keyboard
    /// while it is open, and `wants_visible` defaults to exactly that — so an enclosing
    /// `ScrollRegion` would scroll the page to wherever this widget's layout node happens to sit,
    /// every frame it is open. A layer draws over the page; the page does not come to it.
    fn wants_visible(&self) -> bool {
        false
    }

    /// The menu's **input** surface is the panel. Closed, it takes nothing.
    fn hit_bounds(&self) -> Option<Rectangle> {
        self.is_open().then(|| self.panel_bounds())
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
                    self.selected.set(i);
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
            // **The pointer moves the cursor**: the entry under it becomes the chosen one.
            Event::PointerMove(_) => {
                if let Some(i) = self.pointed_entry() {
                    self.selected.set(i);
                }
                Handled::Yes
            }
            Event::PointerDown(_) => {
                if let Some(i) = self.pointed_entry() {
                    self.selected.set(i);
                    self.run_selected();
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
            // **Claim what you act on, and nothing else.** A catch-all `Handled::Yes` here once ate
            // `Event::TextInput`, and because a host offers text before it resolves the key, every
            // quick-pick letter did nothing at all while 1230 tests passed.
            _ => Handled::No,
        }
    }
}

impl LayoutExt for ContextMenu {}
