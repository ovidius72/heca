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
pub(crate) fn dispatch_surface_pointer(state: &mut crate::app_state::AppState, ev: &Event) -> bool {
    debug_assert!(
        matches!(ev, Event::Raw(_)),
        "dispatch_surface_pointer is the pointer path; keys go through the keymap",
    );
    if !surface_owns_pointer(&mut state.window_root, ev) {
        return false;
    }
    state.mark_full_redraw();
    true
}

/// **A surface that covers the pointer owns it — and everything behind it is told the pointer is
/// not on it.** Returns whether a surface owned the point.
///
/// Hover is worked out by the walk that routes a move. The window root's walk reaches every
/// surface, the chrome and the panes (they are all nodes in it), so a covered dock row or a header
/// button under a blocking overlay loses its hover, and its tooltip, by itself.
pub(crate) fn surface_owns_pointer(window: &mut dyn Component, ev: &Event) -> bool {
    let Some(pos) = ev.position() else {
        return false;
    };
    if !heca_grid_ui::overlay_occluded_at(window, pos) {
        return false;
    }
    let _ = heca_grid_ui::dispatch(window, ev);
    true
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
    let handled = heca_grid_ui::dispatch(&mut state.window_root, ev) == heca_grid_ui::Handled::Yes;
    // A press may have moved the keyboard: settle it and announce it now, so what the key rules read
    // off the tree is current before the next key.
    if matches!(ev, Event::Raw(raw) if raw.kind == heca_grid_ui::RawPointerKind::Pressed) {
        crate::app::tree_focus::settle_window_focus(state);
    }
    handled
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
        crate::app::tree_focus::focused_dock(state)
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
    for (menu, anchor, subject) in queued {
        overlay::present_menu(state, menu, anchor, subject);
    }
}

/// What a name in a drop means: what the chrome tree recorded for it, or — for a pane, which the
/// workspace seats whether or not the sidebar has a row for it — the pane its key names.
fn drag_item_named(state: &crate::app_state::AppState, name: &str) -> Option<ChromeDragItem> {
    state
        .chrome_tree
        .as_ref()
        .and_then(|t| t.drag_items.get(name).cloned())
        .or_else(|| crate::chrome::pane_id_of_key(name).map(ChromeDragItem::Pane))
}

/// **Act on the drops the framework handed back** — the twin of [`drain_pending_menus`], drained in
/// the same breath and for the same reason: a row owns the gesture, but moving a pane between
/// workspaces needs `&mut AppState`, which a sink has not (F003/P097/T496).
///
/// Two **names** arrive, already resolved. What they mean is this component's own knowledge, so it
/// is looked up in the registry the tree wrote, never guessed at. A name neither side recognises is
/// dropped: that is a plugin's row using the same gesture for something the host knows nothing
/// about, and it is not an error.
pub(crate) fn drain_pending_drops(state: &mut crate::app_state::AppState) {
    let queued: Vec<_> = state.pending_drops.borrow_mut().drain(..).collect();
    for dropped in queued {
        // **A place for a new column** is named by the workspace that seats it, and a pane carried
        // to it makes the column there.
        if let Some(at) = crate::chrome::slot_of_key(&dropped.target) {
            if let Some(pane_id) = crate::chrome::pane_id_of_key(&dropped.source) {
                crate::mouse::dispatch_drop(
                    state,
                    crate::app::interaction::InteractionSource::MouseContent,
                    crate::input::WmAction::PlacePane {
                        pane_id,
                        ws_idx: state.session.active_workspace_idx,
                        col_idx: at,
                        pane_idx: None,
                    },
                );
            }
            continue;
        }
        let (Some(source), Some(target)) = (
            drag_item_named(state, &dropped.source),
            drag_item_named(state, &dropped.target),
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

/// **When does anything on screen next need a frame of its own?** Seconds from now, or `None` when
/// nothing is waiting.
///
/// Some behaviour is "after the pointer has been still for a while" — a tooltip revealing, a caret
/// blinking — and a still pointer produces no events, so nothing would draw the moment it comes due.
/// The widget knows when that is and says so through `Component::next_redraw`, which the framework
/// already folds down a whole tree. **The host's only job is to ask, and to ask every tree it
/// draws**, which is why this sits beside [`cancel_every_tree`] and not in the event loop: a fourth
/// tree family is added here, once, rather than in each caller.
///
/// Until this existed the app never asked at all. The library had the answer and the wake was
/// guarded (`a_pending_tooltip_asks_the_host_to_wake_for_it`), but only the showcase read it — so in
/// heca a tooltip stayed hidden under a resting pointer and appeared the instant you nudged the
/// mouse by a pixel, because the nudge was what produced the frame (Antonio, driving, 2026-09-04).
pub(crate) fn next_redraw_across_trees(state: &crate::app_state::AppState) -> Option<f32> {
    state.window_root.next_redraw()
}

/// Tell every retained tree the pointer is gone: hover clears, any capture or drag ends.
///
/// One call per tree the host mounts, because each keeps its own hover — that is the point of the
/// state living on the widgets rather than in one router the host would have to own.
pub(crate) fn cancel_every_tree(state: &mut crate::app_state::AppState, ev: &Event) {
    let _ = deliver(state, ev);
}

#[cfg(test)]
mod surface_pointer_tests {
    use super::*;
    use heca_grid_ui::builders::{ComponentExt, LayoutExt, Parent};
    use heca_grid_ui::widgets::{Button, Flex, Label, Overlay};
    use heca_grid_ui::{LayoutEngine, RawPointer, RawPointerKind, Size};

    fn moved(x: f64, y: f64) -> Event {
        Event::Raw(RawPointer::new(RawPointerKind::Moved, Point::new(x, y)))
    }

    /// A pane's header: one button with a tooltip, filling the box.
    fn header() -> Box<dyn Component> {
        let button = Button::new("Close").tooltip("Close the pane");
        Box::new(Flex::row().width(heca_grid_ui::Length::FULL).child(button))
    }

    fn hovered(root: &dyn Component) -> bool {
        fn any(c: &dyn Component) -> bool {
            c.base().pointer.hovered_for().is_some()
                || c.base().children.iter().any(|k| any(k.as_ref()))
        }
        any(root)
    }

    /// **A blocking layer appears under a pointer that has not moved: what is below it stops being
    /// hovered, and so stops showing its tooltip.**
    ///
    /// The pane is in the window's own tree, so the walk that routes the move reaches it.
    #[test]
    fn a_blocking_surface_over_a_still_pointer_clears_hover_behind_it() {
        let mut window = crate::chrome::new_window_root();
        window.base_mut().children.push(header());
        LayoutEngine::new().compute(&mut window, Size::new(400.0, 300.0));
        // The pointer rests on the button: the tree hovers it and starts the tooltip clock.
        let _ = heca_grid_ui::dispatch(&mut window, &moved(10.0, 10.0));
        assert!(
            hovered(window.base().children[0].as_ref()),
            "precondition: the pointer is on the button"
        );

        // A blocking overlay opens in the window root — it owns every point in the viewport.
        crate::chrome::place_surface(
            &mut window,
            "surface:1",
            Box::new(
                Overlay::new()
                    .panel(Label::new("A note"))
                    .default_open(true),
            ),
        );
        LayoutEngine::new().compute(&mut window, Size::new(400.0, 300.0));

        // The pointer has not moved: the host re-routes a move at the same place.
        let owned = surface_owns_pointer(&mut window, &moved(10.0, 10.0));

        assert!(owned, "a blocking surface owns the point");
        assert!(
            !hovered(window.base().children[0].as_ref()),
            "the button under the overlay must lose hover, or its tooltip stays up"
        );
    }

    /// **With nothing over the pointer, nothing behind is disturbed** — the page keeps its hover.
    #[test]
    fn with_no_surface_over_the_pointer_the_page_keeps_its_hover() {
        let mut window = crate::chrome::new_window_root();
        window.base_mut().children.push(header());
        LayoutEngine::new().compute(&mut window, Size::new(400.0, 300.0));
        let _ = heca_grid_ui::dispatch(&mut window, &moved(10.0, 10.0));

        let owned = surface_owns_pointer(&mut window, &moved(10.0, 10.0));

        assert!(!owned);
        assert!(hovered(window.base().children[0].as_ref()));
    }
}
