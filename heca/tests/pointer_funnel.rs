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
const FUNNEL: &str = "dispatch_modal_pointer";

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
