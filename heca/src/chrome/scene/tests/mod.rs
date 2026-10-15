use super::*;

mod docks;
mod regions;

/// **The two sidebar toggles keep their letters when you toggle a sidebar** (F003/P082/T444).
///
/// Both buttons flip their glyph with the sidebar's state — left shows `ArrowLineLeft` while
/// expanded and `ArrowLineRight` while collapsed, right is the mirror. With no `key` their
/// identity is the glyph's name, so collapsing the left sidebar made *both* buttons derive
/// `arrow_line_right`: one wore the bare name and the other `arrow_line_right[1]`, decided by
/// document order. The left button took the name the right one had, and their remembered
/// `prefix+/` letters swapped with it.
///
/// Toggling the left sidebar must not swap the two buttons' letters.
///
/// Keyed by the action they run, both identities are stable through every combination of
/// states — which is the whole point of a key: it comes from the data, not from the picture.
#[test]
fn the_sidebar_toggles_keep_their_identity_when_a_sidebar_is_toggled() {
    use heca_grid_ui::nav::identity_of;

    let shortcuts = ActionShortcuts::default();
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let emit: ChromeIntentEmitter = ChromeIntentEmitter::of(
        crate::app::interaction::InteractionSource::Keyboard,
        |_, _| {},
    );

    // The bar as it is built: the two toggles side by side, in document order.
    let bar = |left_open: bool, right_open: bool| {
        let left_glyph = if left_open {
            Glyph::ArrowLineLeft
        } else {
            Glyph::ArrowLineRight
        };
        let right_glyph = if right_open {
            Glyph::ArrowLineRight
        } else {
            Glyph::ArrowLineLeft
        };
        Flex::row()
            .child(sidebar_toggle_button(
                left_glyph,
                crate::input::WmAction::SidebarLeft,
                "sidebar_left",
                &shortcuts,
                &catalog,
                emit.clone(),
                Color::rgb(255, 255, 255),
            ))
            .child(sidebar_toggle_button(
                right_glyph,
                crate::input::WmAction::SidebarRight,
                "sidebar_right",
                &shortcuts,
                &catalog,
                emit.clone(),
                Color::rgb(255, 255, 255),
            ))
    };

    // **Ask the collector where the targets are** rather than assuming a depth: the picker
    // identifies the node that declared the hint, and that is the node whose key has to be
    // stable. Assuming the button's own path is what made an earlier version of this test pass
    // while the app still swapped the letters.
    let ids = |left_open: bool, right_open: bool| {
        let mut tree = bar(left_open, right_open);
        heca_grid_ui::LayoutEngine::new()
            .compute(&mut tree, heca_core::layout::Size::new(400.0, 40.0));
        let paths: Vec<Vec<usize>> = heca_grid_ui::collect_hints(&tree)
            .into_iter()
            .map(|(path, _bounds)| path)
            .collect();
        assert_eq!(paths.len(), 2, "one pick target per toggle");
        (identity_of(&tree, &paths[0]), identity_of(&tree, &paths[1]))
    };

    let (l_open, r_open) = ids(true, true);
    assert_eq!(l_open.as_deref(), Some("sidebar_left"));
    assert_eq!(r_open.as_deref(), Some("sidebar_right"));

    // The state that used to collide: left collapsed shows the same arrow the right one does.
    assert_eq!(
        ids(false, true),
        (l_open.clone(), r_open.clone()),
        "collapsing the left sidebar must not rename either button",
    );
    assert_eq!(ids(true, false), (l_open.clone(), r_open.clone()));
    assert_eq!(ids(false, false), (l_open, r_open));
}


#[test]
fn chrome_scene_emits_status_text() {
    use heca_grid_ui::{Color, DrawCommand};
    let theme = GuiTheme::default();
    // No sidebar (collapsed) — just the status bar should produce text.
    let scene = super::chrome_scene(
        &super::ChromeFrame {
            w: 800.0,
            h: 600.0,
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            status: "2 panes | foo | NORMAL",
            side_bg: theme.colors.background,
            fg: Color::new(200, 200, 200, 255),
        },
        &theme,
        None,
        None,
    );
    assert!(!scene.is_empty(), "scene should not be empty");
    assert!(
        scene.iter().any(|cmd| matches!(cmd, DrawCommand::Text(..))),
        "scene should contain at least one Text draw command",
    );
}

/// A one-workspace / one-column / one-pane tree — the smallest projection that still
/// has every level.
pub(super) fn one_pane_tree(pane_id: u64) -> crate::providers::workspaces::WorkspaceTree {
    use crate::app_state::SidebarItemState;
    use crate::providers::workspaces::{ColumnEntry, PaneEntry, WorkspaceEntry, WorkspaceTree};

    let mut tree = WorkspaceTree::new();
    tree.workspaces.push(WorkspaceEntry {
        ws_idx: 0,
        ws_id: heca_core::layout::WorkspaceId(0),
        name: "ws1".into(),
        custom_name: None,
        collapsed: false,
        state: SidebarItemState::Active,
        columns: vec![ColumnEntry {
            col_idx: 0,
            col_id: heca_core::layout::ColumnId(0),
            collapsed: false,
            panes: vec![PaneEntry {
                pane_id: heca_core::layout::PaneId(pane_id),
                name: "pane1".into(),
                custom_name: None,
                state: SidebarItemState::Active,
            }],
        }],
        floating_panes: Vec::new(),
    });
    tree
}

/// Build a region's body the way the app does since the provider cutover: through the
/// `ChromeHost`, by asking the mounted provider for its contribution and calling its
/// `build` seam. These tests deliberately do **not** reach into the container's own
/// builder — that would test a path the app no longer takes.
pub(super) fn region_body(
    tree: &crate::providers::workspaces::WorkspaceTree,
    theme: &GuiTheme,
    chrome: &SharedChromeState,
    signals: &mut super::ChromeSignals,
    drag: &mut super::DragItemRegistry,
) -> Option<super::WidgetModel> {
    let mut host = super::ChromeHost::new(chrome.events());
    host.register(Box::new(
        crate::providers::WorkspacesContainerProvider::new("workspaces"),
    ));
    let emit: super::ChromeIntentEmitter = ChromeIntentEmitter::of(
        crate::app::interaction::InteractionSource::Keyboard,
        |_, _| {},
    );
    // The component reads its model from its own state, so the fixture puts it there rather
    // than handing it to the context (F003/P086/T367).
    *chrome.workspaces.tree_mut() = tree.clone();
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let ctx = crate::providers::ChromeCtx::for_build(
        crate::host::App::new(chrome),
        theme,
        &emit,
        &catalog,
    );
    super::build_region_content(&host, super::RegionId::LeftSidebar, &ctx, signals, drag)
}
