//! Mouse interaction system.
//!
//! The pointer, as far as the host has to know it: where it is, what shape it takes, what a press
//! means once the window tree has had it, and edge scrolling while something is carried.
//!
//! **No gesture is here.** A pane, a column or a row declares that it can be dragged and
//! `heca-grid-ui` runs the gesture over the same tree it lays out and paints; the drop comes back
//! through `chrome::drain_pending_drops`.

pub(crate) mod release;
pub(crate) mod surface_left;

use crate::app::interaction::InteractionSource;
use crate::app_state::AppState;
use crate::chrome::ChromeConfig;
use crate::input::WmAction;

/// **A drop's action goes through the dispatcher**, like a click's.
///
/// Posted to the event loop, which runs it through the interaction policy and the follow-up every
/// action gets (focus re-sync, redraw, refreshing what lists the session). A drop used to call the
/// handler itself and then the follow-up by hand — the one place left that ran an action without
/// the dispatcher, so it had to remember what the dispatcher does.
pub(crate) fn dispatch_drop(state: &AppState, source: InteractionSource, action: WmAction) {
    crate::chrome::ChromeIntentEmitter::new(&state.event_proxy, source).fire(
        crate::app::interaction::InteractionIntent::ActivateAction(action),
    );
}

/// Handle cursor movement. Returns a `WmAction` if one should be dispatched
/// (e.g. focus-follows-mouse triggered), or `None` for internal state updates.
pub fn on_cursor_moved(state: &mut AppState, pos: (f32, f32)) -> Option<WmAction> {
    state.mouse.pos = pos;
    None
}

/// Cursor policy: ask the window tree what the pointer is over and apply it to the window — but only
/// when it changes (cursor-moved fires very often). The widgets say what the cursor is
/// (`heca_grid_ui::cursor_at`: grabbing while anything is dragged, a hand over what can be picked up,
/// whatever a widget declared); this only turns the answer into the window's own icon. `heca-grid-ui`
/// stays cursor-free (it only emits a `Scene`) — the OS cursor is a host concern.
pub(crate) fn update_cursor(state: &mut AppState, pos: (f32, f32)) {
    let icon = cursor_icon(heca_grid_ui::cursor_at(
        &state.window_root,
        heca_core::layout::types::Point::new(pos.0 as f64, pos.1 as f64),
    ));
    if state.current_cursor != icon {
        state.current_cursor = icon;
        state.window.set_cursor(icon);
    }
}

/// The window's icon for a widget's cursor — the one place the two vocabularies meet.
fn cursor_icon(cursor: heca_grid_ui::Cursor) -> winit::window::CursorIcon {
    use heca_grid_ui::Cursor;
    use winit::window::CursorIcon;
    match cursor {
        Cursor::Default => CursorIcon::Default,
        Cursor::Pointer => CursorIcon::Pointer,
        Cursor::Grab => CursorIcon::Grab,
        Cursor::Grabbing => CursorIcon::Grabbing,
        Cursor::Text => CursorIcon::Text,
        Cursor::ResizeHorizontal => CursorIcon::EwResize,
        Cursor::ResizeVertical => CursorIcon::NsResize,
    }
}

/// Logical-pixel center of the app window — the anchor for keyboard/RPC-opened menus so a menu
/// opened with no pointer stays centered on screen instead of at a stale cursor position.
pub(crate) fn window_center_logical(state: &AppState) -> (f32, f32) {
    let phys = state.window.inner_size();
    let s = state.scale_factor as f32;
    (phys.width as f32 / s / 2.0, phys.height as f32 / s / 2.0)
}

/// Check the focus-follows-mouse timer on every frame (even without cursor movement).
/// Call from `about_to_wait` for frame-rate-independent debounce.
/// Handle mouse button events. Returns a `WmAction` if one should be dispatched.
///
/// **Takes the event the loop built, not the winit pair it was built from.** One `Event::Raw` per
/// device event, handed down rather than reconstructed here — a second construction is a second
/// answer, and the one that used to happen further down this file delivered the same press to the
/// chrome tree twice.
pub fn on_mouse_input(
    state: &mut AppState,
    ev: &heca_grid_ui::Event,
) -> Option<(WmAction, InteractionSource)> {
    if !state.settings.mouse_enabled {
        return None;
    }

    let heca_grid_ui::Event::Raw(raw) = ev else {
        return None;
    };
    use heca_grid_ui::PointerButton as Btn;
    use heca_grid_ui::event::RawPointerKind as Kind;
    match (raw.button, raw.kind) {
        (Btn::Left, Kind::Pressed) => {
            // First, and deliberately independent of whether a widget then consumes the press: a
            // click on a scrollbar thumb is still a click *in* that container and must focus it.
            // Each of the branches below returns early, so doing this later would mean repeating it
            // in every one of them and still missing the paths that consume.

            // **The row answers its own press, and there is no branch here per region.** A press
            // goes to the tree like any other, wherever it lands — a scrollbar thumb, a collapse
            // caret, a button, a pane card in either sidebar. A row that says it can be dragged is
            // dragged by the framework, and one whose press never crosses the threshold pairs with
            // its release into a click, which is what focuses the pane.
            //
            // Nothing is asked before the tree. The app used to take the press first to put its own
            // question — is this a drag source? — which is what made dragging exist only where the
            // app had been taught about it.
            if crate::chrome::deliver(state, ev) {
                return None;
            }

            // **Focusing the clicked pane is gone from here, and that is the point.** A pane says
            // what a press on it means, on itself — so it focuses for the left button and the
            // right one alike, and nothing in the mouse layer has to find which pane the cursor is
            // over in order to do it for it.
            //
            // What stood here hit-tested the pane by hand for the left button; the right button had
            // a second copy that opened the menu first and then asked to focus, which is why
            // right-clicking a pane never focused it.
        }
        (Btn::Left, Kind::Released) => {
            // **A dragged row's release is not handled here.** The framework pairs the press with
            // the release, ends the gesture and hands back a drop naming what it landed on, which
            // `chrome::drain_pending_drops` acts on. A release that crossed no threshold becomes a
            // click by the same pairing, which is what focuses the pane.
        }
        // **The release is what makes it a click.** The framework pairs a press with a release on
        // the same widget and only then emits `RightClick` — which is what an unclaimed right-click
        // turns into a declared context menu. Delivering only the press produced
        // no clicks at all, so no sidebar row opened a menu.
        (Btn::Right, Kind::Released) => {
            // **Both halves of the gesture.** A pane is in the same tree as the chrome, so the
            // release reaches it like any other widget — and a right-click opens the menu a pane
            // declares about itself.
            //
            // The release matters as much as the press: `RightClick` is synthesised from the pair
            // on the same widget, so handing over only one half produces no click and no menu —
            // which is what `heca/tests/pointer_funnel.rs` exists to keep true.
            let handled = crate::chrome::deliver(state, ev);
            state.needs_redraw |= handled;
            return None;
        }
        // Right-click → the menu for whatever is under the cursor: a container's row, else the
        // content pane. **A right-click aims the keyboard the same way a left-click does**:
        // clicking a container's row focuses that container, and clicking
        // outside every container releases it — so the menu that opens describes the same thing
        // the keyboard is now on.
        //
        // Aiming the keyboard first used to be impossible here: opening a menu captured the input
        // mode to restore afterwards, and moving focus first would have rewritten what it captured.
        // Nothing is restored any more, so the constraint went with it.
        // **And this is the whole of a right-click.** The press goes to the tree, whatever is
        // under it deals with it, and a widget that declared a menu gets it opened by the
        // framework. There is no branch here that knows what a pane is.
        //
        // What stood here hit-tested to find the pane, built its menu by hand and then asked to
        // focus it, in that order — so the menu it had just opened covered the panes and the focus
        // was refused every single time: right-clicking a pane never focused it. The pane declares
        // its own menu now, like every other widget. Deleting the
        // special case deletes the ordering it got wrong, which is the point: swapping two lines
        // would have left the next person the same trap.
        (Btn::Right, Kind::Pressed) => {
            let handled = crate::chrome::deliver(state, ev);
            state.needs_redraw |= handled;
        }
        _ => {}
    }

    None
}

/// The longest time (seconds) one edge-scroll step may cover: after a stall — the first frame of a
/// drag, a slow frame — the view moves one frame's worth, not a jump.
const LONGEST_EDGE_SCROLL_STEP: f64 = 0.05;

/// **How hard the columns are pushed with the pointer at `pos`**: `-1.0..0.0` within `trigger` of
/// the left edge of `area`, `0.0..1.0` within it of the right edge, strongest at the edge itself —
/// and nothing anywhere else, outside `area` included.
fn edge_push(pos: (f32, f32), area: heca_core::layout::Rectangle, trigger: f32) -> f32 {
    let (left, top) = (area.loc.x as f32, area.loc.y as f32);
    let (right, bottom) = (left + area.size.w as f32, top + area.size.h as f32);
    let (x, y) = pos;
    if trigger <= 0.0 || x < left || x >= right || y < top || y >= bottom {
        return 0.0;
    }
    if x < left + trigger {
        -(left + trigger - x) / trigger
    } else if x > right - trigger {
        (x - (right - trigger)) / trigger
    } else {
        0.0
    }
}

/// Process edge scrolling in `about_to_wait`. Should be called every frame
/// while a drag is active. Returns true if the view was scrolled.
pub fn process_edge_scroll(state: &mut AppState) -> bool {
    if !state.settings.auto_scroll_edge {
        return false;
    }

    // **The columns scroll while a pane is carried over them, near their edges** — a pane from a
    // column or from a sidebar row alike, as the drag says. A row moved inside a sidebar is over
    // the sidebar, not the columns, so it leaves them where they are.
    let carrying_pane = heca_grid_ui::dragged_kind(&state.window_root).as_deref()
        == Some(crate::chrome::PANE_DRAG_KIND);
    if !carrying_pane {
        return false;
    }
    let pane_area = ChromeConfig::of(state).content_rect();
    let raw = edge_push(
        state.mouse.pos,
        pane_area,
        state.settings.edge_scroll_distance,
    );

    if raw == 0.0 {
        return false;
    }

    // Compute dt for frame-rate independence.
    let now = std::time::Instant::now();
    let dt = state
        .mouse
        .last_edge_scroll_time
        .map(|t| now.duration_since(t).as_secs_f64())
        .unwrap_or(1.0 / 60.0)
        .min(LONGEST_EDGE_SCROLL_STEP);
    state.mouse.last_edge_scroll_time = Some(now);

    let scroll_px = raw as f64 * f64::from(state.settings.edge_scroll_speed) * dt;

    if let Some(mut ws) = state.layout_mut().active_workspace_mut() {
        ws.scroll_mut().edge_scroll(scroll_px);
    }

    true
}

#[cfg(test)]
mod tests;
