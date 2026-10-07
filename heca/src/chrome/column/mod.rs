//! **The column** — a column in the scrolling area, built from a [`ColumnShellModel`] and held by
//! the workspace.
//!
//! This file is one of the few in the folder that may touch `AppState` (AGENTS.md § 0b-bis rule 4):
//! it answers the questions about the session that a column's model is made from, so every
//! component below is testable headless.

mod model;
pub(crate) mod shell;

pub(crate) use model::ColumnShellModel;
pub(crate) use shell::{ColumnCallbacks, ColumnShell};

use heca_core::layout::PaneId;

/// **Which pane a pick on this column goes to** — the active one when it is in this column, else
/// the first. Where a column *is*, is the pane you would land on.
pub(crate) fn focus_pane_of(
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
    use heca_core::layout::ColumnId;
    use heca_grid_ui::Component;

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
        };
        ColumnShell {
            model: m,
            cb: &cb,
            pane_cb: &pane_cb(),
            header_env: None,
            contents: Default::default(),
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
        let view = ColumnShell {
            model: &m,
            cb: &cb,
            pane_cb: &pane_cb(),
            header_env: None,
            contents: Default::default(),
        }
        .build();

        view.base().hint.as_ref().expect("declares a pick").run();
        assert_eq!(*seen.borrow(), vec![PaneId(7)]);
    }
}
