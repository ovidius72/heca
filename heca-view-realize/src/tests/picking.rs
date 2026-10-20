//! Picking, verbs and menus declared on any kind.

use heca_view::WidgetKind;

use super::*;

/// **What a plugin actually writes** — the SDK, end to end.
///
/// A plugin cannot hand us a Rust function, so without a declarative spelling it can draw a row
/// and never make that row pickable. The JSON is not a convenience, it is the feature.
///
/// The kind here is a `Label` on purpose: nothing about it is clickable, `realize` wires no
/// `press` for it, and it is still a perfectly good thing to point at — which is why a hint is
/// enough on its own to make a node a target.
#[test]
fn a_plugin_can_make_anything_pickable_from_the_sdk() {
    use heca_view::build::{self, Style as _};

    let (emit, fired) = recording_emitter();
    let node: ViewNode = build::Label::new("nginx")
        .on_hint(Intent::new("docker.reveal").arg("id", PropValue::Text("abc".into())))
        .into();

    let mut widget = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    );
    assert_eq!(
        hints(widget.as_ref()),
        vec![Vec::<usize>::new()],
        "the label is a target"
    );
    assert!(heca_grid_ui::fire_hint(widget.as_mut(), &[]));
    assert_eq!(
        fired.borrow()[0].args.get("id"),
        Some(&PropValue::Text("abc".into())),
        "and picking it fires the plugin's own intent, arguments and all",
    );
}

/// **Nothing paints outside the box it was given** — every kind, at every width.
///
/// The rule `components/mod.rs` already states for anything sized by its container — *lay it out
/// in a box and assert it never exceeds it* — asked of the **whole vocabulary** rather than one
/// composition at a time. It walks `WidgetKind::ALL`, so a kind added next month is covered
/// with nobody remembering, and a plugin's tree is covered by construction: it is built from
/// these kinds.
///
/// **The clip stack and the overlay band are the framework's answer**, not this test's:
/// `Scene::draws_outside` honours the clips in force and reads the base layer only, because
/// the overlay band exists precisely for what must escape its box — a dropdown opened inside a
/// scroll region, a hint keycap on a half-visible row. This sweep and the showcase catalog's
/// asked the same question with two copies of that arithmetic, and both copies were wrong the
/// same way.
///
/// The failure it exists for is invisible to every "was this drawn?" assertion: a name that is
/// drawn *somewhere*, across its neighbour.
#[test]
fn no_kind_paints_outside_the_box_it_is_given() {
    use heca_core::layout::{Point, Rectangle, Size};
    use heca_grid_ui::{LayoutEngine, PaintCx, Scene, Theme};

    let theme = Theme::default();
    let mut escapes: Vec<String> = Vec::new();
    for &kind in WidgetKind::ALL {
        // Down to 32px. **A box is never narrower than its own padding** — that is the box
        // model, not a defect: a card padded 13 a side has a 26px floor, and asking it to fit
        // in 24 is asking it to have no box at all. Every escape this sweep exists for still
        // shows at 32 — all five of the ones it was carrying did — so the floor costs it
        // nothing.
        for box_w in [400.0f64, 120.0, 60.0, 32.0] {
            // A parent that hands it a definite width: a kind sized as a share has nothing to
            // be a share *of* at the root of a layout.
            let node = ViewNode::new(WidgetKind::VStack).child(sample_node(kind));
            let mut root =
                realize(&node, &theme, &noop_emitter(), &mut FormBindings::default());
            root.base_mut().style.layout.width = heca_grid_ui::Length::Px(box_w as f32);
            LayoutEngine::new().compute(root.as_mut(), Size::new(box_w, 200.0));

            // **Do not ask a box to hold a control in less room than a control needs.** A card
            // pads itself 13 a side, so at 32px it has six pixels of content space — narrower
            // than the smallest thing that can stand in it (a glyph plus its padding). Below
            // that, "keep your content inside" is not a defect report, it is the box model:
            // something has to be cut, and which one is a design decision, not this sweep's.
            // The kinds that escape all do so at widths where they *did* have room.
            // The kind itself, not the wrapper that hands it a definite width.
            let pad = root.base().children.first().map_or(0.0, |kind| {
                kind.base().style.layout.padding.resolve(kind.base().font) as f64
            });
            let one_control = (theme.font_size * 2.0) as f64;
            if box_w - 2.0 * pad < one_control {
                continue;
            }

            let mut scene = Scene::new();
            {
                let mut cx = PaintCx::new(&mut scene, &theme);
                root.paint(&mut cx);
            }
            let box_ = Rectangle::new(Point::default(), Size::new(box_w, 200.0));
            if let Some(escape) = scene.draws_outside(box_).first() {
                escapes.push(format!("{kind:?} at {box_w}px: {escape}"));
            }
        }
    }
    assert!(
        escapes.is_empty(),
        "these draw outside their box:\n{}",
        escapes.join("\n")
    );
}

/// **A widget with content never renders as nothing** — every kind, with room to spare.
///
/// The other half, and the one that caught the first attempt at fixing the first: a label that
/// may shrink to zero *does*, in a parent that sizes to its minimum, and a `Card`'s title
/// vanished outright. "Nothing drawn" is a worse answer than "drawn too wide", and no test
/// anywhere asserted against it.
#[test]
fn no_kind_with_content_renders_as_nothing() {
    use heca_core::layout::Size;
    use heca_grid_ui::{LayoutEngine, PaintCx, Scene, Theme};

    let theme = Theme::default();
    let mut silent: Vec<String> = Vec::new();
    for &kind in WidgetKind::ALL {
        if kind == WidgetKind::ScrollBar {
            continue; // host-only: `realize` refuses it outright, by design
        }
        if kind == WidgetKind::Overlay {
            // A **closed** surface draws nothing, and a described `Overlay` starts closed
            // unless it says `opened`. That is the widget working, not a silent one.
            continue;
        }
        let node = ViewNode::new(WidgetKind::VStack).child(sample_node(kind));
        let mut root = realize(&node, &theme, &noop_emitter(), &mut FormBindings::default());
        root.base_mut().style.layout.width = heca_grid_ui::Length::Px(400.0);
        LayoutEngine::new().compute(root.as_mut(), Size::new(400.0, 200.0));

        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            root.paint(&mut cx);
        }
        if scene.is_empty() {
            silent.push(format!("{kind:?}"));
        }
    }
    assert!(
        silent.is_empty(),
        "these draw nothing at all despite having content:\n{}",
        silent.join("\n"),
    );
}

/// **What a plugin actually writes to own a picker** — the SDK, end to end.
///
/// A plugin can be *picked*; this is the other half — opening a picker of its own over its own
/// panel, as heca's exposé does. Everything
/// here is a string in the plugin's own tree plus a key in the user's own config: no registry,
/// no id to hold, no signal, no closure.
#[test]
fn a_plugin_can_own_a_picker_from_the_sdk() {
    use heca_view::build;
    use heca_view::build::{Parent as _, Style as _};

    let (emit, fired) = recording_emitter();
    let node: ViewNode = build::KeyHintGroup::new()
        .opens_on("mypanel.pick")
        .child(build::Row::new().on_hint(Intent::new("docker.restart")))
        .into();

    let mut picker = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    );

    // 1. The verb is on screen, so the host's `[[keys.surface]] pick = "s"` can reach it —
    //    by name, with no path to go stale when the tree is rebuilt.
    assert_eq!(
        heca_grid_ui::collect_actions(picker.as_ref()),
        vec!["mypanel.pick".to_string()],
    );
    assert!(
        !heca_grid_ui::fire_action(picker.as_ref(), "heca.expose.pick"),
        "its own name only"
    );

    // 2. Running it opens the picker and letters what is beneath it.
    assert!(heca_grid_ui::fire_action(picker.as_ref(), "mypanel.pick"));
    picker.tick(0.0);
    let row = &picker.base().children[0];
    assert_eq!(
        row.base().hint_label.get_untracked().as_deref(),
        Some("a"),
        "the plugin's own row wears the letter, and draws it itself",
    );

    // 3. And the letter runs the row's own intent, back out to the plugin.
    picker.on_event_capture(&heca_grid_ui::Event::TextInput("a".to_string()));
    assert_eq!(
        fired
            .borrow()
            .iter()
            .map(|i| i.action.clone())
            .collect::<Vec<_>>(),
        vec!["docker.restart"],
    );
}

/// **A described node declares verbs by name**, on any kind.
///
/// `EventExt::on_action` is universal natively, so this is read once for every kind rather
/// than wired per arm. It is the seam a **surface** has and a dock got from `Provider::actions`:
/// without it a described panel can only bind verbs the app already compiled in.
#[test]
fn a_described_node_answers_to_the_verb_it_declares() {
    use heca_view::build;
    use heca_view::build::Style as _;

    let (emit, fired) = recording_emitter();
    let node: ViewNode = build::Panel::new()
        .on_action("mypanel.reload", Intent::new("docker.refresh"))
        .into();

    let panel = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    );
    assert!(heca_grid_ui::fire_action(panel.as_ref(), "mypanel.reload"));
    assert_eq!(
        fired
            .borrow()
            .iter()
            .map(|i| i.action.clone())
            .collect::<Vec<_>>(),
        vec!["docker.refresh"],
        "the verb the surface named fired the intent it was bound to",
    );
}

/// **A verb that names itself never runs.** The router answers a name by looking for a widget
/// on screen declaring it, so `"mypanel.pick" -> Intent("mypanel.pick")` would find this widget
/// again and re-post itself forever — a description spinning the event loop.
///
/// Refused where both names are known, which is here. `realize` is total for untrusted input:
/// the verb is simply not declared, and resolves to nothing.
#[test]
fn a_verb_that_fires_its_own_name_is_refused() {
    use heca_view::build;
    use heca_view::build::Style as _;

    let node: ViewNode = build::Panel::new()
        .on_action("mypanel.reload", Intent::new("mypanel.reload"))
        .into();

    let panel = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    assert!(
        heca_grid_ui::collect_actions(panel.as_ref()).is_empty(),
        "a self-naming verb is not declared at all, so nothing can reach it",
    );
}

/// **The pick rule holds for a described tree too**: a pick acts on the widget it named and on
/// nothing else, so a plugin's nested pickable rows behave exactly like heca's own.
///
/// Without it a plugin composing a pickable card out of pickable rows would fire both and land
/// the user on the card — and would have to hand-write the DOM's `e.target !== e.currentTarget`
/// guard, which it has no way to express at all.
#[test]
fn a_described_pick_lands_on_the_node_it_named_and_not_its_container() {
    use heca_view::build;

    let (emit, fired) = recording_emitter();
    use heca_view::build::{Parent as _, Style as _};
    let node: ViewNode = build::Card::new("Containers")
        .on_hint(Intent::new("the_card"))
        .child(build::Label::new("row").on_hint(Intent::new("the_row")))
        .into();

    let mut widget = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    );
    let inner = hints(widget.as_ref())
        .into_iter()
        .find(|p| !p.is_empty())
        .expect("the nested label is its own target");
    assert!(heca_grid_ui::fire_hint(widget.as_mut(), &inner));
    assert_eq!(
        fired
            .borrow()
            .iter()
            .map(|i| i.action.clone())
            .collect::<Vec<_>>(),
        vec!["the_row"],
        "the card it sits in must not answer for it",
    );
}

/// **A menu declared on ANY kind reaches the node itself**.
///
/// A menu is a declaration, not a widget an author assembles — natively it is one builder on
/// any widget, so the described form is one field on any node. Until this, a plugin building
/// its UI as a described tree could not attach a menu to its own row at all, while heca's own
/// rows did it in a line: a second-class version of a shipped feature, which ⭐⭐ RULE ZERO
/// exists to forbid.
///
/// It asserts the declaration lands **on the node**, never on a wrapper around it: a wrapper is
/// a widget the description never asked for, and the right-click walk would find it instead of
/// the row.
#[test]
fn a_menu_declared_on_any_kind_lands_on_the_node_itself() {
    let mut missing: Vec<String> = Vec::new();
    for &kind in WidgetKind::ALL {
        let (emit, _fired) = recording_emitter();
        let node = sample_node(kind).menu([heca_view::DropdownItem::with_intent(
            "close",
            "Close",
            Intent::new("plugin.close"),
        )]);
        let widget = realize(
            &node,
            &Theme::default(),
            &emit,
            &mut FormBindings::default(),
        );
        if widget.base().context_menu.is_none() {
            missing.push(format!("{kind:?}"));
        }
    }
    assert!(
        missing.is_empty(),
        "a menu declared on these kinds never reached the widget, so a described row cannot \
         have one while a native row can: {missing:#?}",
    );
}
