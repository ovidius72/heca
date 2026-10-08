mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, Event, LayoutEngine, PaintCx, Point, Scene, Size, Theme};

#[test]
fn tooltip_reveals_after_a_hover_delay_and_hides_on_leave() {
    use heca_grid_ui::Tooltip;

    // Reveal is wall-clock timed (like the Input caret), so the test sleeps past a
    // short delay rather than feeding simulated `dt`.
    let mut tip = Tooltip::new(Item::new("X"), "HELP").delay(0.05);

    // Render + report whether the bubble text was painted.
    let shows_help = |tip: &mut Tooltip| -> bool {
        LayoutEngine::new().compute(tip, Size::new(300.0, 200.0));
        let theme = Theme::default();
        // **`paint_via_child`, the way a host paints a tree** — the bubble is drawn by the
        // framework from the declaration on the widget's base, beside the hint letter and the
        // drag feedback, so a bare `.paint(cx)` shows the widget and none of the three.
        let scene = common::paint_via_child(tip, &theme);
        scene
            .iter()
            .any(|c| matches!(c, DrawCommand::Text(t) if t.text == "HELP"))
    };

    // Idle: no bubble.
    assert!(!shows_help(&mut tip), "hidden before hover");

    // Hover, but not past the delay yet.
    LayoutEngine::new().compute(&mut tip, Size::new(300.0, 200.0));
    let b = tip.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    heca_grid_ui::dispatch(&mut tip, &Event::pointer_moved(center));
    assert!(
        !shows_help(&mut tip),
        "still hidden before the delay elapses"
    );

    // Past the delay: the bubble shows.
    std::thread::sleep(std::time::Duration::from_millis(120));
    assert!(shows_help(&mut tip), "bubble reveals after the hover delay");

    // Pointer leaves: hidden again immediately.
    heca_grid_ui::dispatch(&mut tip, &Event::pointer_moved(Point::new(-50.0, -50.0)));
    assert!(!shows_help(&mut tip), "hidden once the pointer leaves");
}

/// **A tooltip is a property of the widget, not a box around it** — declared with one builder on
/// any widget, revealed and drawn by the framework.
///
/// This is the capability the wrapper used to be the only way to get, and it is what lets a widget
/// held by a typed container (a `ButtonGroup` takes `Button` children) carry one at all: wrapping
/// it would change what it is, so the container would refuse it.
#[test]
fn any_widget_declares_its_own_tooltip_without_being_wrapped() {
    use heca_grid_ui::ComponentExt;

    let mut button = Button::new("Close")
        .tooltip("Close the pane")
        .tooltip_delay(0.02);

    let shows = |b: &mut Button| -> bool {
        LayoutEngine::new().compute(b, Size::new(300.0, 200.0));
        let theme = Theme::default();
        let scene = common::paint_via_child(b, &theme);
        scene
            .iter()
            .any(|c| matches!(c, DrawCommand::Text(t) if t.text == "Close the pane"))
    };

    assert!(
        !shows(&mut button),
        "nothing is said before the pointer arrives"
    );

    LayoutEngine::new().compute(&mut button, Size::new(300.0, 200.0));
    let b = button.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    heca_grid_ui::dispatch(&mut button, &Event::pointer_moved(center));
    std::thread::sleep(std::time::Duration::from_millis(60));
    assert!(
        shows(&mut button),
        "the bubble reveals once the pointer has rested"
    );

    heca_grid_ui::dispatch(&mut button, &Event::pointer_moved(Point::new(-50.0, -50.0)));
    assert!(
        !shows(&mut button),
        "and goes as soon as the pointer leaves"
    );
}

/// **The host is woken to show a bubble the pointer is already resting on.** Without this the
/// reveal waits for the next unrelated event — the user nudging the mouse a second time — because
/// a still pointer produces no frames of its own.
#[test]
fn a_pending_tooltip_asks_the_host_to_wake_for_it() {
    use heca_grid_ui::ComponentExt;

    let mut button = Button::new("Close").tooltip("Close the pane");
    LayoutEngine::new().compute(&mut button, Size::new(300.0, 200.0));
    assert_eq!(
        button.next_redraw(),
        None,
        "nothing pending while unhovered"
    );

    let b = button.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    heca_grid_ui::dispatch(&mut button, &Event::pointer_moved(center));
    let wake = button
        .next_redraw()
        .expect("a pending reveal wakes the host");
    assert!(
        wake > 0.0 && wake <= 0.5,
        "it wakes at the reveal, not sooner or later (got {wake})"
    );
}

#[test]
fn tooltip_is_transparent_to_child_events() {
    use heca_grid_ui::{FocusManager, Tooltip};
    use std::cell::Cell;
    use std::rc::Rc;

    let clicks = Rc::new(Cell::new(0u32));
    let sink = clicks.clone();
    let mut tip = Tooltip::new(
        Item::new("file").on_activate(move || sink.set(sink.get() + 1)),
        "open",
    );
    LayoutEngine::new().compute(&mut tip, Size::new(200.0, 60.0));

    let mut focus = FocusManager::new();
    focus.advance(&mut tip, true);
    assert_eq!(
        focus.focused(&mut tip),
        Some(0),
        "wrapped child is reachable by Tab"
    );
    focus.deliver_key(&mut tip, heca_grid_ui::GridKey::Enter);
    assert_eq!(
        clicks.get(),
        1,
        "Enter activates the wrapped child through the tooltip"
    );
}

#[test]
fn tooltip_flips_to_fit_the_viewport() {
    use heca_grid_ui::{Component, DrawCommand, Tooltip, TooltipSide};

    // A `Bottom` tooltip whose target sits near the viewport's bottom edge has no
    // room below → it must flip above the target.
    let vp = Size::new(300.0, 100.0);
    let mut tip = Tooltip::new(Item::new("X"), "HELP")
        .side(TooltipSide::Bottom)
        .delay(0.0);
    LayoutEngine::new().compute(&mut tip, vp);

    // Shove the whole subtree down so the target is near the bottom edge.
    fn shift(c: &mut dyn Component, dy: f64) {
        c.base_mut().bounds.loc.y += dy;
        for ch in c.base_mut().children.iter_mut() {
            shift(ch.as_mut(), dy);
        }
    }
    shift(&mut tip, 82.0);

    let b = tip.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    heca_grid_ui::dispatch(&mut tip, &Event::pointer_moved(center));

    let theme = Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(vp);
        heca_grid_ui::paint_child(&tip, &mut cx);
    }
    let bubble = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text(t) if t.text == "HELP" => Some(t.rect),
            _ => None,
        })
        .expect("tooltip bubble is painted");
    assert!(
        bubble.loc.y < b.loc.y,
        "Bottom tooltip with no room below flips above the target (bubble {} < target {})",
        bubble.loc.y,
        b.loc.y
    );
}

/// **A quick tooltip is a name for a shorter rest**, on every widget: the library's own quick delay,
/// shorter than the default, and nothing at all on a widget that declared no tooltip.
#[test]
fn a_quick_tooltip_rests_for_the_librarys_quick_delay() {
    use heca_grid_ui::widgets::tooltip::{DEFAULT_DELAY, QUICK_DELAY};

    let plain = Label::new("main").tooltip("feat/long-branch-name");
    let quick = Label::new("main")
        .tooltip("feat/long-branch-name")
        .tooltip_quick(true);
    assert_eq!(
        plain.base().tooltip.as_ref().map(|t| t.delay),
        Some(DEFAULT_DELAY)
    );
    assert_eq!(
        quick.base().tooltip.as_ref().map(|t| t.delay),
        Some(QUICK_DELAY)
    );
    const { assert!(QUICK_DELAY < DEFAULT_DELAY) };

    let none = Label::new("main").tooltip_quick(true);
    assert!(
        none.base().tooltip.is_none(),
        "no tooltip declared: nothing to speed up"
    );
}
