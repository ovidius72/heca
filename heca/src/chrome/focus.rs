//! Chrome **keyboard focus** — which mounted dock the keyboard is aimed at (F003/P011/T020).
//!
//! Focus is a **container id**. Not a side: a sidebar is a shell and left/right is only a position,
//! so a dock is focused wherever it happens to be seated and stays focused when it is moved. The
//! bug this replaces read `left_visible()` and expanded the *left* container, while the workspaces
//! dock — which declares `RegionSet::sidebars()` — may well be seated on the right.
//!
//! The state itself lives on [`SharedChromeState`](super::SharedChromeState) (with the
//! per-placement keyboard-target signals derived from it). What lives here is the part that answers
//! *which* dock: pure functions over the [`ChromeHost`], so they can be tested without a window and
//! so no caller has to re-derive the order the host maintains.

use super::RegionId;
use super::contribution::ContainerId;
use super::host::ChromeHost;

/// The docks a pick offers, in the host's order, one letter each.
///
/// `on_screen` says which regions currently render their containers: a letter stamped over a body
/// that is not drawn is a key that appears to do nothing, so a dock in a hidden (or not-yet-rendered)
/// region is not offered. It can still be focused directly by id — `focus_dock(dock="…")` — which is
/// the path RPC and scripting take anyway.
///
/// Candidates run out after the shared 52-letter alphabet (`a`–`z`, `A`–`Z`), the same cap every
/// other pick has.
pub(crate) fn dock_candidates(
    host: &ChromeHost,
    on_screen: impl Fn(RegionId) -> bool,
) -> Vec<(char, ContainerId)> {
    RegionId::ALL
        .iter()
        .copied()
        .filter(|region| on_screen(*region))
        .flat_map(|region| host.contributions(region).iter())
        .enumerate()
        .filter_map(|(i, mounted)| {
            crate::app::selection::candidate_letter(i).map(|ch| (ch, mounted.id().to_string()))
        })
        .collect()
}

/// The dock a "focus the sidebar and navigate it" action should act on, and the region it is seated
/// in — `None` when no mounted container has keyboard navigation of its own.
///
/// The currently focused dock wins when it is navigable, so repeating the action does not jump
/// somewhere else; otherwise it is the first navigable dock in the host's order. The region comes
/// back with it because the caller has to make *that* region visible — which is how expanding the
/// wrong side stops being possible: nothing here has a side to name.
pub(crate) fn navigable_dock(
    host: &ChromeHost,
    focused: Option<&str>,
) -> Option<(ContainerId, RegionId)> {
    let mut navigable = RegionId::ALL.iter().copied().flat_map(|region| {
        host.contributions(region)
            .iter()
            .filter(|mounted| mounted.keyboard_navigable())
            .map(move |mounted| (mounted.id().to_string(), region))
    });
    let first = navigable.next()?;
    match focused {
        Some(id) if id == first.0 => Some(first),
        Some(id) => navigable.find(|(mounted, _)| mounted == id).or(Some(first)),
        None => Some(first),
    }
}

/// The placement a focus request names — **a mount id, or a component** (F003/P086/T363).
///
/// `global_focus` may be written with an `id`, naming one seating, or without, naming the component
/// as a whole. Both arrive here as one string, and an id wins: it is the more specific answer, and a
/// single-placement container's id and component name are usually the same word anyway.
///
/// Falling back to the component, the rule is the one `owning_mount`
/// ([`crate::providers`]) already uses for a declared action — **the seating you were last in**:
/// the focused one if it is of that component, else the last focused, else the first mounted. So a
/// key written once for a component of two placements keeps landing where the user was working,
/// rather than on whichever happens to be first in the host's order.
pub(crate) fn placement_for(
    host: &ChromeHost,
    target: &str,
    focused: Option<&str>,
    last_focused: Option<&str>,
) -> Option<ContainerId> {
    if host.provider(target).is_some() {
        return Some(target.to_string());
    }
    let is_target = |mount: &str| host.provider(mount).is_some_and(|p| p.kind() == target);
    focused
        .filter(|m| is_target(m))
        .or_else(|| last_focused.filter(|m| is_target(m)))
        .map(str::to_string)
        .or_else(|| {
            host.mounted_providers()
                .find(|p| p.kind() == target)
                .map(|p| p.id().to_string())
        })
}

/// Whether a region currently draws the containers seated in it.
///
/// Only the two sidebars host containers today: `build_chrome_root` builds region content for those,
/// and the bars take toolbar groups / status segments instead (§3.1.1). So a container seated in a
/// bar is deliberately reported as not on screen rather than optimistically offered a letter.
pub(crate) fn region_on_screen(state: &crate::app_state::AppState, region: RegionId) -> bool {
    match region {
        RegionId::LeftSidebar => state.left_sidebar_width() > 0.0,
        RegionId::RightSidebar => state.right_sidebar_width() > 0.0,
        RegionId::TopBar | RegionId::BottomBar => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::{ChromeEventBus, Contribution, RegionSet};
    use crate::providers::{ChromeCtx, Provider};

    /// A stand-in dock: an id, where it sits, and whether it navigates.
    struct Dock {
        id: String,
        region: RegionId,
        navigable: bool,
    }

    impl Dock {
        fn new(id: &str, region: RegionId, navigable: bool) -> Self {
            Self {
                id: id.to_string(),
                region,
                navigable,
            }
        }
    }

    impl Provider for Dock {
        fn id(&self) -> &str {
            &self.id
        }
        /// One component, however many seatings — which is what makes `placement_for`'s fallback
        /// order testable at all.
        fn kind(&self) -> &str {
            "dock"
        }
        fn supported_regions(&self) -> RegionSet {
            RegionSet::sidebars()
        }
        fn default_region(&self) -> RegionId {
            self.region
        }
        fn title(&self) -> &str {
            &self.id
        }
        fn keyboard_navigable(&self) -> bool {
            self.navigable
        }
        fn build_contribution(&self, _ctx: &ChromeCtx<'_>) -> Contribution {
            // These tests read placement + metadata only; the render seam is exercised on the real
            // provider (`providers/workspaces.rs`).
            unimplemented!("a test dock contributes no body")
        }
    }

    fn host(docks: Vec<Dock>) -> ChromeHost {
        let mut host = ChromeHost::new(ChromeEventBus::default());
        for dock in docks {
            host.register(Box::new(dock));
        }
        host
    }

    fn everywhere(_: RegionId) -> bool {
        true
    }

    #[test]
    fn a_pick_offers_every_on_screen_dock_in_the_hosts_order() {
        let host = host(vec![
            Dock::new("workspaces", RegionId::LeftSidebar, true),
            Dock::new("docker", RegionId::RightSidebar, false),
            Dock::new("notes", RegionId::RightSidebar, false),
        ]);
        assert_eq!(
            dock_candidates(&host, everywhere),
            // The first three of the shared alphabet, which is home row first (F003/P082/T443).
            vec![
                ('a', "workspaces".to_string()),
                ('s', "docker".to_string()),
                ('d', "notes".to_string()),
            ],
        );
    }

    #[test]
    fn a_dock_in_a_region_that_is_not_drawn_gets_no_letter() {
        let host = host(vec![
            Dock::new("workspaces", RegionId::LeftSidebar, true),
            Dock::new("docker", RegionId::RightSidebar, false),
        ]);
        let left_only = |region| region == RegionId::LeftSidebar;
        assert_eq!(
            dock_candidates(&host, left_only),
            vec![('a', "workspaces".to_string())],
            "a letter over a body nobody can see is a key that does nothing",
        );
        assert!(
            dock_candidates(&host, |_| false).is_empty(),
            "both sidebars hidden ⇒ nothing to pick",
        );
    }

    /// The live bug: the dock is seated on the **right**, and focusing it must act on the right.
    #[test]
    fn focus_follows_the_dock_to_whichever_region_it_sits_in() {
        let mut host = host(vec![Dock::new("workspaces", RegionId::LeftSidebar, true)]);
        assert_eq!(
            navigable_dock(&host, None),
            Some(("workspaces".to_string(), RegionId::LeftSidebar)),
        );
        host.move_container("workspaces", RegionId::RightSidebar)
            .expect("the dock supports both sidebars");
        assert_eq!(
            navigable_dock(&host, None),
            Some(("workspaces".to_string(), RegionId::RightSidebar)),
            "the same dock, now on the right — nothing names a side",
        );
        // And a focus already held survives the move, because it is an id.
        assert_eq!(
            navigable_dock(&host, Some("workspaces")),
            Some(("workspaces".to_string(), RegionId::RightSidebar)),
        );
    }

    /// A **placement id wins over a component name** — it is the more specific answer, and it is
    /// what an entry with an `id` means (F003/P086/T363).
    #[test]
    fn a_focus_target_resolves_as_a_placement_before_a_component() {
        let host = host(vec![
            Dock::new("workspaces", RegionId::LeftSidebar, true),
            Dock::new("workspaces.right", RegionId::RightSidebar, true),
        ]);
        assert_eq!(
            placement_for(&host, "workspaces.right", None, None),
            Some("workspaces.right".to_string()),
        );
    }

    /// Named by **component**, it lands on the seating you were last in — the rule `owning_mount`
    /// uses for a declared action, so one key written once keeps following the user.
    #[test]
    fn a_component_name_lands_on_the_seating_you_were_last_in() {
        // Both docks report `kind() == "dock"` (the stand-in's fixed kind), so the name is ambiguous
        // by construction — which is exactly the case the fallback order exists for.
        let host = host(vec![
            Dock::new("first", RegionId::LeftSidebar, true),
            Dock::new("second", RegionId::RightSidebar, true),
        ]);
        assert_eq!(
            placement_for(&host, "dock", Some("second"), None),
            Some("second".to_string()),
            "the focused seating of that component wins",
        );
        assert_eq!(
            placement_for(&host, "dock", None, Some("second")),
            Some("second".to_string()),
            "…else the one focused most recently",
        );
        assert_eq!(
            placement_for(&host, "dock", None, None),
            Some("first".to_string()),
            "…else the first mounted, in the host's order",
        );
        assert_eq!(
            placement_for(&host, "dock", Some("gone"), Some("second")),
            Some("second".to_string()),
            "a hint naming a mount that is not of this component is skipped, not obeyed",
        );
    }

    #[test]
    fn a_focus_target_naming_nothing_mounted_resolves_to_nothing() {
        let host = host(vec![Dock::new("workspaces", RegionId::LeftSidebar, true)]);
        assert_eq!(
            placement_for(&host, "docker", None, None),
            None,
            "a typo in a binding's `dock` must be visible, not a silent no-op",
        );
    }

    #[test]
    fn with_no_navigable_dock_there_is_nothing_to_focus() {
        let host = host(vec![Dock::new("docker", RegionId::LeftSidebar, false)]);
        assert_eq!(
            navigable_dock(&host, None),
            None,
            "a dock that only scrolls is not something to navigate",
        );
        assert_eq!(
            navigable_dock(&ChromeHost::new(ChromeEventBus::default()), None),
            None
        );
    }

    #[test]
    fn the_focused_dock_keeps_the_focus_when_it_is_navigable() {
        let host = host(vec![
            Dock::new("workspaces", RegionId::LeftSidebar, true),
            Dock::new("files", RegionId::RightSidebar, true),
        ]);
        assert_eq!(
            navigable_dock(&host, Some("files")),
            Some(("files".to_string(), RegionId::RightSidebar)),
            "repeating the action stays where it is",
        );
        // A focus on a dock that cannot navigate (or is gone) falls back to the first that can.
        assert_eq!(
            navigable_dock(&host, Some("vanished")),
            Some(("workspaces".to_string(), RegionId::LeftSidebar)),
        );
    }
}
