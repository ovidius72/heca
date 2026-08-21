//! The `Overlay` widget's own tests — what it shows, what it holds, and what it lets through.
use super::*;
use crate::builders::Parent;
use crate::event::PointerButton;
use crate::widgets::{Flex, Label};

/// An open modal, **laid out** — as a host has it before any input reaches it. A layer occupies
/// what it draws, and what it draws is decided by the layout pass: the scrim fills the viewport
/// it was given. (It used to answer with a `hit_bounds` rect large enough to cover any point at
/// all, even before it had been laid out or drawn anywhere.)
fn open_overlay() -> Overlay {
    let mut o = Overlay::new()
        .panel(Flex::column().child(Label::new("hi")))
        .opened(true);
    crate::layout::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));
    o
}

/// **A dismissed surface is still on screen, and holds nothing.**
///
/// The whole point of the animation living here: `set_open(false)` begins the exit rather than
/// taking the surface away, and the two halves part company — it keeps *painting* while it
/// leaves, and stops *taking input* the moment it is dismissed. Counting a dissolving map as
/// coverage refused every act on the pane it exists to let you choose (F003/P082).
#[test]
fn a_dismissed_overlay_paints_until_its_exit_has_played_out_and_holds_no_input() {
    let mut o = open_overlay();
    o = o.animation(Animation::Fade);
    o.open(); // the arrival is not our subject here
    while o.tick(0.05) {}

    o.hide();
    assert!(o.presence().is_some_and(|p| p.is_leaving()), "the exit begins at once");
    assert!(!o.focusable(), "…and it stops holding the keyboard");
    assert!(o.hit_bounds().is_none(), "…and every point falls through it");
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
    assert!(painted_frames >= 3, "it played, rather than cutting: {painted_frames} frames");
    assert!(!o.presence().is_some_and(|p| p.is_leaving()), "and then it is gone");
}

/// A surface that declared no animation goes the instant it is dismissed — no special case
/// anywhere for "this one does not animate".
#[test]
fn an_overlay_with_no_animation_is_gone_the_moment_it_is_closed() {
    let mut o = open_overlay();
    o.hide();
    assert!(!o.presence().is_some_and(|p| p.is_leaving()), "nothing to wait for");
    let mut scene = crate::scene::Scene::new();
    let theme = crate::theme::Theme::default();
    let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(Size::new(800.0, 600.0));
    o.paint(&mut cx);
    assert!(scene.is_empty(), "and nothing left to draw");
}

/// **Saying "open" to an open surface is not an arrival.** A host re-states the truth on every
/// rebuild — the exposé is re-registered whenever the session changes underneath it — and
/// replaying the arrival there zoomed the map open on every keystroke (F003/P082, 2026-08-11).
#[test]
fn re_stating_open_does_not_replay_the_arrival() {
    let mut o = open_overlay().animation(Animation::Zoom.from(0.5));
    o.open();
    o.tick(0.05);
    o.tick(0.05);
    let mid = o.presence().expect("an overlay has a presence").frame().scale;
    o.open();
    assert_eq!(
        o.presence().expect("an overlay has a presence").frame().scale,
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
        .opened(true);
    o.hide();
    assert!(o.presence().is_some_and(|p| p.is_leaving()), "the named animation is playing");

    // Untrusted input stays total: an unknown name leaves the surface as it was.
    let mut unknown = Overlay::new()
        .panel(Flex::column())
        .set_prop("animation", &PropInput::Text("bounce".into()))
        .opened(true);
    unknown.hide();
    assert!(!unknown.presence().is_some_and(|p| p.is_leaving()), "a cut, not a panic");
}

#[test]
fn closed_overlay_is_inert() {
    let mut o = Overlay::new().panel(Flex::column());
    assert!(!o.overlay_active());
    assert!(!o.focusable());
    assert!(!o.overlay_occludes(Point::new(1.0, 1.0)));
    assert_eq!(
        crate::component::dispatch(&mut o, &Event::pointer_pressed(Point::new(1.0, 1.0), PointerButton::Left)),
        Handled::No
    );
}

#[test]
fn blocking_overlay_occludes_everywhere_and_swallows_outside_input() {
    let mut o = open_overlay();
    assert!(o.overlay_occludes(Point::new(-500.0, -500.0)), "scrim owns every point");
    // A press on the scrim: inside the viewport the layer covers, outside the panel it holds.
    let outside = Point::new(4.0, 4.0);
    assert!(!o.panel_bounds().contains(outside), "the corner is scrim, not panel");
    assert_eq!(
        crate::component::dispatch(&mut o, &Event::pointer_pressed(outside, PointerButton::Left)),
        Handled::Yes,
        "modal swallows the outside press"
    );
    assert_eq!(crate::component::dispatch(&mut o, &Event::wheel(outside, 0.0, 1.0)), Handled::Yes);
}

#[test]
fn non_blocking_overlay_occludes_only_its_panel_and_lets_outside_fall_through() {
    use std::cell::Cell;
    use std::rc::Rc;
    let dismissed = Rc::new(Cell::new(false));
    let d = dismissed.clone();
    let mut o = Overlay::new()
        .blocking(false)
        .panel(Flex::column().child(Label::new("hi")))
        .opened(true)
        .on_outside_click(move || d.set(true));
    // Give the panel real bounds (as layout would).
    o.base.children[0].base_mut().bounds =
        Rectangle::new(Point::new(100.0, 100.0), Size::new(50.0, 20.0));
    assert!(o.overlay_occludes(Point::new(110.0, 110.0)), "panel point occludes");
    assert!(!o.overlay_occludes(Point::new(0.0, 0.0)), "outside point does not");
    assert_eq!(
        crate::component::dispatch(&mut o, &Event::pointer_pressed(Point::new(0.0, 0.0), PointerButton::Left)),
        Handled::No,
        "light layer lets the outside press fall through"
    );
    assert!(dismissed.get(), "outside press fired the dismissal hook");
}

// ── Panel sizing (.panel_size) ──
/// The size lands on the panel child whichever order the builders are called
/// in — `panel()` clears and re-pushes the child, so the overlay has to keep
/// the size and re-apply it.
#[test]
fn panel_size_applies_regardless_of_builder_order() {
    let want_w = Length::Pct(0.6);
    let want_h = Length::Px(420.0);

    // size first, then panel
    let a = Overlay::new()
        .panel_size(want_w, want_h)
        .panel(Flex::column().child(Label::new("body")));
    let style = &a.base.children[0].base().style.layout;
    assert_eq!(style.width, want_w);
    assert_eq!(style.height, want_h);

    // panel first, then size
    let b = Overlay::new()
        .panel(Flex::column().child(Label::new("body")))
        .panel_size(want_w, want_h);
    let style = &b.base.children[0].base().style.layout;
    assert_eq!(style.width, want_w);
    assert_eq!(style.height, want_h);
}

/// **Regression guard.** A panel must never be larger than the viewport it is
/// centered in: an oversized one gets cut off by the window edge on *both*
/// sides (a dialog bigger than the window — user-reported). The cap is
/// unconditional, so it also protects a content-sized panel, not just a
/// `panel_size`d one.
#[test]
fn panel_never_exceeds_the_viewport() {
    use crate::widgets::ScrollRegion;
    // Ask for a panel far bigger than the viewport we lay out in.
    let mut o = Overlay::new()
        .panel_size(Length::Px(4000.0), Length::Px(3000.0))
        .panel(ScrollRegion::new().child(Label::new("tall")))
        .opened(true);
    crate::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));
    let panel = o.panel_bounds();
    assert!(panel.size.w <= 800.0, "panel width capped, got {}", panel.size.w);
    assert!(panel.size.h <= 600.0, "panel height capped, got {}", panel.size.h);
}

/// Unset (the default) leaves the panel hugging its own content — the sizing
/// API must not silently impose a size on every existing overlay.
#[test]
fn panel_size_is_opt_in() {
    let o = Overlay::new().panel(Flex::column().child(Label::new("body")));
    let style = &o.base.children[0].base().style.layout;
    assert_eq!(style.width, Length::Auto, "untouched by default");
    assert_eq!(style.height, Length::Auto);
}

/// End-to-end: an anchored overlay bakes the placement into the panel child's
/// bounds on layout (so paint/hit-testing follow), and it is idempotent.
#[test]
fn anchored_overlay_places_panel_child_on_layout() {
    let anchor = Rectangle::new(Point::new(40.0, 100.0), Size::new(120.0, 30.0));
    let mut o = Overlay::new()
        .blocking(false)
        .anchored(anchor)
        .panel(Flex::column().child(Label::new("hi")))
        .opened(true);
    // Simulate a layout pass: taffy placed the panel somewhere with a real size.
    o.base.children[0].base_mut().bounds =
        Rectangle::new(Point::new(300.0, 300.0), Size::new(120.0, 80.0));
    o.viewport.set(Size::new(800.0, 600.0));
    o.on_layout();
    let placed = o.panel_bounds();
    assert_eq!(placed.loc, Point::new(40.0, 134.0), "panel anchored below trigger");
    // Idempotent: a second on_layout must not compound the offset.
    o.on_layout();
    assert_eq!(o.panel_bounds().loc, placed.loc, "re-placing is idempotent");
}
