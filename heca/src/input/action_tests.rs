//! Tests for [`super::WmAction`].

use super::*;
use heca_core::layout::PaneId;
use heca_core::runtime::PaneClosePolicy;

#[test]
fn test_parameterized_variants_constructible() {
    // Exercise all parameterized variants so they are not flagged as dead code.
    let _ = WmAction::FocusPane { pane_id: PaneId(1) };
    let _ = WmAction::FocusWorkspace { ws_idx: 0 };
    let _ = WmAction::Swap {
        a_id: PaneId(1),
        b_id: PaneId(2),
    };
    let _ = WmAction::Move {
        pane_id: PaneId(1),
        target_col: 0,
    };
    let _ = WmAction::MovePaneToWorkspace {
        pane_id: PaneId(1),
        ws_idx: 0,
    };
    let _ = WmAction::MovePaneToColumn {
        pane_id: PaneId(1),
        ws_idx: 0,
        col_idx: 0,
    };
    let _ = WmAction::MoveColumnToWorkspace {
        col_idx: 0,
        ws_idx: 1,
        focus: true,
    };
    let _ = WmAction::ZoomColumn;
    let _ = WmAction::Resize {
        target: ResizeTarget::Column,
        amount: 10.0,
        edge: ResizeEdge::Auto,
    };
    let _ = WmAction::ResizeTo {
        target: ResizeTarget::Pane,
        width: 100.0,
        height: 200.0,
    };
    let _ = WmAction::FloatAt {
        pane_id: PaneId(1),
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
    };
    let _ = WmAction::ClosePaneById { pane_id: PaneId(1) };
    let _ = WmAction::RenameTarget {
        pane_id: PaneId(1),
        name: "test".to_string(),
    };
    let _ = WmAction::SpawnCommand {
        command: "lazygit".to_string(),
        kind: SpawnKind::Terminal,
        float: true,
        close_policy: PaneClosePolicy {
            close_pane: true,
            keep_on_error: true,
            keep_on_success: false,
        },
        cwd: None,
    };
    let _ = WmAction::AddPaneToColumn {
        ws_idx: 0,
        col_idx: 0,
    };
    let _ = WmAction::DeleteColumn {
        ws_idx: 0,
        col_idx: 0,
    };
    let _ = WmAction::DeleteWorkspace { ws_idx: 0 };
    let _ = WmAction::TakePane {
        pane_id: PaneId(1),
        focus_after: false,
    };
    let _ = WmAction::RenameColumn;
    let _ = WmAction::PaneTake;
    let _ = WmAction::PaneTakeAndFocus;
    // Scrollback
    let _ = WmAction::ScrollbackPageUp;
    let _ = WmAction::ScrollbackPageDown;
    let _ = WmAction::ScrollbackLineUp { amount: 1 };
    let _ = WmAction::ScrollbackLineDown { amount: 5 };
    let _ = WmAction::ScrollbackToTop;
    let _ = WmAction::ScrollbackToBottom;
    let _ = WmAction::ExitScrollback;
}

#[test]
fn a_kind_groups_a_variant_regardless_of_its_fields() {
    let a = WmAction::FocusPane { pane_id: PaneId(1) };
    let b = WmAction::FocusPane { pane_id: PaneId(2) };
    let c = WmAction::FocusLeft;
    assert_eq!(a.kind(), b.kind());
    assert_ne!(a.kind(), c.kind());
}
