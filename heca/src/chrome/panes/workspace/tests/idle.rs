//! **Moving focus between panes leaves the workspace quiet**: after the few frames a change needs,
//! a frame that changes nothing asks for no more frames, animates nothing and books no wake.

use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// What a frame asked of the host.
#[derive(Debug, PartialEq)]
struct Asked {
    /// Frames requested while it ran.
    frames: usize,
    /// A widget reported itself mid-animation.
    animating: bool,
    /// Seconds until a widget wants drawing, if any does.
    wake: Option<f32>,
    /// A terminal's scrollback controls changed, which asks the host for one more frame.
    controls_changed: bool,
}

const QUIET: Asked = Asked {
    frames: 0,
    animating: false,
    wake: None,
    controls_changed: false,
};

/// A window holding the workspace and its terminals, driven the way the host drives a frame.
struct Window {
    root: Flex,
    asked: Arc<AtomicUsize>,
    terminals: Vec<crate::chrome::panes::terminal::Terminal>,
}

impl Window {
    fn new() -> (Self, WorkspaceModel) {
        let asked = Arc::new(AtomicUsize::new(0));
        let count = asked.clone();
        heca_grid_ui::install_frame_request(move || {
            count.fetch_add(1, Ordering::SeqCst);
        });
        let mut root = crate::chrome::new_window_root();
        root.base_mut()
            .children
            .push(heca_grid_ui::WidgetModel::new(workspace(seams())));
        // Terminals bound to their owner, as the host binds a pane's: they say what they ask for.
        let mut model = two_columns();
        let mut terminals = Vec::new();
        for (pane, id) in [(10, 1), (11, 2), (20, 3)] {
            let (terminal, _said) = crate::chrome::panes::terminal::testing::bound(id);
            if let Some(entry) = model.panes.get_mut(&PaneId(pane)) {
                entry.content = terminal.reference();
            }
            terminals.push(terminal);
        }
        (
            Self {
                root,
                asked,
                terminals,
            },
            model,
        )
    }

    /// One frame: the model handed over, the tree ticked, laid out and painted, the viewports shown.
    fn frame(&mut self, model: &WorkspaceModel) -> Asked {
        heca_grid_ui::frame_served();
        assert!(self.root.base_mut().children[0].set_props(model));
        let animating = self.root.tick(0.016);
        heca_grid_ui::settle_focus(&mut self.root);
        heca_grid_ui::LayoutEngine::new()
            .compute(&mut self.root, heca_grid_ui::Size::new(800.0, 600.0));
        let _ = painted(&self.root);
        let viewport = viewport();
        let controls_changed = self
            .terminals
            .iter()
            .fold(false, |any, t| t.show(&viewport) | any);
        Asked {
            frames: self.asked.swap(0, Ordering::SeqCst),
            animating,
            wake: self.root.next_redraw(),
            controls_changed,
        }
    }
}

fn viewport() -> crate::chrome::panes::terminal::Viewport {
    crate::chrome::panes::terminal::Viewport {
        rows: 12,
        scrollback_rows: 40,
        offset: 0,
        scrollbar: heca_config::appearance::ScrollbarVisibility::WhenNeeded,
        badge: true,
        cell: (8.0, 16.0),
        top_stable_row: 0,
        match_alpha: 64,
        current_match_alpha: 150,
        nominal_cell: (8.0, 16.0),
        open_link_modifier: heca_config::theme::ModifierKey::Super,
    }
}

/// The model with `pane` focused: its column's focus follows, and it is the active one.
fn focused_on(mut model: WorkspaceModel, pane: u64) -> WorkspaceModel {
    for column in &mut model.columns {
        for p in &mut column.panes {
            p.active = p.pane_id == PaneId(pane);
        }
        if column.panes.iter().any(|p| p.pane_id == PaneId(pane)) {
            column.focus_pane = Some(PaneId(pane));
        }
    }
    model
}

#[test]
fn moving_focus_between_panes_does_not_keep_asking_for_frames() {
    let (mut window, model) = Window::new();
    let mut model = focused_on(model, 10);
    for _ in 0..5 {
        window.frame(&model);
    }
    assert_eq!(
        window.frame(&model),
        QUIET,
        "settled before any focus moves"
    );
    // The terminals that hold each pane, in the order the model was made.
    let terminal_of = |pane: u64| match pane {
        10 => 0,
        11 => 1,
        _ => 2,
    };
    for pane in [11, 20, 10, 20] {
        model = focused_on(model, pane);
        window.terminals[terminal_of(pane)].take_keyboard(&mut window.root);
        for _ in 0..4 {
            window.frame(&model);
        }
        for i in 0..4 {
            assert_eq!(
                window.frame(&model),
                QUIET,
                "frame {i} after focusing pane {pane}"
            );
        }
    }
}
