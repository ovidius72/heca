//! [`ChromeHost`] — owns every chrome region's list of mounted docks: which region each one sits
//! in, in what order, and moving or reordering one between the regions it supports. Moving a dock
//! between regions is the host's job; dragging inside a dock stays the dock's own.
//!
//! A dock enters through [`regions`](fn@crate::chrome::regions), queued until the app starts and applied
//! by [`ChromeHost::mount_pending`]; the render path then asks each one for its body.

mod mounted;
mod placing;

#[cfg(test)]
mod region_list_tests;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use super::contribution::ContainerId;
use crate::chrome::{ChromeEvent, ChromeEventBus, RegionId};
use crate::providers::{Provider, ProviderHandles};

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
    /// Every dock `regions(..)` declared, with how to make it again — what a reload of `[[docks]]`
    /// works from ([`placing`]).
    declared: Vec<placing::Declared>,
    /// The user's `[[docks]]` as last applied ([`heca_config::docks`]).
    docks: Vec<heca_config::docks::DockEntry>,
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
        Self {
            regions: super::regions::RegionMap::from_fn(|_| RegionHost::new()),
            declared: Vec::new(),
            docks: Vec::new(),
            placement: HashMap::new(),
            layouts: Default::default(),
            events,
        }
    }

    /// Seat a provider at the end of `region` — the tests' short way in; the app seats through
    /// [`regions`](fn@crate::chrome::regions) and [`mount_pending`](Self::mount_pending).
    #[cfg(test)]
    pub fn register(&mut self, region: RegionId, provider: Box<dyn Provider>) {
        self.seat(region, provider.into(), false);
    }

    /// Seat one container in `region` — the one place a container enters the host, whether it came
    /// from [`regions`](fn@crate::chrome::regions) or from the user's `[[docks]]`. `front` puts it first.
    fn seat(&mut self, region: RegionId, placed: super::regions::Placed, front: bool) {
        let super::regions::Placed {
            provider,
            placement,
            ..
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
            hand_share: None,
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

    /// **How this region was told to arrange its contents** — the air between them. The render
    /// path asks this when it builds the region's body.
    pub fn layout(&self, region: RegionId) -> &super::regions::RegionLayout {
        &self.layouts[region]
    }

    /// **Where every dock sits, as one value** — each region's docks, in order, with their share and
    /// place. The window rebuilds its chrome when this changes, so a dock moved, reordered, resized,
    /// added or taken out by any path (an action, RPC, a reload, `regions(..)`) is on screen at the
    /// next frame, and no path has to ask for it.
    pub fn arrangement(&self, into: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        for region in RegionId::ALL {
            region.hash(into);
            for mounted in self.contributions(region) {
                mounted.id().hash(into);
                let placement = mounted.placement();
                placement.flex.map(f32::to_bits).hash(into);
                placement.order.hash(into);
            }
        }
        // `Space` holds a float, so it is not `Hash`: its spelled form stands in.
        self.layouts
            .iter()
            .for_each(|layout| format!("{:?}", layout.gap).hash(into));
    }

    /// The ordered containers currently mounted in `region`.
    pub fn contributions(&self, region: RegionId) -> &[MountedContribution] {
        &self.regions[region].contributions
    }

    /// Which region a container currently lives in, if registered.
    pub fn placement(&self, id: &str) -> Option<RegionId> {
        self.placement.get(id).copied()
    }

    /// **Move a container to another region, as the user asked** — a drag, a key, a script. It lands
    /// at the end of the region **as the region is shown**, and that order is then the region's own:
    /// what the user did wins over the `order` the docks were first placed with
    /// ([`arrange_by_hand`](Self::arrange_by_hand)). Validated against the container's
    /// `supported_regions`; a no-op (still `Ok`) if it's already there.
    pub fn move_container(&mut self, id: &str, to: RegionId) -> Result<(), MoveError> {
        if self.placement.get(id) == Some(&to) {
            return self.relocate(id, to);
        }
        self.show_order(to);
        self.relocate(id, to)?;
        self.arrange_by_hand(to);
        Ok(())
    }

    /// Move a container to another region, appending it to the region's list — the placing half,
    /// for [`[[docks]]`](crate::chrome::DockEntry) and `regions(..)`, where each dock's own `order`
    /// still decides where it shows. Validated against the container's `supported_regions`; a no-op
    /// (still `Ok`) if it's already there. Emits [`ChromeEvent::ContainerPlacementChanged`] on a
    /// real move.
    pub(super) fn relocate(&mut self, id: &str, to: RegionId) -> Result<(), MoveError> {
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

    /// Put a container immediately before `before` (or at the end of its own region when `before`
    /// is `None`). **Wherever `before` is**: a target in another region moves the container there
    /// first, validated like [`move_container`](Self::move_container) — "put X before Y" is one
    /// request, whichever region Y sits in, which is what dropping a dock beside a dock in the
    /// other sidebar means. A `before` that names no container is an error
    /// ([`MoveError::TargetNotFound`]) — it fails loudly rather than silently appending, and
    /// changes nothing. Emits [`ChromeEvent::ContainerPlacementChanged`].
    pub fn reorder(&mut self, id: &str, before: Option<&str>) -> Result<(), MoveError> {
        if !self.placement.contains_key(id) {
            return Err(MoveError::UnknownContainer);
        }
        if let Some(b) = before {
            let target_region = self
                .placement
                .get(b)
                .copied()
                .ok_or(MoveError::TargetNotFound)?;
            self.move_container(id, target_region)?;
        }
        let region = self
            .placement
            .get(id)
            .copied()
            .ok_or(MoveError::UnknownContainer)?;
        // "Before Y" is before Y **as the user sees the region** — its `order` decides that, not
        // the list — so the list is put in that order first, and the result becomes the order.
        self.show_order(region);
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
        self.arrange_by_hand(region);
        self.events.emit(ChromeEvent::ContainerPlacementChanged {
            container_id: id.to_string(),
            region,
        });
        Ok(())
    }

    /// **The docks of `region` in the order they are shown** (by their placed `order`, ties as
    /// added), with their placed share.
    pub fn shown(&self, region: RegionId) -> Vec<(String, Option<f32>)> {
        let mut docks: Vec<_> = self.regions[region].contributions.iter().collect();
        docks.sort_by_key(|m| m.placement.order.unwrap_or_default());
        docks
            .into_iter()
            .map(|m| (m.id().to_string(), m.placement.flex))
            .collect()
    }

    /// Give dock `id` this share of its region (CSS `flex`), as the user resized it — kept like a
    /// hand move while the dock's own `[[docks]]` entry is unchanged, and **not** a change of
    /// arrangement: the caller sets it on the drawn dock too, so nothing is rebuilt mid-drag.
    pub fn set_share(&mut self, id: &str, flex: f32) {
        if let Some(m) = self
            .regions
            .iter_mut()
            .flat_map(|r| r.contributions.iter_mut())
            .find(|m| m.id() == id)
        {
            m.hand_share = Some(flex);
        }
    }

    /// Put `region`'s list in the order it is shown: by each dock's placed `order`, ties as they
    /// were added (the order `build_region_content` draws them in).
    fn show_order(&mut self, region: RegionId) {
        self.regions[region]
            .contributions
            .sort_by_key(|m| m.placement.order.unwrap_or_default());
    }

    /// **The region's order is now the user's**: every dock in it is placed at its position in the
    /// list, so a dock first placed with an `order` no longer pulls itself back over a move the
    /// user made. Kept across a config reload while the dock's own entry is unchanged — the same
    /// rule as a dock moved to another region by hand.
    fn arrange_by_hand(&mut self, region: RegionId) {
        let docks = &mut self.regions[region].contributions;
        // With no `order` anywhere the list already is the shown order: nothing to write, so a
        // move that changes nothing changes nothing.
        if docks.iter().all(|m| m.placement.order.is_none()) {
            return;
        }
        for (i, m) in docks.iter_mut().enumerate() {
            m.placement.order = Some(heca_grid_ui::order::Order::from(i as i64));
        }
    }

    /// Put a container immediately **after** `after` — wherever `after` is, like
    /// [`reorder`](Self::reorder).
    ///
    /// The mirror of [`reorder`](Self::reorder). "Put X after Y" is not expressible as "put X before
    /// Z" without knowing what follows Y, which only the host knows — a caller (a plugin, a drag
    /// landing on the *trailing* edge of an item) cannot compute it. An `after` that names a
    /// container not in the region is an error ([`MoveError::TargetNotFound`]); when `after` is the
    /// last container, X lands at the end. Emits [`ChromeEvent::ContainerPlacementChanged`].
    pub fn reorder_after(&mut self, id: &str, after: &str) -> Result<(), MoveError> {
        if !self.placement.contains_key(id) {
            return Err(MoveError::UnknownContainer);
        }
        let region = self
            .placement
            .get(after)
            .copied()
            .ok_or(MoveError::TargetNotFound)?;
        // What follows `after` **as the region is shown** — its `order` decides that, not the list.
        self.show_order(region);
        let list = &self.regions[region].contributions;
        // The container that follows `after` is the one to insert before; none ⇒ the end of
        // `after`'s region, so the container is moved there first.
        let after_idx = list
            .iter()
            .position(|m| m.id() == after)
            .ok_or(MoveError::TargetNotFound)?;
        let before = list.get(after_idx + 1).map(|m| m.id().to_string());
        if before.is_none() {
            self.move_container(id, region)?;
        }
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
