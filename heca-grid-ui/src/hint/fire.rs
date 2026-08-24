//! **The pick itself** — delivered as an event on the walk every other event uses.

use crate::component::Component;
use super::collect::is_target;

/// **Pick the widget at `path`.** `false` when the path no longer leads to a target — a tree rebuilt
/// under the letters, which is not an error.
///
/// **The pick is an event** (F003/P082/T432): it is delivered as
/// [`Event::Hint`](crate::event::Event::Hint) on the walk every other event uses — capture down the
/// picked widget's ancestor chain, the target, then back up, with `stop_propagation` ending it.
/// There is no second dispatch path for picks, which is what makes a container able to watch, and
/// take, picks from its children with nothing new to learn.
///
/// What happens *at the target* is [`run_pick`](crate::component) — the declaration if there is one,
/// the widget's own click if there is not. It runs in the target phase only, so a pickable container
/// holding pickable rows never answers for a row.
///
/// It takes `&mut` because handlers are `FnMut`, so delivering anything needs the tree mutably.
pub fn fire_hint(root: &mut dyn Component, path: &[usize]) -> bool {
    let mut node: &dyn Component = root;
    for step in path {
        match node.base().children.get(*step) {
            Some(child) => node = child.as_ref(),
            None => return false,
        }
    }
    if !is_target(node) {
        return false;
    }
    let ev = crate::event::Event::Hint(crate::event::HintEvent {
        key: node.base().key.clone(),
        bounds: node.base().bounds,
    });
    crate::component::deliver_to_path(root, path, &ev);
    true
}

/// **What a pick of the target at `path` would do**, when its declaration said so — `None` when the
/// path leads nowhere, or when the declaration is a closure with nothing to say about itself.
///
/// The whole of the library's part in candidate filtering: it hands back the declaration and takes
/// no view. Which candidates are excluded is the host's judgement (`docs/hint-architecture.md` § 6),
/// and in heca that is `chrome::active_hint_targets` asking `route_interaction` about each one.
pub fn hint_intent(root: &dyn Component, path: &[usize]) -> Option<heca_view::Intent> {
    let mut node: &dyn Component = root;
    for step in path {
        node = node.base().children.get(*step)?.as_ref();
    }
    node.base().hint.as_ref()?.intent.clone()
}
