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
        },
        pane: recording_callbacks().0,
        header_env: None,
        resize_column: Rc::new(|_, _| {}),
        resize_pane: Rc::new(|_, _, _| {}),
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
        working_width: 700.0,
        places: vec![],
        pick: vec![],
        area: Rectangle::new(Point::new(40.0, 30.0), Size::new(700.0, 500.0)),
        columns,
        floats,
        panes,
    }
}

/// The places the layout would open for a carried pane in [`two_columns`]: the gap between the
/// columns and the row between the first column's two panes — each with the box the layout gives.
fn open_places() -> Vec<heca_core::layout::Place> {
    use heca_core::layout::{Place, PlaceKind};
    let rect = |x: f64, y: f64, w: f64, h: f64| Rectangle::new(Point::new(x, y), Size::new(w, h));
    vec![
        Place {
            kind: PlaceKind::Gap(1),
            rect: rect(318.0, 30.0, 54.0, 500.0),
        },
        Place {
            kind: PlaceKind::Row { col: 0, row: 1 },
            rect: rect(40.0, 270.0, 300.0, 0.0),
        },
    ]
}

fn two_columns() -> WorkspaceModel {
    model(
        vec![
            column(1, 40.0, &[(10, 0.0, 240.0), (11, 250.0, 240.0)]),
            column(2, 350.0, &[(20, 0.0, 490.0)]),
        ],
        vec![],
    )
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

/// What painting `root` draws.
fn painted(root: &dyn Component) -> heca_grid_ui::Scene {
    let theme = heca_grid_ui::Theme::default();
    let mut scene = heca_grid_ui::Scene::new();
    heca_grid_ui::paint_child(root, &mut heca_grid_ui::PaintCx::new(&mut scene, &theme));
    scene
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

mod carry;
mod placing;
mod pointer;
mod settle;
