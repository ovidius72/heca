//! [`MountedContribution`] — one dock seated in a region: its own name, its provider, and what
//! whoever placed it said about its size and place.

use crate::chrome::docks::contribution::ContainerId;
use crate::chrome::docks::regions::DockPlacement;
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
    pub(super) id: ContainerId,
    pub(super) provider: Box<dyn Provider>,
    /// What whoever added this dock said about its size and place — wins over the dock's own body.
    pub(super) placement: DockPlacement,
    /// `Some` once [`Provider::on_activate`] has run, or once the host kept a declared action here;
    /// what it holds lives as long as this stays mounted.
    pub(super) handles: Option<ProviderHandles>,
    /// Whether [`activate`](Self::activate) has run — separate from `handles`, which the host may
    /// fill with declared actions before or after.
    pub(super) activated: bool,
}

impl MountedContribution {
    /// **This seating's own name** — see [`id`](Self::id).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What whoever added this dock said about its size and place (`.flex` / `.order` on the dock
    /// when it was appended). Wins over what the dock's own body says.
    pub fn placement(&self) -> DockPlacement {
        self.placement
    }

    /// Does this container do anything with keyboard focus beyond scrolling?
    pub fn keyboard_navigable(&self) -> bool {
        self.provider.keyboard_navigable()
    }

    /// The underlying provider (for the render path to build its contribution).
    pub fn provider(&self) -> &dyn Provider {
        self.provider.as_ref()
    }

    /// **Run the provider's [`on_activate`](Provider::on_activate), once**, and keep what it returns
    /// with this mount — so its subscriptions end when the mount does.
    ///
    /// Merged into [`handles`](Self::handles_mut) rather than replacing it: the host may already
    /// have kept a declared action there, and that must not be dropped.
    pub(super) fn activate(&mut self, ctx: &ChromeCtx) {
        if self.activated {
            return;
        }
        self.activated = true;
        let ProviderHandles {
            subscriptions,
            actions,
        } = self.provider.on_activate(ctx);
        let held = self.handles_mut();
        held.subscriptions.extend(subscriptions);
        held.actions.extend(actions);
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
