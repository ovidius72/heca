//! **`heca::extension("pro")`** — a program says who it is, once.
//!
//! ```ignore
//! let pro = heca::extension("pro");
//! pro.action("start_worker").label("Start worker").run(start);   // the action `pro.start_worker`
//! pro.layer("planner").view(|cx| planner_overlay(cx));           // the layer  `pro.planner`
//! pro.pane_button("start_worker");                               // a button in every pane header
//! pro.pane_chip("status", |facts| Some(PaneChip::new(icon, text))); // a chip in every pane header
//! ```
//!
//! **Why a handle and not a name on every item.** A name is `<owner>.<short>`, and the owner half
//! is never a string an item's author picks each time: that is what makes the namespace
//! unforgeable. Two extensions cannot take each other's names, and nothing can take the app's.
//! The owner is said here, once, and every action and layer hangs off it.
//!
//! - `heca` is the app's own and is refused.
//! - A short name with a dot in it is refused, so a second segment cannot be smuggled in.
//! - A second extension with the same name is refused and reported, and everything hung off it is
//!   dropped — the first one keeps the name.
//! - **Docks keep their own `kind()`**: a dock is the component, and its name is what it is.
//!
//! A refusal is *said* — to whoever is writing the program, once — and never silent: a typo must
//! not take the app down, and must not vanish either.
//!
//! A WASM plugin's name will come from its manifest, through this same handle.

use std::cell::RefCell;
use std::collections::HashSet;

use crate::actions::ActionMeta;
use crate::chrome::{HOST_OWNER, warn_author};

use super::layer::LayerBuilder;

thread_local! {
    /// The extension names taken. Thread-local like the queues its items go into.
    static DECLARED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// **This program's name** — declare it once and hang everything off the handle it returns.
///
/// A refused name (`heca`, empty, or with a dot; or one already taken) still returns a handle so
/// the program's lines keep compiling; every item hung off it is dropped, after one report.
pub fn extension(name: impl Into<String>) -> Extension {
    let name = name.into();
    if name.is_empty() || name.contains('.') || name == HOST_OWNER {
        warn_author(format!(
            "[heca] extension name '{name}' is refused: it must be one word with no dot, and \
             `{HOST_OWNER}` is the app's own — everything added under it is dropped"
        ));
        return Extension { name: None };
    }
    let fresh = DECLARED.with(|d| d.borrow_mut().insert(name.clone()));
    if !fresh {
        warn_author(format!(
            "[heca] an extension called '{name}' was already declared, so this one is refused — \
             everything added under it is dropped"
        ));
        return Extension { name: None };
    }
    Extension { name: Some(name) }
}

/// A program's identity. See [`extension`].
pub struct Extension {
    /// `None` when the name was refused.
    name: Option<String>,
}

impl Extension {
    /// The name every item of this extension carries, unless it was refused.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// **An action** of this extension — `<extension>.<short>`. Say what it is with
    /// [`ActionMeta`]'s methods, then [`run`](ActionMeta::run) it.
    pub fn action(&self, short: &str) -> ActionMeta {
        ActionMeta::new(self.full("action", short).unwrap_or_default())
    }

    /// **A layer** of this extension — `<extension>.<short>` — an overlay opened with
    /// `toggle_layer name=<extension>.<short>`. Say how it is built with
    /// [`view`](LayerBuilder::view).
    pub fn layer(&self, short: &str) -> LayerBuilder {
        LayerBuilder::named(self.full("layer", short).unwrap_or_default())
    }

    /// **A button in every pane's header** that runs this extension's action `short`
    /// (`<extension>.<short>`, declared with [`action`](Self::action)). Its icon, words, tooltip and
    /// danger come from that action; if the action declares an argument named
    /// [`PANE_ARG`](crate::PANE_ARG) (`pane`, an integer) a click sends it the id of the pane it was
    /// clicked on; an action that declares none is sent none. *Client side.*
    ///
    /// It is shown by default — while the user's `[appearance.pane] title_actions` is still heca's
    /// own list. Once they write their own, it shows only if they list it by that name, and they are
    /// told once that it exists.
    pub fn pane_button(&self, short: &str) {
        if let Some(name) = self.full("pane button", short) {
            crate::chrome::pane_items::add_extension_button(name);
        }
    }

    /// **A chip in every pane's header** — `<extension>.<short>` — showing whatever `produce` says
    /// about a pane, or nothing (`None`) when it has nothing to say. *Client side.*
    ///
    /// `produce` is handed [`PaneFacts`](crate::PaneFacts) — plain data about that pane, which grows
    /// as facts are added, so read fields by name. It runs again when a pane's facts change, not
    /// every frame.
    ///
    /// Shown by default while the user's `[appearance.pane] title_segments` is still heca's own list;
    /// once they write their own list it shows only if they list it by that name, and they are told
    /// once that it exists.
    pub fn pane_chip(
        &self,
        short: &str,
        produce: impl Fn(&crate::PaneFacts) -> Option<crate::PaneChip> + 'static,
    ) {
        if let Some(name) = self.full("pane chip", short) {
            crate::chrome::pane_items::add_extension_chip(name, produce);
        }
    }

    /// **A line under every pane's name in the sidebar** — `<extension>.<short>` — showing whatever
    /// `produce` says about a pane, or nothing (`None`). *Client side.*
    ///
    /// Like a chip, `produce` is handed [`PaneFacts`](crate::PaneFacts) and runs again when a pane's
    /// facts change. Shown by default while the user's `[appearance.sidebar] pane_lines` is still
    /// heca's own list; once they write their own list it shows only if they list it by that name,
    /// and they are told once that it exists.
    pub fn pane_line(
        &self,
        short: &str,
        produce: impl Fn(&crate::PaneFacts) -> Option<crate::PaneLine> + 'static,
    ) {
        if let Some(name) = self.full("pane line", short) {
            crate::chrome::pane_items::add_extension_line(name, produce);
        }
    }

    /// `<extension>.<short>`, or `None` — after saying why — when either half is refused. An empty
    /// name is what a refused item carries, and what its `run`/`view` recognise and drop.
    fn full(&self, what: &str, short: &str) -> Option<String> {
        let owner = self.name.as_deref()?;
        if short.is_empty() || short.contains('.') {
            warn_author(format!(
                "[heca] {what} '{short}' of extension '{owner}' is refused: the short name must \
                 be non-empty and have no dot (the extension name is the part before the dot)"
            ));
            return None;
        }
        Some(format!("{owner}.{short}"))
    }
}

/// Whether `owner` is an extension this program declared — what lets an item's name be believed.
pub(super) fn is_declared(owner: &str) -> bool {
    DECLARED.with(|d| d.borrow().contains(owner))
}

/// Forget every declared name, for a test starting from a fresh program.
#[cfg(test)]
pub(super) fn reset() {
    DECLARED.with(|d| d.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every name is `<extension>.<short>`, and the owner half comes from the handle.**
    #[test]
    fn an_items_name_is_the_extension_then_the_short_name() {
        reset();
        let pro = extension("pro");
        assert_eq!(pro.action("start_worker").name, "pro.start_worker");
        assert!(is_declared("pro"));
        assert_eq!(pro.name(), Some("pro"));
    }

    /// **`heca` is the app's; a dotted or empty name is not a name.**
    #[test]
    fn the_apps_name_and_malformed_names_are_refused() {
        reset();
        for bad in ["heca", "", "a.b"] {
            let e = extension(bad);
            assert_eq!(e.name(), None, "'{bad}' must be refused");
            assert_eq!(
                e.action("x").name,
                "",
                "what hangs off a refused name is dropped"
            );
        }
        assert!(!is_declared("heca"));
    }

    /// **A short name cannot smuggle in a second segment or be empty.**
    #[test]
    fn a_short_name_with_a_dot_or_nothing_is_refused() {
        reset();
        let pro = extension("pro");
        assert_eq!(pro.action("a.b").name, "");
        assert_eq!(pro.action("").name, "");
    }

    /// **A second extension with the same name is refused, and the first keeps it.**
    #[test]
    fn a_second_extension_with_the_same_name_is_refused() {
        reset();
        let first = extension("pro");
        let second = extension("pro");
        assert_eq!(first.name(), Some("pro"));
        assert_eq!(second.name(), None);
        assert_eq!(second.action("x").name, "");
    }
}
