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
            "chrome_dispatch_release",
            "a gesture started in the sidebar has to be able to end — a thumb grabbed there stays \
             welded to the cursor otherwise",
        ),
        (
            "WindowEvent::MouseInput",
            "dispatch_pane_header_release",
            "the pane headers were the last seam missing a kind — the first widget mounted there \
             with a gesture would have been broken on arrival",
        ),
        (
            "WindowEvent::MouseWheel",
            "dispatch_pane_header_wheel",
            "nothing in a header scrolls yet, and \"nothing needs it yet\" is the reasoning that \
             produced every other missing kind",
        ),
        (
            "WindowEvent::MouseWheel",
            "chrome_dispatch_wheel",
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
    let src = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mouse.rs"),
    )
    .expect("read the mouse layer");

    assert!(
        src.contains("chrome_dispatch_button_press"),
        "the mouse layer no longer presses into the chrome tree — if that moved, move this guard \
         with it rather than deleting it",
    );
    assert!(
        src.contains("chrome_dispatch_button_release"),
        "the mouse layer dispatches a button PRESS to the chrome tree and never a RELEASE.\n\
         A press with no release is not a click: `Click` and `RightClick` are synthesised from the \
         pair, so nothing that depends on a click happens at all — a widget's declared context \
         menu never opens, and a widget that captured the press never learns the gesture ended.\n\
         This is not caught by any behaviour test: they dispatch both halves themselves.",
    );
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
    let swallows = body
        .find("dispatch_pane_viewport_release")
        .expect("the viewport release moved — move this guard with it rather than deleting it");

    assert!(
        ends < swallows,
        "the divider resize is ended AFTER a branch that can return early and swallow the \
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
