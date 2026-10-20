//! Spacing from the theme, accents, and tooltips.

use heca_view::WidgetKind;

use super::*;

/// **A described tree can space itself from the theme, without a pixel**.
///
/// The library has had semantic spacing all along — steps resolved from the inherited font at
/// layout, so padding follows a font or theme change with nothing rewritten. The **described**
/// side had only raw px, so a plugin author had no way *not* to hardcode: the rule this project
/// states everywhere was one a plugin could not keep.
///
/// It asks the steps arrive as steps, not as numbers frozen at authoring time.
#[test]
fn a_described_tree_spaces_itself_from_the_theme() {
    use heca_grid_ui::style::Spacing;
    use heca_view::ViewSpacing;
    use heca_view::build::{self, Style as _};

    let (emit, _fired) = recording_emitter();
    let node: ViewNode = build::Surface::new()
        .padding(ViewSpacing::Md)
        .gap(ViewSpacing::Xs)
        .into();
    let w = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    );
    let layout = &w.base().style.layout;
    use heca_grid_ui::style::Space;
    assert_eq!(
        layout.padding,
        Space::Step(Spacing::Md),
        "padding is a step"
    );
    assert_eq!(layout.gap, Space::Step(Spacing::Xs), "and so is the gap");
}

/// **A tree written against the old property names still works.**
///
/// Spacing was two settings — a px `gap` beside a `gap_spacing` step — and is now one that
/// takes either. The described vocabulary IS the layout field list, with no table of allowed
/// names, so a renamed field stops naming anything and would be dropped in silence: a plugin's
/// spacing would quietly vanish rather than fail. The retired spellings are mapped onto the
/// surviving one instead.
#[test]
fn the_retired_spacing_names_still_reach_the_setting_that_replaced_them() {
    use heca_grid_ui::style::{Space, Spacing};

    let node = ViewNode::new(WidgetKind::VStack)
        .prop("gap_spacing", PropValue::Text("xs".into()))
        .prop("pad_spacing_x", PropValue::Text("md".into()))
        .prop("pad_spacing_y", PropValue::Text("lg".into()));

    let w = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    let l = w.base().style.layout;
    assert_eq!(
        l.gap,
        Space::Step(Spacing::Xs),
        "gap_spacing still means the gap"
    );
    assert_eq!(l.padding_x, Some(Space::Step(Spacing::Md)));
    assert_eq!(l.padding_y, Some(Space::Step(Spacing::Lg)));
}

/// **The surviving name takes either spelling** — which is the whole point of merging them.
#[test]
fn one_described_name_reads_a_number_and_a_step() {
    use heca_grid_ui::style::{Space, Spacing};

    let of = |v: PropValue| {
        let node = ViewNode::new(WidgetKind::VStack).prop("gap", v);
        realize(
            &node,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        )
        .base()
        .style
        .layout
        .gap
    };
    assert_eq!(of(PropValue::Int(8)), Space::Px(8.0), "a number is pixels");
    assert_eq!(
        of(PropValue::Text("sm".into())),
        Space::Step(Spacing::Sm),
        "and a step name is a step — the thing the wire could not say before",
    );
    assert_eq!(
        of(PropValue::Text("nonsense".into())),
        Space::Px(0.0),
        "an unreadable value still costs only itself",
    );
}

/// **A described node overrides the accent for everything inside it.**
///
/// The accent is a style property, so it needs no arm here and no entry in any list — that is
/// the whole point of reading layout and appearance off the style types. This pins that it
/// actually arrives, and that a theme token name is resolved rather than passed through.
#[test]
fn a_described_node_publishes_an_accent_to_its_subtree() {
    let theme = Theme::default();
    let node = ViewNode::new(WidgetKind::Surface)
        .prop("accent", PropValue::Color("danger".into()))
        .child(ViewNode::new(WidgetKind::Button).text("Delete"));

    let w = realize(&node, &theme, &noop_emitter(), &mut FormBindings::default());
    assert_eq!(
        w.base().style.visual.accent,
        Some(theme.colors.danger),
        "the token was resolved against the theme the tree was built with",
    );
    assert_eq!(
        w.base().children[0].base().style.visual.accent,
        None,
        "the child says nothing — it inherits at paint, it is not stamped at build",
    );
}

/// **A tooltip declared on ANY kind reaches the node itself**.
///
/// A tooltip is a declaration, not a widget an author places. It stopped being a wrapper
/// natively for a concrete reason — a wrapper puts the rule in every caller's discipline, and
/// makes a tooltip impossible on a widget a typed container holds, because wrapping it changes
/// what it is — so it lives in a slot on every widget's base. The described form is therefore
/// one field on every node, and there is no `Tooltip` kind for a plugin to construct, size and
/// anchor (⭐⭐ RULE ZERO — one door, never two).
///
/// Until this, a plugin drawing its UI as a described tree could not put a tooltip on anything,
/// while heca's own chrome put one on every button in a line. Nothing failed: the shared
/// builder trait has no generated property surface, so neither the prop applier nor
/// [`every_widget_property_is_reachable_from_the_sdk`] can see a universal capability at all —
/// which is why this asks the question directly, for every kind.
#[test]
fn a_tooltip_declared_on_any_kind_lands_on_the_node_itself() {
    let mut missing: Vec<String> = Vec::new();
    for &kind in WidgetKind::ALL {
        let (emit, _fired) = recording_emitter();
        let node = sample_node(kind)
            .prop("tooltip", PropValue::Text("Close the pane".into()))
            .prop("tooltip_side", PropValue::Text("bottom".into()))
            .prop("tooltip_delay", PropValue::Int(2));
        let widget = realize(
            &node,
            &Theme::default(),
            &emit,
            &mut FormBindings::default(),
        );
        match &widget.base().tooltip {
            None => missing.push(format!("{kind:?} (no tooltip on the realized widget)")),
            Some(tip) => {
                if tip.side != heca_grid_ui::TooltipSide::Bottom {
                    missing.push(format!("{kind:?} (declared side ignored)"));
                }
                // An integer delay is the same number of seconds as a float one to whoever
                // wrote it, and JSON does not keep them apart.
                if (tip.delay - 2.0).abs() > f32::EPSILON {
                    missing.push(format!("{kind:?} (declared delay ignored: {})", tip.delay));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "a tooltip declared on these kinds never reached the widget, so a described row \
         cannot say what it is while every native one can: {missing:#?}",
    );
}

/// **A quick tooltip is a name, not a number — in a described tree too.** `tooltip_quick` lands
/// on every kind the way `tooltip_delay` does, so a plugin's row can say it the way heca's own
/// does; `Label::new("x").tooltip("full text").tooltip_quick(true)` and the described
/// `tooltip_quick(true)` give the same delay.
#[test]
fn a_quick_tooltip_declared_on_any_kind_takes_the_librarys_quick_delay() {
    let mut wrong: Vec<String> = Vec::new();
    for &kind in WidgetKind::ALL {
        let (emit, _fired) = recording_emitter();
        let node = sample_node(kind)
            .prop("tooltip", PropValue::Text("Close the pane".into()))
            .prop("tooltip_quick", PropValue::Bool(true));
        let widget = realize(
            &node,
            &Theme::default(),
            &emit,
            &mut FormBindings::default(),
        );
        match &widget.base().tooltip {
            Some(tip)
                if (tip.delay - heca_grid_ui::widgets::tooltip::QUICK_DELAY).abs()
                    < f32::EPSILON => {}
            Some(tip) => wrong.push(format!("{kind:?} (delay {})", tip.delay)),
            None => wrong.push(format!("{kind:?} (no tooltip)")),
        }
    }
    assert!(wrong.is_empty(), "tooltip_quick was ignored on: {wrong:#?}");
}
