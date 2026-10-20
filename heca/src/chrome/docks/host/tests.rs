//! Seating, naming each placement, and moving or reordering a dock between regions.

use super::*;
use crate::chrome::{Contribution, RegionSet};
use crate::providers::ChromeCtx;
use std::cell::RefCell;
use std::rc::Rc;

/// **Two placements never answer to one name.**
///
/// A name is how everything else refers to a seating — a keybinding, the action registry,
/// `focus_dock`, RPC. Two of them under one name means one is unreachable, and the reference
/// the author wrote lands on whichever the host happened to find first. Saying so is
/// `warn_author`; the app keeps running under a name the host picked, which is deliberately
/// not a name anything can refer to.
#[test]
fn seating_one_container_twice_gives_each_placement_its_own_name() {
    let mut host = ChromeHost::new(ChromeEventBus::default());
    let dock = |region| {
        Box::new(TestProvider::new(
            "workspaces",
            RegionSet::sidebars(),
            region,
        )) as Box<dyn Provider>
    };
    host.seat(
        RegionId::LeftSidebar,
        dock(RegionId::LeftSidebar).into(),
        false,
    );
    host.seat(
        RegionId::RightSidebar,
        dock(RegionId::RightSidebar).into(),
        false,
    );
    host.seat(
        RegionId::LeftSidebar,
        dock(RegionId::LeftSidebar).into(),
        false,
    );

    let names: Vec<&str> = RegionId::ALL
        .iter()
        .flat_map(|r| host.contributions(*r).iter().map(|m| m.id()))
        .collect();
    assert_eq!(
        names.iter().collect::<std::collections::HashSet<_>>().len(),
        3,
        "three placements, three names: {names:?}",
    );
    assert!(
        names.contains(&"workspaces"),
        "the first keeps the id it was given, so existing bindings and config still mean what \
         they meant: {names:?}",
    );
}

/// A minimal provider that carries only its own metadata. Placement, ordering and moves never ask a
/// dock for its body, so `build_contribution` is unreachable here.
struct TestProvider {
    id: String,
    supported: RegionSet,
    default_region: RegionId,
}

impl TestProvider {
    fn new(id: &str, supported: RegionSet, default_region: RegionId) -> Self {
        Self {
            id: id.to_string(),
            supported,
            default_region,
        }
    }
}

impl Provider for TestProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn supported_regions(&self) -> RegionSet {
        self.supported
    }
    fn default_region(&self) -> RegionId {
        self.default_region
    }
    fn title(&self) -> &str {
        &self.id
    }
    fn build_contribution(&self, _ctx: &ChromeCtx<'_>) -> Contribution {
        // These tests exercise placement/ordering/moves, which read provider
        // *metadata* only — the host never calls the render seam here. (The real
        // seam is implemented and tested on `WorkspacesContainerProvider`.)
        unimplemented!("TestProvider contributes no body; see WorkspacesContainerProvider")
    }
}

fn host() -> ChromeHost {
    ChromeHost::new(ChromeEventBus::default())
}

fn ids(host: &ChromeHost, region: RegionId) -> Vec<String> {
    host.contributions(region)
        .iter()
        .map(|m| m.id().to_string())
        .collect()
}

#[test]
fn register_seats_at_default_region_in_the_order_registered() {
    let mut h = host();
    // The list is the order things were added in; nothing on the provider reorders it.
    h.register(Box::new(TestProvider::new(
        "b",
        RegionSet::sidebars(),
        RegionId::LeftSidebar,
    )));
    h.register(Box::new(TestProvider::new(
        "a",
        RegionSet::sidebars(),
        RegionId::LeftSidebar,
    )));
    assert_eq!(ids(&h, RegionId::LeftSidebar), ["b", "a"]);
    assert_eq!(h.placement("a"), Some(RegionId::LeftSidebar));
    assert!(h.contributions(RegionId::RightSidebar).is_empty());
}

#[test]
fn move_to_supported_region_succeeds_and_emits() {
    let mut h = host();
    let seen: Rc<RefCell<Vec<(String, RegionId)>>> = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    let _sub = h
        .events
        .subscribe("chrome.container.placement.changed", move |e| {
            if let ChromeEvent::ContainerPlacementChanged {
                container_id,
                region,
            } = e
            {
                log.borrow_mut().push((container_id.clone(), *region));
            }
        });

    h.register(Box::new(TestProvider::new(
        "ws",
        RegionSet::sidebars(),
        RegionId::LeftSidebar,
    )));
    h.move_container("ws", RegionId::RightSidebar).unwrap();

    assert!(h.contributions(RegionId::LeftSidebar).is_empty());
    assert_eq!(ids(&h, RegionId::RightSidebar), ["ws"]);
    assert_eq!(h.placement("ws"), Some(RegionId::RightSidebar));
    assert_eq!(
        seen.borrow().as_slice(),
        [("ws".to_string(), RegionId::RightSidebar)]
    );
}

#[test]
fn move_to_unsupported_region_is_rejected() {
    let mut h = host();
    h.register(Box::new(TestProvider::new(
        "ws",
        RegionSet::of(&[RegionId::LeftSidebar]), // left only
        RegionId::LeftSidebar,
    )));
    assert_eq!(
        h.move_container("ws", RegionId::BottomBar),
        Err(MoveError::UnsupportedRegion)
    );
    // Unchanged.
    assert_eq!(ids(&h, RegionId::LeftSidebar), ["ws"]);
}

#[test]
fn move_unknown_container_errors() {
    let mut h = host();
    assert_eq!(
        h.move_container("nope", RegionId::LeftSidebar),
        Err(MoveError::UnknownContainer)
    );
}

#[test]
fn reorder_moves_before_target() {
    let mut h = host();
    for id in ["a", "b", "c"] {
        h.register(Box::new(TestProvider::new(
            id,
            RegionSet::sidebars(),
            RegionId::LeftSidebar,
        )));
    }
    assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "b", "c"]);
    // Move "c" before "a".
    h.reorder("c", Some("a")).unwrap();
    assert_eq!(ids(&h, RegionId::LeftSidebar), ["c", "a", "b"]);
    // Move "c" to the end.
    h.reorder("c", None).unwrap();
    assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "b", "c"]);
    // A missing `before` target errors and leaves the order untouched.
    assert_eq!(h.reorder("a", Some("zzz")), Err(MoveError::TargetNotFound));
    assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "b", "c"]);
}

/// `reorder_after` is the mirror of `reorder(before)` — "put X after Y" resolves to "before
/// whatever follows Y", which only the host can compute.
#[test]
fn reorder_moves_after_target() {
    let mut h = host();
    for id in ["a", "b", "c"] {
        h.register(Box::new(TestProvider::new(
            id,
            RegionSet::sidebars(),
            RegionId::LeftSidebar,
        )));
    }
    assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "b", "c"]);
    // Move "a" after "b" → it lands before "c".
    h.reorder_after("a", "b").unwrap();
    assert_eq!(ids(&h, RegionId::LeftSidebar), ["b", "a", "c"]);
    // After the LAST container ⇒ the end.
    h.reorder_after("b", "c").unwrap();
    assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "c", "b"]);
    // Already immediately after the target ⇒ no-op, not a shuffle.
    h.reorder_after("c", "a").unwrap();
    assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "c", "b"]);
    // A missing target errors and leaves the order untouched.
    assert_eq!(h.reorder_after("a", "zzz"), Err(MoveError::TargetNotFound));
    assert_eq!(ids(&h, RegionId::LeftSidebar), ["a", "c", "b"]);
}
