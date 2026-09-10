//! **The column surface** — one retained tree per column in the scrolling area.
//!
//! This file is the only one in the folder that may touch `AppState` (AGENTS.md § 0b-bis rule 4):
//! it reduces the session to [`ColumnShellModel`]s and hands them down, so every component below is
//! testable headless.

mod model;
mod shell;

pub(crate) use model::ColumnShellModel;
pub(crate) use shell::{ColumnCallbacks, ColumnShell};

use heca_core::layout::{ColumnId, PaneId};
use heca_grid_ui::{Component, LayoutEngine, Size};

/// A retained per-column tree.
///
/// Rebuilt only when [`ColumnShellModel::key`] changes; repositioned every frame by
/// [`sync_columns`]; painted **through `heca_grid_ui::paint_child`**, which is what draws its
/// letter.
pub(crate) struct RetainedColumn {
    pub(crate) root: heca_grid_ui::widgets::Flex,
    /// The model key the tree was built from.
    pub(crate) key: String,
}

/// Drop every retained column — used when a config reload changes the theme baked into the trees.
/// [`sync_columns`] rebuilds them next frame.
pub(crate) fn clear_columns(state: &mut crate::app_state::AppState) {
    state.columns.clear();
}

/// **Offer a letter to the column that answers to `key`** — the column itself, never what is around
/// it.
///
/// A column is a third place a letter can land, beside the chrome tree and the pane trees: it is
/// drawn in the content area, so it is not in the window root, and it is not a pane. Without this
/// the pick lettered only the sidebar's view of each column.
pub(crate) fn offer_to_columns(
    state: &crate::app_state::AppState,
    key: &str,
    label: Option<String>,
) -> bool {
    let mut offered = false;
    for col in state.columns.values() {
        if col.root.base().key.as_deref() == Some(key) {
            offered |= heca_grid_ui::offer_hint(&col.root, &[], label.clone());
        }
    }
    offered
}

/// Withdraw every letter this surface is carrying — the columns' half of `clear_hint_letters`.
pub(crate) fn clear_column_hints(state: &crate::app_state::AppState) {
    for col in state.columns.values() {
        heca_grid_ui::clear_hints(&col.root);
    }
}

/// Bring the retained column trees in line with the session: build the ones whose identity changed,
/// place each at the rect the scrolling engine gave it, and prune the columns that are gone.
pub(crate) fn sync_columns(state: &mut crate::app_state::AppState) {
    let cb = callbacks(state);
    let frames = crate::app::terminal_host::column_frames(state);

    let mut seen: std::collections::HashSet<ColumnId> = std::collections::HashSet::new();
    for col in &frames {
        seen.insert(col.id);
        let model = ColumnShellModel {
            col_id: col.id,
            x: col.rect.loc.x as f32,
            y: col.rect.loc.y as f32,
            w: col.rect.size.w as f32,
            h: col.rect.size.h as f32,
            focus_pane: focus_pane_of(state, col),
        };

        let key = model.key();
        let needs_build = state
            .columns
            .get(&col.id)
            .map(|c| c.key != key)
            .unwrap_or(true);
        if needs_build {
            let root = ColumnShell {
                model: &model,
                cb: &cb,
            }
            .build();
            state.columns.insert(col.id, RetainedColumn { root, key });
        }

        if let Some(retained) = state.columns.get_mut(&col.id) {
            // The rect is a **per-frame input**, written onto the retained tree rather than built
            // into it — so a scroll, a resize or a split moves the column without rebuilding the
            // widget and throwing away its signals.
            LayoutEngine::new().compute(
                &mut retained.root,
                Size::new(model.w as f64, model.h as f64),
            );
            heca_grid_ui::shift_subtree(&mut retained.root, model.x as f64, model.y as f64);
        }
    }
    state.columns.retain(|id, _| seen.contains(id));
}

/// **Which pane a pick on this column goes to** — the active one when it is in this column, else
/// the first. Where a column *is*, is the pane you would land on.
fn focus_pane_of(
    state: &crate::app_state::AppState,
    col: &heca_core::layout::LaidOutColumn,
) -> Option<PaneId> {
    let active = state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.id);
    match active {
        Some(id) if col.panes.iter().any(|p| p.id == id) => Some(id),
        _ => col.panes.first().map(|p| p.id),
    }
}

/// The app's edges, gathered once. A test calls this to get exactly the seams and nothing else
/// (AGENTS.md § 0b-bis rule 3).
pub(crate) fn callbacks(state: &crate::app_state::AppState) -> ColumnCallbacks {
    let proxy = state.event_proxy.clone();
    ColumnCallbacks {
        tint: crate::chrome::chrome_gui_theme(state).colors.success,
        pick: std::rc::Rc::new(move |pane_id: PaneId| {
            // The column asks; the core does. A pick is a **keyboard** gesture: it lands on
            // nothing, so where the keyboard is is its context — the same reasoning a pane's own
            // pick already applies.
            let _ = proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
                source: crate::app::interaction::InteractionSource::Keyboard,
                intent: crate::app::interaction::InteractionIntent::FocusPane { pane_id },
            });
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(col_id: u64, focus: Option<u64>) -> ColumnShellModel {
        ColumnShellModel {
            col_id: ColumnId(col_id),
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 800.0,
            focus_pane: focus.map(PaneId),
        }
    }

    fn built(m: &ColumnShellModel) -> heca_grid_ui::widgets::Flex {
        let cb = ColumnCallbacks {
            tint: heca_grid_ui::Color::new(0, 255, 0, 255),
            pick: std::rc::Rc::new(|_| {}),
        };
        ColumnShell { model: m, cb: &cb }.build()
    }

    /// **The column in the scrolling area owns its identity** (F003/P082/T474).
    ///
    /// Antonio, driving 2026-08-24: *"prefix+shift+c works but shows letters only in the sidebar,
    /// not in the scrolling area."* Nothing out here declared `col:<id>`, so the pick had nowhere to
    /// put the letter and only the sidebar's view of the column wore one.
    #[test]
    fn a_column_declares_the_key_the_pick_addresses_it_by() {
        let view = built(&model(3, Some(7)));
        assert_eq!(
            view.base().key.as_deref(),
            Some(column_key_of(3).as_str()),
            "the column names itself by its own id, never its position",
        );
    }

    fn column_key_of(id: u64) -> String {
        crate::chrome::column_key(ColumnId(id))
    }

    /// …and is a pick target, so a letter offered by that key actually lands on it.
    #[test]
    fn a_column_takes_the_letter_its_own_key_is_offered() {
        use heca_grid_ui::reactive::SignalGet as _;
        let mut view = built(&model(3, Some(7)));
        view.base_mut().bounds = heca_core::layout::Rectangle::new(
            heca_core::layout::types::Point::new(0.0, 0.0),
            heca_core::layout::types::Size::new(400.0, 800.0),
        );
        assert!(heca_grid_ui::offer_hint(&view, &[], Some("a".into())));
        assert_eq!(view.base().hint_label.get_untracked().as_deref(), Some("a"));
    }

    /// A column with no pane has nowhere to go, so it declares nothing and wears no letter.
    #[test]
    fn an_empty_column_is_not_a_pick_target() {
        assert!(built(&model(4, None)).base().hint.is_none());
    }

    /// **Picking a column takes you to the pane you would land on**, so it names an existing verb
    /// rather than adding a focus-column action that would resolve to this one anyway.
    #[test]
    fn picking_a_column_focuses_the_pane_it_would_land_on() {
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = seen.clone();
        let cb = ColumnCallbacks {
            tint: heca_grid_ui::Color::new(0, 255, 0, 255),
            pick: std::rc::Rc::new(move |id: PaneId| sink.borrow_mut().push(id)),
        };
        let m = model(3, Some(7));
        let view = ColumnShell { model: &m, cb: &cb }.build();

        view.base().hint.as_ref().expect("declares a pick").run();
        assert_eq!(*seen.borrow(), vec![PaneId(7)]);
    }
}
