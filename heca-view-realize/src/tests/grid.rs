//! Grid tracks, areas and placement, and label text attributes.

use heca_grid_ui::Track;
use heca_view::WidgetKind;

use crate::widgets::parse_track;
use super::*;

// ── Grid (tracks / areas / placement) ──

/// Tracks are CSS-like strings — the vocabulary plugin authors already know. Anything
/// unrecognised **degrades to `Auto`**: a typo costs that author a differently-sized track, not
/// a broken host.
#[test]
fn grid_tracks_parse_and_garbage_degrades_to_auto() {
    let text = |s: &str| parse_track(&PropValue::Text(s.into()));
    assert_eq!(text("22px"), Track::Px(22.0));
    assert_eq!(text("1fr"), Track::Fr(1.0));
    assert_eq!(text("2.5fr"), Track::Fr(2.5));
    assert_eq!(text("auto"), Track::Auto);
    assert_eq!(text("min"), Track::MinContent);
    assert_eq!(text("min-content"), Track::MinContent);
    assert_eq!(text("max"), Track::MaxContent);
    assert_eq!(text("max-content"), Track::MaxContent);
    assert_eq!(
        text("  1FR  "),
        Track::Fr(1.0),
        "trimmed + case-insensitive"
    );
    assert_eq!(text("22"), Track::Px(22.0), "a bare number is pixels");
    // Garbage of every shape.
    assert_eq!(text("minmax(1fr, 2fr)"), Track::Auto);
    assert_eq!(text("banana"), Track::Auto);
    assert_eq!(text(""), Track::Auto);
    assert_eq!(parse_track(&PropValue::Bool(true)), Track::Auto);
    assert_eq!(parse_track(&PropValue::Int(22)), Track::Px(22.0));
}

/// A `Grid` node: list-shaped tracks + areas on the grid, and **placement as a prop on the
/// child** — `area` by name, or `col`/`row` (+ spans). A child with neither auto-places.
#[test]
fn grid_node_realizes_tracks_areas_and_per_child_placement() {
    let tracks = |t: [&str; 3]| {
        PropValue::List(t.iter().map(|s| PropValue::Text((*s).into())).collect())
    };
    let node = ViewNode::new(WidgetKind::Grid)
        .prop("columns", tracks(["auto", "1fr", "auto"]))
        .prop(
            "rows",
            PropValue::List(vec![PropValue::Text("auto".into())]),
        )
        .prop(
            "areas",
            PropValue::List(vec![
                PropValue::Text("icon title status".into()),
                PropValue::Text("icon subtext .".into()),
            ]),
        )
        // Placed by area name.
        .child(
            ViewNode::new(WidgetKind::Icon)
                .prop("icon", PropValue::Glyph("terminal".into()))
                .prop("area", PropValue::Text("icon".into())),
        )
        // Placed by explicit cell + span, in the older `col` / `col_span` spelling — still read, so a
        // tree already sent keeps working.
        .child(
            ViewNode::new(WidgetKind::Label)
                .text("zsh")
                .prop("col", PropValue::Int(2))
                .prop("row", PropValue::Int(1))
                .prop("col_span", PropValue::Int(2)),
        )
        // Neither → auto-placed.
        .child(ViewNode::new(WidgetKind::Label).text("~/proj"));

    let grid = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    // **A described child carries its own placement**, exactly as a native one does: the area
    // by name, the explicit cell as numbers. The name is resolved during layout, against
    // whichever template ends up holding it — so it is still a name here.
    {
        let children = &grid.base().children;
        assert_eq!(children.len(), 3);
        assert_eq!(
            children[0].base().grid_area.as_deref(),
            Some("icon"),
            "the child names its area; the grid does not write a cell into it",
        );
        assert_eq!(
            children[0].base().style.layout.grid_cell,
            None,
            "and nothing is resolved before there is a layout to resolve it against",
        );
    }

    let mut grid = grid;
    heca_grid_ui::LayoutEngine::new()
        .compute(grid.as_mut(), heca_grid_ui::Size::new(300.0, 200.0));
    let children = &grid.base().children;

    // The `icon` area spans both rows of column 1 (it appears twice in the template).
    assert_eq!(
        children[0].base().style.layout.grid_cell,
        Some(heca_grid_ui::GridCell {
            col: 1,
            row: 1,
            col_span: 1,
            row_span: 2
        }),
        "laid out, the name became the cell the template gives it",
    );
    assert_eq!(
        children[1].base().style.layout.grid_cell,
        Some(heca_grid_ui::GridCell {
            col: 2,
            row: 1,
            col_span: 2,
            row_span: 1
        }),
        "placed by explicit cell; an omitted span defaults to 1",
    );
    assert_eq!(
        children[2].base().style.layout.grid_cell,
        None,
        "no placement props → taffy auto-placement",
    );
}

/// Alignment is authorable too, on both axes: `align` / `justify_items` on the grid, and
/// `align_self` / `justify_self` on any child (they are properties of a node *inside its
/// parent*, so `realize` reads them for every kind, not just grid items).
#[test]
fn grid_alignment_is_authorable_on_both_axes() {
    use heca_grid_ui::Align;
    use heca_view::ViewAlign;

    let node = ViewNode::new(WidgetKind::Grid)
        .prop("align", PropValue::Align(ViewAlign::Center))
        .prop("justify_items", PropValue::Align(ViewAlign::Center))
        .child(
            ViewNode::new(WidgetKind::Label)
                .text("pinned")
                .prop("align_self", PropValue::Align(ViewAlign::End))
                .prop("justify_self", PropValue::Align(ViewAlign::End)),
        );

    let grid = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    let style = grid.base().style.layout;
    assert_eq!(
        style.align,
        Align::Center,
        "vertical: the items in their cells"
    );
    assert_eq!(
        style.justify_items,
        Some(Align::Center),
        "horizontal: `justify_items`, not `justify` (which moves the track set)",
    );
    let child = grid.base().children[0].base().style.layout;
    assert_eq!(child.align_self, Some(Align::End));
    assert_eq!(child.justify_self, Some(Align::End));
}

/// An unknown area name is not an error — the child simply auto-places (realize stays total).
#[test]
fn grid_child_in_an_unknown_area_auto_places() {
    let node = ViewNode::new(WidgetKind::Grid)
        .prop(
            "areas",
            PropValue::List(vec![PropValue::Text("a b".into())]),
        )
        .child(
            ViewNode::new(WidgetKind::Label)
                .text("x")
                .prop("area", PropValue::Text("nope".into())),
        );
    let grid = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    assert_eq!(grid.base().children[0].base().style.layout.grid_cell, None);
}

/// A `Label`'s text attributes are authorable: weight + slant (font attributes) and underline +
/// strikethrough (decorations the widget draws). Absent props keep the widget's default.
#[test]
fn label_text_attributes_are_authorable() {
    let node = ViewNode::new(WidgetKind::Label)
        .text("DONE")
        .prop("bold", PropValue::Bool(true))
        .prop("italic", PropValue::Bool(true))
        .prop("strikethrough", PropValue::Bool(true));

    let label = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    // Realize hands back a `Box<dyn Component>`, so read the state through the scene: paint it
    // and check the run carries the font attributes and the strike is drawn as a rect.
    use heca_core::layout::Size;
    use heca_grid_ui::{DrawCommand, LayoutEngine, PaintCx, Scene, TextStyle, Theme};
    let mut label = label;
    LayoutEngine::new().compute(label.as_mut(), Size::new(200.0, 40.0));
    let theme = Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        label.paint(&mut cx);
    }
    let style = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text(t) => Some(t.style),
            _ => None,
        })
        .expect("the label paints its run");
    assert_eq!(style, TextStyle::REGULAR.bold(true).italic(true));
    let rules = scene
        .iter()
        .filter(|c| matches!(c, DrawCommand::Rect(_)))
        .count();
    assert_eq!(rules, 1, "the strikethrough, drawn as a rect (not shaped)");
}

/// **A described child is placed exactly as a native one is** — the same five words
/// (`area`, `column`, `row`, `column_span`, `row_span`) in the same CSS spellings, applied
/// through the same builders, so the two authoring paths cannot place a child differently.
#[test]
fn a_described_child_is_placed_as_a_native_one_is() {
    use heca_grid_ui::builders::PlaceExt;
    let text = |s: &str| PropValue::Text(s.into());
    let node = ViewNode::new(WidgetKind::Grid)
        .child(
            ViewNode::new(WidgetKind::Label)
                .text("header")
                .prop("column", text("1 / -1"))
                .prop("row", text("1")),
        )
        .child(
            ViewNode::new(WidgetKind::Label)
                .text("wide")
                .prop("column_span", text("span 2"))
                .prop("row_span", PropValue::Int(3)),
        )
        .child(
            ViewNode::new(WidgetKind::Label)
                .text("named")
                .prop("area", text("body")),
        );
    let grid = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    let placed = |i: usize| {
        let c = &grid.base().children[i];
        (c.base().style.layout.grid_cell, c.base().grid_area.clone())
    };
    let native = |w: heca_grid_ui::widgets::Label| {
        (w.base().style.layout.grid_cell, w.base().grid_area.clone())
    };
    use heca_grid_ui::widgets::Label;
    assert_eq!(placed(0), native(Label::new("header").column("1 / -1").row("1")));
    assert_eq!(placed(1), native(Label::new("wide").column_span("span 2").row_span(3)));
    assert_eq!(placed(2), native(Label::new("named").area("body")));
}
