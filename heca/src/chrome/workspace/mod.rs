//! **The workspace** — the columns and the floating panes, as ordinary children of one widget.
//!
//! It is a clipping box placed over the content area. Its children are the columns (each holding
//! its panes) and then the floats, so their order in the tree is their order on screen: what comes
//! later is on top, and the same walk that paints them decides what a press lands on.
//!
//! The host hands it a [`WorkspaceModel`] once a frame (`set_props`) and places nothing. The
//! workspace keeps a child that is still there, builds one that arrived, drops one that left, and
//! places each from the model's rects.

mod find;
mod gather;
mod model;
mod seat;

pub(crate) use find::{offer_to_columns, pane_node, pane_node_mut};
pub(crate) use gather::gather;
pub(crate) use model::{PaneEntry, WorkspaceModel};
pub(crate) use seat::{clear_workspace, seat_workspace};

use std::collections::HashMap;
use std::rc::Rc;

use heca_core::layout::{PaneId, Rectangle};
use heca_grid_ui::component::Base;
use heca_grid_ui::style::Placement;
use heca_grid_ui::widgets::Flex;
use heca_grid_ui::{Component, ComponentExt, Length, reconcile};

use crate::chrome::column::shell::new_column_slot;
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
}

/// A workspace that takes a [`WorkspaceModel`] as its props.
pub(crate) fn workspace(seams: WorkspaceSeams) -> Flex {
    Flex::column()
        .key(WORKSPACE_KEY)
        .clip_children(true)
        .on_props(move |model: &WorkspaceModel, base| place(&seams, model, base))
}

/// Bring the workspace's children in line with `model`, and put each where it says.
fn place(seams: &WorkspaceSeams, model: &WorkspaceModel, base: &mut Base) {
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
    wanted.extend(
        model
            .floats
            .iter()
            .map(|p| (crate::chrome::pane_key(p.pane_id), p.key())),
    );
    reconcile::reconcile_keyed(base, &wanted, |name| {
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
        if let Some(column) = model
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
        if key == crate::chrome::NEW_COLUMN_KEY
            && let Some(share) = column.new_column_slot
        {
            return new_column_slot(&seams.column, share);
        }
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
