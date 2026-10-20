//! [`ChromeHost`] — owns every chrome region's list of mounted docks: which region each one sits
//! in, in what order, and moving or reordering one between the regions it supports. Moving a dock
//! between regions is the host's job; dragging inside a dock stays the dock's own.
//!
//! A dock enters through [`regions`](fn@crate::chrome::regions), queued until the app starts and applied
//! by [`ChromeHost::mount_pending`]; the render path then asks each one for its body.

mod mounted;

#[cfg(test)]
mod region_list_tests;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use super::contribution::ContainerId;
use crate::chrome::{ChromeEvent, ChromeEventBus, RegionId};
use crate::providers::{ChromeCtx, Provider, ProviderHandles};

pub use mounted::MountedContribution;

/// One region's ordered list of mounted containers. (This is placement/ordering state — whether the
/// region is shown, and a sidebar's size, live on `SharedChromeState`.)
pub struct RegionHost {
    contributions: Vec<MountedContribution>,
}

impl RegionHost {
    fn new() -> Self {
        Self {
            contributions: Vec::new(),
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

    /// Seat a provider at the end of its `default_region` — the tests' short way in; the app seats
    /// through [`regions`](fn@crate::chrome::regions) and [`mount_pending`](Self::mount_pending).
    #[cfg(test)]
    pub fn register(&mut self, provider: Box<dyn Provider>) {
        let region = provider.default_region();
        self.seat(region, provider.into(), false);
    }

    /// Seat one container in `region` — the one place a container enters the host, whether it came
    /// from [`regions`](fn@crate::chrome::regions) or from its own default region. `front` puts it first.
    fn seat(&mut self, region: RegionId, placed: super::regions::Placed, front: bool) {
        let super::regions::Placed {
            provider,
            placement,
        } = placed;
        let id = self.free_mount_id(provider.id());
        if !provider.supported_regions().contains(region) {
            crate::chrome::input::identity::warn_author(format!(
                "[heca] '{}' was placed in region '{}', which it does not support — it is shown \
                 there anyway, but moving it may be refused",
                provider.id(),
                region.as_str(),
            ));
        }
        let mc = MountedContribution {
            id: id.clone(),
            provider,
            placement,
            handles: None,
            activated: false,
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
        crate::chrome::input::identity::warn_author(format!(
            "[heca] a second container is already named '{wanted}' — give this placement its own \
             name, or nothing you write in config or a keybinding can refer to it",
        ));
        let mut n = 2;
        loop {
            let candidate = format!("{wanted}.{n}");
            if !self.placement.contains_key(&candidate) {
                return candidate;
            }
            n += 1;
        }
    }

    /// **Apply every `regions(..)` call made so far**, in the order they were made, and close the
    /// queue.
    ///
    /// A caller names a region before this host exists — a plugin loading at startup, the app's own
    /// wiring — so the changes wait and are applied here, once, as the app starts. A change made
    /// after this is refused out loud by [`regions`](fn@crate::chrome::regions); regions that change while the
    /// app runs are not built yet.
    ///
    /// Then every seated dock is started — its [`Provider::on_activate`] runs once, reading the app
    /// through `ctx` — and what it subscribed to lives as long as it stays seated.
    pub fn mount_pending(&mut self, ctx: &ChromeCtx) {
        use super::regions::RegionOp;
        for (region, op) in super::regions::take_for_startup() {
            match op {
                RegionOp::Append(p) => self.seat(region, p, false),
                RegionOp::Prepend(p) => self.seat(region, p, true),
                RegionOp::Remove(id) => {
                    if self.unseat_where(region, |m| m != id) == 0 {
                        crate::chrome::input::identity::warn_author(format!(
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
        // Started once everything is seated, so a dock that was added and then removed never runs.
        for mounted in self
            .regions
            .iter_mut()
            .flat_map(|r| r.contributions.iter_mut())
        {
            mounted.activate(ctx);
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
    /// component that owns a declared action id.
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
    /// outside the render path (their context-menu entries).
    pub fn mounted_providers(&self) -> impl Iterator<Item = &dyn Provider> + '_ {
        self.regions
            .iter()
            .flat_map(|r| r.contributions.iter())
            .map(|m| m.provider())
    }
}
