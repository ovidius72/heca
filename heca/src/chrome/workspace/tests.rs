use std::collections::HashMap;

use heca_core::layout::{ColumnId, PaneId, Point, Rectangle, Size};
use heca_grid_ui::builders::LayoutExt;
use heca_grid_ui::widgets::Flex;
use heca_grid_ui::{Component, LayoutEngine};

use super::*;
use crate::chrome::column::ColumnCallbacks;
use crate::chrome::pane::testing::{model_at, recording_callbacks};
use crate::chrome::terminal::Terminal;

fn seams() -> WorkspaceSeams {
    WorkspaceSeams {
        column: ColumnCallbacks {
            tint: heca_grid_ui::Color::new(0, 255, 0, 255),
            pick: Rc::new(|_| {}),
            new_column: Rc::new(|| {}),
        },
        pane: recording_callbacks().0,
        header_env: None,
    }
}

fn column(id: u64, x: f32, panes: &[(u64, f32, f32)]) -> ColumnShellModel {
    ColumnShellModel {
        col_id: ColumnId(id),
        x,
        y: 30.0,
        w: 300.0,
        h: 500.0,
        focus_pane: panes.first().map(|p| PaneId(p.0)),
        panes: panes
            .iter()
            .map(|(pane, top, h)| model_at(PaneId(*pane), x, 30.0 + top, 300.0, *h))
            .collect(),
        new_column_slot: None,
    }
}

/// Two columns (the first holding two panes) and one float, in a content area that does not start
/// at the window's corner.
fn model(columns: Vec<ColumnShellModel>, floats: Vec<PaneShellModel>) -> WorkspaceModel {
    let panes: HashMap<PaneId, PaneEntry> = columns
        .iter()
        .flat_map(|c| &c.panes)
        .chain(&floats)
        .map(|p| {
            (
                p.pane_id,
                PaneEntry {
                    content: Terminal::new(),
                    header: None,
                },
            )
        })
        .collect();
    WorkspaceModel {
        area: Rectangle::new(Point::new(40.0, 30.0), Size::new(700.0, 500.0)),
        columns,
        floats,
        panes,
    }
}

fn two_columns_and_a_float() -> WorkspaceModel {
    model(
        vec![
            column(1, 40.0, &[(10, 0.0, 240.0), (11, 250.0, 240.0)]),
            column(2, 350.0, &[(20, 0.0, 490.0)]),
        ],
        vec![model_at(PaneId(30), 200.0, 100.0, 250.0, 200.0)],
    )
}

/// The workspace as the window holds it: placed by its own props in a window-sized box, laid out.
fn laid_out(model: &WorkspaceModel) -> (Flex, Box<dyn Component>) {
    let mut window = Flex::column().width(800.0).height(600.0);
    let mut ws: Box<dyn Component> = Box::new(workspace(seams()));
    assert!(ws.set_props(model), "the workspace takes its model");
    window.base_mut().children.push(ws);
    LayoutEngine::new().compute(&mut window, heca_grid_ui::Size::new(800.0, 600.0));
    let ws = window.base_mut().children.remove(0);
    (window, ws)
}

fn bounds(c: &dyn Component) -> (f64, f64, f64, f64) {
    let b = c.base().bounds;
    (b.loc.x, b.loc.y, b.size.w, b.size.h)
}

fn named<'a>(parent: &'a dyn Component, key: &str) -> &'a dyn Component {
    parent
        .base()
        .children
        .iter()
        .find(|c| c.base().key.as_deref() == Some(key))
        .map(|c| c.as_ref())
        .unwrap_or_else(|| panic!("no child {key}"))
}

/// **The workspace places its columns, their panes and its floats where the model says**, in window
/// coordinates, with the columns before the floats — so a float is on top.
#[test]
fn the_workspace_places_what_the_model_says() {
    let (_window, ws) = laid_out(&two_columns_and_a_float());

    assert_eq!(
        bounds(ws.as_ref()),
        (40.0, 30.0, 700.0, 500.0),
        "over the content area"
    );
    let first = named(ws.as_ref(), "col:1");
    assert_eq!(bounds(first), (40.0, 30.0, 300.0, 500.0));
    assert_eq!(bounds(named(first, "pane:11")), (40.0, 280.0, 300.0, 240.0));
    assert_eq!(
        bounds(named(named(ws.as_ref(), "col:2"), "pane:20")),
        (350.0, 30.0, 300.0, 490.0)
    );
    assert_eq!(
        bounds(named(ws.as_ref(), "pane:30")),
        (200.0, 100.0, 250.0, 200.0)
    );

    let order: Vec<_> = ws
        .base()
        .children
        .iter()
        .filter_map(|c| c.base().key.clone())
        .collect();
    assert_eq!(
        order,
        ["col:1", "col:2", "pane:30"],
        "columns, then floats: later is on top"
    );
}

/// **A pane that is still there keeps its widget; one that left goes; one that arrived is built.**
#[test]
fn panes_come_and_go_while_the_others_keep_their_widget() {
    let mut ws: Box<dyn Component> = Box::new(workspace(seams()));
    assert!(ws.set_props(&two_columns_and_a_float()));
    // A mark only this instance of the pane carries.
    ws.base_mut().children[0].base_mut().children[0]
        .base_mut()
        .set_hidden(true);

    // Pane 11 leaves, pane 12 arrives, the float goes.
    let next = model(
        vec![
            column(1, 40.0, &[(10, 0.0, 240.0), (12, 250.0, 240.0)]),
            column(2, 350.0, &[(20, 0.0, 490.0)]),
        ],
        vec![],
    );
    assert!(ws.set_props(&next));

    let first = &ws.base().children[0];
    let keys: Vec<_> = first
        .base()
        .children
        .iter()
        .filter_map(|c| c.base().key.clone())
        .collect();
    assert_eq!(keys, ["pane:10", "pane:12"]);
    assert!(
        first.base().children[0].base().style.layout.hidden,
        "pane 10 was not rebuilt"
    );
    assert_eq!(ws.base().children.len(), 2, "the float left");
}

/// A model of another type is refused rather than ignored.
#[test]
fn the_workspace_refuses_props_that_are_not_its_model() {
    let mut ws: Box<dyn Component> = Box::new(workspace(seams()));
    assert!(!ws.set_props(&"not a model"));
}

/// **The offer of a new column is a child of the column that holds the picked pane**, there while
/// the pick is open and gone after — and it does not rebuild the panes beside it.
#[test]
fn a_column_shows_the_offer_of_a_new_column_while_a_pick_is_open() {
    let keys = |ws: &dyn Component| -> Vec<String> {
        ws.base().children[0]
            .base()
            .children
            .iter()
            .filter_map(|c| c.base().key.clone())
            .collect()
    };
    let mut ws: Box<dyn Component> = Box::new(workspace(seams()));
    let mut open = column(1, 40.0, &[(10, 0.0, 240.0)]);
    open.new_column_slot = Some(0.2);
    assert!(ws.set_props(&model(vec![open], vec![])));
    assert_eq!(
        keys(ws.as_ref()),
        ["pane:10", crate::chrome::NEW_COLUMN_KEY]
    );

    assert!(ws.set_props(&model(vec![column(1, 40.0, &[(10, 0.0, 240.0)])], vec![])));
    assert_eq!(
        keys(ws.as_ref()),
        ["pane:10"],
        "the offer left with the pick"
    );
}

/// A pane behind the sidebar gets no letter; a visible one does — and a column is visible when any
/// pane in it is.
#[test]
fn a_column_shows_a_letter_only_where_it_can_be_seen() {
    use super::find::column_view_seen;
    use crate::providers::workspaces::{column_key, pane_key};
    let (hidden, shown) = (PaneId(1), PaneId(2));
    let seen = |p: PaneId| p == shown;

    assert!(
        !column_view_seen(&[hidden], &pane_key(hidden), seen),
        "a covered pane"
    );
    assert!(
        column_view_seen(&[shown], &pane_key(shown), seen),
        "a visible pane"
    );
    assert!(
        !column_view_seen(&[hidden, shown], &pane_key(hidden), seen),
        "a covered pane in a column whose other pane shows — judged by its own view",
    );
    assert!(
        column_view_seen(&[hidden, shown], &column_key(ColumnId(1)), seen),
        "the column itself shows while any of its panes does",
    );
    assert!(
        !column_view_seen(&[hidden], &column_key(ColumnId(1)), seen),
        "a column entirely covered",
    );
}

/// **A float's terminal is drawn after the tiled one**: tree order is paint order, so the surface of
/// the pane that comes later in the workspace is later in the scene, and the flush draws it over.
#[test]
fn a_floats_terminal_comes_after_the_tiled_ones_in_the_scene() {
    use heca_grid_ui::component::{PaintCx, paint_child};
    use heca_grid_ui::scene::{DrawCommand, HostDraw, Scene};

    let mut model = two_columns_and_a_float();
    for (pane, process) in [(10, 1), (20, 2), (30, 3)] {
        if let Some(entry) = model.panes.get_mut(&PaneId(pane)) {
            entry.content = crate::chrome::terminal::testing::attached(process);
        }
    }
    let (window, ws) = laid_out(&model);
    let _keep = window;
    let theme = heca_grid_ui::theme::Theme::default();
    let mut scene = Scene::new();
    paint_child(ws.as_ref(), &mut PaintCx::new(&mut scene, &theme));

    let order: Vec<u64> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Host(h) => match h.draw {
                HostDraw::Surface { id } => Some(id),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert!(
        order.contains(&1) && order.contains(&3),
        "both are drawn: {order:?}"
    );
    assert_eq!(
        order.last(),
        Some(&3),
        "the float is last, so it is on top: {order:?}"
    );
}
