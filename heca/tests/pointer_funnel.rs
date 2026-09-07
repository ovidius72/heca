//! The guard that stops the host from delivering *part* of a pointer gesture.
//!
//! Companion to `heca-grid-ui/tests/pointer_delivery.rs`, which pins the same rule one layer down:
//! there, a container must forward the whole pointer set to its children; here, the **host** must
//! deliver the whole set to the tree in the first place. A container that forwards perfectly is
//! still useless if winit's `MouseWheel` never reaches it.
//!
//! It happened exactly that way. The modal branch of the event loop forwarded `PointerMoved` and
//! `PointerPressed` and nothing else, for two reasons that both looked reasonable when written: a
//! modal is a *button panel*, so a wheel seemed irrelevant, and a press is what activates a button,
//! so a release seemed redundant. Then a modal body held a scroll region, and it could not be
//! wheeled and its thumb welded itself to the cursor — with nothing failing anywhere, because
//! there is no such thing as a failed event that was never sent.
//!
//! So the branches no longer choose. They all call one funnel, and this reads the event loop and
//! fails when a pointer branch is handled without it. It is a lint, not a unit test: what it
//! guards against is *absence*, which behaviour tests cannot see.

use std::path::{Path, PathBuf};

/// The single funnel: it takes a pointer event and gives it to whatever tree owns the pointer.
const FUNNEL: &str = "dispatch_surface_pointer";

/// The winit branches that carry pointer input. Each must reach the funnel.
const POINTER_BRANCHES: &[(&str, &str)] = &[
    ("WindowEvent::CursorMoved", "a move is the hover gate — a region ignores a wheel without it"),
    ("WindowEvent::MouseInput", "press AND release; a lost release leaves a thumb stuck to the cursor"),
    ("WindowEvent::MouseWheel", "the wheel itself — without it a scroll region simply does not scroll"),
];

fn events_rs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/events.rs")
}

/// The body of a `WindowEvent::X` match arm: from the branch head to the next one at the same
/// indent. Good enough to tell "this branch calls the funnel" from "this branch does its own thing".
fn branch_body<'a>(src: &'a str, head: &str) -> Option<&'a str> {
    let start = src.find(head)?;
    let rest = &src[start..];
    // The arms sit at one indentation level inside a single `match`; the next `WindowEvent::` at
    // the same depth ends this one.
    let end = rest[head.len()..]
        .find("\n        WindowEvent::")
        .map(|i| i + head.len())
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

#[test]
fn every_pointer_branch_goes_through_the_one_funnel() {
    let src = std::fs::read_to_string(events_rs()).expect("read the event loop");
    for (head, why) in POINTER_BRANCHES {
        let body = branch_body(&src, head)
            .unwrap_or_else(|| panic!("{head} is gone — update this guard with the event loop"));
        assert!(
            body.contains(FUNNEL),
            "{head} handles pointer input without calling `{FUNNEL}`.\n\
             Reason this matters: {why}.\n\
             Route it through the funnel instead of forwarding a hand-picked subset — every widget \
             with a gesture or a wheel depends on getting the whole set, and dropping one kind \
             fails silently: the widget lays out, paints and hit-tests perfectly while being dead.",
        );
    }
}

/// The retained **chrome tree** (the sidebar) needs the same completeness as the modal layer, and
/// for the same reason: it holds a scroll region now.
///
/// It got the press and the move for years and neither the release nor the wheel, so a scrollbar
/// grabbed in the sidebar could never be let go and the wheel did nothing there at all. Each entry
/// names the branch and the call it must contain.
#[test]
fn the_chrome_tree_gets_the_release_and_the_wheel_too() {
    let src = std::fs::read_to_string(events_rs()).expect("read the event loop");
    for (head, call, why) in [
        (
            "WindowEvent::MouseInput",
            "crate::chrome::deliver(",
            "a gesture started in the sidebar has to be able to end — a thumb grabbed there stays \
             welded to the cursor otherwise",
        ),
        (
            "WindowEvent::MouseInput",
            "crate::chrome::deliver_to_panes(",
            "a pane's own tree carries its info bar now, so a bar button that captured a press has \
             to learn the gesture ended",
        ),
        (
            "WindowEvent::MouseWheel",
            "crate::chrome::deliver_to_panes(",
            "nothing in a header scrolls yet, and \"nothing needs it yet\" is the reasoning that \
             produced every other missing kind",
        ),
        (
            "WindowEvent::MouseWheel",
            "crate::chrome::deliver(",
            "a scroll region in the sidebar scrolls on the wheel, and the terminal must not also \
             scroll when it takes it",
        ),
    ] {
        let body = branch_body(&src, head).unwrap_or_else(|| panic!("{head} is gone"));
        assert!(
            body.contains(call),
            "{head} never calls `{call}`.\nWhy it matters: {why}.",
        );
    }
}

/// The release is the half that keeps getting dropped, because a press is what "does" something —
/// so it gets its own check, and the check looks at **what feeds the funnel**, not at whether the
/// branch mentions a release somewhere. The shipped bug named `ElementState::Released` further
/// down the same branch, for an unrelated path, while the modal call site was wrapped in
/// `if button_state == ElementState::Pressed`.
#[test]
fn the_button_branch_feeds_the_funnel_both_press_and_release() {
    let src = std::fs::read_to_string(events_rs()).expect("read the event loop");
    let body = branch_body(&src, "WindowEvent::MouseInput").expect("the button branch");
    let call = body.find(FUNNEL).unwrap_or_else(|| {
        panic!("the button branch does not call `{FUNNEL}` — see the other test")
    });
    let feeding = &body[..call];
    for state in ["ElementState::Pressed", "ElementState::Released"] {
        assert!(
            feeding.contains(state),
            "the button branch reaches `{FUNNEL}` without deciding what to do with {state}.\n\
             A press with no matching release is not a click, it is a gesture that never ends: \
             the widget that grabbed the pointer keeps it. This is how a scroll thumb ends up \
             welded to the cursor.",
        );
    }
}

/// `heca/src/mouse.rs` is the **second** place a button reaches the chrome tree, and it had the same
/// half-gesture defect the event loop was fixed for — found by the user in the running app, on the
/// day it was introduced, with the whole suite green.
///
/// The right-button branch dispatched a **press** into the chrome tree and no release. That is not
/// half a click, it is *no* click: the framework synthesises `Click` / `RightClick` from a press and
/// a release on the same widget, so a host that sends only presses produces neither. Every context
/// menu declared on a widget stopped opening, and nothing failed — the declaration tests dispatch
/// both halves themselves, so they passed while the app sent one.
///
/// A lint, like its neighbours above: what it guards is *absence*, which no behaviour test can see.
#[test]
fn the_mouse_layer_sends_a_release_for_every_press_it_sends() {
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mouse.rs"))
        .expect("read the mouse layer");

    // The two arms that hand a right button to the tree. Named by their match pattern, because the
    // point is that BOTH halves of the gesture are handed over — not that the file mentions the
    // call somewhere.
    for arm in [
        "(Btn::Right, Kind::Pressed) =>",
        "(Btn::Right, Kind::Released) =>",
    ] {
        let at = src
            .find(arm)
            .unwrap_or_else(|| panic!("the mouse layer no longer has a `{arm}` arm — if that moved, move this guard with it rather than deleting it"));
        let body = &src[at..];
        let end = body[arm.len()..]
            .find("\n        (")
            .map(|i| i + arm.len())
            .unwrap_or(body.len());
        assert!(
            body[..end].contains("crate::chrome::deliver("),
            "the mouse layer's `{arm}` arm does not hand the event to the tree.\n\
             A press with no release is not a click: `Click` and `RightClick` are synthesised from \
             the pair, so nothing that depends on a click happens at all — a widget's declared \
             context menu never opens, and a widget that captured the press never learns the \
             gesture ended.\n\
             This is not caught by any behaviour test: they dispatch both halves themselves.",
        );
    }
}

/// **A right-click must reach the panes, not only the chrome.** ⚠️ Ran red against its own bug.
///
/// A pane's widgets live in a tree of their own until the pane joins the one tree
/// tree, and only *left* presses were ever handed to them. So a right-click never
/// reached a pane at all — and the moment a pane started declaring its own menu, right-clicking one
/// showed nothing whatsoever, while `prefix+>` still worked because the keyboard path resolves the
/// pane a different way.
///
/// A lint rather than a behaviour test, like its neighbours: what it guards is *absence*, and the
/// suite was fully green with right-click menus completely dead.
#[test]
fn a_right_click_is_handed_to_the_panes_as_well_as_the_chrome() {
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mouse.rs"))
        .expect("read the mouse layer");

    for arm in [
        "(Btn::Right, Kind::Pressed) =>",
        "(Btn::Right, Kind::Released) =>",
    ] {
        let at = src.find(arm).unwrap_or_else(|| {
            panic!("the mouse layer no longer has a `{arm}` arm — move this guard with it")
        });
        let body = &src[at..];
        let end = body[arm.len()..]
            .find("\n        (")
            .map(|i| i + arm.len())
            .unwrap_or(body.len());
        assert!(
            body[..end].contains("deliver_to_panes("),
            "the mouse layer's `{arm}` arm hands the event to the chrome tree but not to the \
             panes.\n\
             A pane is dispatched separately until it joins the one tree, so a right-click that \
             goes only to the chrome never reaches a pane — and the menu a pane declares about \
             itself is then unreachable, with nothing failing anywhere.",
        );
    }
}

/// **A divider resize must end at the same level its press started it.**
///
/// The press starts the drag in the event loop (`mouse::resize::on_press`, before the general mouse
/// path). The release used to end it two layers down, inside `mouse::on_mouse_input` — which sits
/// *behind* an early return: if a pane's viewport scrollbar claimed the release (it answers one
/// whenever it holds a thumb grab), the event loop returned and the resize was never told.
///
/// `state.mouse.resize` then stayed `Some`, and every later cursor move took the resize branch in
/// `mouse::on_cursor_moved` **with no button held** — so the pane went on resizing itself, with the
/// mouse just moving, until it was gone. Same family as the guards above: a gesture that outlives
/// the release that ends it (F004/P084/T409).
///
/// A lint, because what it guards is *ordering*, and the failure is a state that persists rather
/// than an event that is wrong.
#[test]
fn the_divider_resize_ends_before_anything_can_swallow_the_release() {
    let src = std::fs::read_to_string(events_rs()).expect("read the event loop");
    let body = branch_body(&src, "WindowEvent::MouseInput")
        .expect("the button branch is still a `WindowEvent::MouseInput` arm");

    let ends = body
        .find("resize::on_release")
        .expect(
            "the button branch no longer ends the divider resize. It must: the press starts the \
             drag here, so the release has to end it here too, or the drag outlives the button.",
        );
    // Searched FORWARD from where the resize ends, not from the top of the branch: the viewport
    // widgets are given the press too, further up, and that call cannot swallow a release. What
    // has to hold is that the swallowing call comes after the resize has been told.
    assert!(
        body[ends..].contains("crate::chrome::deliver_to_pane_viewports("),
        "the divider resize is ended AFTER the branch that can return early and swallow the \
         release.\nA resize that is never told the button came up keeps resizing on every cursor \
         move, with nothing held down, until the pane is gone.",
    );
}

/// **A key release reaches the tree, or `on_key_up` is a builder nothing can fire.**
///
/// The window loop returned at `event.state != ElementState::Pressed`, so `Event::Key { pressed:
/// false }` did not exist in this app. A widget's release handler was therefore dead — the exposé's
/// `x`/`X`/`d` did nothing on screen while a headless test that dispatched both halves passed
/// (Antonio, driving, 2026-08-11).
///
/// Exactly the shape of the pointer guards above: the funnel delivering only one half of a gesture,
/// invisible to every behaviour test, because a test hands the tree both halves itself.
#[test]
fn the_key_funnel_delivers_releases_and_not_only_presses() {
    let src = std::fs::read_to_string(events_rs()).expect("read the event loop");
    let body = branch_body(&src, "WindowEvent::KeyboardInput")
        .expect("the key branch is still a `WindowEvent::KeyboardInput` arm");

    assert!(
        body.contains("deliver_press"),
        "the key branch no longer delivers presses — if that moved, move this guard with it",
    );
    assert!(
        body.contains("deliver_release"),
        "the key branch delivers a key PRESS and never a RELEASE.\n\
         `ComponentExt::on_key_up` then exists but can never fire, so a widget that declares one \
         is silently dead in the real app.\n\
         This is not caught by any behaviour test: they dispatch both halves themselves.",
    );
}

/// **The move that drives a drag must not be gated on a drag being in flight.**
///
/// A drag is not a state the host maintains alongside the pointer — it is a thing the framework
/// runs *out of* pointer moves. Each move that reaches the tree is what moves the picture under the
/// cursor, re-runs `DragEnter`/`DragOver` to mark the target and pick before/after/onto, and
/// re-reads the modifiers that decide move versus swap. Withhold the move and the gesture freezes:
/// the one move that crosses the threshold gets through — the gate was still false when it was
/// tested — and nothing after it does.
///
/// It shipped exactly that way. The guard had asked whether the app's own drag machine was running,
/// and that machine had stopped being set the moment rows took over their own dragging, so the
/// answer was always "no" and the dispatch always ran. Re-pointing the same question at the tree
/// made it truthful, and truthful is what broke it: the preview stuck where it was picked up and
/// the target marks stopped moving, with the whole suite green.
///
/// Hover was the reason the gate was written, and it is not a reason any more: the framework lights
/// nothing under a drag (`a_drag_in_flight_clears_hover`). A lint, like its neighbours: what it
/// guards is *absence*, and the absence is of an event nobody sent.
#[test]
fn the_move_that_drives_a_drag_is_not_withheld_while_dragging() {
    let src = std::fs::read_to_string(events_rs()).expect("read the event loop");
    let body = branch_body(&src, "WindowEvent::CursorMoved").expect("the move branch");

    let call = body
        .find("crate::chrome::deliver(")
        .expect("the move branch no longer feeds the window tree — move this guard with it");

    // Everything the call is nested inside: the last `if` opened before it still decides whether it
    // runs, so that is the condition to read.
    let guard = body[..call]
        .rfind("if ")
        .map(|i| &body[i..call])
        .unwrap_or("");

    assert!(
        !guard.contains("drag_in_flight"),
        "the move into the window tree is gated on a drag being in flight.\n\
         That is backwards: the move is what DRIVES the drag. Withholding it freezes the picture \
         under the cursor where it was picked up, stops `DragOver` so the insertion line and the \
         swap outline never move, and stops the modifiers being re-read so Shift no longer switches \
         move to swap.\n\
         Hover is not a reason to hold it back — the framework lights nothing under a drag \
         (`a_drag_in_flight_clears_hover`). Gate the OTHER trees if they need it; this one must \
         always be fed.",
    );
}

/// **The tree is told what is held before anyone is asked what it means.**
///
/// The framework records the modifier state from the `ModifiersChanged` broadcast, and the app's
/// own reaction to that same event asks it what a drag now means (`DropAction::held`). Reacting
/// before announcing asks the question before the answer exists, so the answer is the *previous*
/// one: a drag's move-versus-swap trails one event behind and flips when the key comes up instead
/// of when it goes down.
///
/// A lint, like its neighbours, and for the same reason — what it guards is an *ordering*, and the
/// symptom is a value that is merely stale rather than an event that is missing.
#[test]
fn the_tree_learns_the_modifiers_before_the_app_reacts_to_them() {
    let src = std::fs::read_to_string(events_rs()).expect("read the event loop");
    let body = branch_body(&src, "WindowEvent::ModifiersChanged").expect("the modifiers branch");

    // The *call*, not the event name: `WindowEvent::ModifiersChanged` — the branch head itself —
    // ends with the string `Event::ModifiersChanged`, so matching on that finds position zero and
    // the check passes whatever the order is. It did, until the guard was run against the bug it
    // was written for.
    let announced = body
        .find("heca_grid_ui::dispatch(")
        .expect("the modifiers branch no longer announces to the tree — move this guard with it");
    let reacted = body
        .find("on_modifiers_changed")
        .expect("the modifiers branch no longer reacts — move this guard with it");

    assert!(
        announced < reacted,
        "the app reacts to a modifier change BEFORE telling the tree about it.\n\
         Everything that asks what a modifier means now reads the framework's record of what is \
         held, so asking before announcing returns the state from the previous event: a drag \
         switches between move and swap one keystroke late, on the release rather than the press.",
    );
}

/// **One door into the window tree, and no growing a second set beside it.**
///
/// There were eight `chrome_dispatch_*` functions, one per event kind, each rebuilding from a
/// position what the event loop had just been told — and a caller then chose which to call.
/// Choosing is how a kind goes missing: the chrome tree got the press and the move for years and
/// neither the release nor the wheel, so a scrollbar grabbed in the sidebar could never be let go
/// and the wheel did nothing there at all. Every guard above this one exists because of a kind
/// somebody did not think to pass on.
///
/// So the kinds are gone and there is one `deliver`, taking whatever the loop built. This fails if
/// a per-kind wrapper comes back — that is the shape to reject, before it has callers.
#[test]
fn the_window_tree_has_one_door_and_not_a_function_per_kind() {
    let src = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/chrome/dispatch.rs"),
    )
    .expect("read the chrome dispatch module");

    assert!(
        src.contains("pub(crate) fn deliver("),
        "`deliver` is gone — if the one door moved, move this guard with it rather than deleting it",
    );

    // Every tree the host mounts: the window root, the pane headers, the pane viewports. Each gets
    // the event the loop built; none gets a function per kind. The header and viewport sets were
    // the last per-kind survivors, and each spelled `PointerButton::Left` into every call — so a
    // right-click could not reach a pane header at all.
    let per_kind: Vec<_> = ["press", "release", "move", "wheel", "cancelled"]
        .iter()
        .flat_map(|kind| {
            [
                "chrome_dispatch_",
                "dispatch_pane_header_",
                "dispatch_pane_viewport_",
            ]
            .iter()
            .map(move |prefix| format!("fn {prefix}{kind}("))
        })
        .filter(|sig| src.contains(sig.as_str()))
        .collect();
    assert!(
        per_kind.is_empty(),
        "a per-kind dispatch function is back in the chrome module: {per_kind:?}.\n\
         The window tree takes the event the loop already built. A function per kind puts the \
         caller in charge of which kinds get through, and the caller is where kinds go missing — \
         silently, because a widget that never receives one lays out, paints and hit-tests \
         perfectly while being dead.",
    );
}

/// **The loop wakes for what the widgets are waiting for.**
///
/// Some behaviour is due at a *time* rather than on an event: a tooltip revealing once the pointer
/// has rested, a caret blinking. A still pointer produces no events, so unless the loop asks, the
/// frame that would draw it never happens — and the reveal waits for whatever the user does next.
///
/// The library has always had the answer (`Component::next_redraw`, folded down a whole tree and
/// guarded by `a_pending_tooltip_asks_the_host_to_wake_for_it`). **The host simply never asked**, so
/// a tooltip stayed hidden under a resting pointer and appeared the moment the mouse moved by a
/// pixel — the nudge being what produced the frame (Antonio, driving, 2026-09-04). Only the showcase
/// read it, which is why the widget's own tests were green throughout.
///
/// A lint, like the rest of this file: what it guards against is *absence*, and nothing fails when
/// a question is not asked.
#[test]
fn the_loop_wakes_for_what_the_widgets_are_waiting_for() {
    let lifecycle = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/lifecycle.rs");
    let src = std::fs::read_to_string(&lifecycle).expect("lifecycle.rs is readable");
    assert!(
        src.contains("next_redraw_across_trees"),
        "the frame loop decides when to wake without asking the widgets what they are waiting for; \
         a tooltip under a resting pointer will never be drawn ({})",
        lifecycle.display()
    );
    assert!(
        src.contains("ControlFlow::WaitUntil"),
        "…and it must schedule that wake, not merely compute it"
    );
    // **Scheduling is only half of it, and the half that is easy to think is the whole.** Arriving
    // at the deadline, every reason-to-draw is false — the widget no longer reports a pending wake,
    // because it is due *now* — so the loop wakes and goes straight back to sleep. The first attempt
    // at this fix scheduled the wake correctly and changed nothing on screen for exactly that
    // reason.
    let needs_frame = src
        .split_once("let needs_frame")
        .and_then(|(_, rest)| rest.split_once(';'))
        .map(|(decl, _)| decl)
        .expect("the frame loop decides with a `needs_frame`");
    assert!(
        needs_frame.contains("widget_due"),
        "reaching a widget's wake is not itself a reason to draw, so the loop wakes for it and \
         then does nothing"
    );
}
