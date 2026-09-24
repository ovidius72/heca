// ChromeHost is a foundation **runtime** (plugin-02): it is exercised by tests
// today and consumed by first-party providers + the render path in plugin-03, so
// some methods read as dead code in the binary now. Same allow rationale as
// `providers/mod.rs` / `chrome/events.rs`.
#![allow(dead_code)]

//! [`ChromeHost`] — the host/runtime that owns all pluggable chrome regions
//! (contract §3.1.1). It keeps a registry of mounted containers **per
//! [`RegionId`]**, tracks their order and placement, and owns host-level
//! move/reorder between compatible regions (validated against each container's
//! `supported_regions`). Host-level container movement is distinct from
//! container-*internal* drag/drop, which stays inside the mounted container
//! (§2.9).
//!
//! **plugin-02 scope — runtime only, no render.** The app hand-paints its chrome
//! today and wires no provider yet, so `ChromeHost` here is the pure data/runtime
//! core: it seats containers from their [`Provider`] *metadata* (it never calls
//! [`Provider::build_contribution`], the render seam) and exposes move/reorder as
//! plain methods that the `WmAction` layer drives (input parity, §2.9). Rendering
//! and the first real provider land in plugin-03.

use std::collections::HashMap;

use super::contribution::ContainerId;
use super::{ChromeEvent, ChromeEventBus, RegionId};
use crate::providers::{ChromeCtx, Provider, ProviderHandles};

/// A provider seated in a region, with its activation handles once activated.
pub struct MountedContribution {
    /// **This seating's own name**, given by the host when it seated it.
    ///
    /// Usually the provider's id. When that id is already seated somewhere, the host numbers this
    /// one — so putting the same dock in both sidebars is two working placements, each with its own
    /// cursor, scroll position and letter, and **the caller names nothing**. It used to be written
    /// by hand at the call site (`::named("workspaces.right")`), which meant a plugin author placing
    /// a dock twice had to invent an id and know why.
    id: ContainerId,
    provider: Box<dyn Provider>,
    /// What whoever added this dock said about its size and place — wins over the dock's own body.
    placement: super::regions::DockPlacement,
    /// `Some` once [`Provider::on_activate`] has run (plugin-03 wiring); the held
    /// subscriptions live as long as this stays mounted.
    handles: Option<ProviderHandles>,
}

impl MountedContribution {
    /// **This seating's own name** — see [`id`](Self::id).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What whoever added this dock said about its size and place (`.flex` / `.order` on the dock
    /// when it was appended). Wins over what the dock's own body says.
    pub fn placement(&self) -> super::regions::DockPlacement {
        self.placement
    }

    /// Human title.
    pub fn title(&self) -> &str {
        self.provider.title()
    }

    /// Host-level move/reorder allowed?
    pub fn movable(&self) -> bool {
        self.provider.movable()
    }

    /// Does this container do anything with keyboard focus beyond scrolling?
    /// (`Provider::keyboard_navigable`, F003/P011/T020.)
    pub fn keyboard_navigable(&self) -> bool {
        self.provider.keyboard_navigable()
    }

    /// The underlying provider (for the render path to build its contribution).
    pub fn provider(&self) -> &dyn Provider {
        self.provider.as_ref()
    }

    /// The component **type** — the namespace its bindings and declared actions belong to
    /// (`Provider::kind`, F003/P085/T353).
    pub fn kind(&self) -> &str {
        self.provider.kind()
    }

    /// Run the provider's activation, keeping its subscription handles alive.
    /// Called by the render/app wiring (plugin-03), not in plugin-02.
    pub fn activate(&mut self, ctx: &ChromeCtx) {
        if self.handles.is_none() {
            self.handles = Some(self.provider.on_activate(ctx));
        }
    }

    /// This mount's activation handles, created empty on first ask.
    ///
    /// The host puts a declared action's [`ActionHandle`](crate::actions::ActionHandle) here so the
    /// action dies with the mount, exactly as an event subscription does. Created on demand because
    /// a provider that never implemented `on_activate` still has actions to retire.
    pub fn handles_mut(&mut self) -> &mut ProviderHandles {
        self.handles.get_or_insert_with(ProviderHandles::default)
    }
}

/// One region's ordered list of mounted containers plus its host-level
/// visibility. (This is placement/ordering state — the *shell* mode/size still
/// lives on `SharedChromeState`'s `left`/`right` until top/bottom bars exist.)
pub struct RegionHost {
    contributions: Vec<MountedContribution>,
    visible: bool,
}

impl RegionHost {
    fn new() -> Self {
        Self {
            contributions: Vec::new(),
            visible: true,
        }
    }

    /// Add `mc` at the end — the region's list is the order things were added in. A container that
    /// must sit somewhere else says so itself, with `.order(..)` on the body it builds.
    fn push(&mut self, mc: MountedContribution) {
        self.contributions.push(mc);
    }
}

/// Why a host-level container move/reorder was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    /// No container is registered under that id.
    UnknownContainer,
    /// The target region is not in the container's `supported_regions`.
    UnsupportedRegion,
    /// A `reorder` named a `before` target that isn't registered in the region.
    TargetNotFound,
}

impl std::fmt::Display for MoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoveError::UnknownContainer => f.write_str("no container registered under that id"),
            MoveError::UnsupportedRegion => {
                f.write_str("target region is not in the container's supported_regions")
            }
            MoveError::TargetNotFound => {
                f.write_str("reorder target container is not in the region")
            }
        }
    }
}

impl std::error::Error for MoveError {}

/// Owns all four chrome regions, container placement, and host-level moves.
pub struct ChromeHost {
    /// One list of containers per region.
    regions: super::regions::RegionMap<RegionHost>,
    /// Reverse index: which region each container currently lives in.
    placement: HashMap<ContainerId, RegionId>,
    /// What each region was told about arranging its own contents, by [`RegionId::index`].
    layouts: super::regions::RegionMap<super::regions::RegionLayout>,
    events: ChromeEventBus,
}

impl ChromeHost {
    /// A new host with four empty regions, wired to the chrome event bus (from
    /// `SharedChromeState::events()`).
    pub fn new(events: ChromeEventBus) -> Self {
        // A new host is starting, so `regions(..)` calls are taken again until it mounts them.
        super::regions::open_for_startup();
        Self {
            regions: super::regions::RegionMap::from_fn(|_| RegionHost::new()),
            placement: HashMap::new(),
            layouts: Default::default(),
            events,
        }
    }

    /// Register a provider, seating its container at the end of its `default_region`. Reads only
    /// provider metadata — the render seam (`build_contribution`) is not touched here.
    pub fn register(&mut self, provider: Box<dyn Provider>) {
        let region = provider.default_region();
        self.seat(region, provider.into(), false);
    }

    /// Seat one container in `region` — the one place a container enters the host, whether it came
    /// from [`regions`](super::regions) or from its own default region. `front` puts it first.
    fn seat(&mut self, region: RegionId, placed: super::regions::Placed, front: bool) {
        let super::regions::Placed {
            provider,
            placement,
        } = placed;
        let id = self.free_mount_id(provider.id());
        debug_assert!(
            provider.supported_regions().contains(region),
            "a container was seated in a region it does not support: '{}' in {:?}, supports {:?}",
            provider.id(),
            region,
            provider.supported_regions(),
        );
        let mc = MountedContribution {
            id: id.clone(),
            provider,
            placement,
            handles: None,
        };
        let list = &mut self.regions[region];
        if front {
            list.contributions.insert(0, mc);
        } else {
            list.push(mc);
        }
        self.placement.insert(id, region);
    }

    /// Take out every container in `region` that `keep` says no to, by name. Returns how many went.
    fn unseat_where(&mut self, region: RegionId, keep: impl Fn(&str) -> bool) -> usize {
        let list = &mut self.regions[region].contributions;
        let before = list.len();
        let (stay, gone): (Vec<_>, Vec<_>) =
            std::mem::take(list).into_iter().partition(|m| keep(m.id()));
        *list = stay;
        for m in &gone {
            self.placement.remove(m.id());
        }
        before - self.regions[region].contributions.len()
    }

    /// **A name nobody else is using**, from the one the provider gave.
    ///
    /// The first seating of a container keeps its own id, so every existing name, binding and
    /// config line still means what it meant. A second is `workspaces.2`, a third `workspaces.3`.
    /// The host does this because it is the only thing that knows what is already seated — asking
    /// the caller to know it is how `::named("workspaces.right")` ended up written in the app and
    /// would have had to be written again by every plugin.
    fn free_mount_id(&self, wanted: &str) -> ContainerId {
        if !self.placement.contains_key(wanted) {
            return wanted.to_string();
        }
        // **Say so.** A name is how everything else refers to this seating — a keybinding, the
        // action registry, `focus_dock`, RPC — so two placements answering to one name is a
        // mistake, not a shorthand. The app keeps working under a name the host picks, but that
        // name is not one the author chose, so nothing they write can reach it.
        super::identity::warn_author(format!(
            "[heca] a second container is already named '{wanted}' — give this placement its own \
             name, or nothing you write in config or a keybinding can refer to it",
        ));
        (2..)
            .map(|n| format!("{wanted}.{n}"))
            .find(|candidate| !self.placement.contains_key(candidate))
            .expect("the range is unbounded, so some number is free")
    }

    /// **Apply every `regions(..)` call made so far**, in the order they were made, and close the
    /// queue.
    ///
    /// A caller names a region before this host exists — a plugin loading at startup, the app's own
    /// wiring — so the changes wait and are applied here, once, as the app starts. A change made
    /// after this is refused out loud by [`regions`](super::regions); regions that change while the
    /// app runs are not built yet.
    pub fn mount_pending(&mut self) {
        use super::regions::RegionOp;
        for (region, op) in super::regions::take_for_startup() {
            match op {
                RegionOp::Append(p) => self.seat(region, p, false),
                RegionOp::Prepend(p) => self.seat(region, p, true),
                RegionOp::Remove(id) => {
                    if self.unseat_where(region, |m| m != id) == 0 {
                        super::identity::warn_author(format!(
                            "[heca] nothing called '{id}' is in region '{}' to remove — it holds: {}",
                            region.as_str(),
                            self.names_in(region),
                        ));
                    }
                }
                RegionOp::Retain(keep) => {
                    self.unseat_where(region, keep);
                }
                RegionOp::Gap(gap) => self.layouts[region].gap = Some(gap),
            }
        }
    }

    /// The names in `region`, for a message a person reads.
    fn names_in(&self, region: RegionId) -> String {
        let names: Vec<&str> = self.contributions(region).iter().map(|m| m.id()).collect();
        if names.is_empty() {
            "nothing".to_string()
        } else {
            names.join(", ")
        }
    }

    /// **How this region was told to arrange its contents** — the air between them. The render
    /// path asks this when it builds the region's body.
    pub fn layout(&self, region: RegionId) -> &super::regions::RegionLayout {
        &self.layouts[region]
    }

    /// The ordered containers currently mounted in `region`.
    pub fn contributions(&self, region: RegionId) -> &[MountedContribution] {
        &self.regions[region].contributions
    }

    /// Which region a container currently lives in, if registered.
    pub fn placement(&self, id: &str) -> Option<RegionId> {
        self.placement.get(id).copied()
    }

    /// Move a container to another region. Validated against the container's
    /// `supported_regions`; a no-op (still `Ok`) if it's already there. Emits
    /// [`ChromeEvent::ContainerPlacementChanged`] on a real move.
    pub fn move_container(&mut self, id: &str, to: RegionId) -> Result<(), MoveError> {
        let from = self
            .placement
            .get(id)
            .copied()
            .ok_or(MoveError::UnknownContainer)?;
        let idx = self.regions[from]
            .contributions
            .iter()
            .position(|m| m.id() == id)
            .ok_or(MoveError::UnknownContainer)?;

        if !self.regions[from].contributions[idx]
            .provider
            .supported_regions()
            .contains(to)
        {
            return Err(MoveError::UnsupportedRegion);
        }
        if from == to {
            return Ok(());
        }

        let mc = self.regions[from].contributions.remove(idx);
        self.regions[to].push(mc);
        self.placement.insert(id.to_string(), to);
        self.events.emit(ChromeEvent::ContainerPlacementChanged {
            container_id: id.to_string(),
            region: to,
        });
        Ok(())
    }

    /// Reorder a container within its region, inserting it immediately before
    /// `before` (or at the end when `before` is `None`). A `before` that names a
    /// container not in the region is an error ([`MoveError::TargetNotFound`]) —
    /// "put X before Y" fails loudly rather than silently appending. Emits
    /// [`ChromeEvent::ContainerPlacementChanged`].
    pub fn reorder(&mut self, id: &str, before: Option<&str>) -> Result<(), MoveError> {
        let region = self
            .placement
            .get(id)
            .copied()
            .ok_or(MoveError::UnknownContainer)?;
        let list = &mut self.regions[region].contributions;
        let from = list
            .iter()
            .position(|m| m.id() == id)
            .ok_or(MoveError::UnknownContainer)?;
        // Resolve the target position (in the pre-removal list) up front so a
        // missing target errors without mutating anything.
        let target = match before {
            Some(b) => Some(
                list.iter()
                    .position(|m| m.id() == b)
                    .ok_or(MoveError::TargetNotFound)?,
            ),
            None => None,
        };
        let mc = list.remove(from);
        // Removing `from` shifts every later index left by one, so adjust a target
        // that sat after it.
        let insert_at = match target {
            Some(t) if t > from => t - 1,
            Some(t) => t,
            None => list.len(),
        };
        list.insert(insert_at, mc);
        self.events.emit(ChromeEvent::ContainerPlacementChanged {
            container_id: id.to_string(),
            region,
        });
        Ok(())
    }

    /// Reorder a container within its region, inserting it immediately **after** `after`.
    ///
    /// The mirror of [`reorder`](Self::reorder). "Put X after Y" is not expressible as "put X before
    /// Z" without knowing what follows Y, which only the host knows — a caller (a plugin, a drag
    /// landing on the *trailing* edge of an item) cannot compute it. An `after` that names a
    /// container not in the region is an error ([`MoveError::TargetNotFound`]); when `after` is the
    /// last container, X lands at the end. Emits [`ChromeEvent::ContainerPlacementChanged`].
    pub fn reorder_after(&mut self, id: &str, after: &str) -> Result<(), MoveError> {
        let region = self
            .placement
            .get(id)
            .copied()
            .ok_or(MoveError::UnknownContainer)?;
        let list = &self.regions[region].contributions;
        // The container that follows `after` is the one to insert before; none ⇒ append (`None`).
        let after_idx = list
            .iter()
            .position(|m| m.id() == after)
            .ok_or(MoveError::TargetNotFound)?;
        let before = list.get(after_idx + 1).map(|m| m.id().to_string());
        self.reorder(id, before.as_deref())
    }

    /// The provider seated under `id`, across every region — how the action bridge reaches the
    /// component that owns a declared action id (F003/P085/T353).
    pub fn provider(&self, id: &str) -> Option<&dyn Provider> {
        self.regions
            .iter()
            .flat_map(|r| r.contributions.iter())
            .find(|m| m.id() == id)
            .map(|m| m.provider())
    }

    /// The mount that holds the activation handles for `id`, so its declared actions can be
    /// retired when it goes away.
    pub fn handles_mut(&mut self, id: &str) -> Option<&mut ProviderHandles> {
        self.regions
            .iter_mut()
            .flat_map(|r| r.contributions.iter_mut())
            .find(|m| m.id() == id)
            .map(|m| m.handles_mut())
    }

    /// Every mounted provider, across every region, in placement order.
    ///
    /// The host's "who is here right now" — used to collect what mounted providers contribute
    /// outside the render path (their context-menu entries, context-menu-5).
    pub fn mounted_providers(&self) -> impl Iterator<Item = &dyn Provider> + '_ {
        self.regions
            .iter()
            .flat_map(|r| r.contributions.iter())
            .map(|m| m.provider())
    }

    /// Set a region's host-level visibility.
    ///
    /// **plugin-02 caveat:** this flag has no visual effect yet. Region *shell*
    /// mode/size still lives on `SharedChromeState`'s `left`/`right` (see
    /// [`RegionHost`]); the render path that reads this host-level flag — and
    /// reconciles it with the shell state — arrives with the chrome render
    /// migration in plugin-03.
    pub fn set_region_visible(&mut self, region: RegionId, visible: bool) {
        self.regions[region].visible = visible;
    }

    /// Is a region host-level visible?
    pub fn is_region_visible(&self, region: RegionId) -> bool {
        self.regions[region].visible
    }
}

#[cfg(test)]
mod region_list_tests {
    use super::*;
    use crate::chrome::regions;
    use crate::providers::WorkspacesContainerProvider;

    fn ws(id: &str) -> WorkspacesContainerProvider {
        WorkspacesContainerProvider::new(id)
    }

    fn ids(host: &ChromeHost, r: RegionId) -> Vec<String> {
        host.contributions(r)
            .iter()
            .map(|c| c.id().to_string())
            .collect()
    }

    /// Start from an empty queue: build a host (which opens it) and apply whatever was left.
    fn fresh() -> ChromeHost {
        let mut h = ChromeHost::new(ChromeEventBus::default());
        h.mount_pending();
        ChromeHost::new(ChromeEventBus::default())
    }

    /// **A region is fetched by name and holds a list** — append, prepend, one or several, in the
    /// order the calls were made. The line a plugin author writes (P082(F003)/T518).
    #[test]
    fn a_region_fetched_by_name_holds_a_list_in_the_order_it_was_told() {
        let mut host = fresh();
        regions("sidebar.left").append(ws("b"));
        regions("sidebar.left").append([ws("c"), ws("d")]);
        regions("sidebar.left").prepend(ws("a"));
        regions("sidebar.right").prepend([ws("x"), ws("y")]);
        host.mount_pending();

        assert_eq!(ids(&host, RegionId::LeftSidebar), ["a", "b", "c", "d"]);
        assert_eq!(
            ids(&host, RegionId::RightSidebar),
            ["x", "y"],
            "a prepended list keeps the order it was written in",
        );
        assert_eq!(host.placement("x"), Some(RegionId::RightSidebar));
    }

    /// **Remove and retain act by name**, on what is there when they run — so a plugin can take out
    /// a built-in the app added before it.
    #[test]
    fn remove_and_retain_take_out_containers_by_name() {
        let mut host = fresh();
        regions("sidebar.left").append([ws("workspaces"), ws("docker"), ws("notes"), ws("git")]);
        regions("sidebar.left").remove("workspaces");
        regions("sidebar.left").retain(|id| id != "docker");
        host.mount_pending();

        assert_eq!(ids(&host, RegionId::LeftSidebar), ["notes", "git"]);
        assert_eq!(
            host.placement("workspaces"),
            None,
            "a removed container is nowhere"
        );
        assert_eq!(host.placement("docker"), None);
    }

    /// Removing a name the region does not hold is said out loud, with what it does hold.
    #[test]
    fn removing_a_name_that_is_not_there_is_reported() {
        let mut host = fresh();
        regions("sidebar.left").append(ws("notes"));
        regions("sidebar.left").remove("dokcer");
        host.mount_pending();

        assert_eq!(ids(&host, RegionId::LeftSidebar), ["notes"]);
        assert!(
            crate::chrome::identity::said()
                .iter()
                .any(|m| m.contains("'dokcer'") && m.contains("notes")),
            "the mistake and what the region holds are both named",
        );
    }

    /// **A name nobody knows is reported and changes nothing** — a plugin's typo must neither take
    /// the app down nor vanish.
    #[test]
    fn a_region_nobody_knows_is_reported_and_changes_nothing() {
        let mut host = fresh();
        regions("sidebar.lft").append(ws("lost"));
        host.mount_pending();

        assert_eq!(host.placement("lost"), None);
        assert!(
            crate::chrome::identity::said()
                .iter()
                .any(|m| m.contains("'sidebar.lft'") && m.contains("sidebar.left")),
            "the bad name and the real ones are both named",
        );
    }

    /// **After startup a change is refused out loud**, not dropped in silence — regions are set up
    /// once, as the app starts.
    #[test]
    fn a_change_after_startup_is_refused_out_loud() {
        let mut host = fresh();
        regions("sidebar.left").append(ws("early"));
        host.mount_pending();

        regions("sidebar.left").append(ws("late"));
        host.mount_pending();

        assert_eq!(ids(&host, RegionId::LeftSidebar), ["early"]);
        assert!(
            crate::chrome::identity::said()
                .iter()
                .any(|m| m.contains("after startup")),
            "the late call is reported",
        );
    }

    /// Every spelling of a region reads through the one parser RPC and config use.
    #[test]
    fn the_dotted_names_and_the_old_ones_mean_the_same_region() {
        for (name, region) in [
            ("sidebar.left", RegionId::LeftSidebar),
            ("left-sidebar", RegionId::LeftSidebar),
            ("left", RegionId::LeftSidebar),
            ("sidebar.right", RegionId::RightSidebar),
            ("bar.top", RegionId::TopBar),
            ("bar.bottom", RegionId::BottomBar),
        ] {
            assert_eq!(name.parse::<RegionId>(), Ok(region), "{name}");
        }
    }

    /// **A region keeps the air it was given between its containers** — the one thing it says about
    /// arranging them. Sizes are each container's own (`.flex` on its body).
    #[test]
    fn a_region_holds_the_gap_it_was_given() {
        let mut host = fresh();
        regions("sidebar.left").gap("md").append(ws("a"));
        host.mount_pending();

        assert_eq!(
            host.layout(RegionId::LeftSidebar).gap,
            Some(heca_grid_ui::style::Space::from("md")),
        );
        assert_eq!(
            host.layout(RegionId::TopBar).gap,
            None,
            "a region nobody spoke to keeps the default"
        );
    }
}

#[cfg(test)]
mod tests {

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
    use super::*;
    use crate::chrome::{Contribution, RegionSet};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A minimal stand-in for the future WorkspacesContainerProvider. Carries its
    /// own metadata; `build_contribution` is never called in plugin-02, so its
    /// build closure is unreachable.
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

    #[test]
    fn region_visibility_toggles() {
        let mut h = host();
        assert!(h.is_region_visible(RegionId::TopBar));
        h.set_region_visible(RegionId::TopBar, false);
        assert!(!h.is_region_visible(RegionId::TopBar));
    }
}
