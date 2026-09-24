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
    /// **The panes this column holds**, in order. A pane's letter is offered against its own
    /// surface, so whoever offers it has to know which panes are in here without reading a key
    /// back out of the tree.
    pub(crate) panes: Vec<PaneId>,
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
///
/// **Only where it can be seen.** `seen` answers whether a pane's view is visible — the same
/// question every other view's letter is asked. A column behind a sidebar used to be lettered
/// regardless, so a pane under the sidebar drew its keycap on top of the sidebar (Antonio, driving,
/// 2026-09-24): the pane path withdrew the letter and this path put it straight back.
pub(crate) fn offer_to_columns(
    state: &crate::app_state::AppState,
    key: &str,
    label: Option<String>,
    seen: impl Fn(PaneId) -> bool,
) -> bool {
    let mut offered = false;
    for col in state.columns.values() {
        let label = if column_view_seen(&col.panes, key, &seen) {
            label.clone()
        } else {
            // A withdrawal, never "leave whatever is there": a letter given while the column was
            // visible must go when it is covered.
            None
        };
        // **By key, into the tree** — the column names itself, and the offer of a new column beside
        // it is a child. Matching only the root meant a target had to BE the tree it lived in.
        offered |= heca_grid_ui::offer_hint_by_key(&col.root, key, label);
    }
    offered
}

/// **Can this column show the offer for `key`?** A pane it holds is judged by that pane's own view;
/// anything else in the column — the column itself, the new-column slot beside it — by whether any
/// of its panes can be seen.
///
/// Pure, so the rule is tested without a window.
pub(crate) fn column_view_seen(panes: &[PaneId], key: &str, seen: impl Fn(PaneId) -> bool) -> bool {
    match panes
        .iter()
        .find(|p| crate::providers::workspaces::pane_key(**p) == key)
    {
        Some(pane) => seen(*pane),
        None => panes.iter().any(|p| seen(*p)),
    }
}

#[cfg(test)]
mod offer_tests {
    use super::*;
    use crate::providers::workspaces::{column_key, pane_key};

    /// A pane behind the sidebar gets no letter; a visible one does — and a column is visible when
    /// any pane in it is.
    #[test]
    fn a_column_shows_a_letter_only_where_it_can_be_seen() {
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
}

/// Bring the retained column trees in line with the session: build the ones whose identity changed,
/// place each at the rect the scrolling engine gave it, and prune the columns that are gone.
pub(crate) fn sync_columns(state: &mut crate::app_state::AppState) {
    let cb = callbacks(state);
    let pane_cb = crate::chrome::pane::callbacks(state);
    let frames = crate::app::terminal_host::column_frames(state);
    // **The panes come from the same reading of the session the pane surface uses**, so a column
    // and the panes in it can never disagree about where anything is.
    let pane_models = crate::chrome::pane::pane_models(state);
    let mut headers = crate::chrome::build_pane_headers(state);
    // The words a pane shows change constantly; they are written onto the retained child every
    // frame rather than rebuilt for (F003/P097/T500).
    let header_texts: std::collections::HashMap<PaneId, Vec<_>> = headers
        .iter()
        .map(|(id, (_, _, texts))| (*id, texts.clone()))
        .collect();

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
            // Only the column the picked pane is in offers a new one beside it.
            new_column_slot: picking_from_column(state, col),
            // The panes this column holds, in the order the layout engine placed them.
            panes: col
                .panes
                .iter()
                .filter_map(|p| pane_models.iter().find(|m| m.pane_id == p.id).cloned())
                .collect(),
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
                pane_cb: &pane_cb,
                headers: model
                    .panes
                    .iter()
                    .filter_map(|p| {
                        headers.remove(&p.pane_id).map(|(tree, _, _)| {
                            (
                                p.pane_id,
                                Box::new(tree) as Box<dyn heca_grid_ui::Component>,
                            )
                        })
                    })
                    .collect(),
            }
            .build();
            state.columns.insert(
                col.id,
                RetainedColumn {
                    root,
                    key,
                    panes: model.panes.iter().map(|p| p.pane_id).collect(),
                },
            );
        }

        if let Some(retained) = state.columns.get_mut(&col.id) {
            retained.panes = model.panes.iter().map(|p| p.pane_id).collect();
            // **The panes are reconciled, never rebuilt with the column.** A pane that is still
            // here keeps the widget it had — its letter, a gesture in flight, an animation — and
            // only one that arrived is built. Rebuilding them all would be the 100% CPU idle this
            // surface was warned about.
            heca_grid_ui::reconcile::reconcile_children(
                &mut retained.root,
                &model.pane_keys(),
                |key| {
                    let pane = model
                        .panes
                        .iter()
                        .find(|p| crate::chrome::pane_key(p.pane_id) == key)
                        .expect("the wanted keys come from these panes");
                    let header = headers
                        .remove(&pane.pane_id)
                        .map(|(tree, _, _)| Box::new(tree) as Box<dyn heca_grid_ui::Component>);
                    shell::pane_child(pane, &model, &pane_cb, header)
                },
            );
            // Each pane sits where the layout engine put it, and says so itself — the rect is a
            // per-frame input, exactly as the column's own is. What focus changed and the words in
            // its header ride along the same way: written onto the retained child rather than
            // rebuilt for, so a gesture in flight survives a focus change.
            for child in retained.root.base_mut().children.iter_mut() {
                let Some(pane) = model.panes.iter().find(|p| {
                    child.base().key.as_deref() == Some(&crate::chrome::pane_key(p.pane_id))
                }) else {
                    continue;
                };
                let pane = pane.clone();
                if let Some(texts) = header_texts.get(&pane.pane_id) {
                    crate::chrome::pane_header::refresh_pane_header_text(child.as_ref(), texts);
                }
                crate::chrome::pane::shell::focus_state_to(
                    child.as_mut(),
                    pane.active,
                    pane.border_color,
                    pane.accent,
                );
                let l = &mut child.base_mut().style.layout;
                l.placement = Some(heca_grid_ui::style::Placement {
                    left: heca_grid_ui::Length::Px(pane.x - model.x),
                    top: heca_grid_ui::Length::Px(pane.y - model.y),
                    width: heca_grid_ui::Length::Px(pane.w),
                    height: heca_grid_ui::Length::Px(pane.h),
                });
            }
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

/// **Is the pick asking where to put a pane that lives in THIS column?**
///
/// The offer of a new column is drawn beside the column the pane is in, because that is where the
/// new one goes — "you keep your place in the strip".
fn picking_from_column(
    state: &crate::app_state::AppState,
    col: &heca_core::layout::LaidOutColumn,
) -> bool {
    let picking = match &state.input_mode {
        crate::app_state::InputMode::ColumnPick { pane_id, .. } => Some(*pane_id),
        _ => None,
    };
    offers_new_column(picking, col.panes.iter().map(|p| p.id))
}

/// The rule itself, without a window: the offer belongs to the column holding the pane the pick
/// captured, and to nothing else when no pick is open.
fn offers_new_column(picking: Option<PaneId>, panes: impl IntoIterator<Item = PaneId>) -> bool {
    let Some(pane_id) = picking else {
        return false;
    };
    panes.into_iter().any(|id| id == pane_id)
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
        new_column: {
            let proxy = state.event_proxy.clone();
            std::rc::Rc::new(move || {
                let _ = proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
                    source: crate::app::interaction::InteractionSource::Keyboard,
                    intent: crate::app::interaction::InteractionIntent::ActivateAction(
                        crate::input::WmAction::MovePaneToNewColumn,
                    ),
                });
            })
        },
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
        with_panes(col_id, focus, &[])
    }

    /// A column holding panes at the rects the layout engine gave them — 10px apart, as the WM
    /// leaves them.
    fn with_panes(col_id: u64, focus: Option<u64>, panes: &[(u64, f32, f32)]) -> ColumnShellModel {
        ColumnShellModel {
            col_id: ColumnId(col_id),
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 800.0,
            focus_pane: focus.map(PaneId),
            panes: panes
                .iter()
                .map(|(id, top, h)| {
                    crate::chrome::pane::testing::model_at(PaneId(*id), 0.0, *top, 400.0, *h)
                })
                .collect(),
            new_column_slot: false,
        }
    }

    fn pane_cb() -> crate::chrome::pane::PaneCallbacks {
        crate::chrome::pane::PaneCallbacks {
            pick: std::rc::Rc::new(|_| {}),
        }
    }

    fn built(m: &ColumnShellModel) -> heca_grid_ui::widgets::Flex {
        let cb = ColumnCallbacks {
            tint: heca_grid_ui::Color::new(0, 255, 0, 255),
            pick: std::rc::Rc::new(|_| {}),
            new_column: std::rc::Rc::new(|| {}),
        };
        ColumnShell {
            model: m,
            cb: &cb,
            pane_cb: &pane_cb(),
            headers: Default::default(),
        }
        .build()
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

    /// **A column holds its panes** — the thing this task exists for. A pane is a child, so moving
    /// one between containers is a tree operation rather than a rearrangement of two parallel maps.
    #[test]
    fn a_column_holds_its_panes_as_children() {
        let m = with_panes(3, Some(7), &[(7, 0.0, 100.0), (8, 110.0, 100.0)]);
        let view = built(&m);
        let keys: Vec<Option<String>> = view
            .base()
            .children
            .iter()
            .map(|c| c.base().key.clone())
            .collect();
        assert_eq!(
            keys,
            vec![
                Some(crate::chrome::pane_key(PaneId(7))),
                Some(crate::chrome::pane_key(PaneId(8))),
            ],
            "the panes are the column's children, each under its own name",
        );
    }

    /// **And keeps the space the layout engine left between them.**
    ///
    /// The WM owns pane geometry, gaps included. A column that stacked its children would close
    /// them, and every pane below the first would be drawn over its own content.
    #[test]
    fn the_panes_keep_the_gaps_the_layout_engine_gave_them() {
        use heca_grid_ui::{LayoutEngine, Size};
        let m = with_panes(
            3,
            Some(7),
            &[(7, 0.0, 100.0), (8, 110.0, 100.0), (9, 220.0, 100.0)],
        );
        let mut view = built(&m);
        LayoutEngine::new().compute(&mut view, Size::new(400.0, 800.0));

        let placed: Vec<(f64, f64)> = view
            .base()
            .children
            .iter()
            .map(|c| (c.base().bounds.loc.y, c.base().bounds.size.h))
            .collect();
        for (i, (top, h)) in [(0.0, 100.0), (110.0, 100.0), (220.0, 100.0)]
            .iter()
            .enumerate()
        {
            assert!(
                (placed[i].0 - top).abs() < 1.0 && (placed[i].1 - h).abs() < 1.0,
                "pane {i} is not where the layout engine put it: {placed:?}",
            );
        }
    }

    /// **The offer of a new column is drawn beside the column the pane is in**, and only while a
    /// pick is open. It sits outside that column's box, which is why it is placed absolutely
    /// rather than stacked — a child that took space would squeeze the panes.
    #[test]
    fn the_new_column_offer_is_a_child_placed_beside_the_column() {
        let mut m = with_panes(3, Some(7), &[(7, 0.0, 100.0)]);
        m.new_column_slot = true;
        let view = built(&m);
        let slot = view
            .base()
            .children
            .iter()
            .find(|c| c.base().key.as_deref() == Some(crate::chrome::NEW_COLUMN_KEY))
            .expect("the offer is a child of the column");
        let placement = slot
            .base()
            .style
            .layout
            .placement
            .expect("placed, so it takes no space from the panes");
        assert_eq!(
            placement.left,
            heca_grid_ui::Length::Percent(1.0),
            "it starts where the column ends",
        );
        assert!(slot.base().hint.is_some(), "and it is a pick target");
    }

    /// …and it is not there when nothing is picking.
    #[test]
    fn no_offer_is_drawn_when_no_pick_is_open() {
        let view = built(&with_panes(3, Some(7), &[(7, 0.0, 100.0)]));
        assert!(
            !view
                .base()
                .children
                .iter()
                .any(|c| c.base().key.as_deref() == Some(crate::chrome::NEW_COLUMN_KEY)),
            "the offer belongs to the pick, so it goes when the pick does",
        );
    }

    /// **The offer belongs to the column holding the picked pane**, and to no column at all when
    /// nothing is picking.
    #[test]
    fn only_the_column_the_picked_pane_is_in_offers_a_new_one() {
        let here = [PaneId(7), PaneId(8)];
        let elsewhere = [PaneId(9)];
        assert!(super::offers_new_column(Some(PaneId(7)), here));
        assert!(!super::offers_new_column(Some(PaneId(7)), elsewhere));
        assert!(
            !super::offers_new_column(None, here),
            "no pick, no offer — it belongs to the pick and goes when it does",
        );
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
            new_column: std::rc::Rc::new(|| {}),
        };
        let m = model(3, Some(7));
        let view = ColumnShell {
            model: &m,
            cb: &cb,
            pane_cb: &pane_cb(),
            headers: Default::default(),
        }
        .build();

        view.base().hint.as_ref().expect("declares a pick").run();
        assert_eq!(*seen.borrow(), vec![PaneId(7)]);
    }
}
