//! **Events reaching the tree**: the one walk that delivers them, and the focus path keys take.

use super::*;

/// Combine two "seconds until next redraw" requests, keeping the sooner one
/// (`None` means "no request").
pub fn soonest_redraw(a: Option<f32>, b: Option<f32>) -> Option<f32> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

/// Deliver `ev` to `node` and its subtree. **The only child walk in this library.**
///
/// Three steps, and a widget takes part by implementing at most two of them:
///
/// 1. [`on_event_capture`](Component::on_event_capture) — the widget's chance to take the event
///    away from its own subtree. `Yes` stops here.
/// 2. **the children**, last-added first so the top of the z-order wins. No widget writes this.
/// 3. [`on_event`](Component::on_event) — what the widget does with what nobody below wanted.
///
/// Step 2 is the point. It used to live inside each container's `event`, which meant every
/// container decided, one `match` arm at a time, which kinds of event its children were allowed to
/// see — and a container that only cared about presses silently stranded any child that needed the
/// wheel or a release. That is not a mistake anyone makes on purpose; it is what a hand-written
/// list does over time. Now there is no list.
pub fn dispatch(node: &mut dyn Component, ev: &Event) -> Handled {
    // **What is held down is remembered here, once.** It is an announcement about the device, so it
    // is recorded before it is passed on and everything else reads it from
    // [`event::modifiers`](crate::event::modifiers) — instead of each widget keeping a private copy
    // of the same broadcast and each pointer call site remembering to attach it.
    if let Event::ModifiersChanged(m) = ev {
        crate::event::remember_modifiers(*m);
    }
    // The raw pointer stream is not delivered: it is resolved, once, and what it *means* is
    // delivered instead — to the widget under the pointer and then up its ancestors.
    if let Event::Raw(raw) = ev {
        return crate::pointer::route(node, raw);
    }
    deliver(node, ev)
}

/// Deliver an event that carries no position: a key, typed text, a semantic intent, a modifier
/// change, a mount.
///
/// **Keyboard events go to the focused widget and bubble**, exactly as they do in a browser: the
/// walk goes down the ancestor chain of whatever holds focus (each ancestor may take it away in
/// capture), reaches the focus owner, and comes back up through the same chain. Nothing is
/// declared, nothing is forwarded, and a widget that does not hold focus — or contain the thing
/// that does — is not offered the key at all.
///
/// That last clause used to be three separate flags. A container said `routes_own_subtree` to take
/// over the walk, `takes_raw_keys` to be offered keys without focus, and `takes_text_input` to be
/// offered typed text; each was a question about the widget asked so the framework could route,
/// and each had to be answered right by every author. Focus already says all three: a
/// [`FocusScope`](crate::widgets::FocusScope) that does not hold the keyboard is simply not on the
/// path, so it neither hears the key nor has to decline it.
///
/// Everything else still broadcasts — a modifier change is an announcement to the whole tree, and
/// a resolved pointer event a caller delivers by hand carries its own target.
pub fn deliver(node: &mut dyn Component, ev: &Event) -> Handled {
    if is_keyboard(ev) {
        // **Nothing focused, nothing delivered.** A keyboard event with no owner belongs to no
        // widget, and handing it to the tree anyway is the whole family of bugs this replaced: the
        // first row in a list answering an Enter meant for the cursor, an unfocused dock answering
        // an intent aimed at its neighbour. A host that wants a key to reach a surface focuses the
        // surface — which it already does, because that is what the focus ring means.
        crate::focus::settle(node);
        let Some(path) = focus_path(node) else {
            return Handled::No;
        };
        return deliver_to_path(node, &path, ev);
    }
    broadcast(node, ev)
}

/// Events that follow keyboard focus rather than the whole tree.
///
/// [`Event::Widget`] is one of them: a semantic intent is what a key **resolved to**, so it belongs
/// to whoever the key would have gone to. Delivering it any wider is how one dock answered for
/// another.
fn is_keyboard(ev: &Event) -> bool {
    matches!(
        ev,
        Event::Key { .. } | Event::TextInput(_) | Event::Widget(_)
    )
}

/// **Does anything in this subtree hold the keyboard?**
///
/// The public form of `focus_path` — a host asking "is this surface the one taking keys" gets the
/// same answer the event walk uses, rather than inventing a predicate beside it. A surface that
/// wants keys holds focus (`Overlay`, `ContextMenu` and `CommandPalette` follow their open signal
/// for it), so an open layer answers `true` by being open.
///
/// Hidden and invisible subtrees are skipped, so a dismissed surface that still carries the focus
/// flag its field had when it closed does not answer `true`.
pub fn holds_keyboard(node: &dyn Component) -> bool {
    focus_path(node).is_some()
}

/// The path from `node` to the widget in it that owns the keyboard, or `None` when nothing in this
/// tree does. See [`focus::holder_path`](crate::focus::holder_path) for how a holder is chosen.
pub(crate) fn focus_path(node: &dyn Component) -> Option<Vec<usize>> {
    crate::focus::holder_path(node)
}

/// Capture down `path`, then handlers and [`on_event`](Component::on_event) back up — the same
/// target-and-bubble walk the pointer uses, for an event whose target is the focus owner.
///
/// An empty path means `node` **is** the target, and the walk turns around there. **Its children
/// are not visited**, for keys, for typed text and for intents alike: the target is the widget that
/// answers, exactly as it is for the pointer. The difference is only in how each one is found —
/// hit-testing walks to the deepest widget under the cursor, and focus is asserted by the deepest
/// widget that holds it.
///
/// The alternative — letting an intent descend into the target's subtree — was tried and taken out.
/// It made a whole region answer for a capability nobody in it had claimed, so two widgets that
/// both handled one intent was not an error, and reading the tree could not tell you where a key
/// landed.
pub(crate) fn deliver_to_path(node: &mut dyn Component, path: &[usize], ev: &Event) -> Handled {
    if node.on_event_capture(ev) == Handled::Yes {
        return Handled::Yes;
    }
    if let Some((head, rest)) = path.split_first() {
        let from_subtree = match node.base_mut().children.get_mut(*head) {
            Some(child) => deliver_to_path(child.as_mut(), rest, ev),
            None => Handled::No,
        };
        node.after_subtree(ev, from_subtree);
        if from_subtree == Handled::Yes {
            return Handled::Yes;
        }
    } else if matches!(ev, Event::Widget(_)) {
        // **An intent enters the focused region.** It is not a key — it is what a key *resolved to*,
        // a capability named out loud (`ScrollPageDown`, `Dismiss`), so it is addressed to the
        // focused region and whichever widget in there owns that capability answers. A raw key
        // stops at the owner, which is what keeps the first row in a list from eating an Enter
        // meant for the cursor.
        let mut from_subtree = Handled::No;
        for child in node.base_mut().children.iter_mut().rev() {
            if broadcast(child.as_mut(), ev) == Handled::Yes {
                from_subtree = Handled::Yes;
                break;
            }
        }
        node.after_subtree(ev, from_subtree);
        if from_subtree == Handled::Yes {
            return Handled::Yes;
        }
    } else if matches!(ev, Event::Hint(_)) {
        // **A pick acts on the widget it named, and on nothing else**. This is the
        // target phase, and it is the only place a widget's own declaration runs — what continues
        // up the chain is the *event*, which is the delegation seam.
        //
        // The divergence from a click is deliberate: clicking a child of a clickable box **is**
        // clicking the box, because the pointer is over both, while picking a row is **not** picking
        // the pane that holds it. A pick is nominal, not spatial. Without this, a pickable pane
        // holding pickable rows fires both and you land on the pane — and every such container would
        // hand-write the DOM's `e.target !== e.currentTarget` guard, which is N copies of a rule
        // that belongs here.
        run_pick(node);
    }
    if node.base_mut().run_handlers(ev).handled == Handled::Yes {
        return Handled::Yes;
    }
    node.on_event(ev)
}

/// **Do to `node` what picking it means**, in two cases and in this order:
///
/// 1. it **declared** what a pick does ([`Base::hint`]) — run that. The override, for a region that
///    answers a pick differently from a click: heca's sidebar row activates the pane and leaves on
///    a click, and stays in the sidebar on a pick.
/// 2. otherwise **act on it as a click would**, because that is what a letter over an ordinary
///    button promises. Delivered as a real [`Event::Click`] at the widget's centre, through the
///    handlers it already registered — never a second path that could drift from what the mouse
///    does.
pub(crate) fn run_pick(node: &mut dyn Component) {
    if let Some(hint) = &node.base().hint {
        hint.run();
        return;
    }
    if !crate::hint::is_target(node) {
        return;
    }
    // The centre, so a handler reading the position lands inside the widget it was aimed at.
    let b = node.base().bounds;
    let at = heca_core::layout::Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    let click = Event::Click(crate::event::PointerEvent::at(at).with_target_bounds(b));
    node.base_mut().run_handlers(&click);
    node.on_event(&click);
}

/// Capture down, children last-first, bubble up — for the events that are addressed to everything
/// rather than to one widget.
fn broadcast(node: &mut dyn Component, ev: &Event) -> Handled {
    if node.on_event_capture(ev) == Handled::Yes {
        return Handled::Yes;
    }
    let mut from_subtree = Handled::No;
    for child in node.base_mut().children.iter_mut().rev() {
        if broadcast(child.as_mut(), ev) == Handled::Yes {
            from_subtree = Handled::Yes;
            break;
        }
    }
    node.after_subtree(ev, from_subtree);
    if from_subtree == Handled::Yes {
        return Handled::Yes;
    }
    if node.base_mut().run_handlers(ev).handled == Handled::Yes {
        return Handled::Yes;
    }
    node.on_event(ev)
}

/// The bounds of the first descendant (or `node` itself) asking to be kept in view, in tree order.
///
/// Depth-first so the innermost claim wins: a focused row inside a marked group is the thing to
/// reveal, not the group.
pub(crate) fn reveal_target(node: &dyn Component) -> Option<Rectangle> {
    for child in &node.base().children {
        if let Some(found) = reveal_target(child.as_ref()) {
            return Some(found);
        }
    }
    node.wants_visible().then(|| node.base().bounds)
}

/// [`reveal_target`] over a child list — what a container scans, since it never reveals *itself*.
pub(crate) fn reveal_target_in(children: &[Box<dyn Component>]) -> Option<Rectangle> {
    children.iter().find_map(|c| reveal_target(c.as_ref()))
}

/// All outstanding requests, deepest first. A lower-priority request remains the same request
/// when the one before it lets go, so a scroll region must remember it too.
pub(crate) fn reveal_requests_in(children: &[Box<dyn Component>]) -> Vec<(u64, Rectangle)> {
    fn collect(node: &dyn Component, requests: &mut Vec<(u64, Rectangle)>) {
        for child in &node.base().children {
            collect(child.as_ref(), requests);
        }
        if node.wants_visible() {
            requests.push((node.base().focus_id(), node.base().bounds));
        }
    }
    let mut requests = Vec::new();
    for child in children {
        collect(child.as_ref(), &mut requests);
    }
    requests
}
