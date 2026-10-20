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
//! | [`Event::RightClick`] | the widget under the pointer | the pointer |
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

use crate::component::Event;
use heca_core::layout::{Point, Rectangle};

mod presenter;
mod item;
mod menu;

pub use presenter::ContextMenu;
pub use item::{MenuEntry, MenuItem};
pub use menu::Menu;

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
/// is drawn by the shared [`paint_keycap`](crate::widgets::key_hint::paint_keycap) primitive (sized via [`keycap_size`](crate::widgets::key_hint::keycap_size)) — the
/// menu never re-derives the chip metrics.
const KEYCAP_INSET: f64 = 8.0;
/// Quick-pick keycap font as a fraction of the menu font — a compact chip, smaller than the
/// row label (mirrors the sub-font scale the `Tag` chip uses).
const KEYCAP_FONT_SCALE: f32 = 0.72;

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
            Self::At(p) => panel.anchor(p).centered(false).default_open(true),
            Self::Under(b) => panel
                .anchor(Point::new(b.loc.x, b.loc.y + b.size.h))
                .centered(false)
                .default_open(true),
        }
    }
}
