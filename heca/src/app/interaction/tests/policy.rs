//! The policy table: how every action is classified.

use super::*;


/// What these actions are allowed to do. Every variant being classified is the compiler's job:
/// `action_policy`'s match has no catch-all arm, so an unclassified variant does not build.
#[test]
fn action_policy_classifies_these_as_documented() {
    // Spot-check specific classifications
    assert_eq!(action_policy(&WmAction::FocusLeft), ActionPolicy::TiledOnly);
    // Column rename (active or by-idx) is a tiled-layout op → TiledOnly, like RenameColumn.
    assert_eq!(
        action_policy(&WmAction::RenameColumn),
        ActionPolicy::TiledOnly
    );
    assert_eq!(
        action_policy(&WmAction::RenameColumnByIdx {
            ws_idx: 0,
            col_idx: 0
        }),
        ActionPolicy::TiledOnly
    );
    // Reset-name mirrors rename: pane-local for panes, workspace-level for workspaces.
    assert_eq!(
        action_policy(&WmAction::ResetPaneName),
        ActionPolicy::FocusedPaneLocal
    );
    assert_eq!(
        action_policy(&WmAction::ResetWorkspaceNameByIdx { ws_idx: 0 }),
        ActionPolicy::WorkspaceLevel
    );
    assert_eq!(
        action_policy(&WmAction::Float),
        ActionPolicy::FocusedPaneLocal
    );
    assert_eq!(
        action_policy(&WmAction::ClosePane),
        ActionPolicy::FocusedPaneLocal
    );
    // Surface-agnostic: it asks whatever owns the screen for its menu, and bubbling decides
    // whose that is (F003/P082/T416).
    assert_eq!(
        action_policy(&WmAction::OpenContextMenu),
        ActionPolicy::Global
    );
    assert_eq!(
        action_policy(&WmAction::ScrollbackPageUp),
        ActionPolicy::FocusedPaneLocal
    );
    assert_eq!(
        action_policy(&WmAction::ScrollbackToBottom),
        ActionPolicy::FocusedPaneLocal
    );
    assert_eq!(
        action_policy(&WmAction::ExitScrollback),
        ActionPolicy::FocusedPaneLocal
    );
    for terminal_action in [
        WmAction::TerminalRun {
            pane_id: None,
            terminal: None,
            text: String::new(),
            enter: true,
        },
        WmAction::TerminalKill {
            pane_id: None,
            terminal: None,
        },
    ] {
        assert_eq!(
            action_policy(&terminal_action),
            ActionPolicy::FocusedPaneLocal
        );
    }
    assert_eq!(
        action_policy(&WmAction::ScrollToOffset { rows: 0 }),
        ActionPolicy::FocusedPaneLocal
    );
    assert_eq!(
        action_policy(&WmAction::CommandPalette {
            mode: None,
            query: None
        }),
        ActionPolicy::AlwaysAllowed
    );
    assert_eq!(action_policy(&WmAction::ReloadConfig), ActionPolicy::Global);
    // **Taking** chrome focus is tiled-only; **releasing** it is always allowed. A focused
    // container's own `activate`/`hint` move pane focus and are reached through the first, so it
    // must not open while a float owns the domain — but a way out that can be blocked is not a
    // way out (F003/P085/T356).
    assert_eq!(
        action_policy(&WmAction::FocusDock { dock: None }),
        ActionPolicy::TiledOnly
    );
    assert_eq!(
        action_policy(&WmAction::FocusDock {
            dock: Some("workspaces".into())
        }),
        ActionPolicy::TiledOnly
    );
    assert_eq!(action_policy(&WmAction::UnfocusDock), ActionPolicy::Global);
    // The horizontal four reach a chrome container's scroll area only, so they follow chrome
    // focus rather than the pane's tiled/floating domain.
    for action in [
        WmAction::ScrollPageLeft,
        WmAction::ScrollPageRight,
        WmAction::ScrollToLeftEdge,
        WmAction::ScrollToRightEdge,
    ] {
        assert_eq!(action_policy(&action), ActionPolicy::Global, "{action:?}");
    }
    assert_eq!(
        action_policy(&WmAction::OpenLink {
            url: "https://example.com".into()
        }),
        ActionPolicy::Global
    );
    assert_eq!(
        action_policy(&WmAction::WorkspaceNext),
        ActionPolicy::WorkspaceLevel
    );
    assert_eq!(
        action_policy(&WmAction::FocusPane { pane_id: PaneId(0) }),
        ActionPolicy::SourceDependent
    );
}
