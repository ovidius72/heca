//! **A dock that nothing changes asks for no more frames**, however focus moved.

use super::*;
use heca_grid_ui::Component;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn two_columns() -> WorkspaceTree {
    let mut tree = testing::tree();
    tree.workspaces[0].columns = vec![
        ColumnEntry {
            col_idx: 0,
            col_id: heca_core::layout::ColumnId(0),
            collapsed: false,
            panes: vec![
                testing::pane(PaneId(1), "one"),
                testing::pane(PaneId(2), "two"),
            ],
        },
        ColumnEntry {
            col_idx: 1,
            col_id: heca_core::layout::ColumnId(1),
            collapsed: false,
            panes: vec![testing::pane(PaneId(3), "three")],
        },
    ];
    tree
}

#[test]
fn moving_focus_between_dock_rows_does_not_keep_asking_for_frames() {
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    heca_grid_ui::install_frame_request(move || {
        count.fetch_add(1, Ordering::SeqCst);
    });
    let tree = two_columns();
    let mut fx = testing::Fixture::default();
    let mut root = {
        let (seams, mut reg) = fx.split("left");
        DockView {
            tree: &tree,
            scroll: heca_grid_ui::reactive::signal(0.0),
        }
        .build(&seams, &mut reg)
    };
    let frame = |fx: &testing::Fixture, root: &mut heca_grid_ui::widgets::ScrollRegion| {
        heca_grid_ui::frame_served();
        let active = fx.store.workspaces.active_pane();
        for (pane, signal) in &fx.signals.pane_active {
            let v = active == Some(*pane);
            if heca_grid_ui::reactive::SignalGet::get_untracked(signal) != v {
                heca_grid_ui::reactive::SignalUpdate::set(signal, v);
            }
        }
        for (panes, signal) in fx.signals.col_active.iter().chain(&fx.signals.ws_active) {
            let v = active.is_some_and(|a| panes.contains(&a));
            if heca_grid_ui::reactive::SignalGet::get_untracked(signal) != v {
                heca_grid_ui::reactive::SignalUpdate::set(signal, v);
            }
        }
        let animating = root.tick(0.016);
        heca_grid_ui::LayoutEngine::new().compute(root, heca_grid_ui::Size::new(300.0, 600.0));
        let theme = heca_grid_ui::Theme::default();
        let mut scene = heca_grid_ui::Scene::new();
        heca_grid_ui::paint_child(root, &mut heca_grid_ui::PaintCx::new(&mut scene, &theme));
        (
            asked.swap(0, Ordering::SeqCst),
            animating,
            root.next_redraw(),
        )
    };
    for _ in 0..5 {
        frame(&fx, &mut root);
    }
    for pane in [2, 3, 1, 3] {
        fx.store.workspaces.set_active_pane(Some(PaneId(pane)));
        let trace: Vec<_> = (0..8).map(|_| frame(&fx, &mut root)).collect();
        eprintln!("focus {pane}: {trace:?}");
        for (i, (asked, animating, wake)) in trace.iter().enumerate().skip(4) {
            assert_eq!(
                *asked, 0,
                "frame {i} after focusing {pane} asked for another"
            );
            assert!(!animating, "frame {i} after focusing {pane} is animating");
            assert!(
                wake.is_none(),
                "frame {i} after focusing {pane} books a wake: {wake:?}"
            );
        }
    }
}

/// **A pane's row flashing while the dock is not on screen keeps no frame coming.**
///
/// The sidebar can be hidden or the workspace collapsed while an agent in an unfocused pane asks
/// for attention. The row's flash is bound and never painted; it used to leave the loop drawing
/// every frame until the app was closed, at full CPU.
#[test]
fn a_flash_on_a_row_that_is_not_on_screen_does_not_keep_the_loop_awake() {
    use heca_grid_ui::Tone;
    let tree = two_columns();
    let mut fx = testing::Fixture::default();
    let mut root = {
        let (seams, mut reg) = fx.split("left");
        DockView {
            tree: &tree,
            scroll: heca_grid_ui::reactive::signal(0.0),
        }
        .build(&seams, &mut reg)
    };
    root.base_mut().set_hidden(true);
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    heca_grid_ui::install_frame_request(move || {
        count.fetch_add(1, Ordering::SeqCst);
    });
    fx.store
        .panes
        .signals(PaneId(2))
        .flash
        .repeat(Tone::Warning);
    for pane in [1, 3] {
        fx.store
            .panes
            .signals(PaneId(pane))
            .flash
            .trigger(Tone::Success);
    }
    for i in 0..6 {
        heca_grid_ui::frame_served();
        let animating = root.tick(0.016);
        heca_grid_ui::LayoutEngine::new().compute(&mut root, heca_grid_ui::Size::new(300.0, 600.0));
        let theme = heca_grid_ui::Theme::default();
        let mut scene = heca_grid_ui::Scene::new();
        heca_grid_ui::paint_child(&root, &mut heca_grid_ui::PaintCx::new(&mut scene, &theme));
        assert!(!animating, "frame {i}");
        assert_eq!(
            root.next_redraw(),
            None,
            "frame {i} books a wake for a row nobody can see"
        );
    }
}
