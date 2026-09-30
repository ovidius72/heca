use super::*;
use serde::Serialize;

/// A menu entry's place is `order` — a number or a list — and a plugin written when it was
/// called `weight` still reads.
#[test]
fn a_menu_entry_reads_its_order_under_either_name() {
    let base =
        r#""id": "x", "label": "X", "intent": {"action": "x"}, "danger": false, "enabled": true"#;
    let one: DropdownItem = serde_json::from_str(&format!("{{{base}, \"order\": -1}}")).unwrap();
    let list: DropdownItem =
        serde_json::from_str(&format!("{{{base}, \"order\": [0, 5]}}")).unwrap();
    let old: DropdownItem =
        serde_json::from_str(&format!("{{{base}, \"weight\": [1, 2]}}")).unwrap();
    assert_eq!(one.order, Some(ViewOrder(vec![-1])));
    assert_eq!(list.order, Some(ViewOrder(vec![0, 5])));
    assert_eq!(
        old.order,
        Some(ViewOrder(vec![1, 2])),
        "the old name still reads"
    );
    assert_eq!(
        serde_json::to_value(&one).unwrap()["order"],
        serde_json::json!(-1),
        "and it is written back under the new name, as a number when it is one place",
    );
}

// ── The identity rule, declarative half: a `press` in a collection needs a `key` ──────────

fn row(action: &str) -> ViewNode {
    ViewNode::new(WidgetKind::Row).on_press(Intent::new(action))
}

/// The case this exists for: rows built by iterating, none of them keyed. Every one is
/// reported, because every one loses its cursor position and its letter on the next rebuild.
#[test]
fn every_unkeyed_pressable_row_of_a_collection_is_reported() {
    let tree = ViewNode::new(WidgetKind::VStack)
        .child(row("focus_pane"))
        .child(row("focus_pane"))
        .child(row("focus_pane"));

    let found = unkeyed_collection_items(&tree);
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].path, vec![0]);
    assert_eq!(found[2].path, vec![2]);
    assert_eq!(found[0].kind, WidgetKind::Row);
    assert_eq!(found[0].action, "focus_pane");
    assert_eq!(
        found[0].siblings, 3,
        "what the author has to look at to see the collection"
    );
}

/// Keying the items is the fix, and it is the only thing the report ever asks for.
#[test]
fn keyed_items_are_never_reported() {
    let tree = ViewNode::new(WidgetKind::VStack)
        .child(row("focus_pane").key("pane:7"))
        .child(row("focus_pane").key("pane:9"));

    assert_eq!(unkeyed_collection_items(&tree), vec![]);
}

/// A composed control is not a collection: `WidgetKind` says so by construction, with nothing
/// to infer about what the author meant.
#[test]
fn a_composed_control_is_not_a_collection() {
    let tree = ViewNode::new(WidgetKind::Choice)
        .on_press(Intent::new("set_level"))
        .child(ViewNode::new(WidgetKind::Icon))
        .child(ViewNode::new(WidgetKind::Label).text("HIGH"));

    assert_eq!(unkeyed_collection_items(&tree), vec![]);
}

/// **Only an actionable node is reported.** Identity is what a cursor, a right-click, a drag and
/// a remembered letter are kept on — decorative siblings have none of those to lose.
#[test]
fn a_collection_with_nothing_to_press_is_not_reported() {
    let tree = ViewNode::new(WidgetKind::VStack)
        .child(ViewNode::new(WidgetKind::Label).text("one"))
        .child(ViewNode::new(WidgetKind::Label).text("two"))
        .child(ViewNode::new(WidgetKind::Label).text("three"));

    assert_eq!(unkeyed_collection_items(&tree), vec![]);
}

/// A single pressable child is not a collection — the ordinary case of a button in a box, which
/// needs no key and must never be asked for one.
#[test]
fn a_lone_pressable_child_is_not_a_collection() {
    let tree = ViewNode::new(WidgetKind::HStack)
        .child(ViewNode::new(WidgetKind::Label).text("Delete pane?"))
        .child(
            ViewNode::new(WidgetKind::Button)
                .text("OK")
                .on_press(Intent::new("confirm_ok")),
        );

    assert_eq!(unkeyed_collection_items(&tree), vec![]);
}

/// Two buttons side by side **are** a collection of two, so an unkeyed pressable one is
/// reported — a confirm dialog's Cancel/OK pair is the everyday example, and the everyday fix
/// is a key naming the action.
#[test]
fn a_pair_of_buttons_is_a_collection_of_two() {
    let tree = ViewNode::new(WidgetKind::HStack)
        .child(
            ViewNode::new(WidgetKind::Button)
                .text("Cancel")
                .on_press(Intent::new("cancel")),
        )
        .child(
            ViewNode::new(WidgetKind::Button)
                .text("OK")
                .on_press(Intent::new("confirm_ok")),
        );

    let found = unkeyed_collection_items(&tree);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].action, "cancel");
    assert_eq!(found[1].action, "confirm_ok");
}

/// The report reaches the whole tree, not only its top: a list nested inside a card is still a
/// list, and its path says where to look.
#[test]
fn a_nested_collection_is_reported_with_its_path() {
    let tree = ViewNode::new(WidgetKind::VStack).child(
        ViewNode::new(WidgetKind::Card)
            .child(row("open"))
            .child(row("open")),
    );

    let found = unkeyed_collection_items(&tree);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].path, vec![0, 0]);
    assert_eq!(found[1].path, vec![0, 1]);
}

/// Siblings of **different** kinds are not one collection, however many of them there are.
#[test]
fn different_kinds_are_not_one_collection() {
    let tree = ViewNode::new(WidgetKind::HStack)
        .child(ViewNode::new(WidgetKind::Button).on_press(Intent::new("a")))
        .child(ViewNode::new(WidgetKind::Row).on_press(Intent::new("b")))
        .child(ViewNode::new(WidgetKind::Item).on_press(Intent::new("c")));

    assert_eq!(unkeyed_collection_items(&tree), vec![]);
}

/// `realize` writes a described key into the very slot a native `.key(..)` writes — held on the
/// realize side; here we only hold that the node carries it and reads it back.
#[test]
fn a_key_is_an_ordinary_prop_read_back_by_name() {
    let node = ViewNode::new(WidgetKind::Row).key("pane:7");
    assert_eq!(node.declared_key(), Some("pane:7"));
    assert_eq!(
        node.props.get("key"),
        Some(&PropValue::Text("pane:7".into()))
    );
    assert_eq!(ViewNode::new(WidgetKind::Row).declared_key(), None);
}

/// A small confirm-dialog-shaped tree: a column with a message + two action buttons.
fn confirm_tree() -> ViewNode {
    ViewNode::new(WidgetKind::VStack)
        .prop("gap", PropValue::Int(8))
        .child(ViewNode::new(WidgetKind::Label).text("Delete pane?"))
        .child(
            ViewNode::new(WidgetKind::HStack)
                .child(
                    ViewNode::new(WidgetKind::Button)
                        .text("Cancel")
                        .prop("variant", PropValue::Variant(ViewVariant::Secondary))
                        .on_press(Intent::new("confirm_cancel")),
                )
                .child(
                    ViewNode::new(WidgetKind::Button)
                        .text("Delete")
                        .prop("variant", PropValue::Variant(ViewVariant::Destructive))
                        .prop("size", PropValue::Size(ViewSize::Normal))
                        .on_press(Intent::new("confirm_ok")),
                ),
        )
}

/// `ALL` must list the whole vocabulary, in declaration order — the realize **coverage guard**
/// walks it, and a guard is only as good as the list it walks. The enum and `ALL` come from one
/// list; this fails if that ever stops being so.
#[test]
fn all_lists_every_widget_kind() {
    for (i, kind) in WidgetKind::ALL.iter().enumerate() {
        assert_eq!(
            kind.ordinal(),
            i,
            "{kind:?} is out of order in ALL (or missing from it)",
        );
    }
    let highest = WidgetKind::ALL
        .iter()
        .map(|k| k.ordinal())
        .max()
        .expect("the vocabulary is not empty");
    assert_eq!(
        WidgetKind::ALL.len(),
        highest + 1,
        "a variant exists that ALL does not list",
    );
}

#[test]
fn actionable_reflects_click_binding() {
    let btn = ViewNode::new(WidgetKind::Button)
        .text("OK")
        .on_press(Intent::new("ok"));
    assert!(btn.is_actionable());
    assert_eq!(btn.intent("press").unwrap().action, "ok");
    assert!(!ViewNode::new(WidgetKind::Label).text("hi").is_actionable());
}

#[test]
fn json_round_trips_wasm_ready() {
    let tree = confirm_tree();
    let json = serde_json::to_string(&tree).expect("serialize");
    let back: ViewNode = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        tree, back,
        "ViewNode must round-trip through JSON (WASM boundary)"
    );
    // Spot-check the shape survived.
    assert_eq!(back.kind, WidgetKind::VStack);
    assert_eq!(back.children.len(), 2);
    assert!(back.children[1].children[1].is_actionable());
}

/// The whole `PropValue` vocabulary — including the nested [`PropValue::List`] a `Grid`'s track
/// templates ride on — must survive the WASM boundary intact.
#[test]
fn every_prop_value_round_trips_including_lists() {
    let node = ViewNode::new(WidgetKind::Grid)
        .prop(
            "columns",
            PropValue::List(vec![
                PropValue::Text("auto".into()),
                PropValue::Text("1fr".into()),
            ]),
        )
        .prop(
            "areas",
            PropValue::List(vec![PropValue::Text("icon title".into())]),
        )
        .prop("flag", PropValue::Bool(true))
        .prop("count", PropValue::Int(3))
        .prop("ratio", PropValue::Float(0.5))
        .prop("tint", PropValue::Color("#ff00ff".into()))
        .prop("icon", PropValue::Glyph("terminal".into()))
        .prop("size", PropValue::Size(ViewSize::Small))
        .prop("align", PropValue::Align(ViewAlign::Center))
        .prop("variant", PropValue::Variant(ViewVariant::Ghost));

    let json = serde_json::to_string(&node).expect("serialize");
    let back: ViewNode = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(node, back, "every PropValue must round-trip through JSON");
    assert!(
        matches!(back.props.get("columns"), Some(PropValue::List(items)) if items.len() == 2),
        "the list survives as a list: {json}",
    );
}

#[test]
fn empty_maps_are_omitted_in_json() {
    let json = serde_json::to_string(&ViewNode::new(WidgetKind::Label).text("x")).unwrap();
    assert!(!json.contains("events"), "empty events omitted: {json}");
    assert!(!json.contains("children"), "empty children omitted: {json}");
}
/// A value set's `name()` and its serde spelling are the same word.
///
/// Two things have to agree for a fixed value to survive the trip: `name()`, which the
/// authoring layer writes into a property, and the serde `rename_all` spelling, which is what a
/// serialized description carries. If they ever differed, a value written through the type
/// would arrive as a word the widget does not know — silently, which is the failure these types
/// exist to remove. `ALL` comes from the same list as `name`, so this cannot miss a variant.
#[test]
fn a_value_sets_name_matches_how_it_serializes() {
    fn check<T: Copy + Serialize + std::fmt::Debug>(all: &[T], name: impl Fn(T) -> &'static str) {
        for &v in all {
            let json = serde_json::to_value(v).unwrap();
            assert_eq!(
                json.as_str(),
                Some(name(v)),
                "{v:?}: name() and serde disagree"
            );
        }
    }
    check(ViewOrientation::ALL, ViewOrientation::name);
    check(ViewScrollAxes::ALL, ViewScrollAxes::name);
    check(ViewRevealAlign::ALL, ViewRevealAlign::name);
    check(ViewAnimation::ALL, ViewAnimation::name);
    check(ViewSeverity::ALL, ViewSeverity::name);
    check(ViewLabelSide::ALL, ViewLabelSide::name);
    check(ViewTooltipSide::ALL, ViewTooltipSide::name);
    check(ViewHintPlacement::ALL, ViewHintPlacement::name);
    check(ViewNfGlyph::ALL, ViewNfGlyph::name);
    check(ViewDisplay::ALL, ViewDisplay::name);
    check(ViewSpacing::ALL, ViewSpacing::name);
    check(ViewMarker::ALL, ViewMarker::name);
    check(ViewTextAlign::ALL, ViewTextAlign::name);
    check(ViewGlyph::ALL, ViewGlyph::name);
    check(ViewEvent::ALL, ViewEvent::name);
}

/// A fixed value converts into a property as its name, and **not** as a new `PropValue`
/// variant.
///
/// The wire format stays open on purpose: a fixed set travels as text, so the next one added
/// needs no change to `PropValue` at all. `Size`/`Variant`/`Align` predate that rule. A glyph
/// is the exception that proves it — it uses the `Glyph` variant, which already existed and
/// says what the string is.
#[test]
fn a_value_set_travels_as_text() {
    assert_eq!(
        PropValue::from(ViewOrientation::Vertical),
        PropValue::Text("vertical".into()),
    );
    assert_eq!(
        PropValue::from(ViewSeverity::Danger),
        PropValue::Text("danger".into())
    );
    assert_eq!(
        PropValue::from(ViewGlyph::GitBranch),
        PropValue::Glyph("git_branch".into()),
    );
}

/// A menu entry made from an id alone is the entry whose behaviour is that same id — one
/// constructor, so a new field's default is written once and both spellings start the same.
#[test]
fn a_menu_entry_from_an_id_is_one_that_runs_that_id() {
    assert_eq!(
        DropdownItem::new("zoom_column", "Zoom"),
        DropdownItem::with_intent("zoom_column", "Zoom", Intent::new("zoom_column")),
    );
}

/// A spacing written as a word reads through the one list of step names, so every step a
/// description can name is a step a builder can be given by name — and the other way round.
#[test]
fn every_spacing_step_is_readable_by_its_own_name() {
    for step in ViewSpacing::ALL {
        assert_eq!(
            step.name().parse::<ViewSpace>(),
            Ok(ViewSpace::Step(*step)),
            "`{}` is a step name but does not read back as one",
            step.name(),
        );
    }
}

/// A length reads with or without its unit, and anything else says so when asked — and costs
/// its author a gap, not the host, when it arrives through `From`.
#[test]
fn an_unreadable_spacing_is_reported_when_asked_and_is_no_space_otherwise() {
    assert_eq!("8".parse::<ViewSpace>(), Ok(ViewSpace::Px(8.0)));
    assert_eq!(" 8px ".parse::<ViewSpace>(), Ok(ViewSpace::Px(8.0)));
    assert_eq!(
        "SM".parse::<ViewSpace>(),
        Ok(ViewSpace::Step(ViewSpacing::Sm))
    );
    assert!("sm ".parse::<ViewSpace>().is_ok());
    assert!("smm".parse::<ViewSpace>().is_err());
    assert_eq!(ViewSpace::from("smm"), ViewSpace::Px(0.0));
}

/// An event is named by one closed list, and that list is what a node binds and reads by — so a
/// renamed or added event is one edit, not a search for string literals in three crates.
#[test]
fn an_event_is_bound_and_read_by_the_one_vocabulary() {
    let node = ViewNode::new(WidgetKind::Row).on(ViewEvent::Press, Intent::new("open"));

    assert_eq!(
        node.intent(ViewEvent::Press).map(|i| i.action.as_str()),
        Some("open")
    );
    assert!(node.is_actionable());
    assert!(
        node.events.contains_key(ViewEvent::Press.name()),
        "the wire key is the event's name",
    );
    assert_eq!(
        ViewEvent::ALL.len(),
        8,
        "press, hint, change, toggle, dismiss, action, activate, move"
    );
}
