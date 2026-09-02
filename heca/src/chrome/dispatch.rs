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

/// Feed a pointer-press into the retained chrome tree so widget callbacks can route
/// sidebar intents through the app event loop.
///
/// **The tree is kept.** It used to be dropped here (`chrome_tree = None`) to stop incidental
/// widget-local state drifting from the canonical store — but that is a rebuild used as a reset,
/// and it takes everything else with it. Nothing that spans two events can survive: a scrollbar
/// grab, a scroll position, a hover. A scroll region in the sidebar was impossible for exactly this
/// reason, not for any reason to do with scrolling.
///
/// The drift it guarded against is already handled properly, twice over: the tree is rebuilt
/// whenever `chrome_signature` changes (which is what a press that alters canonical state does),
/// and `sync_chrome_signals` pushes value-state into the tree's bound signals every frame. State
/// that must not drift belongs in one of those — in the store, read through a signal — which is the
/// read-via-signals/write-via-actions rule this codebase already runs on.
/// Returns `true` when a widget consumed the press — the caller must then treat it as spoken for
/// and not also resolve it by geometry. That gate is what stops a press on the sidebar's scrollbar
/// thumb being read as a press on the pane card behind it.
pub(crate) fn chrome_dispatch_press(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> bool {
    chrome_dispatch_button_press(state, pos, heca_grid_ui::PointerButton::Left)
}

/// The same, for a **named button** — so a right-press reaches the tree instead of being read off
/// it from the outside.
///
/// A press carries its button now, which is what makes a widget able to answer a right-click at
/// all. The app still has its own menu path behind this (F004/P084/T395 is what removes it); this
/// is the door that lets a widget claim the press before any of that runs.
pub(crate) fn chrome_dispatch_button_press(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
    button: heca_grid_ui::PointerButton,
) -> bool {
    heca_grid_ui::dispatch(
        &mut state.window_root,
        &Event::pointer_pressed(Point::new(pos.0 as f64, pos.1 as f64), button),
    ) == heca_grid_ui::Handled::Yes
}

/// Deliver a **button release** to the chrome tree.
///
/// The other half of [`chrome_dispatch_button_press`], and not optional: the framework synthesises
/// `Click` / `RightClick` from a press **and** a release on the same widget, so a host that
/// delivers only presses produces no clicks at all — a declared context menu would never open, and
/// a widget that captured the press would never learn the gesture ended.
pub(crate) fn chrome_dispatch_button_release(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
    button: heca_grid_ui::PointerButton,
) -> bool {
    heca_grid_ui::dispatch(
        &mut state.window_root,
        &Event::pointer_released(Point::new(pos.0 as f64, pos.1 as f64), button),
    ) == heca_grid_ui::Handled::Yes
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
        // heca reads Shift as "swap these two". The framework reports the modifiers and has no
        // opinion, which is what lets another host read them differently.
        let swap = dropped.modifiers.shift;
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
pub(crate) fn chrome_dispatch_cancelled(state: &mut crate::app_state::AppState, ev: &Event) {
    let _ = heca_grid_ui::dispatch(&mut state.window_root, ev);
    for header in state.pane_headers.values_mut() {
        let _ = heca_grid_ui::dispatch(&mut header.root, ev);
    }
}



/// Feed a pointer-release into the retained chrome tree, so a gesture that started there can end.
///
/// Without it a scrollbar thumb grabbed in the sidebar stays welded to the cursor — the widget is
/// still waiting for the end of a gesture nobody told it about. Deliberately not hit-tested: a
/// release ends the gesture wherever the cursor drifted to.
pub(crate) fn chrome_dispatch_release(state: &mut crate::app_state::AppState, pos: (f32, f32)) {
    heca_grid_ui::dispatch(
        &mut state.window_root,
        &Event::pointer_released(
            Point::new(pos.0 as f64, pos.1 as f64),
            heca_grid_ui::PointerButton::Left,
        ),
    );
}

/// Feed the wheel into the retained chrome tree. Returns `true` when it was consumed — a hovered
/// scroll region took it — so the caller leaves the terminal alone.
pub(crate) fn chrome_dispatch_wheel(
    state: &mut crate::app_state::AppState,
    ev: &Event,
) -> bool {
    heca_grid_ui::dispatch(&mut state.window_root, ev) == heca_grid_ui::Handled::Yes
}

/// Feed a semantic [`WidgetIntent`](heca_grid_ui::WidgetIntent) into the retained chrome tree.
///
/// One intent goes to the **root**, not to a container the host picked: every mounted container sits
/// inside a [`FocusScope`](heca_grid_ui::FocusScope), and only the focused one lets a
/// `Event::Widget` into its subtree (F003/P085/T351). So the host says *what*, and the tree decides
/// *where* — which is what keeps this working when a second dock is mounted, or the same dock is
/// placed twice.
///
/// Returns `true` when something acted on it. A `false` is a legitimate answer, not a failure: a
/// container with nothing scrollable **declines**, and the caller must not then fall through to the
/// pane — the pane is not an outer scroll area of the sidebar.
pub(crate) fn chrome_dispatch_widget(
    state: &mut crate::app_state::AppState,
    intent: heca_grid_ui::WidgetIntent,
) -> bool {
    heca_grid_ui::dispatch(&mut state.window_root, &Event::Widget(intent))
        == heca_grid_ui::Handled::Yes
}

/// Feed a pointer-move into the retained chrome tree so its **hover affordances**
/// update in the real app — the `MarkerGroup` grip brightening (the column's "grab
/// me" cue) and `Row` hover. The app otherwise only dispatches `PointerPressed`, so
/// these were inert in the sidebar though they work in the showcase. Unlike
/// [`chrome_dispatch_press`] this does **not** discard the tree — hover is transient
/// and must persist across moves; the caller already requests a repaint.
pub(crate) fn chrome_dispatch_move(state: &mut crate::app_state::AppState, pos: (f32, f32)) {
    heca_grid_ui::dispatch(
        &mut state.window_root,
        &Event::pointer_moved(Point::new(pos.0 as f64, pos.1 as f64)),
    );
}
