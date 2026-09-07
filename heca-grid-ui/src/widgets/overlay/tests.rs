//! The `Overlay` widget's own tests — what it shows, what it holds, and what it lets through.
use super::*;
use crate::builders::Parent;
use crate::event::PointerButton;
use crate::scene::{DrawCommand, HostCmd, HostDraw, Scene};
use crate::widgets::{Flex, Label};

/// An open modal, **laid out** — as a host has it before any input reaches it. A layer occupies
/// what it draws, and what it draws is decided by the layout pass: the scrim fills the viewport
/// it was given. (It used to answer with a `hit_bounds` rect large enough to cover any point at
/// all, even before it had been laid out or drawn anywhere.)
fn open_overlay() -> Overlay {
    let mut o = Overlay::new()
        .panel(Flex::column().child(Label::new("hi")))
        .default_open(true);
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
/// replaying the arrival there zoomed the map open on every keystroke (F003/P082, 2026-08-11).
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

#[test]
fn closed_overlay_is_inert() {
    let mut o = Overlay::new().panel(Flex::column());
    assert!(!o.overlay_active());
    assert!(!o.focusable());
    assert!(!o.overlay_occludes(Point::new(1.0, 1.0)));
    assert_eq!(
        crate::component::dispatch(
            &mut o,
            &Event::pointer_pressed(Point::new(1.0, 1.0), PointerButton::Left)
        ),
        Handled::No
    );
}

#[test]
fn blocking_overlay_occludes_everywhere_and_swallows_outside_input() {
    let mut o = open_overlay();
    assert!(
        o.overlay_occludes(Point::new(-500.0, -500.0)),
        "scrim owns every point"
    );
    // A press on the scrim: inside the viewport the layer covers, outside the panel it holds.
    let outside = Point::new(4.0, 4.0);
    assert!(
        !o.panel_bounds().contains(outside),
        "the corner is scrim, not panel"
    );
    assert_eq!(
        crate::component::dispatch(
            &mut o,
            &Event::pointer_pressed(outside, PointerButton::Left)
        ),
        Handled::Yes,
        "modal swallows the outside press"
    );
    assert_eq!(
        crate::component::dispatch(&mut o, &Event::wheel(outside, 0.0, 1.0)),
        Handled::Yes
    );
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
        .default_open(true)
        .on_outside_click(move || d.set(true));
    // Give the panel real bounds (as layout would).
    o.base.children[0].base_mut().bounds =
        Rectangle::new(Point::new(100.0, 100.0), Size::new(50.0, 20.0));
    assert!(
        o.overlay_occludes(Point::new(110.0, 110.0)),
        "panel point occludes"
    );
    assert!(
        !o.overlay_occludes(Point::new(0.0, 0.0)),
        "outside point does not"
    );
    assert_eq!(
        crate::component::dispatch(
            &mut o,
            &Event::pointer_pressed(Point::new(0.0, 0.0), PointerButton::Left)
        ),
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
        .default_open(true);
    crate::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));
    let panel = o.panel_bounds();
    assert!(
        panel.size.w <= 800.0,
        "panel width capped, got {}",
        panel.size.w
    );
    assert!(
        panel.size.h <= 600.0,
        "panel height capped, got {}",
        panel.size.h
    );
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
        .default_open(true);
    // Simulate a layout pass: taffy placed the panel somewhere with a real size.
    o.base.children[0].base_mut().bounds =
        Rectangle::new(Point::new(300.0, 300.0), Size::new(120.0, 80.0));
    o.viewport.set(Size::new(800.0, 600.0));
    o.on_layout();
    let placed = o.panel_bounds();
    assert_eq!(
        placed.loc,
        Point::new(40.0, 134.0),
        "panel anchored below trigger"
    );
    // Idempotent: a second on_layout must not compound the offset.
    o.on_layout();
    assert_eq!(o.panel_bounds().loc, placed.loc, "re-placing is idempotent");
}

/// Paint `o` into a fresh scene and report every host request it recorded, split by band.
///
/// The split is the point: a backdrop must land in the **base** commands, never the deferred
/// overlay band, because the host performs the requests between flushing the two.
fn host_draws(o: &Overlay) -> (Vec<HostCmd>, Vec<HostCmd>) {
    let theme = crate::theme::Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(Size::new(800.0, 600.0));
        o.paint(&mut cx);
    }
    let host = |s: &Scene| -> Vec<HostCmd> {
        s.iter()
            .filter_map(|c| match c {
                DrawCommand::Host(h) => Some(*h),
                _ => None,
            })
            .collect()
    };
    (host(&scene.base_layer()), host(&scene.overlay_layer()))
}

/// **A frosted surface blurs exactly what it occludes, and asks for it in the base band.**
///
/// The band is the whole mechanism: everything an overlay *draws* is deferred so it composites
/// above its siblings, but a backdrop is the opposite — it must be performed after what is beneath
/// it and before the surface itself. The host flushes base, performs the requests, then flushes the
/// overlay bands, so a request recorded in the wrong band blurs the wrong frame.
#[test]
fn a_frosted_overlay_records_its_blur_in_the_base_band_over_what_it_occludes() {
    let mut o = Overlay::new()
        .frosted(true)
        .panel(Flex::column().child(Label::new("hi")))
        .default_open(true);
    crate::layout::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));

    let (base, overlay) = host_draws(&o);
    let radius = crate::theme::Theme::default().colors.overlay_frost_radius;
    assert_eq!(
        base,
        vec![HostCmd {
            draw: HostDraw::Backdrop { radius },
            rect: Rectangle::new(Point::new(0.0, 0.0), Size::new(800.0, 600.0)),
            alpha: 1.0,
        }],
        "blocking, so it blurs the viewport it covers — and in the base band",
    );
    assert!(
        overlay.is_empty(),
        "a backdrop is never deferred with what the surface draws"
    );
}

/// **A surface that did not ask for a frost summons no blur pass.** The blur is a GPU pass over
/// the whole frame; a dialog that never wanted one must not be paying for it.
#[test]
fn a_plain_overlay_records_no_backdrop() {
    let o = open_overlay();
    assert!(host_draws(&o).0.is_empty());
}

/// **A non-blocking frosted surface blurs its panel, not the screen.** It occludes only its panel,
/// and the frost reads the same reach the input policy does — so the two can never disagree about
/// how far this surface goes.
#[test]
fn a_non_blocking_frost_reaches_only_as_far_as_the_panel() {
    let mut o = Overlay::new()
        .blocking(false)
        .frosted(true)
        .panel(Flex::column().child(Label::new("hi")))
        .default_open(true);
    crate::layout::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));

    let (base, _) = host_draws(&o);
    assert_eq!(base.len(), 1);
    assert_eq!(
        base[0].rect,
        o.panel_bounds(),
        "the panel's reach, not the viewport's"
    );
}

/// **The backdrop dissolves with the surface that asked for it.**
///
/// Left at full strength it holds the whole session out of focus for the length of the fade and
/// then snaps sharp in one frame — the exact pop the fade exists to remove. It takes the
/// animation's *opacity* and nothing else: a blurred region that also zoomed would be blurring
/// somewhere the surface is not.
#[test]
fn a_frosted_overlays_backdrop_fades_with_it() {
    let mut o = Overlay::new()
        .frosted(true)
        .animation(Animation::ZoomFade)
        .panel(Flex::column().child(Label::new("hi")))
        .default_open(true);
    crate::layout::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));
    o.show();
    while o.tick(0.05) {}

    o.close();
    o.tick(0.05);
    let (base, _) = host_draws(&o);
    assert_eq!(base.len(), 1, "still asking while it leaves");
    assert!(
        base[0].alpha < 1.0,
        "and asking more faintly: {}",
        base[0].alpha
    );
    assert_eq!(
        base[0].rect,
        Rectangle::new(Point::new(0.0, 0.0), Size::new(800.0, 600.0)),
        "the zoom moves the panel, never the region being blurred",
    );
}

// ── The keyboard an overlay holds (F003/P097/T502) ──────────────────────────────────────────
//
// An open overlay binds its own focus to being open, so the keys have always arrived here — and
// were dropped, because this widget answered pointer events and nothing else. Every surface built
// on top wrote its own dismissal and its own traversal instead (`Dialog`, `ContextMenu`,
// `CommandPalette`: three copies), and a surface **composed** rather than built — which is what a
// plugin writes — had neither and could not be used from the keyboard at all.

/// **The dismiss key closes a surface that said what dismissal means.**
#[test]
fn the_dismiss_key_runs_the_dismissal_a_surface_declared() {
    use crate::event::WidgetIntent;
    use std::cell::Cell;
    use std::rc::Rc;

    let closed = Rc::new(Cell::new(false));
    let flag = closed.clone();
    let mut o = Overlay::new()
        .panel(Flex::column().child(Label::new("body")))
        .on_dismiss(move || flag.set(true));
    o.show();

    assert_eq!(
        crate::component::dispatch(&mut o, &Event::Widget(WidgetIntent::Dismiss)),
        Handled::Yes,
    );
    assert!(
        closed.get(),
        "Escape ran what the surface said dismissal means"
    );
}

/// **A surface that declared none leaves the key alone.**
///
/// The half that matters more: `Dialog`, `ContextMenu` and `CommandPalette` each answer this key
/// themselves and set no dismissal on their inner overlay. An overlay that swallowed it regardless
/// would have taken Escape from all three at once — three shipped surfaces broken by a fix aimed
/// at a fourth.
#[test]
fn an_overlay_with_no_dismissal_declared_does_not_swallow_the_key() {
    use crate::event::WidgetIntent;

    let mut o = Overlay::new().panel(Flex::column().child(Label::new("body")));
    o.show();
    assert_eq!(
        crate::component::dispatch(&mut o, &Event::Widget(WidgetIntent::Dismiss)),
        Handled::No,
        "the key passes to whatever composes this overlay, untouched",
    );
}

/// **A surface says which control the keyboard starts on, and needs no `key` to do it.**
///
/// The author's call, because only the author knows which control is safe. The name is the
/// control's declared `key` when it has one and **the words it reads by** when it does not —
/// `key` is optional everywhere in this library and a control you point at is no exception.
#[test]
fn a_surface_places_the_keyboard_on_the_control_it_named() {
    use crate::reactive::SignalGet;
    use crate::widgets::Button;

    let mut o = Overlay::new().default_focus("Cancel").panel(
        Flex::row()
            .child(Button::new("Cancel"))
            .child(Button::new("Delete")),
    );
    o.show();

    let panel = &o.base().children[0];
    let cancel = &panel.base().children[0];
    let delete = &panel.base().children[1];
    assert!(
        cancel.base().focused.get_untracked(),
        "named by the words it reads by, with no key declared anywhere",
    );
    assert!(!delete.base().focused.get_untracked(), "and only that one");
}

// ── Showing a surface: three doors, one thing (F003/P097/T502) ──────────────────────────────

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

/// **Taking the caller's signal rebinds the keyboard to it.**
///
/// `base.focused` is bound to whichever signal says whether the surface is up — that binding is
/// the whole of how keys reach a panel. Adopting the caller's without rebinding would leave the
/// keyboard following a signal nobody writes any more, so an overlay that was visibly open would
/// answer nothing.
#[test]
fn following_your_signal_rebinds_the_keyboard_to_it() {
    use crate::reactive::{SignalGet, SignalUpdate, signal};

    let editing = signal(false);
    let o = Overlay::new()
        .panel(Flex::column().child(Label::new("body")))
        .open_when(editing);

    editing.set(true);
    assert!(
        o.base().focused.get_untracked(),
        "an open surface holds the keyboard, whichever signal says it is open",
    );
}

/// **Opening by flag puts the keyboard where opening by call does.** ⚠️ Ran red against its own bug.
///
/// There are two ways a surface goes up — a caller says [`Overlay::show`], or the open flag is
/// simply set (a [`SurfaceHandle`], [`open_when`](Overlay::open_when), a host binding its own
/// state) — and only the first used to place the keyboard, because placing it lived inside the
/// verb. The arrival played either way, so the second looked right and was deaf: it came up with
/// the keyboard on nothing, and the first Tab was spent travelling to the control the surface had
/// already named.
///
/// The two existing guards each cover half of this and cross in the middle: one proves the handle
/// moves the flag, the other proves `show()` places focus. Neither proves the handle places focus,
/// which is exactly what it did not do.
#[test]
fn opening_by_flag_places_the_keyboard_the_same_as_opening_by_call() {
    use crate::component::Component as _;
    use crate::reactive::SignalGet;
    use crate::widgets::Button;

    let mut o = Overlay::new().default_focus("Cancel").panel(
        Flex::row()
            .child(Button::new("Cancel"))
            .child(Button::new("Delete")),
    );
    // The door a click handler uses: copied into a closure, nothing borrowed.
    let handle = o.handle();
    let opener = move || handle.show();
    opener();

    let focused = |o: &Overlay| {
        o.base().children[0].base().children[0]
            .base()
            .focused
            .get_untracked()
    };
    assert!(
        !focused(&o),
        "the flag is set, but nothing has reacted to it yet",
    );

    o.tick(0.016);
    assert!(
        focused(&o),
        "a surface raised by its handle starts on the control it named, exactly as one raised by \
         the call does — otherwise the first Tab is a wasted press",
    );
}

/// **The same, for a signal the caller owns.** A host binding its own state gets the keyboard
/// placed too; the surface does not care who set the flag.
#[test]
fn following_your_own_signal_also_places_the_keyboard() {
    use crate::component::Component as _;
    use crate::reactive::{SignalGet, SignalUpdate, signal};
    use crate::widgets::Button;

    let editing = signal(false);
    let mut o = Overlay::new()
        .default_focus("Cancel")
        .panel(
            Flex::row()
                .child(Button::new("Cancel"))
                .child(Button::new("Delete")),
        )
        .open_when(editing);

    editing.set(true);
    o.tick(0.016);
    assert!(
        o.base().children[0].base().children[0]
            .base()
            .focused
            .get_untracked(),
        "your signal raises it and the keyboard lands where the surface said",
    );
}

/// **Placing the keyboard happens once per arrival, not every frame.**
///
/// Settling runs on every tick, so it has to react to the *change* rather than to the state. If it
/// re-placed on each frame it would drag the keyboard back to the default control continuously,
/// and Tab would appear to do nothing at all while the surface was open.
#[test]
fn a_surface_places_the_keyboard_once_and_then_leaves_it_alone() {
    use crate::component::Component as _;
    use crate::reactive::SignalGet;
    use crate::widgets::Button;

    let mut o = Overlay::new().default_focus("Cancel").panel(
        Flex::row()
            .child(Button::new("Cancel"))
            .child(Button::new("Delete")),
    );
    o.show();
    // Move on, the way a Tab would.
    o.advance_focus(true);
    let moved = |o: &Overlay| {
        o.base().children[0].base().children[1]
            .base()
            .focused
            .get_untracked()
    };
    assert!(moved(&o), "the keyboard moved off the default control");

    o.tick(0.016);
    o.tick(0.016);
    assert!(
        moved(&o),
        "and stayed there — settling reacts to the arrival, not to being open",
    );
}

// ── Showing and closing is something EVERY widget does ─────────────────────

// **The universal guard is not here, and deliberately.** "Every widget can be shown and closed" is
// a claim about the whole catalogue, so it has to be asked of the whole catalogue — a list of
// widgets typed out by hand is the same hardcoding one level up, and a widget added next month
// would not be in it. It walks `WidgetKind::ALL` in `heca-view-realize`
// (`every_widget_kind_can_be_shown_and_closed`), where adding a widget without adding it to that
// list is already a compile error.

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
