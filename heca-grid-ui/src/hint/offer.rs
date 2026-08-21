//! **Handing out the letters** — and taking them back. The host's half of a picker is this and
//! nothing more: assign the letters and hand each one to the widget that declared the pick, which
//! draws it in its own paint.

use crate::component::Component;
use super::collect::{is_target, skip};
use crate::reactive::SignalUpdate;

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
        declaring: Option<&dyn Component>,
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
        // …and separately, the nearest **declaring** ancestor. `enclosing` takes anything that
        // counts as a target, which includes the named node itself once it is merely actionable —
        // so by the time we arrive at a keyed row it has overwritten the wrapper we were looking
        // for. Two questions, two variables.
        let declaring = if node.base().hint.is_some() {
            Some(node)
        } else {
            declaring
        };
        let names_itself = node.base().key.as_deref() == Some(key)
            || node.base().scope_key.as_deref() == Some(key);
        if names_itself {
            // A declaration inside wins first (a dock names itself on the outside and declares the
            // pick within), then a declaration *enclosing* it, and only then anything merely
            // actionable inside. Same precedence in both directions: whoever DECLARED what a pick
            // does owns the letter — and the placement it drew with.
            if label_nearest_matching(node, label, &|c: &dyn Component| c.base().hint.is_some()) {
                return true;
            }
            if let Some(outer) = declaring {
                outer.base().hint_label.set(label.clone());
                return true;
            }
            if label_nearest_matching(node, label, &|_: &dyn Component| true) {
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
            if walk(child.as_ref(), key, label, enclosing, declaring) {
                found = true;
            }
        }
        found
    }
    walk(root, key, &label, None, None)
}

/// The nearest target in this subtree that `pick` accepts, labelled.
///
/// Split in two passes so a **declaration wins over mere actionability**, which is the precedence
/// T441 settled: a declared hint shadows something merely actionable beneath it, but never another
/// declaration. One pass could not express it — the first target found won, whatever it was.
///
/// The case that forced it: heca's sidebar pane row carries the identity and is *actionable*
/// (`on_activate`), while the `KeyHint` around it carries the *declaration* and the placement that
/// goes with it (`CenterRight`, so the letter clears the row's label). A single inner-first pass
/// labelled the row, so the same pane wore a right-aligned keycap under one picker and a
/// top-centred one under the other — same letter, two widgets, two looks.
fn label_nearest_matching(
    node: &dyn Component,
    label: &Option<String>,
    pick: &dyn Fn(&dyn Component) -> bool,
) -> bool {
    if skip(node) {
        return false;
    }
    if is_target(node) && pick(node) {
        node.base().hint_label.set(label.clone());
        return true;
    }
    node.base()
        .children
        .iter()
        .any(|c| label_nearest_matching(c.as_ref(), label, pick))
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
