//! The generic layout merge.

use heca_grid_ui::ScrollRegion;
use heca_view::WidgetKind;

use super::*;

// ── Generic layout merge ──────────────────────────────────────────────
// The point of these: `realize` holds NO list of layout property names. Everything below
// works because `Layout`'s own fields are the vocabulary.

/// Properties that NO arm in this file has ever read — `padding`, `width`, `justify`,
/// `flex_grow`, `margin` — reach the widget anyway, on a kind with no layout code of its own.
/// This is the regression guard for the whole task: it fails the moment someone reintroduces
/// a hand-written property list that happens to omit one of these.
#[test]
fn layout_properties_never_named_in_realize_still_reach_the_widget() {
    let node = ViewNode::new(WidgetKind::VStack)
        .prop("padding", PropValue::Int(12))
        .prop("width", PropValue::Int(240))
        .prop("justify", PropValue::Text("space_between".into()))
        .prop("flex_grow", PropValue::Float(1.0))
        .prop("margin", PropValue::Float(6.0));

    let w = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    let l = w.base().style.layout;
    assert_eq!(
        l.padding,
        heca_grid_ui::style::Space::Px(12.0),
        "padding — in the original design doc, never implemented",
    );
    assert_eq!(l.width, Length::Px(240.0));
    assert_eq!(l.justify, Justify::SpaceBetween, "enum by name, snake_case");
    assert_eq!(l.flex_grow, 1.0);
    assert_eq!(l.margin, heca_grid_ui::style::Space::Px(6.0));
}

/// A plugin sizes a node by its parts with the same word native code uses: the typed SDK's
/// `.flex(3.0)` writes the `flex` property, and it lands in the one slot `LayoutExt::flex` writes.
#[test]
fn a_described_node_takes_its_parts_with_flex() {
    use heca_view::build::{Style as _, VStack};
    let node: ViewNode = VStack::new().flex(3.0).into();
    assert_eq!(node.props.get("flex"), Some(&PropValue::Float(3.0)));

    let w = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    assert_eq!(w.base().style.layout.flex, Some(3.0));
}

/// A plugin orders a node with the same word native code uses — a number, or a list to slot
/// between two siblings it does not own — and it lands in the one slot `LayoutExt::order` writes.
#[test]
fn a_described_node_says_its_order() {
    use heca_view::build::{Style as _, VStack};
    let one: ViewNode = VStack::new().order(-1).into();
    let list: ViewNode = VStack::new().order([0, 5]).into();
    let realized = |n: &ViewNode| {
        realize(
            n,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        )
        .base()
        .style
        .layout
        .order
    };
    assert_eq!(realized(&one), Some(heca_grid_ui::order::Order::from(-1)));
    assert_eq!(
        realized(&list),
        Some(heca_grid_ui::order::Order::from([0, 5]))
    );
}

/// `Length` reads the way an author would write it: a bare number is px, `"auto"` is auto,
/// and a percentage string is a fraction — not the enum's `{"px": 240}` shape.
#[test]
fn length_accepts_the_spelling_an_author_would_reach_for() {
    let case = |p: PropValue| {
        let node = ViewNode::new(WidgetKind::Surface).prop("width", p);
        realize(
            &node,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        )
        .base()
        .style
        .layout
        .width
    };
    assert_eq!(case(PropValue::Int(240)), Length::Px(240.0));
    assert_eq!(case(PropValue::Float(12.5)), Length::Px(12.5));
    assert_eq!(case(PropValue::Text("auto".into())), Length::Auto);
    assert_eq!(case(PropValue::Text("50%".into())), Length::Percent(0.5));
}

/// Untrusted input stays total, and — the part that matters — a single bad value costs only
/// itself. A wrong type, an unparseable length and an unknown key all get dropped while the
/// good properties on the same node still land.
#[test]
fn a_bad_property_never_takes_the_good_ones_with_it() {
    let node = ViewNode::new(WidgetKind::VStack)
        .prop("gap", PropValue::Int(8))
        .prop("padding", PropValue::Text("not a number".into()))
        .prop("width", PropValue::Text("50 furlongs".into()))
        .prop("nonsense_key", PropValue::Int(3))
        .prop("justify", PropValue::Text("sideways".into()))
        .prop("margin", PropValue::Float(4.0));

    let w = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    let l = w.base().style.layout;
    use heca_grid_ui::style::Space;
    assert_eq!(
        l.gap,
        Space::Px(8.0),
        "good property survives a bad neighbour"
    );
    assert_eq!(
        l.margin,
        Space::Px(4.0),
        "and so does one declared after the bad ones"
    );
    assert_eq!(
        l.padding,
        Space::Px(0.0),
        "bad value ignored — the default stands"
    );
    assert_eq!(l.width, Length::Auto, "unparseable length ignored");
    assert_eq!(l.justify, Justify::Start, "unknown enum name ignored");
}

/// The merge lands ON TOP of the constructed widget. `ScrollRegion::new()` zeroes its min
/// sizes and opts into shrinking so a viewport can be smaller than its content; rebuilding
/// from `Layout::default()` would undo that and the region would silently stop scrolling.
#[test]
fn merging_preserves_layout_the_widget_set_in_its_constructor() {
    let node = ViewNode::new(WidgetKind::Scroll).prop("padding", PropValue::Int(4));

    let w = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    use heca_grid_ui::style::Space;
    let l = w.base().style.layout;
    assert_eq!(l.padding, Space::Px(4.0), "the property the node did carry");
    assert_eq!(
        l.min_height,
        Some(Length::Px(0.0)),
        "constructor value survives"
    );
    assert_eq!(l.min_width, Some(Length::Px(0.0)));
    assert_eq!(
        l.flex_shrink,
        Some(1.0),
        "without this a scroll region cannot shrink"
    );
    assert!(
        matches!(l.gap, Space::Step(_)),
        "theme spacing step survives"
    );
}

/// **A described node can place itself at a fractional rect** — the declarative half of
/// [`LayoutExt::at_rect`](heca_grid_ui::builders::LayoutExt::at_rect), which a plugin needs for
/// the same reason the exposé does: a box whose position its parent cannot express.
///
/// ⚠️ Written because **adding serde to a type is not the same as being able to author it**
/// (AGENTS). `Placement` groups four `Length`s, so it can only travel as a
/// [`PropValue::Map`] — a scalar channel would have carried nothing and nothing would have
/// failed. This is the check that the value channel is real.
#[test]
fn a_description_can_place_a_node_at_a_fractional_rect() {
    let node = ViewNode::new(WidgetKind::VStack).prop(
        "placement",
        PropValue::Map(
            [
                ("left".to_string(), PropValue::Text("25%".into())),
                ("top".to_string(), PropValue::Text("10%".into())),
                ("width".to_string(), PropValue::Text("50%".into())),
                ("height".to_string(), PropValue::Int(120)),
            ]
            .into_iter()
            .collect(),
        ),
    );

    let w = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    let placement = w
        .base()
        .style
        .layout
        .placement
        .expect("the rect was authored");
    assert_eq!(placement.left, Length::Percent(0.25));
    assert_eq!(placement.top, Length::Percent(0.10));
    assert_eq!(placement.width, Length::Percent(0.5));
    assert_eq!(
        placement.height,
        Length::Px(120.0),
        "a bare number is pixels"
    );
}

/// A node with no properties leaves the widget exactly as its constructor built it.
#[test]
fn a_node_with_no_properties_changes_nothing() {
    let bare = realize(
        &ViewNode::new(WidgetKind::Scroll),
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    assert_eq!(
        bare.base().style.layout,
        ScrollRegion::new().base().style.layout
    );
}
