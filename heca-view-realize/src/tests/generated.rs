//! The generated surface, end to end.

use heca_grid_ui::{Input, ScrollRegion};
use heca_view::WidgetKind;

use crate::props::text_of;
use crate::style::with_props;
use super::*;

// ── The generated surface, end to end ─────────────────────────────────
// The two capabilities that started this phase: both existed in the widgets, and neither
// could be set from a description while this file named properties by hand.

/// An input's placeholder. `Input::placeholder` has existed all along and the showcase uses it
/// twice, including the command palette — yet a description could not say it.
#[test]
fn a_description_can_now_set_an_inputs_placeholder() {
    let node = ViewNode::new(WidgetKind::Input)
        .text("current")
        .prop("placeholder", PropValue::Text("type to filter…".into()));

    let input = with_props(Input::new().value(text_of(&node)), &node, &Theme::default());
    assert_eq!(input.placeholder_str(), "type to filter…");
    assert_eq!(
        input.value_str(),
        "current",
        "the value still lands alongside it"
    );
}

/// A scroll region's axes. `ScrollRegion` has supported both all along — the showcase's own
/// root is `.both()` — but every declarative region was vertical, forever.
#[test]
fn a_description_can_now_ask_for_a_two_axis_scroll_region() {
    let both = ViewNode::new(WidgetKind::Scroll).prop("axes", PropValue::Text("both".into()));
    assert_eq!(
        with_props(ScrollRegion::new(), &both, &Theme::default()).clone_axes(),
        heca_grid_ui::ScrollAxes::Both,
    );
    assert_eq!(
        with_props(
            ScrollRegion::new(),
            &ViewNode::new(WidgetKind::Scroll),
            &Theme::default()
        )
        .clone_axes(),
        heca_grid_ui::ScrollAxes::Vertical,
        "unset still means the widget's own default",
    );
}

/// Total for untrusted input: an unknown enum name and a property belonging to a different
/// widget both leave the widget alone, and the good property on the same node still lands.
#[test]
fn the_generated_surface_ignores_what_it_cannot_use() {
    let node = ViewNode::new(WidgetKind::Scroll)
        .prop("axes", PropValue::Text("sideways".into()))
        .prop(
            "placeholder",
            PropValue::Text("not a scroll property".into()),
        )
        .prop("gap", PropValue::Int(6));

    assert_eq!(
        with_props(ScrollRegion::new(), &node, &Theme::default()).clone_axes(),
        heca_grid_ui::ScrollAxes::Vertical,
        "unknown variant name keeps the default",
    );
    let w = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    assert_eq!(
        w.base().style.layout.gap,
        heca_grid_ui::style::Space::Px(6.0),
        "the good property still lands",
    );
}

/// The two tests above read the widget through `with_props`, which is the surface but not the
/// path a real description takes. These two go through **`realize` itself** and read the result
/// the only way a `Box<dyn Component>` allows — by painting it — so the arm is proven to wire
/// the surface, not just the surface proven to exist.
///
/// An empty input paints its placeholder, so the text is in the scene when the arm passed it on
/// and absent when it did not.
#[test]
fn a_realized_input_paints_the_placeholder_it_was_given() {
    use heca_core::layout::Size;
    use heca_grid_ui::{DrawCommand, LayoutEngine, PaintCx, Scene, Theme};

    let runs = |node: &ViewNode| {
        let mut widget = realize(
            node,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        LayoutEngine::new().compute(widget.as_mut(), Size::new(240.0, 40.0));
        let theme = Theme::default();
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            widget.paint(&mut cx);
        }
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    // No value, so the placeholder is what shows.
    let with = ViewNode::new(WidgetKind::Input)
        .prop("placeholder", PropValue::Text("type to filter…".into()));
    assert!(
        runs(&with).contains(&"type to filter…".to_string()),
        "the realized input paints the placeholder it was described with",
    );
    assert!(
        !runs(&ViewNode::new(WidgetKind::Input)).contains(&"type to filter…".to_string()),
        "and it is the property that put it there, not the widget's own default",
    );
}

/// A described two-axis region **is** the native one: same widget, same content, same scene.
/// The `axes` property is the only difference between the two calls, and it is what makes the
/// horizontal bar appear — every declarative region was vertical forever before it.
#[test]
fn a_realized_two_axis_region_is_the_native_one() {
    use heca_core::layout::Size;
    use heca_grid_ui::{LayoutEngine, PaintCx, Parent, Scene, ScrollAxes, Theme};

    // Content wider AND taller than the viewport, so both axes overflow and both bars draw.
    let content = ViewNode::new(WidgetKind::VStack)
        .prop("width", PropValue::Int(400))
        .prop("height", PropValue::Int(300))
        .child(ViewNode::new(WidgetKind::Label).text("content"));
    let realized_content = || {
        realize(
            &content,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        )
    };

    let paint = |mut widget: Box<dyn Component>| {
        LayoutEngine::new().compute(widget.as_mut(), Size::new(120.0, 80.0));
        let theme = Theme::default();
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            widget.paint(&mut cx);
        }
        scene.iter().cloned().collect::<Vec<_>>()
    };

    let described = |axes: Option<&str>| {
        let mut node = ViewNode::new(WidgetKind::Scroll)
            .prop("width", PropValue::Int(120))
            .prop("height", PropValue::Int(80))
            .child(content.clone());
        if let Some(axes) = axes {
            node = node.prop("axes", PropValue::Text(axes.into()));
        }
        paint(realize(
            &node,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        ))
    };

    let native = |axes: ScrollAxes| {
        paint(Box::new(
            ScrollRegion::new()
                .axes(axes)
                .width(Length::Px(120.0))
                .height(Length::Px(80.0))
                .child(realized_content()),
        ))
    };

    assert_eq!(
        described(Some("both")),
        native(ScrollAxes::Both),
        "same widget, same scene"
    );
    assert_eq!(
        described(None),
        native(ScrollAxes::Vertical),
        "unset = the widget's default"
    );
    assert_ne!(
        described(Some("both")),
        described(None),
        "the property is what adds the second axis (and its bar)",
    );
    assert_eq!(
        described(Some("sideways")),
        native(ScrollAxes::Vertical),
        "an unknown axis name keeps the default, through the whole path",
    );
}

/// And it **scrolls** both ways, not just paints a second bar: a described two-axis region
/// consumes a horizontal wheel delta, where a described default region leaves it for the host.
/// (The visible behaviour is confirmed in the running app; this pins the routing.)
#[test]
fn a_realized_two_axis_region_consumes_a_horizontal_wheel() {
    use heca_core::layout::{Point, Size};
    use heca_grid_ui::{Event, Handled, LayoutEngine};

    let horizontal_wheel = |axes: Option<&str>| {
        let mut node = ViewNode::new(WidgetKind::Scroll)
            .prop("width", PropValue::Int(120))
            .prop("height", PropValue::Int(80))
            .child(
                ViewNode::new(WidgetKind::VStack)
                    .prop("width", PropValue::Int(400))
                    .prop("height", PropValue::Int(300))
                    .child(ViewNode::new(WidgetKind::Label).text("content")),
            );
        if let Some(axes) = axes {
            node = node.prop("axes", PropValue::Text(axes.into()));
        }
        let mut region = realize(
            &node,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        LayoutEngine::new().compute(region.as_mut(), Size::new(120.0, 80.0));
        // The wheel is hover-gated (`Event::Scroll` carries no position), so hover it first.
        heca_grid_ui::dispatch(
            region.as_mut(),
            &Event::pointer_moved(Point::new(60.0, 40.0)),
        );
        heca_grid_ui::dispatch(
            region.as_mut(),
            &Event::wheel(Point::new(60.0, 40.0), -1.0, 0.0),
        )
    };

    assert_eq!(
        horizontal_wheel(Some("both")),
        Handled::Yes,
        "a described two-axis region scrolls horizontally",
    );
    assert_eq!(
        horizontal_wheel(None),
        Handled::No,
        "a described default region still has no horizontal axis to scroll",
    );
}
