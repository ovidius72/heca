use super::*;

#[test]
fn a_signal_driven_animated_surface_can_open_and_close_repeatedly() {
    let theme = crate::Theme::default();
    let viewport = Size::new(800.0, 600.0);
    let mut overlay = Overlay::new().animation(Animation::ZoomFade)
        .panel(Flex::column().child(Label::new("Repeatable surface")));
    let handle = overlay.handle();
    for cycle in 0..3 {
        handle.show();
        for _ in 0..60 { overlay.tick(1.0 / 60.0); }
        crate::LayoutEngine::new().compute(&mut overlay, viewport);
        let mut scene = Scene::new();
        overlay.paint(&mut PaintCx::new(&mut scene, &theme).with_viewport(viewport));
        assert!(!scene.is_empty(), "opening cycle {cycle} must paint the surface");
        assert!(overlay.is_shown(), "opening cycle {cycle} must participate in input routing");

        handle.close();
        for _ in 0..60 { overlay.tick(1.0 / 60.0); }
        let mut scene = Scene::new();
        overlay.paint(&mut PaintCx::new(&mut scene, &theme).with_viewport(viewport));
        assert!(scene.is_empty(), "closing cycle {cycle} must finish its exit");
        assert!(!overlay.is_shown());
    }
}

/// **A dismissed surface is still on screen, and holds nothing.**
///
/// The whole point of the animation living here: `set_open(false)` begins the exit rather than
/// taking the surface away, and the two halves part company — it keeps *painting* while it
/// leaves, and stops *taking input* the moment it is dismissed. Counting a dissolving map as
/// coverage refused every act on the pane it exists to let you choose.
#[test]
fn a_dismissed_overlay_paints_until_its_exit_has_played_out_and_holds_no_input() {
    let mut o = open_overlay();
    o = o.animation(Animation::Fade);
    o.show(); // the arrival is not our subject here
    while o.tick(0.05) {}

    o.close();
    assert!(
        o.presence().is_some_and(|p| p.is_leaving()),
        "the exit begins at once"
    );
    assert!(!o.focusable(), "…and it stops holding the keyboard");
    assert!(
        o.hit_bounds().is_none(),
        "…and every point falls through it"
    );
    assert!(!o.overlay_occludes(Point::new(1.0, 1.0)));

    let mut painted_frames = 0;
    while o.tick(0.05) {
        painted_frames += 1;
        let mut scene = crate::scene::Scene::new();
        let theme = crate::theme::Theme::default();
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(Size::new(800.0, 600.0));
        o.paint(&mut cx);
        assert!(!scene.is_empty(), "a leaving surface is still drawn");
    }
    assert!(
        painted_frames >= 3,
        "it played, rather than cutting: {painted_frames} frames"
    );
    assert!(
        !o.presence().is_some_and(|p| p.is_leaving()),
        "and then it is gone"
    );
}

/// A surface that declared no animation goes the instant it is dismissed — no special case
/// anywhere for "this one does not animate".
#[test]
fn an_overlay_with_no_animation_is_gone_the_moment_it_is_closed() {
    let mut o = open_overlay();
    o.close();
    assert!(
        !o.presence().is_some_and(|p| p.is_leaving()),
        "nothing to wait for"
    );
    let mut scene = crate::scene::Scene::new();
    let theme = crate::theme::Theme::default();
    let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(Size::new(800.0, 600.0));
    o.paint(&mut cx);
    assert!(scene.is_empty(), "and nothing left to draw");
}

/// **Saying "open" to an open surface is not an arrival.** A host re-states the truth on every
/// rebuild — the exposé is re-registered whenever the session changes underneath it — and
/// replaying the arrival there zoomed the map open on every keystroke.
#[test]
fn re_stating_open_does_not_replay_the_arrival() {
    let mut o = open_overlay().animation(Animation::Zoom.from(0.5));
    o.show();
    o.tick(0.05);
    o.tick(0.05);
    let mid = o
        .presence()
        .expect("an overlay has a presence")
        .frame()
        .scale;
    o.show();
    assert_eq!(
        o.presence()
            .expect("an overlay has a presence")
            .frame()
            .scale,
        mid,
        "the arrival carried on from where it was",
    );
}

/// The declarative half is the same door: a described surface names a built-in and gets the
/// same gesture native code gets.
#[test]
fn a_described_overlay_names_its_animation() {
    use crate::{PropInput, SetProp};
    let mut o = Overlay::new()
        .panel(Flex::column())
        .set_prop("animation", &PropInput::Text("fade".into()))
        .default_open(true);
    o.close();
    assert!(
        o.presence().is_some_and(|p| p.is_leaving()),
        "the named animation is playing"
    );

    // Untrusted input stays total: an unknown name leaves the surface as it was.
    let mut unknown = Overlay::new()
        .panel(Flex::column())
        .set_prop("animation", &PropInput::Text("bounce".into()))
        .default_open(true);
    unknown.close();
    assert!(
        !unknown.presence().is_some_and(|p| p.is_leaving()),
        "a cut, not a panic"
    );
}

/// **A handle shows and closes it from anywhere** — the door a click handler needs.
///
/// `show`/`hide` on the widget take `&mut self`, so a closure living inside a button can never hold
/// one while the surface sits beside it. Every caller reached for the raw signal instead. This is
/// that signal with the two verbs on it, and it is `Copy`, so it goes into any closure.
#[test]
fn a_handle_shows_and_closes_a_surface_from_anywhere() {
    let o = Overlay::new().panel(Flex::column().child(Label::new("body")));
    let h = o.handle();
    assert!(!h.is_open(), "built, not shown");

    // Into a closure, by value — the case a `&mut` method cannot serve.
    let opener = move || h.show();
    opener();
    assert!(
        h.is_open(),
        "shown from a closure that owns nothing but the handle"
    );

    h.close();
    assert!(!h.is_open());
    h.toggle();
    assert!(h.is_open(), "and toggle is the same door");
}

/// **A surface follows a signal you already hold** — the door state binding needs.
///
/// It owned a signal and lent it out, so a caller could drive *its* state but never hand it
/// *theirs*. This is the other direction: the surface is up exactly when your signal is true.
#[test]
fn a_surface_follows_a_signal_of_your_own() {
    use crate::reactive::{SignalUpdate, signal};

    let editing = signal(false);
    let o = Overlay::new()
        .panel(Flex::column().child(Label::new("body")))
        .open_when(editing);

    assert!(!o.handle().is_open());
    editing.set(true);
    assert!(
        o.handle().is_open(),
        "your signal is the surface's state, not a copy of it"
    );
    editing.set(false);
    assert!(!o.handle().is_open());
}

/// **A widget that declared no gesture is gone the moment it is closed**, and one that declared a
/// gesture stays until it has played out — the same call either way.
///
/// The point is that a caller never has to know which kind it is holding. Guarded because the
/// tempting shape is a caller asking "does this animate?" before deciding what to do, which is a
/// rule at the call site and therefore the bug.
#[test]
fn closing_cuts_or_plays_out_without_the_caller_knowing_which() {
    use crate::component::Component as _;
    use crate::widgets::Button;

    let mut cut = Button::new("Delete");
    cut.show();
    cut.close();
    assert!(
        !cut.presence().is_some_and(|p| p.is_leaving()),
        "nothing declared, so it is simply gone",
    );

    let mut plays = Overlay::new()
        .panel(Flex::column().child(Label::new("body")))
        .animation(crate::animation::Animation::Fade);
    plays.show();
    plays.close();
    assert!(
        plays.presence().is_some_and(|p| p.is_leaving()),
        "a declared gesture is still playing, so it is still on screen and still inert",
    );
}
