//! The drag half of the pointer router: starting a drag from the widget a button went down on,
//! carrying it across drop targets, and ending it.

use super::*;

/// A move while a press is live: start a drag once the pointer has travelled far enough, then keep
/// the source and whatever it is over informed.
pub(super) fn drive_drag(root: &mut dyn Component, press: &[usize], raw: &RawPointer) -> Handled {
    // A drag already in flight is followed whatever the pick-up rule says now; only a new one asks.
    let carried = dragging_path(root).and_then(|path| drag_identity(root, &path).map(|id| (path, id)));
    let Some((source_path, item)) = carried.or_else(|| drag_source_on(root, press)) else {
        return Handled::No;
    };
    let dragging = node_at(root, &source_path).base().pointer.dragging.get();
    if !dragging {
        let origin = node_at(root, &source_path)
            .base()
            .pointer
            .press
            .get()
            .or_else(|| node_at(root, press).base().pointer.press.get());
        let far = origin.is_some_and(|(p, _)| dist(p, raw.pos) >= DRAG_THRESHOLD);
        if !far {
            return Handled::No;
        }
        node_at(root, &source_path)
            .base()
            .pointer
            .dragging
            .set(true);
        if let Some((at, _)) = origin {
            node_at(root, &source_path).base().pointer.drag_origin.set(at);
        }
        let ev = Event::DragStart(drag_event(&item, raw, DropSide::Onto));
        let _ = deliver_path(root, &source_path, &ev);
        announce(
            root,
            &source_path,
            &Event::DragInFlight(drag_event(&item, raw, DropSide::Onto)),
        );
    }

    let kind = node_at(root, &source_path).base().drag_kind.clone();
    let hit = crate::drag::resolve_at_for(root, raw.pos, kind.as_deref());
    {
        // The source draws what follows the cursor, so it is the source that must know where the
        // cursor is: its own bounds still say where it was picked up from.
        let p = &node_at(root, &source_path).base().pointer;
        p.drag_pos.set(raw.pos);
    }
    let side = hit.as_ref().map_or(DropSide::Onto, |h| h.side);
    update_drag_over(
        root,
        hit.as_ref().map(|h| (h.path.clone(), h.side)),
        &item,
        raw,
    );
    deliver_path(
        root,
        &source_path,
        &Event::Drag(drag_event(&item, raw, side)),
    )
}

/// The release that ends a drag: the target it landed on hears [`Drop`](Event::Drop), then the
/// source hears [`DragEnd`](Event::DragEnd) — always, so a source can undo its own state whether
/// or not anything accepted it.
pub(super) fn finish_drag(root: &mut dyn Component, raw: &RawPointer) -> Handled {
    let Some(source_path) = dragging_path(root) else {
        return Handled::No;
    };
    let Some(item) = drag_identity(root, &source_path) else {
        return Handled::No;
    };
    let kind = node_at(root, &source_path).base().drag_kind.clone();
    let hit = crate::drag::resolve_at_for(root, raw.pos, kind.as_deref());
    let mut handled = Handled::No;
    if let Some(hit) = hit.as_ref() {
        let ev = Event::Drop(drag_event(&item, raw, hit.side));
        handled = deliver_path(root, &hit.path, &ev);
        // **What happens to a drop nobody took is the host's**, the same way an unclaimed
        // right-click with a declared menu is. A row can say it accepts drops; it cannot move a
        // pane into another workspace. So this crosses back once, with both identities already
        // resolved — and a row that wants to answer for itself still wins, because it consumed the
        // event above and never reaches here.
        if handled == Handled::No {
            crate::drag::sink::present(crate::drag::Dropped {
                source: item.clone(),
                target: hit.key.clone(),
                side: hit.side,
                action: crate::drag::DropAction::held(),
                modifiers: raw.modifiers,
            });
            handled = Handled::Yes;
        }
    }
    clear_drag_over(root);
    node_at(root, &source_path)
        .base()
        .pointer
        .dragging
        .set(false);
    announce(
        root,
        &source_path,
        &Event::DragSettled(drag_event(&item, raw, DropSide::Onto)),
    );
    let side = hit.as_ref().map_or(DropSide::Onto, |h| h.side);
    let ev = Event::DragEnd(drag_event(&item, raw, side));
    or(handled, deliver_path(root, &source_path, &ev))
}

/// **Tell every drop target that accepts what is being carried that a drag began or ended** —
/// wherever it sits in the tree, shown or not, and only that target (nothing bubbles). The source
/// itself is not told: it has its own events.
pub(super) fn announce(root: &mut dyn Component, source: &[usize], ev: &Event) {
    let kind = node_at(root, source).base().drag_kind.clone();
    fn tell(node: &mut dyn Component, kind: Option<&str>, ev: &Event, here: &mut Path, source: &[usize]) {
        if node.is_drop_target() && node.accepts_drag(kind) && here.as_slice() != source {
            let _ = node.base_mut().run_handlers(ev);
            let _ = node.on_event(ev);
        }
        for i in 0..node.base().children.len() {
            here.push(i);
            tell(node.base_mut().children[i].as_mut(), kind, ev, here, source);
            here.pop();
        }
    }
    tell(root, kind.as_deref(), ev, &mut Path::new(), source);
}

/// Move the "a drag is over me" flag to `now`, emitting enter/leave/over as it goes.
pub(super) fn update_drag_over(
    root: &mut dyn Component,
    now: Option<(Path, DropSide)>,
    item: &str,
    raw: &RawPointer,
) {
    let previous = drag_over_path(root);
    // **The node the walk found, not a second search for its name.** Two seatings of one container
    // give their rows the same name, so a search lights whichever comes first — the other sidebar.
    let current = now.as_ref().map(|(path, _)| path.clone());
    if previous != current
        && let Some(path) = previous.clone()
    {
        node_at(root, &path).base().pointer.drag_over.set(false);
        let ev = Event::DragLeave(drag_event(item, raw, DropSide::Onto));
        let _ = deliver_path(root, &path, &ev);
    }
    if let Some(path) = current {
        let side = now.map_or(DropSide::Onto, |(_, s)| s);
        node_at(root, &path).base().pointer.drag_side.set(side);
        if previous.as_ref() != Some(&path) {
            node_at(root, &path).base().pointer.drag_over.set(true);
            let ev = Event::DragEnter(drag_event(item, raw, side));
            let _ = deliver_path(root, &path, &ev);
        }
        let ev = Event::DragOver(drag_event(item, raw, side));
        let _ = deliver_path(root, &path, &ev);
    }
}

/// The innermost drag source at or above the pressed widget, and its path.
pub(super) fn drag_source_on(root: &dyn Component, press: &[usize]) -> Option<(Path, String)> {
    let mut node = root;
    let mut best: Option<(Path, String)> = None;
    let mut here = Path::new();
    if let Some(id) = startable_identity(root, &here) {
        best = Some((here.clone(), id));
    }
    for i in press {
        let Some(child) = node.base().children.get(*i) else {
            break;
        };
        node = child.as_ref();
        here.push(*i);
        if let Some(id) = startable_identity(root, &here) {
            best = Some((here.clone(), id));
        }
    }
    best
}

/// **Is a drag in flight anywhere in this tree?**
///
/// The one question a host asks about a drag it does not own: while something is being carried, the
/// cursor changes shape, nothing hovers, and a move must not reach the program in a pane. The app
/// used to answer it from a drag machine of its own; the framework runs the gesture, so the
/// framework answers (F003/P097/T496).
pub fn dragging(root: &dyn Component) -> bool {
    dragging_path(root).is_some()
}

/// **Where the widget being carried sits** — its own laid-out bounds, not the pointer's — or `None`
/// when nothing is being dragged. What lets a host tell a drag that began in one region from one
/// that began in another without keeping a list of what can be dragged.
pub fn dragged_bounds(root: &dyn Component) -> Option<heca_core::layout::Rectangle> {
    dragging_path(root).map(|path| bounds_at(root, &path))
}

/// The path to the widget currently dragging, if any.
pub(super) fn dragging_path(root: &dyn Component) -> Option<Path> {
    find(root, &|c| c.base().pointer.dragging.get())
}

/// The path to the drop target a drag is currently over, if any.
pub(super) fn drag_over_path(root: &dyn Component) -> Option<Path> {
    find(root, &|c| c.base().pointer.drag_over.get())
}

/// The path to the drop target registered under `id`.
/// The dragged identity of the widget at `path`, when it is a drag source at all — the same
/// answer [`drag::source_at`](crate::drag::source_at) gives, so the gesture and the resolution
/// cannot disagree about what is being carried.
pub(super) fn drag_identity(root: &dyn Component, path: &[usize]) -> Option<String> {
    let node = node_at(root, path);
    if !node.is_drag_source() {
        return None;
    }
    node.base()
        .key
        .clone()
        .or_else(|| crate::nav::identity_of(root, path))
}

/// [`drag_identity`], but only for a widget whose pick-up rule holds right now — what a *new* drag
/// asks. One in flight uses [`drag_identity`], which is never refused.
fn startable_identity(root: &dyn Component, path: &[usize]) -> Option<String> {
    node_at(root, path)
        .may_start_drag()
        .then(|| drag_identity(root, path))
        .flatten()
}

pub(super) fn clear_drag_over(root: &dyn Component) {
    walk(root, &|c| c.base().pointer.drag_over.set(false));
}

pub(super) fn drag_event(item: &str, raw: &RawPointer, side: DropSide) -> DragEvent {
    DragEvent {
        item: item.to_string(),
        pos: raw.pos,
        modifiers: raw.modifiers,
        side,
    }
}
