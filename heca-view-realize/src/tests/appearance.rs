//! Appearance is overridable; the theme is the default.

use heca_grid_ui::Flex;
use heca_view::WidgetKind;

use super::*;

// ── Appearance is overridable; the theme is the default ──

/// A description sets **any** of `Visual`, not just colour: fill, border, glow, radius and
/// font size all arrive through the same generic merge as `Layout`.
#[test]
fn a_description_can_override_the_whole_of_visual() {
    let node = ViewNode::new(WidgetKind::Surface)
        .prop("fill", PropValue::Color("#ff8800".into()))
        .prop("radius", PropValue::Float(12.0))
        .prop("font_size", PropValue::Float(18.0));
    let w = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    let visual = w.base().style.visual;
    assert_eq!(
        visual.fill,
        Some(heca_grid_ui::Color::rgb(0xff, 0x88, 0x00))
    );
    assert_eq!(visual.radius, 12.0);
    assert_eq!(visual.font_size, 18.0);
}

/// **Unset still means "ask the theme".** A node that overrides one field leaves every other
/// one at the widget's own value, so nothing about a non-overriding widget changed.
#[test]
fn an_unset_appearance_field_is_left_alone() {
    let plain = realize(
        &ViewNode::new(WidgetKind::Surface),
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    let one_field = realize(
        &ViewNode::new(WidgetKind::Surface).prop("radius", PropValue::Float(9.0)),
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    assert_eq!(one_field.base().style.visual.radius, 9.0, "the one it set");
    assert_eq!(
        one_field.base().style.visual.fill,
        plain.base().style.visual.fill,
        "everything else is untouched — unset is not 'set to nothing'",
    );
}

/// **A theme token name resolves against the theme the tree is built with**, so the same
/// description gives a different pixel under a different theme — which is what makes a token
/// follow `prefix+Shift+r`, since a reload drops the retained trees and rebuilds them.
///
/// A hex literal is the same pixel under either theme. That is the trade a caller makes by
/// writing one, and the reason the docs steer toward token names.
#[test]
fn a_theme_token_follows_the_theme_and_a_hex_literal_does_not() {
    let mut dark = Theme::default();
    dark.colors.accent = heca_grid_ui::Color::rgb(0x11, 0x22, 0x33);
    let mut light = Theme::default();
    light.colors.accent = heca_grid_ui::Color::rgb(0xee, 0xdd, 0xcc);

    let fill_of = |node: &ViewNode, theme: &Theme| {
        realize(node, theme, &noop_emitter(), &mut FormBindings::default())
            .base()
            .style
            .visual
            .fill
    };

    let token =
        ViewNode::new(WidgetKind::Surface).prop("fill", PropValue::Color("accent".into()));
    assert_eq!(fill_of(&token, &dark), Some(dark.colors.accent));
    assert_eq!(fill_of(&token, &light), Some(light.colors.accent));

    let hex =
        ViewNode::new(WidgetKind::Surface).prop("fill", PropValue::Color("#ff0000".into()));
    let literal = Some(heca_grid_ui::Color::rgb(0xff, 0, 0));
    assert_eq!(fill_of(&hex, &dark), literal);
    assert_eq!(fill_of(&hex, &light), literal, "a literal is not a token");
}

/// The accepted token vocabulary **is** the theme's own colour fields — nothing is written down
/// twice. Add a colour to the theme and a description can name it with no table to update.
#[test]
fn the_token_vocabulary_is_the_themes_own_fields() {
    let theme = Theme::default();
    for token in [
        "accent",
        "foreground",
        "muted",
        "border",
        "danger",
        "warning",
        "success",
    ] {
        assert!(
            resolve_color(token, &theme).is_some(),
            "{token} is a theme colour and should resolve",
        );
    }
    assert!(
        resolve_color("chartreuse", &theme).is_none(),
        "not a theme colour"
    );
    // A non-colour theme field cannot be named by accident.
    assert!(
        resolve_color("name", &theme).is_none(),
        "the theme's NAME is not a colour"
    );
}

/// A token reaches a **widget's own colour builder**, not just `Visual`.
///
/// `Label::color`, `Icon::color`, `Tag`'s hue, `Row::highlight` and the rest were all marked
/// `host_only`. This is
/// that promise being kept: the host resolves the token to hex before the value crosses, so the
/// library still knows nothing about themes and `Color::from_str` still only knows hex.
#[test]
fn a_token_reaches_a_widgets_own_colour_builder() {
    let mut theme = Theme::default();
    theme.colors.danger = heca_grid_ui::Color::rgb(0xc0, 0x10, 0x20);

    let painted = |node: &ViewNode, theme: &Theme| {
        use heca_core::layout::Size;
        use heca_grid_ui::{LayoutEngine, PaintCx, Scene};
        let mut w = realize(node, theme, &noop_emitter(), &mut FormBindings::default());
        LayoutEngine::new().compute(w.as_mut(), Size::new(300.0, 40.0));
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, theme);
            w.paint(&mut cx);
        }
        scene
    };

    let tinted = ViewNode::new(WidgetKind::Label)
        .text("This action cannot be undone.")
        .prop("color", PropValue::Color("danger".into()));
    let plain = ViewNode::new(WidgetKind::Label).text("This action cannot be undone.");

    // The label paints; the tinted one does not paint the same thing as the untinted one.
    let a = format!("{:?}", painted(&tinted, &theme));
    let b = format!("{:?}", painted(&plain, &theme));
    assert!(!a.is_empty(), "the label painted something");
    assert_ne!(a, b, "the token override changed what was drawn");
    // And the colour it used is the THEME's danger, so a different theme paints differently.
    let mut other = Theme::default();
    other.colors.danger = heca_grid_ui::Color::rgb(0x10, 0xc0, 0x20);
    assert_ne!(
        a,
        format!("{:?}", painted(&tinted, &other)),
        "a token follows the theme it was built with",
    );
}

/// A **struct-shaped** appearance property reaches the widget.
///
/// `border` and `glow` were unreachable from a description for as long as a property value was
/// a closed list of scalars: `Border` is `{color, width}` and nothing could carry it.
/// Adding serde to the two types is not the same thing, and no test looked — so nothing
/// failed.
/// `PropValue::Map` is what makes the claim true.
#[test]
fn a_struct_shaped_appearance_property_reaches_the_widget() {
    let node = ViewNode::new(WidgetKind::Surface)
        .prop(
            "border",
            PropValue::Map(PropMap::from([
                ("color".into(), PropValue::Color("#ff8800".into())),
                ("width".into(), PropValue::Float(2.0)),
            ])),
        )
        .prop(
            "glow",
            PropValue::Map(PropMap::from([
                ("color".into(), PropValue::Color("#00ccff".into())),
                ("radius".into(), PropValue::Float(12.0)),
                ("intensity".into(), PropValue::Float(0.4)),
            ])),
        );
    let w = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    let border = w
        .base()
        .style
        .visual
        .border
        .expect("a described border reaches the widget");
    assert_eq!(border.width, 2.0);
    assert_eq!(border.color, heca_grid_ui::Color::rgb(0xff, 0x88, 0x00));
    let glow = w
        .base()
        .style
        .visual
        .glow
        .expect("a described glow reaches the widget");
    assert_eq!(glow.radius, 12.0);
    assert_eq!(glow.intensity, 0.4);
    assert_eq!(glow.color, heca_grid_ui::Color::rgb(0x00, 0xcc, 0xff));
}

/// A described node stretches to its parent, the way CSS flexbox does.
///
/// Set no width and a node fills its parent's cross axis; set `width_pct(0.5)` and it takes
/// half; set `width(px)` and it takes exactly that. Authors rely on the first without asking
/// for it — the showcase's panels are only a fixed size because they say so — and it is
/// currently true because a container's default `align` is `Stretch`, which is taffy agreeing
/// with CSS.
///
/// **Nothing pinned that until this test.** Changing a container's default alignment would
/// silently turn every full-width described panel into a content-width one, with no test
/// failing and nothing to read in a diff. It is a contract now.
#[test]
fn a_described_node_stretches_to_its_parent_like_css() {
    use heca_view::build::{self, Parent as _, Style as _};

    // Mount a described tree in a plain column of a known width, as a page lays sections out,
    // and report the node's width plus its first child's.
    let mounted = |node: ViewNode| {
        let realized = realize(
            &node,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        let mut parent = Flex::column().width(heca_grid_ui::Length::Px(600.0));
        parent.base_mut().children.push(realized);
        heca_grid_ui::LayoutEngine::new()
            .compute(&mut parent, heca_core::layout::Size::new(600.0, 400.0));
        let child = &parent.base().children[0];
        let inner = child
            .base()
            .children
            .last()
            .map(|c| c.base().bounds.size.w)
            .unwrap_or_default();
        (child.base().bounds.size.w, inner)
    };

    let panel = |b: build::Panel| {
        b.child(build::Row::new().child(build::Label::new("nginx")))
            .into_node()
    };

    // No width: full parent width, and the row inside fills the panel's content box (600 less
    // the panel's 10px padding a side).
    assert_eq!(
        mounted(panel(build::Panel::new().title("P"))),
        (600.0, 580.0),
        "an unsized node fills its parent, and its child fills it in turn",
    );

    // The same result asked for explicitly.
    assert_eq!(
        mounted(panel(build::Panel::new().title("P").width_pct(1.0))).0,
        600.0,
        "width_pct(1.0) is the full parent width",
    );

    // A fraction, which is the case a percentage is actually needed for.
    assert_eq!(
        mounted(panel(build::Panel::new().title("P").width_pct(0.5))).0,
        300.0,
        "width_pct(0.5) is half the parent",
    );

    // And a fixed size wins over the stretch — this is what keeps the showcase's two demo
    // panels side by side instead of splitting the row.
    assert_eq!(
        mounted(panel(build::Panel::new().title("P").width(240.0))),
        (240.0, 220.0),
        "an explicit width is honoured, padding still taken off the child",
    );
}
