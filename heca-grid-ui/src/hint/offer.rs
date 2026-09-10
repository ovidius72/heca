//! **Handing out the letters** — and taking them back. The host's half of a picker is this and
//! nothing more: assign the letters and hand each one to the widget that declared the pick, which
//! draws it in its own paint.

use crate::component::Component;
use super::collect::{is_target, narrowed, skip, unseen};
use crate::reactive::SignalUpdate;
use heca_core::layout::Rectangle;

/// **Hand `label` to `node`, unless nothing can see it there.**
///
/// The one place a letter is written, so the rule that a view you cannot see does not get one is
/// stated once per walk rather than at each of the four places a letter is handed out. **Cannot
/// see** covers both halves — clipped away by an ancestor, or squeezed to nothing of its own
/// ([`unseen`]).
///
/// **Withdrawal is never refused.** `None` takes a letter back and must reach a widget wherever it
/// has scrolled to since it got one, or the keycap outlives the picker that put it up — the exact
/// failure [`clear_hints`] exists to prevent.
///
/// This filters the **view, not the candidate**, which is the rule the app already follows for a
/// pane covered by a sidebar (`chrome/hint/letters.rs`): the same pane is shown in several places,
/// and the letter belongs to the pane. A row past the sidebar's fold simply is not one of the
/// places that can show it; the pane keeps its letter and its other views still wear it.
fn give(node: &dyn Component, label: &Option<String>, clip: Option<Rectangle>) -> bool {
    if label.is_some() && unseen(node, clip) {
        return false;
    }
    node.base().hint_label.set(label.clone());
    true
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
    let mut clip = narrowed(None, root);
    for step in path {
        match node.base().children.get(*step) {
            Some(child) => {
                node = child.as_ref();
                clip = narrowed(clip, node);
            }
            None => return false,
        }
    }
    if !is_target(node) {
        return false;
    }
    give(node, &label, clip)
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
        clip: Option<Rectangle>,
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
        let names_itself = node.base().answers_to(key);
        if names_itself {
            // **Nothing here can show it.** Asked before the search below, not inside it: the four
            // steps exist to find *which widget draws this row's letter*, and one of them is the
            // nearest target ENCLOSING the row — so a row past the sidebar's fold would otherwise
            // hand its letter up to the workspace header, which is a wrong letter rather than no
            // letter. The named thing is not visible in this tree, so this tree is not one of the
            // places that can show it (F003/P082/T438).
            if label.is_some() && unseen(node, clip) {
                return false;
            }
            // A declaration inside wins first (a dock names itself on the outside and declares the
            // pick within), then a declaration *enclosing* it, and only then anything merely
            // actionable inside. Same precedence in both directions: whoever DECLARED what a pick
            // does owns the letter — and the placement it drew with.
            if label_nearest_matching(
                node,
                label,
                clip,
                &|c: &dyn Component| c.base().hint.is_some(),
                Some(key),
            ) {
                return true;
            }
            if let Some(outer) = declaring
                && give(outer, label, clip)
            {
                return true;
            }
            if label_nearest_matching(node, label, clip, &|_: &dyn Component| true, Some(key)) {
                return true;
            }
            if let Some(outer) = enclosing
                && give(outer, label, clip)
            {
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
        let clip = narrowed(clip, node);
        for child in &node.base().children {
            if walk(child.as_ref(), key, label, enclosing, declaring, clip) {
                found = true;
            }
        }
        found
    }
    walk(root, key, &label, None, None, None)
}

/// Whether this subtree **is somebody else** — it holds an identity that is not `owner`'s.
///
/// Asked of a subtree rather than of a node, because the declaration and the name are on different
/// widgets and usually the wrong way round: a row names itself on the inside and the `KeyHint`
/// carrying its declaration wraps it. Asking only whether *this* node names something else would
/// therefore never fire — the wrapper names nothing, and the letter lands on it.
///
/// A subtree that names nothing is part of whatever encloses it, which is what keeps a `KeyHint`, a
/// `Flex` or a `Surface` transparent here.
fn governs_foreign_identity(node: &dyn Component, owner: Option<&str>) -> bool {
    if skip(node) {
        return false;
    }
    let base = node.base();
    if let Some(declared) = base.key.as_deref().or(base.scope_key.as_deref())
        && Some(declared) != owner
    {
        return true;
    }
    base.children
        .iter()
        .any(|c| governs_foreign_identity(c.as_ref(), owner))
}

/// The nearest target in this subtree that `pick` accepts, labelled — **without crossing into
/// something that is somebody else.**
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
///
/// # A nested identity owns its own letter
///
/// `key` is what stops the descent. Searching inside the named thing is right — a container names
/// itself on the outside and may declare its pick within — but a child that **names something
/// else** is a different thing, and its declaration says what a pick does to *it*. Handing it a
/// letter that belongs to its parent draws the parent's keycap on the child, in the child's place
/// and the child's colour.
///
/// That is what happened: a workspace dock declares its pick on the wrapper *outside* itself, so
/// the search went in and found the first pane row's — and the workspace's letter appeared over a
/// pane, blue and right-aligned, instead of orange on the workspace's own header. Everything the
/// caller sees is the same; the letter simply lands on the wrong widget.
fn label_nearest_matching(
    node: &dyn Component,
    label: &Option<String>,
    clip: Option<Rectangle>,
    pick: &dyn Fn(&dyn Component) -> bool,
    owner: Option<&str>,
) -> bool {
    if skip(node) {
        return false;
    }
    if is_target(node) && pick(node) && give(node, label, clip) {
        return true;
    }
    let clip = narrowed(clip, node);
    // **Do not go into somebody else.** Checked per child, never of the node the search started
    // from: that one *is* `owner`. A child subtree holding a different identity is a different
    // thing, and its declaration says what a pick does to it.
    node.base().children.iter().any(|c| {
        !governs_foreign_identity(c.as_ref(), owner)
            && label_nearest_matching(c.as_ref(), label, clip, pick, owner)
    })
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

/// **Change what a named node says, without rebuilding anything** (F003/P097/T500).
///
/// The text is a signal, so this is a write to reactive state: the tree keeps its identity, its
/// layout and every widget inside it. That matters because a rebuild is not free and not invisible
/// — a freshly built widget has no layout node until the walk reaches it, and **nothing is painted
/// before it has a box**, so the frame after a rebuild draws nothing where the old tree was.
///
/// The case it was written for: a pane's header shows the foreground program and whether the last
/// command succeeded. Both change twice per command, and while they were part of the header's
/// *identity* every command threw the whole header away and built a new one — the buttons blinked
/// out and back twice, which is what a user sees as a flash (Antonio, driving, 2026-09-05).
///
/// Addressed by the node's own declared key, exactly as [`offer_hint_by_key`] is, so there is
/// nothing to register and nothing to release when the tree does change for a real reason.
/// Answers `true` if anything took it. Every matching node is written, because one thing may be
/// shown in more than one place.
pub fn set_text_by_key(root: &dyn Component, key: &str, text: &str) -> bool {
    fn walk(node: &dyn Component, key: &str, text: &str) -> bool {
        if skip(node) {
            return false;
        }
        let mut wrote = false;
        if node.base().key.as_deref() == Some(key) {
            wrote |= node.set_text(text.to_string());
            // A named wrapper is allowed to hold the words rather than be them — the same shape
            // `offer_hint_by_key` handles, where the named node's own answer may live just beneath
            // it. Only the nearest one, so a panel with a name does not rewrite every label in it.
            if !wrote {
                for child in node.base().children.iter() {
                    if child.set_text(text.to_string()) {
                        return true;
                    }
                }
            }
        }
        for child in node.base().children.iter() {
            wrote |= walk(child.as_ref(), key, text);
        }
        wrote
    }
    walk(root, key, text)
}

/// **Light the named node, and darken every other one it knows about** (F003/P097/T501).
///
/// A container that moves a cursor over its children says which one it is on by that child's own
/// key — the same addressing [`offer_hint_by_key`] and [`set_text_by_key`] use, and for the same
/// reason: nothing is registered, so nothing has to be released when the tree is rebuilt.
///
/// It replaces the caller wiring the two together. A grid used to be handed each card's own state
/// signal (`GridCell::new(key, card.nav_state())`), which works but means every caller has to know
/// the connection exists and make it — and a *described* card cannot make it at all, because a
/// description has no way to name another node's signal. Addressing by key is something both
/// authoring paths can do.
///
/// `keys` is every key the container owns, so exactly one ends lit and the rest are cleared in the
/// same walk — a cursor is single-valued, and clearing separately is how two claims survive at once.
pub fn set_selected_by_key(root: &dyn Component, keys: &[String], lit: Option<&str>) -> bool {
    fn walk(node: &dyn Component, keys: &[String], lit: Option<&str>, any: &mut bool) {
        if skip(node) {
            return;
        }
        if let Some(k) = node.base().key.as_deref()
            && keys.iter().any(|owned| owned == k)
        {
            *any |= node.set_selected(Some(k) == lit);
        }
        for child in node.base().children.iter() {
            walk(child.as_ref(), keys, lit, any);
        }
    }
    let mut any = false;
    walk(root, keys, lit, &mut any);
    any
}

