//! Glyph names, fixed value sets, and values nested in objects.

use heca_grid_ui::{NfGlyph, ScrollRegion};
use heca_view::WidgetId;

use crate::style::with_props;
use super::*;

/// The mirrored glyph list cannot fall behind the real one.
///
/// `heca-view` must not depend on `heca-grid-ui` — that independence is why a plugin can depend
/// on the vocabulary at all — so the 52 icon names are copied into it. A copy of a list that
/// grows is the failure this codebase keeps repeating, so this crate, which sees both, is where
/// the copy is held to account. It compares **both ways** and names what is wrong: a glyph
/// added to the library and not mirrored, or a mirror naming something that no longer exists.
#[test]
fn every_glyph_name_has_a_mirror() {
    use std::collections::BTreeSet;
    let library: BTreeSet<&str> = <Glyph as heca_grid_ui::PropName>::VARIANT_NAMES
        .iter()
        .copied()
        .collect();
    let mirrored: BTreeSet<&str> = heca_view::ViewGlyph::ALL.iter().map(|g| g.name()).collect();

    let missing: Vec<_> = library.difference(&mirrored).collect();
    assert!(
        missing.is_empty(),
        "these glyphs exist in heca-grid-ui but not in ViewGlyph, so a description cannot name \
         them: {missing:?} — add them to the glyph list in heca-view/src/scalars/glyph.rs",
    );
    let stale: Vec<_> = mirrored.difference(&library).collect();
    assert!(
        stale.is_empty(),
        "ViewGlyph names glyphs the library no longer has: {stale:?}",
    );

    // **The keyboard font is a second vocabulary, held to the same account**.
    //
    // Checked here rather than in a test of its own: it is the same question about the same
    // kind of copied list, and two tests would be two places to remember when a third font
    // arrives.
    let nf_library: BTreeSet<&str> = <NfGlyph as heca_grid_ui::PropName>::VARIANT_NAMES
        .iter()
        .copied()
        .collect();
    let nf_mirrored: BTreeSet<&str> = heca_view::ViewNfGlyph::ALL
        .iter()
        .map(|g| g.name())
        .collect();
    let missing: Vec<_> = nf_library.difference(&nf_mirrored).collect();
    assert!(
        missing.is_empty(),
        "these keyboard glyphs exist in heca-grid-ui but not in ViewNfGlyph, so a description \
         cannot name them: {missing:?}",
    );
    let stale: Vec<_> = nf_mirrored.difference(&nf_library).collect();
    assert!(
        stale.is_empty(),
        "ViewNfGlyph names keyboard glyphs the library no longer has: {stale:?}",
    );
}

/// A mirrored glyph actually resolves to the icon it names.
///
/// Matching names is not the same as the name being understood: this takes one through
/// `PropValue` into a realized widget and asserts a different glyph paints differently.
#[test]
fn a_mirrored_glyph_reaches_the_icon() {
    let painted = |g: heca_view::ViewGlyph| {
        let node = ViewNode::new(WidgetId::Icon).prop("glyph", g.into());
        let w = realize(
            &node,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        ).expect("valid described test widget");
        let mut scene = heca_grid_ui::Scene::new();
        let theme = Theme::default();
        {
            let mut cx = heca_grid_ui::PaintCx::new(&mut scene, &theme);
            w.paint(&mut cx);
        }
        format!("{:?}", scene.iter().collect::<Vec<_>>())
    };
    let folder = painted(heca_view::ViewGlyph::Folder);
    assert!(!folder.is_empty(), "a named glyph painted something");
    assert_ne!(
        folder,
        painted(heca_view::ViewGlyph::Terminal),
        "the name picked the icon, rather than every name giving the same one",
    );

    // **And the keyboard font, the same way**. A shortcut drawn with the
    // wrong key is worse than one drawn with none — ⌘ where the author asked for ⇧ reads as
    // correct — and matching names cannot catch that, because the two lists agree by name while
    // the codepoints behind them are a separate mapping.
    let nf_painted = |g: heca_view::ViewNfGlyph| {
        let node = ViewNode::new(WidgetId::NfIcon).prop("glyph", g.into());
        let w = realize(
            &node,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        ).expect("valid described test widget");
        let mut scene = heca_grid_ui::Scene::new();
        let theme = Theme::default();
        {
            let mut cx = heca_grid_ui::PaintCx::new(&mut scene, &theme);
            w.paint(&mut cx);
        }
        format!("{:?}", scene.iter().collect::<Vec<_>>())
    };
    let shift = nf_painted(heca_view::ViewNfGlyph::Shift);
    assert!(!shift.is_empty(), "a named key painted something");
    assert_ne!(
        shift,
        nf_painted(heca_view::ViewNfGlyph::Command),
        "the name picked the key, rather than every name giving the same one",
    );
}

/// Every fixed value set reaches the widget it belongs to.
///
/// The fixed-value types stop a misspelling from compiling, but nothing about them
/// guarantees the *name* they travel under is one the widget parses — that agreement runs
/// across two crates and a serde attribute. So each is checked against a real realized widget
/// rather than against a parse: written through the type, it must change the widget.
///
/// `PropValue::from` is what the authoring layer calls, so this exercises the whole path.
#[test]
fn every_fixed_value_set_reaches_its_widget() {
    let realize_one = |node: &ViewNode| {
        realize(
            node,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        ).expect("valid described test widget")
    };
    let theme = Theme::default();
    let painted = |node: &ViewNode| {
        let mut w = realize_one(node);
        heca_grid_ui::LayoutEngine::new()
            .compute(w.as_mut(), heca_core::layout::Size::new(300.0, 80.0));
        let mut scene = heca_grid_ui::Scene::new();
        {
            let mut cx = heca_grid_ui::PaintCx::new(&mut scene, &theme);
            w.paint(&mut cx);
        }
        format!("{:?}", scene.iter().collect::<Vec<_>>())
    };

    // Orientation shows up in layout: a vertical rule is tall and thin, a horizontal one wide
    // and thin, so the property is visible in the box rather than merely stored.
    let rule = |o: ViewOrientation| {
        let n = ViewNode::new(WidgetId::Separator)
            .prop("orientation", o.into())
            .prop("length", PropValue::Float(40.0));
        let w = realize_one(&n);
        format!("{:?}", w.base().style.layout)
    };
    assert_ne!(
        rule(ViewOrientation::Vertical),
        rule(ViewOrientation::Horizontal),
        "orientation reached the separator",
    );

    // Axes are the region's own state, not a `Layout` field, so ask the widget — the same way
    // `a_scroll_regions_axes_are_authorable` does.
    let region = |a: ViewScrollAxes| {
        let node = ViewNode::new(WidgetId::Scroll).prop("axes", a.into());
        with_props(ScrollRegion::new(), &node, &Theme::default()).clone_axes()
    };
    assert_ne!(
        region(ViewScrollAxes::Both),
        region(ViewScrollAxes::Vertical),
        "axes reached the scroll region",
    );

    let toast = |s: ViewSeverity| {
        painted(
            &ViewNode::new(WidgetId::Toast)
                .text("Build failed")
                .prop("severity", s.into()),
        )
    };
    assert_ne!(
        toast(ViewSeverity::Danger),
        toast(ViewSeverity::Info),
        "severity reached the toast",
    );

    let row = |m: ViewMarker| {
        painted(
            &ViewNode::new(WidgetId::Row)
                .prop("active", PropValue::Bool(true))
                .prop("marker", m.into())
                .child(ViewNode::new(WidgetId::Label).text("x")),
        )
    };
    assert_ne!(
        row(ViewMarker::Check),
        row(ViewMarker::Bar),
        "marker reached the row"
    );

    let label = |a: ViewTextAlign| {
        painted(
            &ViewNode::new(WidgetId::Label)
                .text("STATUS")
                .prop("align", a.into()),
        )
    };
    assert_ne!(
        label(ViewTextAlign::Center),
        label(ViewTextAlign::Start),
        "text align reached the label",
    );

    // Truncation is a described property too: a plugin's label must be able to fit its box
    // without the plugin measuring anything. `Start` and `End` keep opposite halves, so the two
    // paint differently the moment the box is too small for the text.
    // The label is wrapped in a narrow container, because that is the only way truncation is
    // ever reached: a `Label` sizes itself from its text, and the cut happens when the layout
    // hands it less than that. A width prop on the label itself would be overwritten by its own
    // remeasure.
    let cut = |mode: heca_view::ViewEllipsis| {
        painted(
            &ViewNode::new(WidgetId::HStack)
                .prop("width", PropValue::Int(60))
                .child(
                    ViewNode::new(WidgetId::Label)
                        .text("projects/heca/src/widgets")
                        .prop("truncate", mode.into()),
                ),
        )
    };
    assert_ne!(
        cut(heca_view::ViewEllipsis::Start),
        cut(heca_view::ViewEllipsis::End),
        "truncate reached the label — and the two ends keep different halves",
    );

    let checkbox = |side: ViewLabelSide| {
        painted(
            &ViewNode::new(WidgetId::Checkbox)
                .prop("label", PropValue::Text("Enable".into()))
                .prop("label_side", side.into()),
        )
    };
    assert_ne!(
        checkbox(ViewLabelSide::Left),
        checkbox(ViewLabelSide::Right),
        "label side reached the checkbox",
    );
}

/// A theme token **nested inside** an object is still a token.
///
/// This is the part that had to be got right: resolution happens per value, at any depth, so a
/// nested `"accent"` becomes the live theme's accent rather than reaching serde as the literal
/// word — which would drop the whole field, and drop it silently.
#[test]
fn a_token_nested_in_an_object_resolves_against_the_theme() {
    let bordered = |theme: &Theme| {
        let node = ViewNode::new(WidgetId::Surface).prop(
            "border",
            PropValue::Map(PropMap::from([
                ("color".into(), PropValue::Color("accent".into())),
                ("width".into(), PropValue::Float(1.0)),
            ])),
        );
        realize(&node, theme, &noop_emitter(), &mut FormBindings::default()).expect("valid described test widget")
            .base()
            .style
            .visual
            .border
            .expect("the token resolved")
            .color
    };

    let theme = Theme::default();
    assert_eq!(
        bordered(&theme),
        theme.colors.accent,
        "the token is the theme's accent"
    );

    let mut other = Theme::default();
    other.colors.accent = heca_grid_ui::Color::rgb(0x10, 0xc0, 0x20);
    assert_eq!(
        bordered(&other),
        other.colors.accent,
        "and it follows the theme"
    );
}

/// A malformed member costs only itself, at depth too.
///
/// The object keeps its usable fields and the node keeps its other properties — the same
/// totality rule the flat case has, now that values nest.
#[test]
fn a_bad_member_of_an_object_does_not_discard_the_rest() {
    let node = ViewNode::new(WidgetId::Surface)
        .prop(
            "border",
            PropValue::Map(PropMap::from([
                ("color".into(), PropValue::Color("not-a-colour".into())),
                ("width".into(), PropValue::Float(3.0)),
            ])),
        )
        .prop("radius", PropValue::Float(5.0));
    let w = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    assert_eq!(
        w.base().style.visual.radius,
        5.0,
        "the neighbouring property still applied"
    );
    assert!(
        w.base().style.visual.border.is_none(),
        "a border with no usable colour is dropped, not fatal",
    );
}

/// A bad colour costs only itself — `realize` stays total for untrusted input, so the good
/// properties on the same node still apply.
#[test]
fn a_bad_colour_does_not_discard_its_neighbours() {
    let node = ViewNode::new(WidgetId::Surface)
        .prop("fill", PropValue::Color("not-a-colour".into()))
        .prop("radius", PropValue::Float(7.0));
    let w = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    assert_eq!(
        w.base().style.visual.radius,
        7.0,
        "the good one still applied"
    );
    assert!(
        w.base().style.visual.fill.is_none(),
        "the bad one was dropped, not fatal"
    );
}
