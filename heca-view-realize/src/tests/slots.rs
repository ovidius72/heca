//! Named child slots, and the toast.

use heca_view::WidgetKind;

use crate::vocab::severity_prop;
use super::*;

// ── Named child slots ──

/// A child says **where it goes** with a `slot` prop, so a widget with several places for
/// children needs no change to `ViewNode`'s shape: `children` stays one flat vector.
///
/// `DockFrame` has a **default** slot (the body), so an unslotted child lands there.
#[test]
fn dock_frame_routes_its_header_slot_and_defaults_the_rest_to_the_body() {
    let node = ViewNode::new(WidgetKind::DockFrame)
        .text("EXPLORER")
        .prop("frameless", PropValue::Bool(true))
        .child(
            ViewNode::new(WidgetKind::Badge)
                .text("3")
                .prop("slot", PropValue::Text("header".into())),
        )
        .child(ViewNode::new(WidgetKind::Item).text("src"))
        .child(ViewNode::new(WidgetKind::Item).text("tests"));

    let dock = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    // The widget's own shape: children[0] = header row [toggle, CONTROLS], children[1] = body.
    let header_controls = &dock.base().children[0].base().children[1];
    assert_eq!(
        header_controls.text_summary().as_deref(),
        Some("3"),
        "the slot=\"header\" child fills the controls slot",
    );
    assert_eq!(
        dock.base().children[1].base().children.len(),
        2,
        "the unslotted children are body content (the body is the default slot)",
    );
}

/// `Item` has **no default slot** — its middle is the label, which comes from `text` — so a child
/// naming no slot (or an unknown one) is ignored rather than dropped somewhere it doesn't belong.
/// Either way: no panic. Realize stays total for untrusted input.
/// A described tile places each part by its `slot`, and a child with none becomes a line.
#[test]
fn tile_routes_each_part_by_its_slot_and_the_rest_become_lines() {
    let slot = |s: &str| PropValue::Text(s.into());
    let node = ViewNode::new(WidgetKind::Tile)
        .child(
            ViewNode::new(WidgetKind::Label)
                .text("T")
                .prop("slot", slot("title")),
        )
        .child(ViewNode::new(WidgetKind::Label).text("line one"))
        .child(ViewNode::new(WidgetKind::StatusDot).prop("slot", slot("status")))
        .child(ViewNode::new(WidgetKind::Label).text("line two"));
    let w = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    let parts = &w.base().children;
    assert_eq!(parts.len(), 3, "the head and two lines");
    assert_eq!(parts[1].text_summary().as_deref(), Some("line one"));
    assert_eq!(parts[2].text_summary().as_deref(), Some("line two"));
    let head = &parts[0].base().children;
    assert!(
        !head[0].base().style.layout.hidden,
        "the status slot is filled"
    );
    assert!(
        head[1].base().style.layout.hidden,
        "no icon was given, so none is shown"
    );
    assert_eq!(
        head[2].text_summary().as_deref(),
        Some("T"),
        "the title sits in the text run"
    );
}

#[test]
fn item_routes_leading_and_trailing_slots_and_ignores_the_rest() {
    let node = ViewNode::new(WidgetKind::Item)
        .text("main.rs")
        .child(
            ViewNode::new(WidgetKind::StatusDot)
                .prop("slot", PropValue::Text("leading".into())),
        )
        .child(
            ViewNode::new(WidgetKind::Badge)
                .text("M")
                .prop("slot", PropValue::Text("trailing".into())),
        )
        .child(ViewNode::new(WidgetKind::Label).text("nowhere")) // no slot → ignored
        .child(
            ViewNode::new(WidgetKind::Label)
                .text("also nowhere")
                .prop("slot", PropValue::Text("bogus".into())), // unknown → ignored
        );

    let item = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    // The widget's own shape: [LEADING, LABEL, TRAILING].
    let slots = &item.base().children;
    assert_eq!(
        slots.len(),
        3,
        "the row keeps its three slots — nothing appended"
    );
    assert_eq!(
        slots[2].text_summary().as_deref(),
        Some("M"),
        "the trailing slot holds the badge",
    );
    assert_eq!(
        slots[1].text_summary().as_deref(),
        Some("main.rs"),
        "the label is still the middle — an unslotted child did not overwrite it",
    );
}

/// The card's props and its three intents still work — the plain-data path a `ToastSpec` uses.
#[test]
fn toast_node_realizes_its_props_and_three_intents() {
    use std::cell::RefCell;

    let fired: Rc<RefCell<Vec<Intent>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = fired.clone();
    let emit: IntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

    let node = ViewNode::new(WidgetKind::Toast)
        .text("Build failed")
        .prop("severity", PropValue::Text("danger".into()))
        .prop(
            "body_text",
            PropValue::Text("3 errors in heca-grid-ui".into()),
        )
        .prop("action_text", PropValue::Text("RETRY".into()))
        .on("action", Intent::new("rebuild"))
        .on("dismiss", Intent::new("close_toast"));

    let toast = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    );
    assert_eq!(toast.base().children.len(), 3, "icon, text column, dismiss");

    // An unknown severity degrades to the widget's default rather than erroring.
    let bogus = ViewNode::new(WidgetKind::Toast)
        .text("x")
        .prop("severity", PropValue::Text("catastrophic".into()));
    assert_eq!(severity_prop(&bogus), heca_grid_ui::ToastSeverity::Info);
    assert_eq!(severity_prop(&node), heca_grid_ui::ToastSeverity::Danger);
}

/// The card's text column: `[title, body, actions]`.
fn toast_column(toast: &dyn Component) -> &[Box<dyn Component>] {
    &toast.base().children[1].base().children
}

/// **A described toast has slots again** — `body` (the default) and `actions`.
///
/// The card once had no slots, because it hand-drew itself and could not hold arbitrary
/// content. It now lays out real children. An unslotted child is the body, so the common case needs no slot name.
#[test]
fn a_described_toast_takes_a_body_and_several_actions() {
    use heca_core::layout::Point;
    use heca_grid_ui::Event;
    use std::cell::RefCell;

    let fired: Rc<RefCell<Vec<Intent>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = fired.clone();
    let emit: IntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

    let node = ViewNode::new(WidgetKind::Toast)
        .text("Build failed")
        // No slot named: the body is the DEFAULT slot.
        .child(ViewNode::new(WidgetKind::Label).text("3 errors"))
        .child(
            ViewNode::new(WidgetKind::Button)
                .text("Retry")
                .prop("slot", PropValue::Text("actions".into()))
                .on_press(Intent::new("rebuild")),
        )
        .child(
            ViewNode::new(WidgetKind::Button)
                .text("View log")
                .prop("slot", PropValue::Text("actions".into()))
                .on_press(Intent::new("open_log")),
        );

    let toast = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    );
    let column = toast_column(toast.as_ref());
    assert_eq!(
        column[1].text_summary().as_deref(),
        Some("3 errors"),
        "an unslotted child is the body",
    );
    let actions = &column[2].base().children;
    assert_eq!(actions.len(), 2, "two described actions, two controls");

    // Each action fires its OWN intent — a described action is an ordinary described Button,
    // so a real click on it is the whole of the wiring.
    for (i, want) in ["rebuild", "open_log"].iter().enumerate() {
        fired.borrow_mut().clear();
        let mut a = realize(
            &node.children[i + 1],
            &Theme::default(),
            &emit,
            &mut FormBindings::default(),
        );
        heca_grid_ui::LayoutEngine::new()
            .base_font(14.0)
            .compute(a.as_mut(), heca_core::layout::Size::new(200.0, 60.0));
        let b = a.base().bounds;
        let at = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
        heca_grid_ui::dispatch(
            a.as_mut(),
            &Event::pointer_pressed(at, heca_grid_ui::PointerButton::Left),
        );
        heca_grid_ui::dispatch(
            a.as_mut(),
            &Event::pointer_released(at, heca_grid_ui::PointerButton::Left),
        );
        assert_eq!(
            fired.borrow().first().map(|i| i.action.as_str()),
            Some(*want),
            "action {i} fired the wrong intent",
        );
    }
}

/// **Children win over the text sugar** — one content model, two spellings, the way a
/// `Button`'s children win over its `text`/`icon`.
#[test]
fn a_described_toasts_children_win_over_its_text_props() {
    let node = ViewNode::new(WidgetKind::Toast)
        .text("Build failed")
        .prop("body_text", PropValue::Text("the sugar body".into()))
        .prop("action_text", PropValue::Text("SUGAR".into()))
        .on("action", Intent::new("rebuild"))
        .child(ViewNode::new(WidgetKind::Label).text("the composed body"))
        .child(
            ViewNode::new(WidgetKind::Button)
                .text("Composed")
                .prop("slot", PropValue::Text("actions".into()))
                .on_press(Intent::new("composed")),
        );

    let toast = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    let column = toast_column(toast.as_ref());
    assert_eq!(
        column[1].text_summary().as_deref(),
        Some("the composed body"),
        "the text sugar overwrote a composed body",
    );
    let actions = &column[2].base().children;
    assert_eq!(
        actions.len(),
        1,
        "action_text built a second control beside the composed one"
    );
    assert_eq!(actions[0].text_summary().as_deref(), Some("Composed"));
}

/// **An unknown slot name is logged and falls back to the body** — `realize` is total for
/// untrusted input, so a plugin's typo costs it a misplaced child, never a panic.
#[test]
fn a_described_toasts_unknown_slot_falls_back_to_the_body() {
    let node = ViewNode::new(WidgetKind::Toast).text("t").child(
        ViewNode::new(WidgetKind::Label)
            .text("typo")
            .prop("slot", PropValue::Text("bodyy".into())),
    );
    let toast = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    assert_eq!(
        toast_column(toast.as_ref())[1].text_summary().as_deref(),
        Some("typo"),
        "an unknown slot should fall back to the body, not vanish",
    );
}
