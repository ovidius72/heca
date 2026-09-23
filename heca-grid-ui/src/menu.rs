//! **A context menu is declared on the widget it belongs to.**
//!
//! ```
//! use heca_grid_ui::prelude::*;
//! use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};
//!
//! let save = Button::new("Save").context_menu(
//!     ContextMenu::new("save-menu").child(
//!         Menu::new("Save", "Ways to save this file")
//!             .child(MenuItem::new().label("Save as…").on_click(|| {}))
//!             .child(MenuItem::new().label("Revert").enabled(false).on_click(|| {})),
//!     ),
//! );
//! ```
//!
//! That is the whole of it: no row identity to declare, no path string, no menu id, no registry of
//! builders, no host hit-test, no `Shift+F10` handling. **Any** widget can carry one — an `Icon`, a
//! `Label`, a plugin's own type — because the declaration lives on [`Base`](crate::component::Base)
//! like every other universal slot.
//!
//! # What it replaces
//!
//! Getting a menu onto a row used to take **four** things, three of them invisible:
//!
//! 1. implement `context_path(key, ctx)` mapping a row key to a menu-id string;
//! 2. register a builder for that id;
//! 3. build the items;
//! 4. declare `.key(..)` on the row — because the host resolved "what did you right-click"
//!    from a *position*, and read the answer off `Base::key`.
//!
//! Step 4 is the one nobody would think of, and it is the one that was missing: a workspace header
//! pushed its key into a signal and never called `.key(..)`, so right-clicking a workspace
//! opened **nothing** while panes and columns worked. No error, no failing test. The design was the
//! bug — and its root cause was that a press carried no button, so a right-click could not reach a
//! widget at all and the app had to reconstruct the target from geometry.
//!
//! # A value or a closure — realized at trigger time either way
//!
//! The slot is `Fn() -> ContextMenu`, and
//! [`IntoContextMenu`](crate::builders::IntoContextMenu) lets you fill it with a menu value or a
//! closure. It runs **at trigger time**, which is what makes two things work: rows reflect live
//! state ("Use default name" greys out exactly when the row has a custom one), and a row composed
//! of widgets can be built again for the second right-click, since a widget subtree is owned and
//! can only be handed over once. Building menus lazily was the only reason a host-owned builder
//! registry existed.
//!
//! # Two triggers, one rule
//!
//! | Trigger | Target | Anchor |
//! |---|---|---|
//! | [`Event::RightClick`](crate::event::Event::RightClick) | the widget under the pointer | the pointer |
//! | the host's `open_context_menu` action (`Shift+F10`, the Menu key, a binding) | the focused widget | the widget's bounds |
//!
//! Both **bubble to the nearest ancestor that declares a menu**: you right-click the `Label` inside
//! a row, not the row; focus sits on a cell, and the menu belongs to the row. Bubbling stops at the
//! first declaring ancestor — menus are never merged. Nothing in the chain declares one ⇒ nothing
//! opens, with no hidden fallback. (A host that wants "right-click empty space" puts a menu on the
//! **root**, which needs no empty-space hit-test.)
//!
//! # The one host dependency
//!
//! A menu opens above everything, which is a *layer*, and a widget cannot reach one. So the host
//! installs a sink once at startup — the same shape as
//! [`install_frame_request`](crate::component::install_frame_request) — and every `show*` posts to
//! it. No `AppState`, no host type, in any closure a widget holds.

use crate::component::Component;
use crate::widgets::{ContextMenu, MenuAnchor};
use heca_core::layout::{Point, Rectangle};
use std::cell::RefCell;

/// The host's presenter: given a built, anchored menu, put it on screen.
type MenuSink = Box<dyn Fn(ContextMenu, MenuAnchor, Option<String>)>;

thread_local! {
    /// Host-installed sink every [`ContextMenu::show_at`] posts to. Thread-local because the UI
    /// runs single-threaded, exactly like the frame-request hook beside it.
    static MENU_SINK: RefCell<Option<MenuSink>> = const { RefCell::new(None) };
}

/// Install the callback that **presents** a menu — the host's one job in this.
///
/// The widget layer builds the menu value and says where it goes; only the host can put it in a
/// layer above everything, so that step, and only that step, crosses back. Call once at startup.
///
/// ```ignore
/// heca_grid_ui::menu::install_menu_sink(move |menu| {
///     // The menu arrives already anchored and open — mount it as a layer.
///     layers.insert_menu(menu);
/// });
/// ```
pub fn install_menu_sink(f: impl Fn(ContextMenu, MenuAnchor, Option<String>) + 'static) {
    MENU_SINK.with(|c| *c.borrow_mut() = Some(Box::new(f)));
}

/// **Show a menu at an anchor you chose yourself** — the low road, for a host or a trigger with no
/// event behind it.
///
/// Prefer [`ContextMenu::show`](crate::widgets::ContextMenu::show), which reads the anchor out of
/// the event that asked for the menu; this exists for the caller that genuinely has a position and
/// no event, and it is the same door — every road ends here.
pub fn show(menu: ContextMenu, anchor: MenuAnchor) {
    present(menu, anchor, None);
}

/// Hand a built, anchored menu to the host. A no-op until a sink is installed, so widget
/// code and headless tests can always call it.
pub(crate) fn present(menu: ContextMenu, anchor: MenuAnchor, subject: Option<String>) {
    MENU_SINK.with(|c| {
        if let Some(f) = c.borrow().as_ref() {
            f(menu, anchor, subject);
        }
    });
}

/// Whether a sink is installed — what a host-less test asserts against, and what lets a caller
/// tell "nothing opened because nothing declared a menu" from "nothing opened because there is no
/// host".
pub fn has_menu_sink() -> bool {
    MENU_SINK.with(|c| c.borrow().is_some())
}

/// Open the menu declared **nearest** to the widget at `path`, walking outwards, anchored at
/// `at`. Returns whether anything declared one.
///
/// The pointer trigger. The framework calls it when a [`RightClick`](crate::event::Event::RightClick)
/// reaches the end of its walk unclaimed — so a widget that answers its own right-click still wins,
/// and a declared menu is simply what happens when nothing does.
pub(crate) fn open_declared_at(root: &dyn Component, path: &[usize], at: Point) -> bool {
    match menu_on(root, path, at) {
        Some((menu, subject)) => {
            present(menu, MenuAnchor::At(at), subject);
            true
        }
        None => false,
    }
}

/// **The keyboard trigger** — open the menu declared nearest to wherever the keyboard is, anchored
/// under that widget. Returns whether anything declared one. Called by the host's
/// `open_context_menu` action; nothing there ⇒ `false`, and the host does whatever it does with an
/// action that found no target.
///
/// **"Where the keyboard is" has two spellings and this is the one place that knows both.**
/// [`Base::focused`](crate::Base::focused) is real keyboard focus — a text field has it, a list row
/// does not — while a container tracks *its* cursor as the `key` of the row it sits on, the
/// same declaration [`key_at`](crate::nav::key_at) reads for the mouse. Asking only about
/// `focused` found nothing in a chrome tree and `prefix+>` opened nothing at all; asking only about
/// the cursor would miss a genuinely focused widget (a plugin's input). So `cursor` is tried first
/// and focus second — and a caller passes what it knows rather than writing the fallback again.
///
/// It was two exported functions with three identical lines each, and the host wrote the "try both,
/// in this order" rule beside them. Two spellings of one question, in two places (F004/P084/T399).
pub fn open_for_keyboard(root: &dyn Component, cursor: Option<&str>) -> bool {
    let path = cursor
        .and_then(|key| key_path(root, key))
        .or_else(|| focused_path(root));
    let Some(path) = path else {
        return false;
    };
    let bounds = node_bounds(root, &path);
    // **A keyboard-opened menu is asked about the widget, not a point**, so it is handed the
    // widget's own origin — the nearest thing to "where this was opened" when no pointer was
    // involved. A builder that varies by point (a terminal's *Open link*) therefore offers nothing
    // extra here, which is right: there is no cell under a keystroke.
    match menu_on(root, &path, bounds.loc) {
        Some((menu, subject)) => {
            present(menu, MenuAnchor::Under(bounds), subject);
            true
        }
        None => false,
    }
}

/// The path to the widget that declared `key`.
fn key_path(root: &dyn Component, key: &str) -> Option<Vec<usize>> {
    fn walk(node: &dyn Component, want: &str, at: &mut Vec<usize>) -> bool {
        if node.base().answers_to(want) {
            return true;
        }
        for (i, child) in node.base().children.iter().enumerate() {
            at.push(i);
            if walk(child.as_ref(), want, at) {
                return true;
            }
            at.pop();
        }
        false
    }
    let mut path = Vec::new();
    walk(root, key, &mut path).then_some(path)
}

/// Build the menu declared by the innermost widget at or above `path`.
///
/// **Nearest wins, and it stops there.** Two ancestors declaring menus do not produce a merged one:
/// a menu is a statement about one thing, and stitching two together would make the entries mean
/// different targets in the same list.
fn menu_on(
    root: &dyn Component,
    path: &[usize],
    at: Point,
) -> Option<(ContextMenu, Option<String>)> {
    let mut node = root;
    let mut best = root.base().context_menu.as_deref();
    // **Who the menu is about**, taken from the identity the declaring widget already publishes.
    // A menu is opened *about* something, and the thing it is about is the widget that declared it
    // — which, if it is anything a cursor, a right-click or a drag can point at, has already said
    // so via `Base::key`. So the host learns the subject from a declaration that exists rather than
    // from a hit test of its own, and an author writes nothing new.
    let mut subject = root.base().key.clone();
    for i in path {
        let Some(child) = node.base().children.get(*i) else {
            break;
        };
        node = child.as_ref();
        if let Some(f) = node.base().context_menu.as_deref() {
            best = Some(f);
            subject = node.base().key.clone();
        }
    }
    // Built **now**, not when it was written: an item's label and its `enabled` are answers about
    // the state the menu is being opened in.
    best.map(|f| (f(at), subject))
}

/// The path to the focused widget, if one holds the keyboard.
fn focused_path(root: &dyn Component) -> Option<Vec<usize>> {
    fn walk(node: &dyn Component, at: &mut Vec<usize>) -> bool {
        if node.base().focused.get_untracked() {
            return true;
        }
        for (i, child) in node.base().children.iter().enumerate() {
            at.push(i);
            if walk(child.as_ref(), at) {
                return true;
            }
            at.pop();
        }
        false
    }
    let mut path = Vec::new();
    walk(root, &mut path).then_some(path)
}

/// The laid-out bounds of the node `path` leads to.
fn node_bounds(root: &dyn Component, path: &[usize]) -> Rectangle {
    let mut node = root;
    for i in path {
        match node.base().children.get(*i) {
            Some(child) => node = child.as_ref(),
            None => break,
        }
    }
    node.base().bounds
}

use crate::reactive::SignalGet;
