//! **The identity rule, reported where it can be seen and fixed** (F003/P082/T444).
//!
//! A row's identity — its [`key`](heca_grid_ui::builders::ComponentExt::key) — is what the keyboard
//! cursor, the right-click target, the drag identity and the picker's remembered letter are kept
//! *on*. Declare none and one is derived from the row's content, which is a floor rather than a
//! guarantee: a derived identity changes when the text does, and two rows that read the same have
//! no way to be told apart at all.
//!
//! Nothing fails when that happens. The letters simply move under whoever is driving, and no test
//! anywhere goes red — which is exactly how the pane-header buttons' letters came to shift while
//! Antonio was using them (F011/P094/T451). So the rule is enforced by **reporting**, in the two
//! places a rule can be reported from:
//!
//! | half | what it looks at | who it is for |
//! |---|---|---|
//! | [`report_ambiguous_widgets`] | a live widget tree | us — a warning while building chrome |
//! | [`report_unkeyed_description`] | a `ViewNode` before it is realized | a **plugin author** |
//!
//! Both walks live below this file — `heca_grid_ui::nav::ambiguous_identities` and
//! `heca_view::unkeyed_collection_items` — and both are pure functions returning findings. This
//! module is only the reporting: the library holds the data, the app owns the failure behaviour,
//! the same split the search history already uses.
//!
//! **Each distinct finding is reported once.** A chrome tree is rebuilt for reasons that have
//! nothing to do with identity (a pane's git status changing is enough) and a description is
//! re-realized on every theme reload, so a diagnostic that repeated with them would be one nobody
//! reads.

use std::cell::RefCell;
use std::collections::HashSet;

use heca_grid_ui::Component;
use heca_view::ViewNode;

thread_local! {
    /// What has already been said. Keyed by the message itself, so two findings that differ in any
    /// way are two reports and the same finding twice is one.
    static SAID: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// Print `message` unless this exact one has been printed before.
fn say_once(message: String) {
    SAID.with(|said| {
        if said.borrow_mut().insert(message.clone()) {
            eprintln!("{message}");
        }
    });
}

/// Warn about every collection in `root` whose items were never keyed — two or more unkeyed
/// siblings that derive the same name, so nothing can tell them apart.
///
/// `surface` names where to look (`"chrome"`, `"pane-header"`, a layer's own name): a widget tree
/// has no file and no line, so it is the only locator a reader gets besides the scope.
///
/// Debug builds only. This is a message to whoever is writing the UI, and there is no one to read
/// it in a release binary.
#[cfg(debug_assertions)]
pub(crate) fn report_ambiguous_widgets(surface: &str, root: &dyn Component) {
    for a in heca_grid_ui::nav::ambiguous_identities(root) {
        let scope = if a.scope.is_empty() {
            "the root".to_string()
        } else {
            format!("'{}'", a.scope)
        };
        say_once(format!(
            "[heca] {surface}: {} unkeyed children of {scope} are all called '{}' — \
             give each one .key(its own id) or their cursor position and hint letters \
             will not survive a rebuild",
            a.count, a.name,
        ));
    }
}

#[cfg(not(debug_assertions))]
pub(crate) fn report_ambiguous_widgets(_surface: &str, _root: &dyn Component) {}

/// Report every node in a **description** that binds a `press` inside a collection and declares no
/// `key` — the half of the rule that reaches someone whose UI is JSON or a WASM module.
///
/// Called at the one bridge, **per description rather than per realize**: the same node is realized
/// again on every theme reload, and this is a statement about the description, not about any one
/// realization of it.
///
/// It runs in release too, unlike its widget-tree twin: a plugin author is not building heca, so a
/// diagnostic they can only see in our debug build is one they never see.
pub(crate) fn report_unkeyed_description(source: &str, node: &ViewNode) {
    for item in heca_view::unkeyed_collection_items(node) {
        let path = item
            .path
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("/");
        say_once(format!(
            "[heca] {source}: a {:?} at child path {path} presses '{}' but declares no key, \
             and it is one of {} siblings of that kind — add \"key\" (the item's own id from your \
             data) or this row cannot keep a cursor position, a right-click target or a hint letter \
             across a rebuild",
            item.kind, item.action, item.siblings,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_view::{Intent, WidgetKind};

    /// The reporting is a thin skin over the two walks; what is worth holding here is that it
    /// **says a thing once**, because both callers run on a cadence that would otherwise repeat it.
    #[test]
    fn the_same_finding_is_only_said_once() {
        SAID.with(|s| s.borrow_mut().clear());
        let message = "[heca] a finding".to_string();
        say_once(message.clone());
        say_once(message.clone());
        SAID.with(|s| assert_eq!(s.borrow().len(), 1));
    }

    /// A clean description says nothing at all — the common case, and the one a noisy diagnostic
    /// would ruin.
    #[test]
    fn a_keyed_description_reports_nothing() {
        SAID.with(|s| s.borrow_mut().clear());
        let node = ViewNode::new(WidgetKind::VStack)
            .child(
                ViewNode::new(WidgetKind::Row)
                    .key("pane:7")
                    .on_press(Intent::new("focus_pane")),
            )
            .child(
                ViewNode::new(WidgetKind::Row)
                    .key("pane:9")
                    .on_press(Intent::new("focus_pane")),
            );
        report_unkeyed_description("test", &node);
        SAID.with(|s| assert!(s.borrow().is_empty()));
    }

    /// …and an unkeyed collection says exactly one thing per item that lost its identity.
    #[test]
    fn an_unkeyed_description_reports_each_item() {
        SAID.with(|s| s.borrow_mut().clear());
        let row = || ViewNode::new(WidgetKind::Row).on_press(Intent::new("focus_pane"));
        let node = ViewNode::new(WidgetKind::VStack).child(row()).child(row());
        report_unkeyed_description("test", &node);
        SAID.with(|s| assert_eq!(s.borrow().len(), 2));
    }
}
