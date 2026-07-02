// Provider foundation (plugin-02): the trait + facade are the seam first-party
// providers (plugin-03: WorkspacesContainerProvider) and the future WASM bridge
// implement. No provider is registered in the app yet, so parts read as dead
// code until plugin-03 — same allow rationale as `host.rs` / `chrome/events.rs`.
#![allow(dead_code)]

//! Chrome **providers** — the built-in (later WASM-backed) contributors of chrome
//! content, per contract §3.4.1.
//!
//! A provider never mutates app state directly (§2.3): it reads through
//! [`ChromeCtx`] selectors, reacts to events, and (in later phases) dispatches
//! actions. It declares placement metadata and builds a
//! [`Contribution`](crate::chrome::Contribution) model; the
//! [`ChromeHost`](crate::chrome::ChromeHost) owns mounting, ordering, and moves.
//!
//! **plugin-02 scope:** this defines the trait + facade (pulling plugin-03's
//! `plugin-task-09` trait definition forward, since `ChromeHost::register` needs
//! it). The first real implementation — `WorkspacesContainerProvider` — and the
//! app-side registration land in plugin-03.

use crate::chrome::{ChromeEvent, ChromeSubscription, Contribution, RegionId, RegionSet};
use crate::host::{App, StateView};

/// A built-in (later WASM-backed) contributor of chrome content.
pub trait Provider {
    /// Stable identity — also the `ContainerId` when it contributes a container.
    fn id(&self) -> &str;

    /// Regions this provider's contribution may be placed in.
    fn supported_regions(&self) -> RegionSet;

    /// Region it mounts in on first run.
    fn default_region(&self) -> RegionId;

    /// Default stacking order within a region (lower = earlier).
    fn default_order(&self) -> i32 {
        0
    }

    /// Human title (rail/tab label, move menu).
    fn title(&self) -> &str;

    /// Host-level move/reorder allowed?
    fn movable(&self) -> bool {
        true
    }

    /// Collapsible within its region shell?
    fn collapsible(&self) -> bool {
        true
    }

    /// Build the contribution model. Called on mount and on each invalidation —
    /// the **render seam**, not exercised in plugin-02.
    fn build_contribution(&self, ctx: &ChromeCtx) -> Contribution;

    /// Subscribe to events / register actions on activation. The returned
    /// [`ProviderHandles`] are held by the host while the provider is mounted and
    /// dropped (unsubscribing) on unmount. Default: no subscriptions.
    fn on_activate(&mut self, _ctx: &ChromeCtx) -> ProviderHandles {
        ProviderHandles::default()
    }
}

/// RAII bundle a provider returns from [`Provider::on_activate`]: the host keeps
/// it alive for the provider's mounted lifetime and drops it on unmount, which
/// unsubscribes every held event subscription. (Dynamic-action unregister
/// handles join here in plugin-04.)
#[derive(Default)]
pub struct ProviderHandles {
    pub subscriptions: Vec<ChromeSubscription>,
}

impl ProviderHandles {
    /// Take ownership of an event subscription so it lives as long as the
    /// provider is mounted.
    pub fn keep(&mut self, sub: ChromeSubscription) {
        self.subscriptions.push(sub);
    }
}

/// The provider/plugin-facing facade (contract §3.4.1). It *extends* the shipped
/// read/observe [`App`] host API (`heca/src/host.rs`) with the write/contribute
/// halves deferred to §3.5 rows 3–10. plugin-02 wires only the read/observe half;
/// `actions` / `overlay` / `regions` arrive in plugin-04/05.
pub struct ChromeCtx {
    app: App,
    // actions: ActionDispatch,   // plugin-04/05
    // overlay: OverlayHandle,    // plugin-05 / Phase 8 (§2.7.1)
    // regions: RegionHandle,     // plugin-05
}

impl ChromeCtx {
    /// Build a context over the app's host facade.
    pub fn new(app: App) -> Self {
        Self { app }
    }

    /// The underlying host facade.
    pub fn app(&self) -> &App {
        &self.app
    }

    /// Subscribe to a typed event by name (or `"*"`). Passthrough to `app.on`.
    pub fn on(
        &self,
        event: impl Into<String>,
        handler: impl FnMut(&ChromeEvent) + 'static,
    ) -> ChromeSubscription {
        self.app.on(event, handler)
    }

    /// Read-only state selectors (`app.state.*`).
    pub fn state(&self) -> StateView<'_> {
        self.app.state()
    }
}
