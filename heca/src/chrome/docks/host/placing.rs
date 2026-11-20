//! **Where each dock goes and in what order** — the `regions(..)` queue applied at any time, the
//! user's `[[docks]]` deciding first, and a reload of it moving, reordering, taking out or making
//! again the docks whose entry changed.
//!
//! The rule has one order, read at every step: the user's entry for the dock — the one naming its
//! id, else the one for its kind ([`entry_for`]) — for its `location` (a region, or `"none"`) and its
//! `order`; what the entry does not say, the code that added the dock says.

use heca_config::docks::{DockEntry, entry_for};

use super::ChromeHost;
use crate::chrome::RegionId;
use crate::chrome::docks::regions::{DockPlacement, Placed, Recipe, RegionOp};
use crate::providers::ChromeCtx;

/// **A dock `regions(..)` declared**, kept after it is seated — or left out — so a reload of
/// `[[docks]]` knows where its code wanted it and, for a recipe, how to make it again.
pub(super) struct Declared {
    id: String,
    kind: String,
    /// The region the code named: where it goes when the user says nothing.
    region: RegionId,
    front: bool,
    placement: DockPlacement,
    recipe: Option<Recipe>,
}

/// Where the user's `location` says a dock goes: `Some(region)`, or `None` for `"none"`. A location
/// naming no region is said out loud and the code's region stands.
fn resolve(location: Option<&str>, id: &str, code: RegionId) -> Option<RegionId> {
    match location {
        None => Some(code),
        Some("none") => None,
        Some(name) => match name.parse::<RegionId>() {
            Ok(region) => Some(region),
            Err(()) => {
                crate::chrome::input::identity::warn_author(format!(
                    "[heca] [[docks]] '{id}': location \"{name}\" is no region — it goes to '{}'. \
                     The regions are 'sidebar.left', 'sidebar.right', 'bar.top', 'bar.bottom', or \
                     \"none\" to leave it out",
                    code.as_str(),
                ));
                Some(code)
            }
        },
    }
}

/// What an entry says, compared between two configs: a dock is touched by a reload only when this
/// changed.
fn said(entry: Option<&DockEntry>) -> (Option<&str>, Option<i64>) {
    entry.map_or((None, None), |e| (e.location.as_deref(), e.order))
}

impl Declared {
    /// The placement it takes: the code's, with the user's `order` over it.
    fn placement_with(&self, entry: Option<&DockEntry>) -> DockPlacement {
        let mut placement = self.placement;
        if let Some(order) = entry.and_then(|e| e.order) {
            placement.order = Some(order.into());
        }
        placement
    }
}

impl ChromeHost {
    /// **Apply every `regions(..)` call made so far**, as the app starts, with `docks` — the
    /// user's `[[docks]]` — deciding where each dock goes. Every seated dock is then started:
    /// its [`Provider::on_activate`](crate::providers::Provider::on_activate) runs once, and what it
    /// subscribed to lives as long as it stays seated. Later calls are applied by
    /// [`apply_pending`](Self::apply_pending), a later `[[docks]]` by [`reload_docks`](Self::reload_docks).
    pub fn mount_pending(&mut self, ctx: &ChromeCtx, docks: &[DockEntry]) {
        self.docks = docks.to_vec();
        self.apply_pending(ctx);
    }

    /// **Apply the `regions(..)` calls made since the last time** — the window calls this every
    /// frame, so a plugin adds or takes out a dock while the app runs. Nothing queued, nothing done.
    /// Says whether anything was applied, so the window draws the change.
    pub fn apply_pending(&mut self, ctx: &ChromeCtx) -> bool {
        let pending = crate::chrome::docks::regions::take_pending();
        if pending.is_empty() {
            return false;
        }
        for (region, op) in pending {
            match op {
                RegionOp::Append(placed) => self.declare(region, placed, false),
                RegionOp::Prepend(placed) => self.declare(region, placed, true),
                RegionOp::Remove(id) => self.remove_by_id(&id),
                RegionOp::Retain(keep) => {
                    self.unseat_where(region, &keep);
                    let seated = &self.placement;
                    self.declared.retain(|d| d.region != region || keep(&d.id) || seated.contains_key(&d.id));
                }
                RegionOp::Gap(gap) => self.layouts[region].gap = Some(gap),
            }
        }
        self.start_seated(ctx);
        true
    }

    /// **The user changed `[[docks]]` and reloaded**: each dock whose entry changed goes where it now
    /// says — moved, reordered, taken out (`"none"`), or made again from its recipe when it comes
    /// back. A dock whose entry did not change stays where it is, wherever the user moved it by
    /// hand. A dock handed over as itself, not as a recipe, cannot be made again once out: that is
    /// said, and it comes back with a restart.
    pub fn reload_docks(&mut self, ctx: &ChromeCtx, docks: &[DockEntry]) {
        let before = std::mem::replace(&mut self.docks, docks.to_vec());
        for i in 0..self.declared.len() {
            let (id, kind, code) = {
                let d = &self.declared[i];
                (d.id.clone(), d.kind.clone(), d.region)
            };
            let old = entry_for(&before, &kind, &id);
            let new = entry_for(docks, &kind, &id).cloned();
            if said(old) == said(new.as_ref()) {
                continue;
            }
            let placement = self.declared[i].placement_with(new.as_ref());
            let wanted = resolve(new.as_ref().and_then(|e| e.location.as_deref()), &id, code);
            match (wanted, self.placement.get(&id).copied()) {
                (None, Some(_)) => self.unseat_id(&id),
                (Some(to), Some(at)) => {
                    self.set_placement(&id, placement);
                    if to != at && let Err(error) = self.move_container(&id, to) {
                        crate::chrome::input::identity::warn_author(format!(
                            "[heca] [[docks]] could not move '{id}' to '{}': {error}", to.as_str()));
                    }
                }
                (Some(to), None) => match self.declared[i].recipe.clone() {
                    Some(recipe) => {
                        let front = self.declared[i].front;
                        self.seat(to, Placed { provider: recipe(), placement, recipe: Some(recipe) }, front);
                    }
                    None => crate::chrome::input::identity::warn_author(format!(
                        "[heca] [[docks]] '{id}' was added as a dock, not as a recipe, so it cannot \
                         be made again — restart heca to bring it back")),
                },
                (None, None) => {}
            }
        }
        self.start_seated(ctx);
    }

    /// Remember a declared dock (a later declaration of the same id replaces it), then seat it
    /// where the user's `[[docks]]` says — or leave it out for `"none"`.
    fn declare(&mut self, region: RegionId, mut placed: Placed, front: bool) {
        let id = placed.provider.id().to_string();
        let declared = Declared {
            id: id.clone(),
            kind: placed.provider.kind().to_string(),
            region,
            front,
            placement: placed.placement,
            recipe: placed.recipe.clone(),
        };
        let entry = entry_for(&self.docks, &declared.kind, &id).cloned();
        placed.placement = declared.placement_with(entry.as_ref());
        self.declared.retain(|d| d.id != id);
        self.declared.push(declared);
        if let Some(to) = resolve(entry.as_ref().and_then(|e| e.location.as_deref()), &id, region) {
            self.seat(to, placed, front);
        }
    }

    /// A seated dock's placement (share and order), replaced — a reload changed its `order`.
    fn set_placement(&mut self, id: &str, placement: DockPlacement) {
        if let Some(mounted) = self.regions.iter_mut()
            .flat_map(|r| r.contributions.iter_mut())
            .find(|m| m.id() == id)
        {
            mounted.placement = placement;
        }
    }

    /// `regions(..).remove(id)`: out of whichever region holds it, and not made again by a reload.
    fn remove_by_id(&mut self, id: &str) {
        let known = self.placement.contains_key(id) || self.declared.iter().any(|d| d.id == id);
        if !known {
            crate::chrome::input::identity::warn_author(format!(
                "[heca] there is no dock '{id}' to remove — the docks are: {}",
                self.placement.keys().cloned().collect::<Vec<_>>().join(", "),
            ));
            return;
        }
        self.unseat_id(id);
        self.declared.retain(|d| d.id != id);
    }

    /// Take the dock with this id out of whichever region holds it. What it subscribed to goes
    /// with it.
    fn unseat_id(&mut self, id: &str) {
        if let Some(region) = self.placement.get(id).copied() {
            self.unseat_where(region, |m| m != id);
        }
    }

    /// Start every seated dock not started yet (starting is once per dock).
    fn start_seated(&mut self, ctx: &ChromeCtx) {
        for mounted in self.regions.iter_mut().flat_map(|r| r.contributions.iter_mut()) {
            mounted.activate(ctx);
        }
    }
}
