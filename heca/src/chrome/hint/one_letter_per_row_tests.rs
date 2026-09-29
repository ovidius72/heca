//! **A pane pick letters the pane's row in each dock, and nothing else in the sidebar.**
//!
//! Written while chasing a report (Antonio, driving, 2026-09-24): with two docks in the left sidebar,
//! `prefix+q` drew a second keycap over the rule between them. This test showed the sidebar was NOT
//! the source — the stray keycap belonged to a pane tile behind the sidebar, lettered through the
//! column path with no visibility check (fixed in `column::offer_to_columns`, held by
//! `column::offer_tests`). It stays as the guard that the sidebar half is right.

use heca_grid_ui::builders::{LayoutExt, Parent};
use heca_grid_ui::reactive::SignalGet;
use heca_grid_ui::widgets::Flex;
use heca_grid_ui::{Component, LayoutEngine, Rectangle, Size};

use crate::app_state::SidebarItemState;
use crate::chrome::{ChromeHost, ChromeIntentEmitter, SharedChromeState};
use crate::providers::workspaces::{
    ColumnEntry, PaneEntry, WorkspaceEntry, WorkspaceTree, pane_key,
};
use crate::providers::{ChromeCtx, WorkspacesContainerProvider};
use heca_core::layout::{ColumnId, PaneId, WorkspaceId};

/// One workspace, eight panes, one per column — the screenshot's shape. Pane 5 is the active one.
fn eight_panes() -> WorkspaceTree {
    let mut tree = WorkspaceTree::new();
    tree.workspaces.push(WorkspaceEntry {
        ws_idx: 0,
        ws_id: WorkspaceId(0),
        name: "Workspace 1".into(),
        custom_name: None,
        collapsed: false,
        state: SidebarItemState::Active,
        columns: (1..=8u64)
            .map(|n| ColumnEntry {
                col_idx: n as usize - 1,
                col_id: ColumnId(n),
                collapsed: false,
                panes: vec![PaneEntry {
                    pane_id: PaneId(n),
                    name: "zsh".into(),
                    custom_name: None,
                    state: if n == 5 {
                        SidebarItemState::Active
                    } else {
                        SidebarItemState::None
                    },
                }],
            })
            .collect(),
        floating_panes: Vec::new(),
    });
    tree
}

/// Every widget in `node` showing `letter`, with its box and its name.
fn wearing(node: &dyn Component, letter: &str, out: &mut Vec<(Rectangle, String)>) {
    if node.base().hint_label.get_untracked().as_deref() == Some(letter) {
        let name = node
            .base()
            .key
            .clone()
            .or_else(|| node.base().scope_key.clone().map(|s| format!("scope {s}")))
            .unwrap_or_else(|| "(unnamed)".into());
        out.push((node.base().bounds, name));
    }
    for child in &node.base().children {
        wearing(child.as_ref(), letter, out);
    }
}

#[test]
fn a_pane_pick_letters_its_row_in_each_dock_and_nothing_else() {
    for lower_dock_scrolled in [0.0, 40.0, 80.0, 200.0] {
        pick_in_two_docks(lower_dock_scrolled);
    }
}

fn pick_in_two_docks(lower_dock_scrolled: f32) {
    use heca_grid_ui::reactive::SignalUpdate;
    let chrome = SharedChromeState::new(280.0, true, 260.0, false);
    // The screenshot's lower dock was scrolled: its workspace header was off the top.
    chrome
        .container_scroll("workspaces.left2")
        .set(lower_dock_scrolled);
    *chrome.workspaces.tree_mut() = eight_panes();
    chrome.workspaces.set_active_pane(Some(PaneId(5)));

    // Two docks in one sidebar, as the app starts today.
    let mut host = ChromeHost::new(chrome.events());
    host.register(Box::new(WorkspacesContainerProvider::new("workspaces")));
    host.register(Box::new(WorkspacesContainerProvider::new(
        "workspaces.left2",
    )));

    let theme = crate::chrome::GuiTheme::default();
    let emit = ChromeIntentEmitter::of(
        crate::app::interaction::InteractionSource::Keyboard,
        |_, _| {},
    );
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let ctx = ChromeCtx::for_build(crate::host::App::new(&chrome), &theme, &emit, &catalog);
    let body = crate::chrome::build_region_content(
        &host,
        crate::chrome::RegionId::LeftSidebar,
        &ctx,
        &mut crate::chrome::ChromeSignals::default(),
        &mut crate::chrome::DragItemRegistry::default(),
    )
    .expect("two docks are seated");

    // The screenshot's sidebar: 580 wide, 1290 tall.
    let mut root: Box<dyn Component> =
        Box::new(Flex::column().width(580.0).height(1290.0).child(body));
    LayoutEngine::new().compute(root.as_mut(), Size::new(580.0, 1290.0));

    // `prefix+q` offers pane 4 the letter `c`, by the pane's name — the same call the app makes.
    heca_grid_ui::offer_hint_by_key(root.as_ref(), &pane_key(PaneId(4)), Some("c".into()));

    let mut shown = Vec::new();
    wearing(root.as_ref(), "c", &mut shown);
    for (bounds, name) in &shown {
        println!("scrolled {lower_dock_scrolled}: `c` on {name} at {bounds:?}");
    }
    assert_eq!(
        shown.len(),
        2,
        "one keycap per dock, both on pane 4's row: {shown:?}",
    );
    assert!(
        shown.iter().all(|(_, name)| name == &pane_key(PaneId(4))),
        "every keycap sits on the row named for the pane, never on a dock or a header: {shown:?}",
    );
}
