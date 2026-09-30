use super::*;
use crate::{Intent, PropValue, ViewEvent, ViewNode, WidgetKind};

/// The builders produce the same node hand-authoring does — they are sugar, not a second model.
#[test]
fn a_built_tree_is_an_ordinary_view_node() {
    let built = VStack::new()
        .gap(8.0)
        .child(Label::new("nginx").bold(true))
        .into_node();

    let by_hand = ViewNode::new(WidgetKind::VStack)
        .prop("gap", PropValue::Float(8.0))
        .child(
            ViewNode::new(WidgetKind::Label)
                .text("nginx")
                .prop("bold", PropValue::Bool(true)),
        );

    assert_eq!(built, by_hand);
}

/// Appearance goes through the builders too, including the struct-shaped half that only became
/// authorable in F003/P011/T018.
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

/// **Every kind of the vocabulary has a builder that can be given a hint** — or is named here with
/// its reason. The match is exhaustive, so a kind added to `WidgetKind` does not compile here until
/// someone writes its builder or its exemption: a kind cannot be left without one unnoticed.
///
/// The hint is one method on `Style`, so a builder made any other way than the builder macros
/// would have none; this is what would say so.
#[test]
fn every_kind_has_a_builder_that_can_be_given_a_hint() {
    /// The builder for `kind` as a node, or `None` where the SDK has none — with the reason.
    fn builder(kind: WidgetKind) -> Option<ViewNode> {
        use WidgetKind as K;
        Some(match kind {
            K::VStack => VStack::new().on_hint(Intent::new("pick")).into_node(),
            K::HStack => HStack::new().on_hint(Intent::new("pick")).into_node(),
            K::Row => Row::new().on_hint(Intent::new("pick")).into_node(),
            K::Grid => Grid::new().on_hint(Intent::new("pick")).into_node(),
            K::Card => Card::new("x").on_hint(Intent::new("pick")).into_node(),
            K::Scroll => Scroll::new().on_hint(Intent::new("pick")).into_node(),
            K::Panel => Panel::new().on_hint(Intent::new("pick")).into_node(),
            K::Surface => Surface::new().on_hint(Intent::new("pick")).into_node(),
            K::ItemGroup => ItemGroup::new().on_hint(Intent::new("pick")).into_node(),
            K::DockFrame => DockFrame::new().on_hint(Intent::new("pick")).into_node(),
            K::Overlay => Overlay::new().on_hint(Intent::new("pick")).into_node(),
            K::MarkerGroup => MarkerGroup::new().on_hint(Intent::new("pick")).into_node(),
            K::Tabs => Tabs::new().on_hint(Intent::new("pick")).into_node(),
            K::Choice => Choice::new().on_hint(Intent::new("pick")).into_node(),
            K::KeyHintGroup => KeyHintGroup::new().on_hint(Intent::new("pick")).into_node(),
            K::Label => Label::new("x").on_hint(Intent::new("pick")).into_node(),
            K::Button => Button::new().on_hint(Intent::new("pick")).into_node(),
            K::IconButton => IconButton::new().on_hint(Intent::new("pick")).into_node(),
            K::Badge => Badge::new("x").on_hint(Intent::new("pick")).into_node(),
            K::BadgeButton => BadgeButton::new("x")
                .on_hint(Intent::new("pick"))
                .into_node(),
            K::Tag => Tag::new("x").on_hint(Intent::new("pick")).into_node(),
            K::Icon => Icon::new().on_hint(Intent::new("pick")).into_node(),
            K::Input => Input::new().on_hint(Intent::new("pick")).into_node(),
            K::Select => Select::new().on_hint(Intent::new("pick")).into_node(),
            K::Toggle => Toggle::new().on_hint(Intent::new("pick")).into_node(),
            K::Checkbox => Checkbox::new().on_hint(Intent::new("pick")).into_node(),
            K::StatusDot => StatusDot::new().on_hint(Intent::new("pick")).into_node(),
            K::Gauge => Gauge::new().on_hint(Intent::new("pick")).into_node(),
            // Host-only: its state is live host signals, and `realize` refuses it outright.
            K::ScrollBar => return None,
            K::Alert => Alert::new().on_hint(Intent::new("pick")).into_node(),
            K::Toast => Toast::new().on_hint(Intent::new("pick")).into_node(),
            K::RailCell => RailCell::new().on_hint(Intent::new("pick")).into_node(),
            K::Item => Item::new().on_hint(Intent::new("pick")).into_node(),
            K::Separator => Separator::new().on_hint(Intent::new("pick")).into_node(),
            K::CardGrid => CardGrid::new().on_hint(Intent::new("pick")).into_node(),
            K::Spinner => Spinner::new().on_hint(Intent::new("pick")).into_node(),
            K::Progress => Progress::new().on_hint(Intent::new("pick")).into_node(),
            K::NfIcon => NfIcon::new().on_hint(Intent::new("pick")).into_node(),
            K::ButtonGroup => ButtonGroup::new().on_hint(Intent::new("pick")).into_node(),
            K::Tile => Tile::new().on_hint(Intent::new("pick")).into_node(),
        })
    }

    for &kind in WidgetKind::ALL {
        let Some(node) = builder(kind) else { continue };
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
