use super::*;
use crate::{Intent, PropValue, ViewEvent, ViewNode, WidgetId};

#[test]
fn marker_and_tag_properties_reach_the_described_tree() {
    let marker = MarkerGroup::new().marker(false).gutter(crate::ViewSpacing::Lg).into_node();
    assert_eq!(marker.props.get("marker"), Some(&PropValue::Bool(false)));
    assert_eq!(marker.props.get("gutter"), Some(&PropValue::Text("lg".into())));
    let tag = Tag::new("status").segment_shrink(false).into_node();
    assert_eq!(tag.props.get("segment_shrink"), Some(&PropValue::Bool(false)));
}

/// The builders produce the same node hand-authoring does — they are sugar, not a second model.
#[test]
fn a_built_tree_is_an_ordinary_view_node() {
    let built = VStack::new()
        .gap(8.0)
        .child(Label::new("nginx").bold(true))
        .into_node();

    let by_hand = ViewNode::new(WidgetId::VStack)
        .prop("gap", PropValue::Float(8.0))
        .child(
            ViewNode::new(WidgetId::Label)
                .text("nginx")
                .prop("bold", PropValue::Bool(true)),
        );

    assert_eq!(built, by_hand);
}

/// Appearance goes through the builders too, including the struct-shaped half.
#[test]
fn appearance_including_border_and_glow_is_authorable() {
    let node = Surface::new()
        .fill("accent")
        .border("muted", 2.0)
        .glow("accent", 12.0, 0.4)
        .radius(6.0)
        .into_node();

    assert_eq!(
        node.props.get("fill"),
        Some(&PropValue::Color("accent".into()))
    );
    assert!(matches!(node.props.get("border"), Some(PropValue::Map(_))));
    assert!(matches!(node.props.get("glow"), Some(PropValue::Map(_))));
}

/// A percentage width travels in the spelling `Length`'s deserializer accepts.
#[test]
fn a_percentage_width_is_written_the_way_length_reads_it() {
    let node = Surface::new().width_pct(0.5).into_node();
    assert_eq!(
        node.props.get("width"),
        Some(&PropValue::Text("50%".into()))
    );
}

/// An event lands under the name `realize` looks for.
#[test]
fn an_event_is_bound_under_its_canonical_name() {
    let node = Row::new()
        .on_press(Intent::new("docker.select"))
        .into_node();
    assert_eq!(
        node.events.get("press").map(|i| i.action.as_str()),
        Some("docker.select")
    );
}

/// **One builder writes ONE property, whichever kind of space it was given.**
///
/// The retired names are still read on the far side, so a tree emitting `"gap_spacing"` keeps
/// working — which means nothing downstream can tell you the builder picked the wrong name.
/// This is the only place that can, so it pins the name rather than the effect.
#[test]
fn spacing_is_written_under_one_property_name() {
    let px = VStack::new().gap(8).into_node();
    assert_eq!(px.props.get("gap"), Some(&PropValue::Float(8.0)));
    assert!(
        !px.props.contains_key("gap_spacing"),
        "the retired name must not be emitted — it is read, not written",
    );

    let step = VStack::new().gap(crate::ViewSpacing::Sm).into_node();
    assert_eq!(
        step.props.get("gap"),
        Some(&PropValue::Text("sm".into())),
        "a step travels as its name, under the same property",
    );
    assert!(!step.props.contains_key("gap_spacing"));
}

/// Padding does the same, and a string is read as either kind.
#[test]
fn padding_is_written_under_one_property_name() {
    let node = Surface::new().padding("md").padding_x(4).into_node();
    assert_eq!(
        node.props.get("padding"),
        Some(&PropValue::Text("md".into()))
    );
    assert_eq!(node.props.get("padding_x"), Some(&PropValue::Float(4.0)));
    for retired in ["pad_spacing_x", "pad_spacing_y"] {
        assert!(
            !node.props.contains_key(retired),
            "{retired} must not be emitted"
        );
    }
}

/// A builder made with a text in its constructor and one made without are the same thing once
/// built: both are a `Style` over their node and both hand that node back. One definition, so
/// neither can drift from the other.
#[test]
fn a_builder_with_text_and_one_without_convert_the_same_way() {
    let text = Label::new("x");
    assert_eq!(ViewNode::from(text.clone()), text.into_node());

    let plain = VStack::new();
    assert_eq!(ViewNode::from(plain.clone()), plain.into_node());
}

/// The legacy typed builders preserve their hint capability through the identifier migration.
/// Current host registrations are audited in heca-view-realize, where implementations exist;
/// this serialization-only crate does not maintain the host inventory.
///
/// The hint is one method on `Style`, so a builder made any other way than the builder macros
/// would have none; this is what would say so.
#[test]
fn every_kind_has_a_builder_that_can_be_given_a_hint() {
    /// The builder for `kind` as a node, or `None` where the SDK has none — with the reason.
    fn builder(kind: WidgetId) -> Option<ViewNode> {
        Some(match kind.as_str() {
            "image" => Image::new().svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#).on_hint(Intent::new("pick")).into_node(),
            "v_stack" => VStack::new().on_hint(Intent::new("pick")).into_node(),
            "h_stack" => HStack::new().on_hint(Intent::new("pick")).into_node(),
            "row" => Row::new().on_hint(Intent::new("pick")).into_node(),
            "tree_layout" => TreeLayout::new().on_hint(Intent::new("pick")).into_node(),
            "message_list" => MessageList::new().on_hint(Intent::new("pick")).into_node(),
            "resizable" => Resizable::new().on_hint(Intent::new("pick")).into_node(),
            "line" => Line::new().on_hint(Intent::new("pick")).into_node(),
            "grid" => Grid::new().on_hint(Intent::new("pick")).into_node(),
            "card" => Card::new("x").on_hint(Intent::new("pick")).into_node(),
            "scroll" => Scroll::new().on_hint(Intent::new("pick")).into_node(),
            "panel" => Panel::new().on_hint(Intent::new("pick")).into_node(),
            "surface" => Surface::new().on_hint(Intent::new("pick")).into_node(),
            "item_group" => ItemGroup::new().on_hint(Intent::new("pick")).into_node(),
            "dock_frame" => DockFrame::new().on_hint(Intent::new("pick")).into_node(),
            "overlay" => Overlay::new().on_hint(Intent::new("pick")).into_node(),
            "marker_group" => MarkerGroup::new().on_hint(Intent::new("pick")).into_node(),
            "tabs" => Tabs::new().on_hint(Intent::new("pick")).into_node(),
            "choice" => Choice::new().on_hint(Intent::new("pick")).into_node(),
            "key_hint_group" => KeyHintGroup::new().on_hint(Intent::new("pick")).into_node(),
            "label" => Label::new("x").on_hint(Intent::new("pick")).into_node(),
            "button" => Button::new().on_hint(Intent::new("pick")).into_node(),
            "icon_button" => IconButton::new().on_hint(Intent::new("pick")).into_node(),
            "badge" => Badge::new("x").on_hint(Intent::new("pick")).into_node(),
            "badge_button" => BadgeButton::new("x")
                .on_hint(Intent::new("pick"))
                .into_node(),
            "tag" => Tag::new("x").on_hint(Intent::new("pick")).into_node(),
            "icon" => Icon::new().on_hint(Intent::new("pick")).into_node(),
            "input" => Input::new().on_hint(Intent::new("pick")).into_node(),
            "select" => Select::new().on_hint(Intent::new("pick")).into_node(),
            "toggle" => Toggle::new().on_hint(Intent::new("pick")).into_node(),
            "checkbox" => Checkbox::new().on_hint(Intent::new("pick")).into_node(),
            "status_dot" => StatusDot::new().on_hint(Intent::new("pick")).into_node(),
            "gauge" => Gauge::new().on_hint(Intent::new("pick")).into_node(),
            // Host-only: its state is live host signals, and `realize` refuses it outright.
            "scroll_bar" => ScrollBar::new().content_extent(100.0).viewport_extent(10.0).on_hint(Intent::new("pick")).into_node(),
            "alert" => Alert::new().on_hint(Intent::new("pick")).into_node(),
            "toast" => Toast::new().on_hint(Intent::new("pick")).into_node(),
            "rail_cell" => RailCell::new().on_hint(Intent::new("pick")).into_node(),
            "item" => Item::new().on_hint(Intent::new("pick")).into_node(),
            "separator" => Separator::new().on_hint(Intent::new("pick")).into_node(),
            "splitter" => Splitter::new().on_hint(Intent::new("pick")).into_node(),
            "landing_slot" => LandingSlot::new().on_hint(Intent::new("pick")).into_node(),
            "card_grid" => CardGrid::new().on_hint(Intent::new("pick")).into_node(),
            "spinner" => Spinner::new().on_hint(Intent::new("pick")).into_node(),
            "progress" => Progress::new().on_hint(Intent::new("pick")).into_node(),
            "nf_icon" => NfIcon::new().on_hint(Intent::new("pick")).into_node(),
            "button_group" => ButtonGroup::new().on_hint(Intent::new("pick")).into_node(),
            "tile" => Tile::new().on_hint(Intent::new("pick")).into_node(),
            _ => return None,
        })
    }

    for kind in crate::kind::legacy_wire_ids() {
        let Some(node) = builder(kind.clone()) else { continue };
        assert_eq!(
            node.kind, kind,
            "the builder for {kind:?} builds another kind"
        );
        assert!(
            node.intent(ViewEvent::Hint).is_some(),
            "the builder for {kind:?} cannot be given a hint",
        );
    }
}
