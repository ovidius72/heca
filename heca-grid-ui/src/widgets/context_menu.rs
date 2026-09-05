//! **Menus, split by what each part actually knows.**
//!
//! | type | what it is |
//! |---|---|
//! | [`MenuItem`] | one row: either sugar (a label and an icon) or **any widget subtree** |
//! | [`Menu`] | a titled list of items. **Content only** — it knows nothing about triggers, anchors or keys |
//! | [`ContextMenu`] | a named presenter: contains one [`Menu`] and shows it on right-click or the host's `open_context_menu` action |
//! | `MenuBar` | *not built yet*: contains `Menu`s and shows them as a strip, on a click of a title or its own keybinding |
//!
//! **There is no fourth type, and the panel is not one.** A `ContextMenu` *is* the panel it shows —
//! it holds the rows, lays them out, paints them and hit-tests them. The split above is at the joint
//! a menu bar proves is real: a `MenuBar` will show the **same `Menu` value** as a strip, with its
//! own trigger and its own keyboard convention. The trigger, the anchor and the shortcut belong to
//! whatever *contains* the menu; the menu is content, and stays reusable across every surface that
//! shows one.
//!
//! # What you write
//!
//! ```
//! use heca_grid_ui::prelude::*;
//! use heca_grid_ui::widgets::{ContextMenu, Glyph, Grid, Icon, Label, Menu, MenuItem};
//!
//! # let id = 7u64;
//! # fn rename(_: u64) {}
//! let ctx = ContextMenu::new("pane-menu").child(
//!     Menu::new("Pane", "What you can do with this pane")
//!         // sugar form — a label and an optional icon
//!         .child(MenuItem::new().label("Rename").icon(Glyph::Pencil).on_click(move || rename(id)))
//!         // composed form — any widget subtree, with props
//!         .child(MenuItem::new().child(|| {
//!             Grid::new()
//!                 .child(Icon::new(Glyph::Trash))
//!                 .child(Label::new("Close"))
//!         })),
//! );
//!
//! let row = Row::new().child(Label::new("nvim")).context_menu(ctx);
//! ```
//!
//! There is no row identity to declare, no path string, no menu id to register and no builder
//! registry: the closure captured `id` in the loop that was already drawing that row.
//!
//! # The two forms of a row, and why `child` takes a closure
//!
//! [`MenuItem::label`] / [`MenuItem::icon`] are **sugar** for the row nearly every menu wants.
//! [`MenuItem::child`] takes any widget subtree instead, and **children win when both are given** —
//! the same precedence [`Button`](super::Button) already has.
//!
//! `child` takes a `Fn() -> impl Component` rather than a widget value because a menu can be shown
//! more than once, and a widget subtree is owned ([`Box<dyn Component>`](crate::component::Component)):
//! handed over once, it is gone. A **builder** can run again, which is what makes the whole chain —
//! [`MenuItem`], [`Menu`], [`ContextMenu`] — [`Clone`], so one menu value can be declared on several
//! rows and captured by handlers. It also means the rows are built **at the moment the menu opens**,
//! so `enabled(is_custom_name)` is an answer about the state you are opening it in rather than the
//! state it was written in.
//!
//! The rows are **real children**, laid out by the same engine as everything else — `gap`, `grow`,
//! padding and font inheritance all work inside a row, and this widget contains no layout code of
//! its own beyond placing the finished panel at its anchor.
//!
//! # Two roads to show a menu
//!
//! ```ignore
//! // 1. Declared on the widget — covers the two standard triggers.
//! Row::new().context_menu(ctx);
//!
//! // 2. Shown from a handler — for a trigger you invent.
//! Button::new("More").on_click(move |ev| ctx.show(ev));
//! ```
//!
//! The declaration exists **because the keyboard needs it**: with the menu only inside a closure,
//! `prefix+>` / `Shift+F10` has nothing to find. Both roads run the same code — the declaration is
//! implemented in terms of [`ContextMenu::show`].
//!
//! # Anchors, and why an author never picks one
//!
//! | Trigger | Target | Anchor |
//! |---|---|---|
//! | [`Event::RightClick`](crate::event::Event::RightClick) | the widget under the pointer | the pointer |
//! | the host's `open_context_menu` action (`prefix+>`, `Shift+F10`, the Menu key) | the focused widget | under the widget |
//!
//! The anchor is read **out of the event**: a pointer event carries a cursor, a keyboard event
//! carries the target widget's bounds. No event in flight ⇒ no `show` ⇒ nothing to guess. Both
//! triggers bubble to the **nearest ancestor that declares a menu**: you right-click the `Label`
//! inside a row, not the row; focus sits on a cell, and the menu belongs to the row. Bubbling stops
//! at the first declaring ancestor — menus are never merged, because a menu is a statement about one
//! thing. Nothing in the chain declares one ⇒ nothing opens, with no hidden fallback. (A host that
//! wants "right-click empty space" puts a menu on the **root**, which needs no empty-space hit-test.)
//!
//! # The one host dependency
//!
//! A menu opens above everything, which is a *layer*, and a widget cannot reach one. So the host
//! installs a sink once at startup — the same shape as
//! [`install_frame_request`](crate::component::install_frame_request) — and every show posts to it.
//! No `AppState`, no host type, in any closure a widget holds. See [`crate::menu`].

use crate::builders::{LayoutExt, Parent};
use crate::color::Color;
use crate::component::{
    paint_child, shift_subtree, Base, Component, Event, GridKey, Handled, PaintCx, WidgetIntent,
};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Glow, TextAlign, TextStyle};
use crate::style::{Align, Direction, Length};
use crate::widgets::key_hint::{keycap_size, paint_keycap, KeycapVariant};
use crate::widgets::{paint_panel_chrome, place_at_point, Glyph, Icon, Label, PanelChrome, PanelElevation};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::Cell;
use std::rc::Rc;

/// Panel inner padding (around the list).
const PAD: f32 = 4.0;
/// Vertical / horizontal padding inside each row.
const ROW_PAD_Y: f32 = 6.0;
const ROW_PAD_X: f32 = 12.0;
/// Gap between an icon and the label in a **sugar** row.
const ICON_GAP: f32 = 10.0;
/// Minimum gap between a row's content and its right-aligned shortcut.
const SHORTCUT_GAP: f64 = 28.0;
/// Content-width clamp for the panel.
const MIN_W: f32 = 160.0;
const MAX_W: f32 = 380.0;
/// Inset of the anchor from the cursor so the menu doesn't sit directly under it.
const ANCHOR_INSET: f64 = 2.0;
/// Inset of the quick-pick keycap from the row's right edge. The keycap chip itself
/// is drawn by the shared [`paint_keycap`] primitive (sized via [`keycap_size`]) — the
/// menu never re-derives the chip metrics.
const KEYCAP_INSET: f64 = 8.0;
/// Quick-pick keycap font as a fraction of the menu font — a compact chip, smaller than the
/// row label (mirrors the sub-font scale the `Tag` chip uses).
const KEYCAP_FONT_SCALE: f32 = 0.72;

// ══════════════════════════════════════════════════════════════════════════════
//  MenuItem — one row
// ══════════════════════════════════════════════════════════════════════════════

/// One row in a [`Menu`]: what it shows, and what it does.
///
/// Two ways to say what it shows, and they are the same two every composed widget in this library
/// offers — **sugar or children, children win**:
///
/// ```
/// use heca_grid_ui::prelude::*;
/// use heca_grid_ui::widgets::{Glyph, Grid, Icon, Label, MenuItem};
///
/// // Sugar: the row nearly every menu wants.
/// let save = MenuItem::new().label("Save as…").icon(Glyph::File).on_click(|| {});
///
/// // Composed: any subtree, laid out by the engine like any other tree.
/// let close = MenuItem::new()
///     .child(|| Grid::new().child(Icon::new(Glyph::Trash)).child(Label::new("Close")))
///     .danger(true)
///     .on_click(|| {});
/// ```
///
/// [`child`](MenuItem::child) takes a **builder**, not a widget — see the [module docs](self#the-two-forms-of-a-row-and-why-child-takes-a-closure)
/// for why. Everything else on the row ([`key`](MenuItem::key), [`shortcut`](MenuItem::shortcut),
/// [`danger`](MenuItem::danger), [`enabled`](MenuItem::enabled)) applies to both forms.
#[derive(Clone)]
pub struct MenuItem {
    label: Option<String>,
    icon: Option<Glyph>,
    /// A builder for composed content. `Rc` so the item stays [`Clone`]; a `Fn` so it can run once
    /// per open.
    content: Option<Rc<dyn Fn() -> Box<dyn Component>>>,
    key: Option<char>,
    shortcut: Option<String>,
    danger: bool,
    enabled: bool,
    on_select: Rc<dyn Fn()>,
}

/// The name this row had when only a host built menus, kept so existing callers still compile.
pub type MenuEntry = MenuItem;

#[heca_grid_ui_macros::props]
impl MenuItem {
    /// An empty row. Give it content with [`label`](MenuItem::label) /
    /// [`icon`](MenuItem::icon) or [`child`](MenuItem::child), and behaviour with
    /// [`on_click`](MenuItem::on_click).
    pub fn new() -> Self {
        Self {
            label: None,
            icon: None,
            content: None,
            key: None,
            shortcut: None,
            danger: false,
            enabled: true,
            on_select: Rc::new(|| {}),
        }
    }

    /// The row's text — the sugar form. Ignored if the row also has
    /// [`child`](MenuItem::child) content.
    #[heca_grid_ui_macros::prop]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A leading icon for the sugar form. Ignored if the row also has
    /// [`child`](MenuItem::child) content.
    #[heca_grid_ui_macros::prop]
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// **Compose the row out of widgets** instead of a label and an icon. Replaces the sugar.
    ///
    /// Takes a builder rather than a widget because a menu can be shown more than once and a
    /// subtree is owned — see the [module docs](self#the-two-forms-of-a-row-and-why-child-takes-a-closure).
    /// The subtree becomes a real child of the panel, so the layout engine sizes it exactly as it
    /// would anywhere else.
    #[heca_grid_ui_macros::host_only("a subtree builder — composed content crosses as ViewNode children")]
    pub fn child<C: Component + 'static>(mut self, f: impl Fn() -> C + 'static) -> Self {
        self.content = Some(Rc::new(move || Box::new(f()) as Box<dyn Component>));
        self
    }

    /// What this row does when chosen. Replaces whatever was there.
    ///
    /// A **plain closure**: this library knows nothing about actions, and an item that took an
    /// action id would tie every menu in it to one host's dispatch. Firing a catalogued action
    /// inside the closure is the author's choice — and the way to stay reachable from the command
    /// palette and RPC, which a closure alone is not.
    #[heca_grid_ui_macros::host_only("a closure — behaviour crosses a description as an Intent")]
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_select = Rc::new(f);
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

    /// Mark this entry as **destructive** — its content renders in the `danger` hue.
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

impl Default for MenuItem {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuItem {
    /// The row's text, for a caller (or a test) reading a menu back. `None` for a composed row,
    /// whose text lives in its subtree.
    pub fn label_text(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Width this row reserves on its right for the shortcut hint and the quick-pick keycap. The
    /// row's content is padded away from it so composed content never has to know they exist.
    fn trailing_width(&self, font: f32) -> f64 {
        let adv = (font * crate::font::MONO_ADVANCE_RATIO) as f64;
        let mut w = 0.0;
        if let Some(s) = &self.shortcut {
            w += SHORTCUT_GAP + s.chars().count() as f64 * adv;
        }
        if let Some(k) = self.key {
            w += SHORTCUT_GAP + keycap_size(font * KEYCAP_FONT_SCALE, &k.to_string()).w + KEYCAP_INSET;
        }
        w
    }

    /// Build this row's widget subtree: the composed content if there is any, else the sugar row.
    ///
    /// **Children win over sugar** — the same precedence [`Button`](super::Button) has. The row box
    /// itself carries the padding and the trailing reservation, so composed content is laid out in
    /// the space that is actually free.
    fn build_row(&self, font: f32) -> Box<dyn Component> {
        let inner: Box<dyn Component> = match &self.content {
            Some(build) => build(),
            None => {
                let mut row = crate::widgets::container()
                    .direction(Direction::Row)
                    .align(Align::Center)
                    .gap(ICON_GAP);
                if let Some(g) = self.icon {
                    row = row.child(Icon::new(g).size(font * 1.05));
                }
                if let Some(l) = &self.label {
                    row = row.child(Label::new(l));
                }
                Box::new(row)
            }
        };
        Box::new(
            crate::widgets::container()
                .direction(Direction::Row)
                .align(Align::Center)
                .padding_xy(ROW_PAD_X, ROW_PAD_Y)
                .padding_right(ROW_PAD_X + self.trailing_width(font) as f32)
                .child_boxed(inner),
        )
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
///     .child(MenuItem::new().label("Rename").on_click(move || { let _ = id; }))
///     .child(MenuItem::new().label("Close").danger(true).on_click(move || { let _ = id; }));
///
/// let row = Row::new().child(Label::new("nvim")).context_menu(ContextMenu::new("pane").child(menu));
/// ```
///
/// **One `Menu` per [`ContextMenu`], several items in it.** A context menu does not hold sections:
/// two lists in one panel would make the entries mean different targets in the same place.
#[derive(Clone, Default)]
pub struct Menu {
    title: String,
    description: String,
    name: Option<String>,
    items: Vec<MenuItem>,
}

impl Menu {
    /// A menu titled `title`, described by `description`.
    ///
    /// The title heads the panel — and would be the strip label in a menu bar, which is why it
    /// lives on the content rather than on a presenter. The description is what makes a menu
    /// self-documenting instead of needing a declaration somewhere else.
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
    /// A contributed row has **no per-row payload**, so it acts on app state (the focused pane, the
    /// selected row) rather than on the row the menu was opened for. Opaque here — this library
    /// neither parses the name nor knows who answers it.
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
    /// items. A composed row has no label of its own and reads back as `None`.
    pub fn item_labels(&self) -> Vec<Option<String>> {
        self.items.iter().map(|i| i.label.clone()).collect()
    }

    /// How many rows this menu has.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether this menu has no rows at all.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Append rows a host collected from elsewhere (contributions to a [named](Menu::name) menu).
    pub fn extend(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
        self.items.extend(items);
        self
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  ContextMenu — the presenter, and the panel
// ══════════════════════════════════════════════════════════════════════════════

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
    /// **Open is focused.** `open` is bound to [`Base::focused`], which is the whole of how the
    /// menu hears the keyboard: keys and the intents they resolve to go to the focus owner and
    /// bubble, so an open menu is on the path and a closed one is not. It replaces a declaration
    /// that it took raw keys plus a catch-all that swallowed everything else while open — the pair
    /// that ate every quick-pick letter.
    fn panel_base(open: Signal<bool>) -> Base {
        let mut base = Base::new();
        base.style.layout.direction = Direction::Column;
        base.style.layout.padding = PAD;
        base.style.layout.min_width = Some(Length::Px(MIN_W));
        base.style.layout.max_width = Some(Length::Px(MAX_W));
        base.focused = open;
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
            crate::menu::present(self.clone(), anchor);
        }
    }

    /// Open or close the panel. Opening **realizes the rows** — the builders run here.
    #[heca_grid_ui_macros::prop]
    pub fn open(mut self, open: bool) -> Self {
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
        let rows: Vec<Box<dyn Component>> =
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
            i = if forward { (i + 1) % n } else { (i + n - 1) % n };
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
    /// wires this to its overlay-close path (mirrors [`Dialog::on_dismiss`](super::Dialog)).
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

    /// Layout just placed the panel wherever its parent put it; move it to the anchor the trigger
    /// chose (idempotent — see [`place_panel`](ContextMenu::place_panel)).
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
                t.colors.accent,
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
                    glow: Some(Glow { color: glow_c, radius: 12.0, intensity: 0.3 }),
                    elevation: PanelElevation::Panel,
                },
            );

            for (i, e) in self.menu.items.iter().enumerate() {
                let row = self.row_rect(i);
                let is_sel = i == self.selected && e.enabled;
                if is_sel {
                    let row_border =
                        cx.border(accent.with_alpha(cx.theme().colors.interaction.panel_row_border));
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
            Event::Key { key: GridKey::Char(c), pressed: true } => {
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

// ══════════════════════════════════════════════════════════════════════════════
//  MenuAnchor — where it appears
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
    /// Read the anchor out of the event that asked for the menu.
    ///
    /// This is the whole of "an author never picks an anchor": a pointer event carries a cursor, a
    /// keyboard event carries the target widget's bounds, and an event carrying neither produces
    /// `None` — which shows nothing, rather than guessing a position.
    pub fn from_event(ev: &Event) -> Option<Self> {
        if let Some(pos) = ev.position() {
            return Some(Self::At(pos));
        }
        ev.target_bounds().map(Self::Under)
    }

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
