//! **Pointer and widget-intent dispatch into the retained chrome trees.**
//!
//! One place that feeds real input to the chrome root, the per-pane header trees and the pane
//! viewports.
//!
//! Split out of `chrome/mod.rs`. Nothing here is new; the passes are unchanged.

use super::*;

/// Offer a pointer event to the **surfaces above the page** — the whole set, in one place.
///
/// Returns `true` when one of them took it, so the caller stops there and nothing leaks to the
/// page behind. There is deliberately **one** function rather than a branch per winit event: the
/// input a surface needs is not a per-caller choice.
///
/// That choice is what kept breaking. A widget with a *gesture* needs the whole set or it fails in
/// a way nothing catches — a [`ScrollRegion`](heca_grid_ui::ScrollRegion) that never receives the
/// release leaves its thumb welded to the cursor, and one that never receives the wheel simply
/// does not scroll. Both are silent: it lays out, paints and hit-tests perfectly. The modal path
/// used to forward the move and the press only, so both happened, and the fix had already been
/// written twice elsewhere without the hole here being visible from either.
///
/// Since F004/P084/T394 there is only one pointer event to forward — [`Event::Raw`] — and the
/// framework resolves it into whatever it meant. The set cannot go missing a kind because there
/// are no longer kinds to choose between.
///
/// **This asks the tree, not the registry** (F003/P097/T495). It used to look up the front-most
/// modal layer and dispatch straight into that node, which is a second answer to "what is in
/// front" beside the one the walk already has — and the two disagreed once already. Now it asks
/// the tree whether a surface owns this **point** and lets the ordinary walk pick the target
/// itself: a modal's scrim owns every point, a notification card owns only itself, and with
/// nothing over the pointer the page keeps its own gesture order — the drag question is still
/// asked before a row can claim the press (F003/P085/T368).
///
/// **Positional, and once.** The page's own pipeline feeds this same tree further down (a press
/// reaches the chrome after the drag question), so a gate that dispatched and then let the caller
/// continue would deliver one press twice — pairing a click out of the halves twice with it. So
/// the question is asked *before* the event moves: owned ⇒ deliver here and stop; not owned ⇒
/// touch nothing and let the page run.
pub(crate) fn dispatch_surface_pointer(
    state: &mut crate::app_state::AppState,
    ev: &Event,
) -> bool {
    debug_assert!(
        matches!(ev, Event::Raw(_)),
        "dispatch_surface_pointer is the pointer path; keys go through the keymap",
    );
    let Some(pos) = ev.position() else {
        return false;
    };
    if !heca_grid_ui::overlay_occluded_at(&state.window_root, pos) {
        return false;
    }
    let _ = heca_grid_ui::dispatch(&mut state.window_root, ev);
    state.mark_full_redraw();
    true
}

/// Dispatch a pointer press at `pos` into the retained pane headers. Returns
/// `Some((pane_id, consumed))` when the press lands inside a header's bounds:
/// `consumed = true` if an action button handled it (caller must not forward to
/// the terminal); `false` for the header band's empty area (caller focuses the
/// pane, treating the band as chrome — no terminal selection). `None` off any header.
pub(crate) fn dispatch_pane_header_press(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> Option<(PaneId, bool)> {
    let point = Point::new(pos.0 as f64, pos.1 as f64);
    // Collect candidate ids first (avoid holding the map borrow across the dispatch).
    let hit = state
        .pane_headers
        .iter()
        .find(|(_, h)| rect_contains(h.root.base().bounds, point))
        .map(|(id, _)| *id)?;
    let header = state.pane_headers.get_mut(&hit)?;
    let consumed =
        heca_grid_ui::dispatch(&mut header.root, &Event::pointer_pressed(point, heca_grid_ui::PointerButton::Left))
            == heca_grid_ui::Handled::Yes;
    Some((hit, consumed))
}

/// Dispatch a pointer move at `pos` into the retained pane headers so the action
/// buttons' hover affordance updates. Returns `true` if the pointer is over any
/// header (the caller requests a repaint). Does not discard the trees (hover is
/// transient and must persist across moves).
pub(crate) fn dispatch_pane_header_move(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> bool {
    let point = Point::new(pos.0 as f64, pos.1 as f64);
    let mut over = false;
    for header in state.pane_headers.values_mut() {
        let _ = heca_grid_ui::dispatch(&mut header.root, &Event::pointer_moved(point));
        if rect_contains(header.root.base().bounds, point) {
            over = true;
        }
    }
    over
}

/// Feed a pointer release into the retained pane headers, so a gesture that started on one can end.
///
/// The header seam had a press and a move and no release — the last of the four surfaces to be
/// missing a kind. Nothing there grabs the pointer *today*, which is exactly why it went unnoticed:
/// the first widget mounted here that does would have been broken on arrival, the same way a scroll
/// region was in three other places. Not hit-tested, deliberately: a release ends the gesture
/// wherever the cursor drifted to.
pub(crate) fn dispatch_pane_header_release(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) {
    let point = Point::new(pos.0 as f64, pos.1 as f64);
    for header in state.pane_headers.values_mut() {
        let _ = heca_grid_ui::dispatch(&mut header.root, &Event::pointer_released(point, heca_grid_ui::PointerButton::Left));
    }
}

/// Feed the wheel into the retained pane headers. Returns `true` when one consumed it.
///
/// Nothing in a header scrolls today. It is wired anyway, because "no widget here needs it yet" is
/// the reasoning that produced every other missing kind.
pub(crate) fn dispatch_pane_header_wheel(
    state: &mut crate::app_state::AppState,
    ev: &Event,
) -> bool {
    let mut handled = false;
    for header in state.pane_headers.values_mut() {
        handled |= heca_grid_ui::dispatch(&mut header.root, ev) == heca_grid_ui::Handled::Yes;
    }
    handled
}

/// Feed a pointer press into the retained terminal viewport widgets. Returns
/// `true` when any widget consumed the press (badge click or scrollbar drag).
pub(crate) fn dispatch_pane_viewport_press(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> bool {
    let point = Point::new(pos.0 as f64, pos.1 as f64);
    for widgets in state.pane_viewport_widgets.values_mut() {
        if heca_grid_ui::dispatch(&mut widgets.badge, &Event::pointer_pressed(point, heca_grid_ui::PointerButton::Left))
            == heca_grid_ui::Handled::Yes
            || heca_grid_ui::dispatch(&mut widgets.scrollbar, &Event::pointer_pressed(point, heca_grid_ui::PointerButton::Left))
                == heca_grid_ui::Handled::Yes
        {
            return true;
        }
    }
    false
}

/// Feed pointer motion into the retained terminal viewport widgets so hover and
/// scrollbar drags update. Returns `true` if the pointer is over any widget.
pub(crate) fn dispatch_pane_viewport_move(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> bool {
    let point = Point::new(pos.0 as f64, pos.1 as f64);
    let mut over = false;
    for widgets in state.pane_viewport_widgets.values_mut() {
        let badge_handled = heca_grid_ui::dispatch(&mut widgets.badge, &Event::pointer_moved(point))
            == heca_grid_ui::Handled::Yes;
        let scrollbar_handled = heca_grid_ui::dispatch(&mut widgets.scrollbar, &Event::pointer_moved(point))
            == heca_grid_ui::Handled::Yes;
        if badge_handled || scrollbar_handled {
            over = true;
        }
        if (widgets.badge.base().visible.get_untracked()
            && rect_contains(widgets.badge.base().bounds, point))
            || (widgets.scrollbar.base().visible.get_untracked()
                && rect_contains(widgets.scrollbar.base().bounds, point))
        {
            over = true;
        }
    }
    over
}

/// Feed a pointer release into the retained terminal viewport widgets so a
/// scrollbar drag can end even when released outside its bounds.
pub(crate) fn dispatch_pane_viewport_release(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> bool {
    let point = Point::new(pos.0 as f64, pos.1 as f64);
    let mut handled = false;
    for widgets in state.pane_viewport_widgets.values_mut() {
        handled |= heca_grid_ui::dispatch(&mut widgets.badge, &Event::pointer_released(point, heca_grid_ui::PointerButton::Left))
            == heca_grid_ui::Handled::Yes;
        handled |= heca_grid_ui::dispatch(&mut widgets.scrollbar, &Event::pointer_released(point, heca_grid_ui::PointerButton::Left))
            == heca_grid_ui::Handled::Yes;
    }
    handled
}

fn rect_contains(r: Rectangle, p: Point) -> bool {
    p.x >= r.loc.x && p.x <= r.loc.x + r.size.w && p.y >= r.loc.y && p.y <= r.loc.y + r.size.h
}

/// **The one door into the window tree.** Every event the host gives the chrome goes through here.
///
/// It takes the event the loop already built — one `Event::Raw` per device event — rather than a
/// kind per call site. There used to be eight functions here, one per kind, each re-deriving from a
/// position what the loop had just been told; a caller then picked which to call, and picking is
/// how a kind goes missing. A widget with a gesture needs the whole set or it fails in a way
/// nothing catches: a scroll region that never receives the release leaves its thumb welded to the
/// cursor, and one that never receives the wheel simply does not scroll (F003/P097/T496).
///
/// Returns whether a widget consumed it — a `false` is a legitimate answer, not a failure. The host
/// uses it to decide whether the input **also** belongs to whatever sits behind the tree: `false`
/// on a wheel is what lets the pane scroll instead of the sidebar, and on a press it is what stops
/// a press on the sidebar's scrollbar thumb also reading as a press on the card behind it.
///
/// **The tree is kept.** A press used to drop it (`chrome_tree = None`) to stop widget-local state
/// drifting from the canonical store — a rebuild used as a reset, which takes everything else with
/// it. Nothing spanning two events could survive: a scrollbar grab, a scroll position, a hover. A
/// scroll region in the sidebar was impossible for exactly that reason, nothing to do with
/// scrolling. The drift is handled properly twice over: the tree rebuilds when `chrome_signature`
/// changes, and `sync_chrome_signals` pushes value-state into its bound signals every frame.
pub(crate) fn deliver(state: &mut crate::app_state::AppState, ev: &Event) -> bool {
    heca_grid_ui::dispatch(&mut state.window_root, ev) == heca_grid_ui::Handled::Yes
}

/// **Open the menu declared nearest the focused widget**, bubbling outwards — the keyboard
/// counterpart of an unclaimed right-click (F004/P084/T395).
///
/// Returns whether anything declared one. The action that calls this used to mean "the focused
/// pane's menu"; it now means "the focused widget's", which is what makes one binding work for a
/// pane, a column, a workspace and a plugin's own row without the host knowing any of them exist.
pub(crate) fn open_declared_menu_for_focus(state: &mut crate::app_state::AppState) -> bool {
    // **Where the keyboard is, in a chrome surface, is the focused container's cursor** — the
    // `key` of the row it sits on. That is the half only the host knows, so it is the half the
    // host passes; `open_for_keyboard` owns the rest, including falling back to a genuinely focused
    // widget (a plugin's input) when the cursor names nothing.
    let cursor = {
        use heca_grid_ui::reactive::SignalGet as _;
        state
            .chrome_state
            .focused_container()
            .and_then(|mount| state.chrome_state.container_cursor(&mount).get())
    };
    heca_grid_ui::open_for_keyboard(&state.window_root, cursor.as_deref())
}

/// Mount every menu a widget declared and asked to open since the last frame.
///
/// The other half of the sink installed at startup: the widget layer cannot reach a layer, so it
/// queues, and the host drains. One place, called once a frame, so a menu opened from a click, from
/// a key, or from a widget's own timer all arrive the same way.
pub(crate) fn drain_pending_menus(state: &mut crate::app_state::AppState) {
    let queued: Vec<_> = state.pending_menus.borrow_mut().drain(..).collect();
    for (menu, anchor) in queued {
        overlay::present_menu(state, menu, anchor);
    }
}

/// **Act on the drops the framework handed back** — the twin of [`drain_pending_menus`], drained in
/// the same breath and for the same reason: a row owns the gesture, but moving a pane between
/// workspaces needs `&mut AppState`, which a sink has not (F003/P097/T496).
///
/// Two **names** arrive, already resolved. What they mean is this component's own knowledge, so it
/// is looked up in the registry the tree wrote — never parsed. A name neither side recognises is
/// dropped: that is a plugin's row using the same gesture for something the host knows nothing
/// about, and it is not an error.
pub(crate) fn drain_pending_drops(state: &mut crate::app_state::AppState) {
    let queued: Vec<_> = state.pending_drops.borrow_mut().drain(..).collect();
    for dropped in queued {
        let (Some(source), Some(target)) = (
            state
                .chrome_tree
                .as_ref()
                .and_then(|t| t.drag_items.get(&dropped.source).cloned()),
            state
                .chrome_tree
                .as_ref()
                .and_then(|t| t.drag_items.get(&dropped.target).cloned()),
        ) else {
            continue;
        };
        // **Already decided.** Move or swap is the drag API's answer, from the one place that
        // decides it — reading the modifier again here is what let the swap outline promise
        // something this code then did not do (F003/P097/T496).
        let swap = dropped.action.is_swap();
        match source {
            ChromeDragItem::Pane(pane_id) => {
                let Some((ws_idx, _, _)) = crate::find_pane_location(&state.session, pane_id)
                else {
                    continue;
                };
                let row = pane_drop_row(state, target).map(|row| (row, dropped.side));
                crate::mouse::surface_left::accept_drop(state, pane_id, ws_idx, swap, row);
            }
            ChromeDragItem::Column { ws, col } => {
                crate::mouse::release::column_drop(state, ws, col, swap, target, dropped.side);
            }
            // A workspace is a drop target only — nothing picks one up.
            ChromeDragItem::Workspace { .. } => {}
        }
    }
}

/// **Is something being dragged right now?** Asked by the cursor shape, by hover suppression and by
/// whatever decides a move must not reach the program in a pane.
///
/// One place, so the eight callers that need it cannot drift, and it asks the tree — the framework
/// runs the gesture, so it is the only thing that knows (F003/P097/T496). It replaces the app's own
/// drag machine answering for itself, which stopped being true the moment a row owned its dragging.
pub(crate) fn drag_in_flight(state: &crate::app_state::AppState) -> bool {
    heca_grid_ui::dragging(&state.window_root)
}

/// Tell every retained tree the pointer is gone: hover clears, any capture or drag ends.
///
/// One call per tree the host mounts, because each keeps its own hover — that is the point of the
/// state living on the widgets rather than in one router the host would have to own.
pub(crate) fn cancel_every_tree(state: &mut crate::app_state::AppState, ev: &Event) {
    let _ = deliver(state, ev);
    for header in state.pane_headers.values_mut() {
        let _ = heca_grid_ui::dispatch(&mut header.root, ev);
    }
}



