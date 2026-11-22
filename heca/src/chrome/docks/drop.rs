//! **What dropping a dock means** — the host's, because the host is what seats docks.
//!
//! The seat round every dock is carried by the window's move key and is a place to land beside;
//! the space between two docks and an empty sidebar are places to land too (`window/scene`). The
//! framework hands back what was carried and where it landed; this answers with the
//! `chrome.container.*` action that means, so a drop moves a dock exactly as a key or RPC does.

use crate::chrome::{ChromeDragItem, RegionId};
use crate::input::WmAction;
use heca_grid_ui::drag::DropSide;

/// The action dropping dock `dock` means: beside another dock (before it on its first half, after
/// it on its second), after the dock above a gap, or into an empty region named `target_name`.
/// `None` when it means nothing.
pub(crate) fn dock_drop_action(
    dock: &str,
    target: Option<ChromeDragItem>,
    target_name: &str,
    side: DropSide,
) -> Option<WmAction> {
    let container_id = dock.to_string();
    match target {
        Some(ChromeDragItem::Dock(other)) if other == dock => None,
        Some(ChromeDragItem::Dock(other)) => Some(match side {
            DropSide::Before => WmAction::ReorderContainerBefore {
                container_id,
                before_id: Some(other),
            },
            DropSide::Onto | DropSide::After => WmAction::ReorderContainerAfter {
                container_id,
                after_id: other,
            },
        }),
        Some(ChromeDragItem::DockGap { after }) if after == dock => None,
        Some(ChromeDragItem::DockGap { after }) => Some(WmAction::ReorderContainerAfter {
            container_id,
            after_id: after,
        }),
        Some(_) => None,
        // **An empty region**, by the name it is written as.
        None => RegionId::ALL
            .into_iter()
            .find(|region| region.as_str() == target_name)
            .map(|region| WmAction::MoveContainerToRegion {
                container_id,
                region,
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dock dropped on a dock lands before or after it, in the gap after the one above, and in an
    /// empty sidebar moves there; on itself it means nothing.
    #[test]
    fn a_dock_drop_means_one_container_action() {
        let dock = |id: &str| Some(ChromeDragItem::Dock(id.into()));
        assert_eq!(
            dock_drop_action("notes", dock("workspaces"), "workspaces", DropSide::Before),
            Some(WmAction::ReorderContainerBefore {
                container_id: "notes".into(),
                before_id: Some("workspaces".into()),
            })
        );
        assert_eq!(
            dock_drop_action("notes", dock("workspaces"), "workspaces", DropSide::After),
            Some(WmAction::ReorderContainerAfter {
                container_id: "notes".into(),
                after_id: "workspaces".into(),
            })
        );
        assert_eq!(
            dock_drop_action(
                "notes",
                Some(ChromeDragItem::DockGap {
                    after: "workspaces".into()
                }),
                "dock-gap:workspaces",
                DropSide::Onto
            ),
            Some(WmAction::ReorderContainerAfter {
                container_id: "notes".into(),
                after_id: "workspaces".into(),
            })
        );
        assert_eq!(
            dock_drop_action("notes", None, "sidebar.right", DropSide::Onto),
            Some(WmAction::MoveContainerToRegion {
                container_id: "notes".into(),
                region: RegionId::RightSidebar,
            })
        );
        assert_eq!(
            dock_drop_action("notes", dock("notes"), "notes", DropSide::After),
            None
        );
        assert_eq!(
            dock_drop_action("notes", None, "pane:3", DropSide::Onto),
            None
        );
    }
}
