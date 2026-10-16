//! Capabilities every kind carries.

use heca_view::WidgetKind;

use super::*;

/// **Every universal capability is reachable from the typed SDK**.
///
/// [`every_widget_property_is_reachable_from_the_sdk`] walks each *kind's* generated
/// `PROP_NAMES`, and that is the whole of its reach. A capability that belongs to no particular
/// widget — `tooltip`, `hintable`, the ones that come next — lives on a builder trait that
/// every widget gets (`TooltipExt`, `HintExt`, …), a plain trait with no generated property
/// surface, so it appears in no kind's `PROP_NAMES` and that guard cannot see it. `tooltip` was
/// undeclarable for exactly as long as that was true, with nothing failing anywhere.
///
/// So this asks the other half of the question, and **fails closed** in the same way: every
/// `#[prop]` builder on a trait implemented for every component must have a setter of the same
/// name in the SDK, or be named below with a reason. It reads both sources rather than calling
/// them, because *"does a method exist"* is not a question a running test can ask.
#[test]
fn every_universal_capability_is_reachable_from_the_sdk() {
    /// Universal builders with no SDK spelling, each with the reason. An entry here is a
    /// capability a described tree cannot use — keep it empty if you can.
    const NOT_IN_SDK: &[(&str, &str)] = &[];

    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../heca-grid-ui/src/builders");
    let sources: Vec<String> = std::fs::read_dir(&dir)
        .expect("the native builders are where they are expected")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "rs").then(|| std::fs::read_to_string(path).ok())?
        })
        .collect();
    let sdk = sdk_source();

    // A trait implemented for every component is where a capability lands when it belongs to
    // every widget rather than to one, so those traits are the list this guard is about.
    let mut universal: Vec<String> = Vec::new();
    for native in &sources {
        let Some(name) = native
            .split("\npub trait ")
            .skip(1)
            .filter_map(|after| after.split(':').next())
            .find(|name| {
                native.contains(&format!("impl<T: Component + Sized> {name} for T {{}}"))
            })
        else {
            continue;
        };
        let head = format!("\npub trait {name}: Component + Sized {{\n");
        let Some(start) = native.find(&head).map(|at| at + head.len()) else {
            continue;
        };
        let body = &native[start..start + native[start..].find("\n}\n").unwrap_or(0)];
        universal.extend(
            body.split("#[heca_grid_ui_macros::prop]")
                .skip(1)
                .filter_map(|after| {
                    let f = after.find("fn ")? + 3;
                    let rest = &after[f..];
                    Some(rest[..rest.find(['(', '<'])?].to_string())
                }),
        );
    }
    assert!(
        !universal.is_empty(),
        "no `#[prop]` builders found on the universal builder traits — this guard has \
         stopped reading anything, which is worse than the gap it exists for",
    );

    let missing: Vec<&String> = universal
        .iter()
        .filter(|name| !NOT_IN_SDK.iter().any(|(n, _)| n == name))
        .filter(|name| !sdk.contains(&format!("fn {name}(")))
        .collect();
    assert!(
        missing.is_empty(),
        "these capabilities are on every widget natively and cannot be said in a description at \
         all, so a plugin can draw a row and never give it what heca's own rows have: {missing:#?}",
    );
}

/// **Hiding the ink and collapsing the box are two things, and both are declarable**.
///
/// There is no `Visibility` widget kind and there must not be one, because the wrapper's whole
/// job is one of these two properties — and a wrapper cannot be put around a widget a typed
/// container holds, nor expressed in a description at all.
///
/// The pair is CSS's, deliberately: `hidden` is `display: none` (out of the layout, neighbours
/// close up) and `visible` is `visibility: hidden` (ink gone, box kept). Confusing them is the
/// whole trap — a row of four status slots showing one at a time stays still only under the
/// second, and `hidden` was already reachable while `visible` was reachable from neither
/// authoring path, which is exactly the sort of half-capability that reads as working.
///
/// So this asks both, on every kind, and asks that each leaves the *other* alone.
#[test]
fn hiding_the_ink_and_collapsing_the_box_are_separately_declarable_on_any_kind() {
    use heca_grid_ui::reactive::SignalGet as _;

    let mut wrong: Vec<String> = Vec::new();
    for &kind in WidgetKind::ALL {
        let (emit, _fired) = recording_emitter();
        let realized = |node: &ViewNode| {
            realize(node, &Theme::default(), &emit, &mut FormBindings::default())
        };

        // `visible: false` — the ink goes, the box stays.
        let w = realized(&sample_node(kind).prop("visible", PropValue::Bool(false)));
        if w.base().visible.get_untracked() {
            wrong.push(format!("{kind:?} (`visible: false`, still draws)"));
        }
        if w.base().is_hidden() {
            wrong.push(format!("{kind:?} (`visible` collapsed the box)"));
        }

        // `hidden: true` — out of the layout entirely, and the ink flag untouched.
        let h = realized(&sample_node(kind).prop("hidden", PropValue::Bool(true)));
        if !h.base().is_hidden() {
            wrong.push(format!("{kind:?} (`hidden`, still takes space)"));
        }
        if !h.base().visible.get_untracked() {
            wrong.push(format!("{kind:?} (`hidden` cleared `visible`)"));
        }

        // Nothing declared changes neither.
        let plain = realized(&sample_node(kind));
        if !plain.base().visible.get_untracked() || plain.base().is_hidden() {
            wrong.push(format!("{kind:?} (declared neither, not shown)"));
        }
    }
    assert!(
        wrong.is_empty(),
        "a described node cannot say what it shows on these kinds, or the two ways of not \
         showing have been collapsed into one: {wrong:#?}",
    );
}

/// **A described action row is the same row heca's own chrome uses**.
///
/// The pane header's action row is a `ButtonGroup`, and a whole session went into making it
/// behave: words become icons as the room runs out, and whatever still does not fit collapses
/// into a ⋮ that runs the same actions. A plugin could not ask for any of it — it would have
/// hand-built a row of buttons and got the collapse, the overflow menu and the hover words
/// approximately right, which is the second path this project forbids.
///
/// What it asks is that the **buttons are real buttons**: the group reads a label, a glyph and
/// a click out of each child to build that menu, so a described action must arrive with all
/// three or the menu row it produces is blank and does nothing. Building them through anything
/// but the one button path is how that would quietly happen.
#[test]
fn a_described_action_row_carries_its_actions_into_the_overflow_menu() {
    let (emit, fired) = recording_emitter();
    let action = |name: &str, icon: &str| {
        ViewNode::new(WidgetKind::Button)
            .text(name)
            .prop("icon", PropValue::Glyph(icon.into()))
            .on_press(Intent::new(format!("pane.{}", name.to_lowercase())))
    };
    let node = ViewNode::new(WidgetKind::ButtonGroup)
        .prop("display", PropValue::Text("icon_only".into()))
        .child(action("Close", "close"))
        .child(action("Split", "square_split_horizontal"))
        // Not a button: the group has no label, glyph or click to read out of it, so it is
        // skipped rather than wrapped into an action that does nothing.
        .child(ViewNode::new(WidgetKind::Label).text("not an action"));

    let mut group = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    );
    assert_eq!(
        group.base().children.len(),
        3,
        "two actions and the ⋮ the group builds for itself — the label is not an action",
    );

    // **Asked through the picker**, not by reaching into the group. The group wraps each button
    // in its own arrangement, so an index into its children is a fact about today's internals;
    // `prefix+/` is the contract, and it is also how a user reaches a collapsed action.
    let targets = hints(group.as_ref());
    assert_eq!(
        targets.len(),
        3,
        "each described action is pickable in its own right, plus the ⋮ that reaches the ones \
         that did not fit — and the label, which is not an action, is not among them",
    );
    assert!(heca_grid_ui::fire_hint(group.as_mut(), &targets[0]));
    assert_eq!(
        fired.borrow().first().map(|i| i.action.clone()),
        Some("pane.close".to_string()),
        "a grouped action runs the intent the description gave it — which is what makes its \
         overflow row run the plugin's own action rather than nothing",
    );
}

/// **Where a letter sits is declarable on ANY kind**.
///
/// `Base::hint_style` was universal from the day the picker stopped drawing every cap the same
/// way — but the only builders that wrote it were on [`KeyHint`], the wrapper. So moving a
/// letter meant wrapping the widget, which is the wrapper rule living in every caller's
/// discipline, is impossible on a widget a typed container holds, and could not be said in a
/// description at all. A plugin could make its row pickable and then had to accept whatever
/// position heca chose for it.
///
/// **Placement is not what makes a node pickable** — anything actionable already wears a letter
/// with nothing declared — so this asks only that the four knobs arrive, on every kind.
#[test]
fn where_a_letter_sits_is_declarable_on_any_kind() {
    let mut missing: Vec<String> = Vec::new();
    for &kind in WidgetKind::ALL {
        let (emit, _fired) = recording_emitter();
        let node = sample_node(kind)
            .prop("hint_placement", PropValue::Text("center_right".into()))
            .prop("hint_size", PropValue::Int(18))
            .prop("hint_offset_y", PropValue::Float(4.0))
            .prop("hint_color", PropValue::Color("danger".into()));
        let theme = Theme::default();
        let widget = realize(&node, &theme, &emit, &mut FormBindings::default());
        let style = widget.base().hint_style;
        if style.placement != heca_grid_ui::widgets::HintPlacement::CenterRight {
            missing.push(format!("{kind:?} (placement ignored)"));
        }
        // An integer size is the same number of pixels as a fraction to whoever wrote it.
        if style.size != Some(18.0) {
            missing.push(format!("{kind:?} (size ignored: {:?})", style.size));
        }
        if (style.offset_y - 4.0).abs() > f64::EPSILON {
            missing.push(format!("{kind:?} (offset ignored: {})", style.offset_y));
        }
        // A **token name**, resolved against the live theme — never a hex literal written into
        // the description, so a letter follows a theme change with nothing rewritten.
        let danger: Option<heca_grid_ui::Color> =
            resolve_color("danger", &theme).and_then(|hex| hex.parse().ok());
        if style.color != danger {
            missing.push(format!(
                "{kind:?} (colour token unresolved: {:?})",
                style.color
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "a described node cannot say where its letter goes on these kinds, while a native one \
         can: {missing:#?}",
    );
}
