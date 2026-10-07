//! **The workspace** — the columns and the floating panes, as ordinary children of one widget.
//!
//! It is a clipping box placed over the content area. Its children are the columns (each holding
//! its panes) and then the floats, so their order in the tree is their order on screen: what comes
//! later is on top, and the same walk that paints them decides what a press lands on.
//!
//! The host hands it a [`WorkspaceModel`] once a frame (`set_props`) and places nothing. The
//! workspace keeps a child that is still there, builds one that arrived, drops one that left, and
//! places each from the model's rects.

mod dividers;
mod find;
mod gather;
mod model;
mod seat;

pub(crate) use find::{offer_to_columns, pane_node, pane_node_mut};
pub(crate) use gather::gather;
pub(crate) use model::{PaneEntry, PickMark, WorkspaceModel};
pub(crate) use seat::{clear_workspace, seat_workspace};

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use heca_core::layout::{PaneId, Rectangle};
use heca_grid_ui::component::Base;
use heca_grid_ui::style::Placement;
use heca_grid_ui::widgets::{Flex, LandingSlot, Splitter};
use heca_grid_ui::{Component, ComponentExt, Length, reconcile};

use crate::chrome::column::{ColumnCallbacks, ColumnShell, ColumnShellModel};
use crate::chrome::pane::shell::focus_state_to;
use crate::chrome::pane::{PaneCallbacks, PaneShell, PaneShellModel};
use crate::chrome::{HeaderEnv, PaneHeader};

/// **The workspace's identity in the window root.** Found by this, never by position.
pub(crate) const WORKSPACE_KEY: &str = "heca.workspace";

/// What the workspace builds its children from, taken when it is built. It lives as long as the
/// workspace does and a config reload builds a new one, so nothing in it is refreshed per frame.
pub(crate) struct WorkspaceSeams {
    pub(crate) column: ColumnCallbacks,
    pub(crate) pane: PaneCallbacks,
    /// What a pane's header is built from, when the info bar is on.
    pub(crate) header_env: Option<Rc<HeaderEnv>>,
    /// What dragging the edge right of a column means: that column takes this share of the working
    /// width more.
    pub(crate) resize_column: Rc<dyn Fn(usize, f64)>,
    /// What dragging the edge below a pane means: that pane takes this many px more height.
    pub(crate) resize_pane: Rc<dyn Fn(usize, usize, f64)>,
}

/// A workspace that takes a [`WorkspaceModel`] as its props.
pub(crate) fn workspace(seams: WorkspaceSeams) -> Flex {
    // The working width the edges convert a drag by, kept where the edges can read it as it changes.
    let working = Rc::new(Cell::new(0.0_f32));
    Flex::column()
        .key(WORKSPACE_KEY)
        .clip_children(true)
        .on_props(move |model: &WorkspaceModel, base| {
            working.set(model.working_width);
            place(&seams, &working, model, base)
        })
}

/// Bring the workspace's children in line with `model`, and put each where it says.
fn place(seams: &WorkspaceSeams, working: &Rc<Cell<f32>>, model: &WorkspaceModel, base: &mut Base) {
    let area = model.area;
    base.style.layout.placement = Some(Placement {
        left: Length::Px(area.loc.x as f32),
        top: Length::Px(area.loc.y as f32),
        width: Length::Px(area.size.w as f32),
        height: Length::Px(area.size.h as f32),
    });

    // What each child is, by name and by the version it is built from — columns first, then the
    // floats. A child built from another version is rebuilt; it is never rebuilt for moving.
    let mut wanted: Vec<(String, String)> = model
        .columns
        .iter()
        .map(|c| (crate::chrome::column_key(c.col_id), c.key()))
        .collect();
    // The edges come after the columns and before the floats: a float is over them.
    let edges = dividers::edges(model);
    wanted.extend(edges.iter().map(|(edge, _)| (edge.name(), String::new())));
    // The places a carried pane can start a column, over the columns and under the floats. Each is
    // hidden until a pane is picked up, and tells itself when that happens.
    // The places a carried pane can be put, over the columns and under the floats — laid out by
    // the layout itself, so each is real space. A keyboard pick marks them with letters instead.
    if model.pick.is_empty() {
        wanted.extend(
            model
                .places
                .iter()
                .map(|p| (place_key(p.kind), String::new())),
        );
    }
    // What an open column pick marks, over the places and under the floats: an outline round each
    // column that can be picked, an empty place with its letter at each gap that can be.
    wanted.extend(model.pick.iter().map(|mark| match mark {
        PickMark::Gap { at, letter } => (pick_gap_key(*at), letter.to_string()),
        PickMark::Row { col, row, letter } => (pick_row_key(*col, *row), letter.to_string()),
        PickMark::Column(col) => (pick_column_key(*col), String::new()),
    }));
    wanted.extend(
        model
            .floats
            .iter()
            .map(|p| (crate::chrome::pane_key(p.pane_id), p.key())),
    );
    reconcile::reconcile_keyed(base, &wanted, |name| {
        if let Some(mark) = model.pick.iter().find(|m| pick_key(m) == name) {
            return pick_mark(mark, name);
        }
        if let Some((edge, _)) = edges.iter().find(|(edge, _)| edge.name() == name) {
            return Box::new(build_edge(seams, working, *edge).key(name));
        }
        if crate::chrome::slot_of_key(name).is_some() || crate::chrome::row_of_key(name).is_some() {
            return Box::new(
                LandingSlot::new()
                    .accepting(crate::chrome::PANE_DRAG_KIND)
                    .edge(crate::chrome::row_of_key(name).is_some())
                    .key(name),
            );
        }
        match model
            .columns
            .iter()
            .find(|c| crate::chrome::column_key(c.col_id) == name)
        {
            Some(column) => build_column(seams, model, column),
            None => match model
                .floats
                .iter()
                .find(|p| crate::chrome::pane_key(p.pane_id) == name)
            {
                Some(pane) => build_float(seams, model, pane),
                None => Box::new(Flex::column()),
            },
        }
    });

    // Each child where the model puts it. Rects are in window coordinates; the children sit in the
    // workspace, so they are measured from its corner.
    for child in base.children.iter_mut() {
        let Some(name) = child.base().identity().map(str::to_string) else {
            continue;
        };
        if let Some(rect) = model
            .pick
            .iter()
            .find(|m| pick_key(m) == name)
            .and_then(|mark| pick_rect(mark, model))
        {
            place_at(
                child.as_mut(),
                area,
                rect.loc.x as f32,
                rect.loc.y as f32,
                rect.size.w as f32,
                rect.size.h as f32,
            );
        } else if let Some(place) = model.places.iter().find(|p| place_key(p.kind) == name) {
            place_at(
                child.as_mut(),
                area,
                place.rect.loc.x as f32,
                place.rect.loc.y as f32,
                place.rect.size.w as f32,
                place.rect.size.h as f32,
            );
        } else if let Some((_, gap)) = edges.iter().find(|(edge, _)| edge.name() == name) {
            place_at(
                child.as_mut(),
                area,
                gap.loc.x as f32,
                gap.loc.y as f32,
                gap.size.w as f32,
                gap.size.h as f32,
            );
        } else if let Some(column) = model
            .columns
            .iter()
            .find(|c| crate::chrome::column_key(c.col_id) == name)
        {
            place_column(seams, model, column, child.as_mut());
        } else if let Some(pane) = model
            .floats
            .iter()
            .find(|p| crate::chrome::pane_key(p.pane_id) == name)
        {
            give_pane(model, pane, child.as_mut());
            place_at(child.as_mut(), area, pane.x, pane.y, pane.w, pane.h);
        }
    }
}

/// What a pick mark is called in the workspace.
fn pick_key(mark: &PickMark) -> String {
    match mark {
        PickMark::Gap { at, .. } => pick_gap_key(*at),
        PickMark::Row { col, row, .. } => pick_row_key(*col, *row),
        PickMark::Column(col) => pick_column_key(*col),
    }
}

fn pick_row_key(col: usize, row: usize) -> String {
    format!("pick:row:{col}:{row}")
}

/// What a place is called in the workspace — what a drop on it names.
fn place_key(kind: heca_core::layout::PlaceKind) -> String {
    match kind {
        heca_core::layout::PlaceKind::Gap(at) => crate::chrome::slot_key(at),
        heca_core::layout::PlaceKind::Row { col, row } => crate::chrome::row_key(col, row),
    }
}

fn pick_gap_key(at: usize) -> String {
    format!("pick:gap:{at}")
}

fn pick_column_key(col: heca_core::layout::ColumnId) -> String {
    format!("pick:col:{}", col.0)
}

/// A mark of the open pick, built: an empty place with its letter, or an outline round a column. It
/// decorates — a press goes through to what is under it.
fn pick_mark(mark: &PickMark, name: &str) -> Box<dyn Component> {
    let slot = match mark {
        PickMark::Gap { letter, .. } => LandingSlot::new().label(letter.to_string()),
        PickMark::Row { letter, .. } => LandingSlot::new().edge(true).label(letter.to_string()),
        PickMark::Column(_) => LandingSlot::new().filled(true),
    };
    Box::new(slot.pointer_transparent(true).key(name))
}

/// Where a pick mark sits, in window coordinates.
fn pick_rect(mark: &PickMark, model: &WorkspaceModel) -> Option<Rectangle> {
    let place = |kind| model.places.iter().find(|p| p.kind == kind).map(|p| p.rect);
    match mark {
        PickMark::Gap { at, .. } => place(heca_core::layout::PlaceKind::Gap(*at)),
        PickMark::Row { col, row, .. } => place(heca_core::layout::PlaceKind::Row {
            col: *col,
            row: *row,
        }),
        PickMark::Column(col) => model.columns.iter().find(|c| c.col_id == *col).map(|c| {
            Rectangle::new(
                heca_core::layout::Point::new(f64::from(c.x), f64::from(c.y)),
                heca_core::layout::Size::new(f64::from(c.w), f64::from(c.h)),
            )
        }),
    }
}

/// The grab zone of one edge, which tells the seams how far it was dragged.
fn build_edge(seams: &WorkspaceSeams, working: &Rc<Cell<f32>>, edge: dividers::Edge) -> Splitter {
    match edge {
        dividers::Edge::Column { col } => {
            let (resize, working) = (seams.resize_column.clone(), working.clone());
            Splitter::vertical().on_resize(move |px| {
                let width = working.get();
                if width > 0.0 {
                    resize(col, (px / width) as f64);
                }
            })
        }
        dividers::Edge::Pane { col, pane } => {
            let resize = seams.resize_pane.clone();
            Splitter::horizontal().on_resize(move |px| resize(col, pane, px as f64))
        }
    }
}

/// One column and the panes it holds.
fn build_column(
    seams: &WorkspaceSeams,
    model: &WorkspaceModel,
    column: &ColumnShellModel,
) -> Box<dyn Component> {
    let contents = column
        .panes
        .iter()
        .filter_map(|p| {
            model
                .panes
                .get(&p.pane_id)
                .map(|e| (p.pane_id, Box::new(e.content.clone()) as Box<dyn Component>))
        })
        .collect();
    Box::new(
        ColumnShell {
            model: column,
            cb: &seams.column,
            pane_cb: &seams.pane,
            header_env: seams.header_env.clone(),
            contents,
        }
        .build(),
    )
}

/// One floating pane.
fn build_float(
    seams: &WorkspaceSeams,
    model: &WorkspaceModel,
    pane: &PaneShellModel,
) -> Box<dyn Component> {
    Box::new(
        PaneShell {
            model: pane,
            cb: &seams.pane,
            header: seams.header_env.clone().map(PaneHeader::new),
            content: content_of(model, pane.pane_id),
        }
        .build(),
    )
}

fn content_of(model: &WorkspaceModel, pane: PaneId) -> Option<Box<dyn Component>> {
    model
        .panes
        .get(&pane)
        .map(|e| Box::new(e.content.clone()) as Box<dyn Component>)
}

/// A column that is still here: reconcile its panes, then place it and them.
fn place_column(
    seams: &WorkspaceSeams,
    model: &WorkspaceModel,
    column: &ColumnShellModel,
    child: &mut dyn Component,
) {
    // The panes are reconciled, never rebuilt with the column: one that is still here keeps the
    // widget it had — its letter, a gesture in flight, an animation.
    let by_key: HashMap<String, &PaneShellModel> = column
        .panes
        .iter()
        .map(|p| (crate::chrome::pane_key(p.pane_id), p))
        .collect();
    reconcile::reconcile_children(child, &column.child_keys(), |key| {
        let Some(pane) = by_key.get(key).copied() else {
            return Box::new(Flex::column());
        };
        crate::chrome::column::shell::pane_child(
            pane,
            column,
            &seams.pane,
            seams.header_env.clone().map(PaneHeader::new),
            content_of(model, pane.pane_id),
        )
    });
    for pane_child in child.base_mut().children.iter_mut() {
        let Some(pane) = column.panes.iter().find(|p| {
            pane_child.base().key.as_deref() == Some(&crate::chrome::pane_key(p.pane_id))
        }) else {
            continue;
        };
        give_pane(model, pane, pane_child.as_mut());
        place_at(
            pane_child.as_mut(),
            Rectangle::new(
                heca_core::layout::Point::new(column.x as f64, column.y as f64),
                heca_core::layout::Size::new(column.w as f64, column.h as f64),
            ),
            pane.x,
            pane.y,
            pane.w,
            pane.h,
        );
    }
    place_at(child, model.area, column.x, column.y, column.w, column.h);
}

/// What is true of a pane now, handed to the pane: what focus changed, and the facts its header
/// shows.
fn give_pane(model: &WorkspaceModel, pane: &PaneShellModel, child: &mut dyn Component) {
    focus_state_to(child, pane);
    if let Some(input) = model
        .panes
        .get(&pane.pane_id)
        .and_then(|e| e.header.as_ref())
    {
        crate::chrome::give_header_facts(child, input);
    }
}

/// Put `child` at the window rect `(x, y, w, h)`, measured from `origin`'s corner.
fn place_at(child: &mut dyn Component, origin: Rectangle, x: f32, y: f32, w: f32, h: f32) {
    child.base_mut().style.layout.placement = Some(Placement {
        left: Length::Px(x - origin.loc.x as f32),
        top: Length::Px(y - origin.loc.y as f32),
        width: Length::Px(w),
        height: Length::Px(h),
    });
}

#[cfg(test)]
mod tests;
