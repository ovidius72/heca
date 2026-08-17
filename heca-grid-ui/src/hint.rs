//! Universal leader / vimium **hint targets** over a laid-out component tree.
//!
//! The host's global picker (a leader like `prefix+/`) lights a letter over **every region that
//! said what a pick does to it** and runs that region's declaration on the keypress. This module is
//! the framework's half: [`collect_hints`] walks the **retained** widget tree (whose `Base.bounds`
//! are filled in by layout each frame) and enumerates every declaration with the rect its letter
//! goes over, and [`fire_hint`] runs one.
//!
//! **Nothing is registered and no id exists.** A widget declares the behaviour itself with
//! [`KeyHint::on_hint`](crate::widgets::KeyHint::on_hint); a target is addressed by its **path**
//! from the root, which lives exactly as long as the frame it was collected in. That is what makes
//! the picker something a plugin can join: there is no host registry to reach and no host-private
//! intent type to name. It replaced an opaque `HintTargetId` the host mapped back to a
//! `pub(crate)` enum — a shipped feature a plugin could only have a second-class version of.

use crate::component::Component;
use crate::reactive::{SignalGet, SignalUpdate};
use heca_core::layout::Rectangle;

/// Should this subtree be enumerated? Hidden widgets have stale bounds and never
/// receive input, so they're skipped (matching paint / event / drag resolution).
fn skip(c: &dyn Component) -> bool {
    !c.base().visible.get_untracked() || c.base().style.layout.hidden
}

/// **Can the user act on this widget?** A click, a double click, or a key — the three ways a widget
/// says "do something to me" (Antonio, 2026-08-17: *"if we have on_click, on_key_up,
/// on_double_click also maybe it needs to be hintable"*).
///
/// **It reads one flag and not the handler list**, because the answer is not in the handler list:
/// eight widgets — `Button`, `IconButton`, `BadgeButton`, `Toast`, `Choice`, `RailCell`, `Item`,
/// `Row` — keep their action in a private field of their own, so `Handlers::has(Click)` is `false`
/// for a `Button`. Two red tests said so before this was written. And the two spellings
/// (`on_click`, `on_activate`) mean the same thing, so no set of `EventKind`s names it either.
/// [`Base::activatable`](crate::component::Base::activatable) is set wherever the action is wired,
/// which is the only place that knows.
fn actionable(c: &dyn Component) -> bool {
    c.base().activatable
}

/// **Does this widget get a letter?**
///
/// Anything you can act on, plus anything that declared what a pick does — minus anything that said
/// [`hintable(false)`](crate::builders::ComponentExt::hintable). **Being pickable is not opt-in**
/// (F003/P082/T441): requiring a declaration is what made the picker show a curated handful and feel
/// not worth having, and it is the drift this closes — the declarative side has defaulted a pick to
/// the node's `press` all along (`heca-view-realize`).
///
/// [`Base::hint`] stays meaningful as the **override**: it says a pick does something *other* than
/// acting on the widget normally.
pub(crate) fn is_target(c: &dyn Component) -> bool {
    c.base().hintable && (c.base().hint.is_some() || actionable(c))
}

/// **Every widget in this tree that says what a pick does to it**, with the rect the letter goes
/// over, in document order. The path addresses the widget so [`fire_hint`] can reach it again.
///
/// The framework's half of [`on_hint`](crate::builders::ComponentExt::on_hint): a host walks its trees,
/// lays the letters out and draws them, and hands the pick back here. Nothing is registered, and no
/// id outlives the frame it was collected in — a retained tree rebuilt between the letters
/// appearing and one being picked simply offers a fresh set.
pub fn collect_hints(root: &dyn Component) -> Vec<(Vec<usize>, Rectangle)> {
    let mut out = Vec::new();
    hints_into(root, &mut Vec::new(), false, &mut out);
    out
}

/// `declared_above` — is some ancestor already saying what a pick of this region does? See the
/// shadowing rule below.
fn hints_into(
    node: &dyn Component,
    path: &mut Vec<usize>,
    declared_above: bool,
    out: &mut Vec<(Vec<usize>, Rectangle)>,
) {
    if skip(node) {
        return;
    }
    let declares = node.base().hint.is_some();
    // **A declared hint shadows the mere actionability beneath it — but never another declaration**
    // (F003/P082/T441).
    //
    // `on_hint` says *"picking this does X"* about a whole region, so the widget it wraps must not
    // also wear a letter for the same gesture: the exposé's card declares a hint on its `KeyHint`
    // and an `on_activate` on the card within, and every card came out with two letters.
    //
    // But a declaration **inside** a declaration is a genuinely different target, and suppressing
    // that broke the sidebar instantly — a workspace row declares a pick and *contains* pane rows
    // that each declare their own, so the panes vanished from the picker.
    if node.base().hintable && (declares || (actionable(node) && !declared_above)) {
        out.push((path.clone(), node.base().bounds));
    }
    for (i, child) in node.base().children.iter().enumerate() {
        path.push(i);
        hints_into(child.as_ref(), path, declared_above || declares, out);
        path.pop();
    }
}

/// **Pick the widget at `path`.** `false` when the path no longer leads to a target — a tree rebuilt
/// under the letters, which is not an error.
///
/// Two ways it can answer, in order (F003/P082/T441):
///
/// 1. the widget **declared** what a pick does ([`Base::hint`]) — run that. The override, for a
///    region that answers a pick differently from a click: heca's sidebar row activates the pane and
///    leaves on a click, and stays in the sidebar on a pick.
/// 2. otherwise **act on it as a click would**, because that is what a letter over an ordinary
///    button promises. Delivered as a real [`Event::Click`](crate::event::Event::Click) at the
///    widget's centre, through the handlers it already registered — not a second path that would
///    drift from what clicking does.
///
/// It takes `&mut` for step 2: handlers are `FnMut`, so running one needs the tree mutably. Step 1
/// alone never did, which is why this used to be `&`.
pub fn fire_hint(root: &mut dyn Component, path: &[usize]) -> bool {
    let mut node: &mut dyn Component = root;
    for step in path {
        match node.base_mut().children.get_mut(*step) {
            Some(child) => node = child.as_mut(),
            None => return false,
        }
    }
    if let Some(f) = &node.base().hint {
        f();
        return true;
    }
    if !is_target(node) {
        return false;
    }
    // The centre, so a handler reading the position lands inside the widget it was aimed at.
    let b = node.base().bounds;
    let at = heca_core::layout::Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    let ev = crate::event::Event::Click(crate::event::PointerEvent::at(at).with_target_bounds(b));
    node.base_mut().run_handlers(&ev);
    node.on_event(&ev);
    true
}

/// **A verb a widget answers to, by name** — see [`Base::actions`](crate::component::Base::actions).
pub struct DeclaredAction {
    /// The id a binding, a menu entry or a script uses. Namespaced by whoever declared it, the same
    /// way a provider's are (`heca.expose.pick`, `mypanel.pick`).
    pub name: String,
    /// What it does. A closure, because this side of the boundary is native; the declarative path
    /// maps the same name to an `Intent`.
    pub run: Box<dyn Fn()>,
}

impl std::fmt::Debug for DeclaredAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeclaredAction").field("name", &self.name).finish()
    }
}

/// **Every action declared anywhere in this tree**, in document order.
///
/// What a host lists to answer "what can this surface do right now" — the same uniform walk
/// [`collect_hints`] is, over the same retained trees.
pub fn collect_actions(root: &dyn Component) -> Vec<String> {
    let mut out = Vec::new();
    actions_into(root, &mut out);
    out
}

fn actions_into(node: &dyn Component, out: &mut Vec<String>) {
    if skip(node) {
        return;
    }
    out.extend(node.base().actions.iter().map(|a| a.name.clone()));
    for child in &node.base().children {
        actions_into(child.as_ref(), out);
    }
}

/// **Run the action `name` if some widget in this tree declares it.** `false` when none does.
///
/// The name is the address — there is no path to keep, and therefore nothing that goes stale when
/// the tree is rebuilt between a binding being read and the key being pressed. The first
/// declaration in document order wins, so a nested surface's verb shadows an outer one's of the
/// same name, which is the same nearest-declaration rule menus and keys already follow.
pub fn fire_action(root: &dyn Component, name: &str) -> bool {
    if skip(root) {
        return false;
    }
    if let Some(action) = root.base().actions.iter().find(|a| a.name == name) {
        (action.run)();
        return true;
    }
    root.base()
        .children
        .iter()
        .any(|c| fire_action(c.as_ref(), name))
}


/// **Offer the letter `label` to the hint target at `path`.** `None` withdraws it.
///
/// The host's half of the picker is *this* and nothing more: assign the letters and hand each one
/// to the widget that declared the pick. The widget draws it, in its own paint — which is the only
/// way the letter lands in the same place on screen as the region it names.
///
/// **Do not paint keycaps host-side.** A host walking the trees has to guess which scene, and which
/// half of it, the declaring widget painted into: a [`Scene`](crate::Scene) defers overlay segments
/// to a frame-final band ordered by nesting depth, so a cap written into the base draws *under*
/// every overlay, and one written at depth 1 draws under anything nested deeper. Both look exactly
/// like "the picker does nothing", and no test can see either. That is what made the exposé's
/// letters invisible while every other part of the picker was correct (F003/P082/T427).
///
/// Returns `false` when the path no longer leads to a widget that declared a hint — a tree rebuilt
/// under the letters, which is not an error.
pub fn offer_hint(root: &dyn Component, path: &[usize], label: Option<String>) -> bool {
    let mut node = root;
    for step in path {
        match node.base().children.get(*step) {
            Some(child) => node = child.as_ref(),
            None => return false,
        }
    }
    if !is_target(node) {
        return false;
    }
    node.base().hint_label.set(label);
    true
}

/// **Offer the letter `label` to the hint target declaring `key`.** `None` withdraws it.
///
/// The id-addressed twin of [`offer_hint`], for a host mode that knows *what* it is lettering (this
/// pane, that workspace) but not where the widget drawing it sits. It reuses the identity the
/// widget already declares — the one the cursor, the right-click and the drag all read — rather
/// than inventing a second addressing scheme for the same rows.
///
/// **Whichever identity the target declares.** A row names itself with
/// [`Base::key`](crate::component::Base::key) and a mounted container with
/// [`Base::scope_key`](crate::component::Base::scope_key) — they answer different questions (which
/// row is the cursor on; which container did this press land in) and a target has one or the other.
/// Matching both is what keeps this a single door for the caller, who only knows the name.
///
/// # The name and the pick are rarely on the same widget, and they sit on either side
///
/// Being pickable is something you *wrap* a region in, so the two declarations end up on different
/// nodes — and which one is on top depends on what was wrapped:
///
/// - a **row** carries `key` and the [`KeyHint`](crate::widgets::KeyHint) around it carries the
///   hint — the pick is on the **parent**;
/// - a **mounted dock** names itself on the outer wrapper and declares the pick within — the pick
///   is on a **child**.
///
/// So the search is: the named node's own hint, else the nearest one **beneath** it, else the
/// nearest one **enclosing** it. Searching one direction only is why every sidebar row stayed dark
/// under `prefix+q` while the docks lit correctly (Antonio, driving, 2026-08-14).
pub fn offer_hint_by_key(root: &dyn Component, key: &str, label: Option<String>) -> bool {
    fn walk(
        node: &dyn Component,
        key: &str,
        label: &Option<String>,
        enclosing: Option<&dyn Component>,
    ) -> bool {
        if skip(node) {
            return false;
        }
        // The nearest hint *above* the target, remembered on the way down so it is there if the
        // named node turns out to be inside a wrapper that declared one.
        let enclosing = if is_target(node) {
            Some(node)
        } else {
            enclosing
        };
        let names_itself = node.base().key.as_deref() == Some(key)
            || node.base().scope_key.as_deref() == Some(key);
        if names_itself {
            if label_nearest(node, label.clone()) {
                return true;
            }
            if let Some(outer) = enclosing {
                outer.base().hint_label.set(label.clone());
                return true;
            }
            // Named, but nothing anywhere around it says what a pick would do — so there is nothing
            // a letter could run, and lettering it would put up a keycap that does nothing.
            return false;
        }
        // **Every node naming `key`, not the first one.** `.any()` here stopped the walk at the
        // first match, so a pane listed in the left sidebar *and* the right one was lettered only
        // on the left — the same target, one of its two places silently dark (Antonio, driving,
        // 2026-08-17; the trace read `nodes naming it: chrome=2` while one letter was handed out).
        //
        // `||` cannot be used to fold this either: it short-circuits the same way.
        let mut found = false;
        for child in &node.base().children {
            if walk(child.as_ref(), key, label, enclosing) {
                found = true;
            }
        }
        found
    }
    walk(root, key, &label, None)
}

/// Give `label` to the nearest hint declaration at or under `node`.
fn label_nearest(node: &dyn Component, label: Option<String>) -> bool {
    if skip(node) {
        return false;
    }
    if is_target(node) {
        node.base().hint_label.set(label);
        return true;
    }
    node.base()
        .children
        .iter()
        .any(|c| label_nearest(c.as_ref(), label.clone()))
}

/// Withdraw every letter in this tree — what a host calls when the picker closes.
///
/// Walks the whole tree rather than the paths it offered, so a target that moved between opening
/// the picker and closing it still has its letter taken away. A stale keycap left over a card is
/// the failure this prevents.
pub fn clear_hints(root: &dyn Component) {
    if is_target(root) {
        root.base().hint_label.set(None);
    }
    for child in &root.base().children {
        clear_hints(child.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactive::SignalUpdate;
    use crate::widgets::{Flex, KeyHint, Surface};
    use heca_core::layout::{Point, Size};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A hinting wrapper with laid-out bounds, appending `tag` to `log` when it is picked.
    fn hint_at(
        tag: &'static str,
        log: &Rc<RefCell<Vec<&'static str>>>,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) -> Box<dyn Component> {
        let log = log.clone();
        let mut wrapper = KeyHint::new(Surface::new()).on_hint(move || log.borrow_mut().push(tag));
        wrapper.base_mut().bounds = Rectangle::new(Point::new(x, y), Size::new(w, h));
        Box::new(wrapper)
    }

    #[test]
    fn collects_declarations_with_bounds_in_document_order() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        root.base_mut()
            .children
            .push(hint_at("first", &log, 0.0, 0.0, 100.0, 40.0));
        root.base_mut()
            .children
            .push(hint_at("second", &log, 0.0, 50.0, 100.0, 40.0));

        let targets = collect_hints(&root);
        assert_eq!(
            targets.iter().map(|(path, _)| path.clone()).collect::<Vec<_>>(),
            vec![vec![0], vec![1]],
            "document order, addressed by path"
        );
        assert_eq!(targets[1].1.loc.y, 50.0, "…each with the rect its letter goes over");

        assert!(fire_hint(&mut root, &targets[1].0));
        assert_eq!(*log.borrow(), vec!["second"]);
    }

    #[test]
    fn skips_hidden_subtrees_and_silent_nodes() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        // A silent parent holding a declaring child, then a hidden declaring node.
        let mut silent_parent = Flex::column();
        silent_parent
            .base_mut()
            .children
            .push(hint_at("nested", &log, 0.0, 0.0, 10.0, 10.0));
        root.base_mut().children.push(Box::new(silent_parent));

        let mut hidden = KeyHint::new(Surface::new()).on_hint(|| unreachable!("hidden"));
        hidden.base_mut().visible.set(false);
        root.base_mut().children.push(Box::new(hidden));

        let targets = collect_hints(&root);
        assert_eq!(
            targets.iter().map(|(path, _)| path.clone()).collect::<Vec<_>>(),
            vec![vec![0, 0]],
            "a silent parent is transparent; a hidden declaration is skipped"
        );
    }
}

#[cfg(test)]
mod actionable_tests {
    use super::*;
    use crate::builders::{ComponentExt, Parent};
    use crate::widgets::{Button, Flex, Label};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// **Being pickable is not opt-in** (F003/P082/T441). A button gets a letter for having a click
    /// handler, and nothing else. Before this, a widget had to be wrapped in a `KeyHint` and given
    /// an `on_hint`, which is why `prefix+/` showed a curated handful and felt not worth having.
    #[test]
    fn anything_actionable_is_lettered_with_nothing_declared() {
        let tree = Flex::column()
            .child(Button::new("Restart").on_click(|| {}))
            .child(Button::new("Stop").on_click(|| {}).hintable(false))
            .child(Label::new("just text"));

        let targets = collect_hints(&tree);
        assert_eq!(
            targets.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
            vec![vec![0]],
            "the plain button is a target; `hintable(false)` is not, and a Label has nothing to run"
        );
    }

    /// **Picking an ordinary widget does what clicking it does.** The letter over a button promises
    /// the button's own behaviour, so it is delivered as a real `Click` through the handler that is
    /// already there — not a second path that could drift from what the mouse does.
    #[test]
    fn picking_an_ordinary_button_runs_its_click() {
        let ran = Rc::new(RefCell::new(0));
        let seen = ran.clone();
        let mut tree =
            Flex::column().child(Button::new("Restart").on_click(move || *seen.borrow_mut() += 1));

        let targets = collect_hints(&tree);
        assert_eq!(targets.len(), 1);
        assert!(fire_hint(&mut tree, &targets[0].0));
        assert_eq!(*ran.borrow(), 1, "the button's own click handler ran");
    }

}

#[cfg(test)]
mod offer_tests {
    use super::*;
    use crate::builders::{ComponentExt, Parent};
    use crate::widgets::{Flex, KeyHint, Label, Row};

    /// **The pick is on the PARENT** — a row names itself and the `KeyHint` wrapping it says what a
    /// pick does. This is every sidebar row, and searching only downward from the named node left
    /// all of them dark under `prefix+q` while the docks lit correctly.
    #[test]
    fn a_named_row_wrapped_in_a_keyhint_is_letterable() {
        let tree = Flex::column().child(
            KeyHint::new(Row::new().key("pane:7").child(Label::new("zsh"))).on_hint(|| {}),
        );

        assert!(offer_hint_by_key(&tree, "pane:7", Some("a".into())));
        let hint = &tree.base().children[0];
        assert_eq!(hint.base().hint_label.get_untracked().as_deref(), Some("a"));
    }

    /// **The pick is on a CHILD** — a mounted dock names itself on the outer wrapper and declares
    /// the pick within. The other direction, from the same one call.
    #[test]
    fn a_named_wrapper_declaring_its_pick_within_is_letterable() {
        let tree = Flex::column().child(
            Flex::column()
                .scope_key("workspaces")
                .child(KeyHint::new(Label::new("dock")).on_hint(|| {})),
        );

        assert!(offer_hint_by_key(&tree, "workspaces", Some("b".into())));
        let inner = &tree.base().children[0].base().children[0];
        assert_eq!(inner.base().hint_label.get_untracked().as_deref(), Some("b"));
    }

    /// **The same target shown twice must be lettered twice** — one pane listed in the left
    /// sidebar *and* the right one is two places you can pick it, and both wear the letter.
    ///
    /// Antonio, driving, 2026-08-17: *"prefix+q, m, M, t, T show letters on the left sidebar but not
    /// on the right one. WHY? they are the same component so nothing should be adapted."*
    #[test]
    fn a_target_listed_in_two_places_is_lettered_in_both() {
        let tree = Flex::column()
            .child(KeyHint::new(Row::new().key("pane:7").child(Label::new("zsh"))).on_hint(|| {}))
            .child(KeyHint::new(Row::new().key("pane:7").child(Label::new("zsh"))).on_hint(|| {}));

        assert!(offer_hint_by_key(&tree, "pane:7", Some("a".into())));
        let left = &tree.base().children[0];
        let right = &tree.base().children[1];
        assert_eq!(left.base().hint_label.get_untracked().as_deref(), Some("a"), "the first one");
        assert_eq!(
            right.base().hint_label.get_untracked().as_deref(),
            Some("a"),
            "and the second — the walk must not stop at the first match"
        );
    }

    /// A name nobody declares, and a name with no pick anywhere around it, are both "not
    /// letterable" — never a keycap that does nothing.
    #[test]
    fn a_target_with_no_pick_is_not_letterable() {
        let tree = Flex::column().child(Row::new().key("pane:9"));
        assert!(!offer_hint_by_key(&tree, "pane:9", Some("c".into())));
        assert!(!offer_hint_by_key(&tree, "nothing", Some("c".into())));
    }
}
