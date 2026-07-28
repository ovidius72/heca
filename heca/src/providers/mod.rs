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

mod workspaces;

use crate::chrome::{
    ChromeEvent, ChromeIntentEmitter, ChromeSubscription, ContextMenuContribution, Contribution,
    RegionId, RegionSet,
};
use crate::host::{App, StateView};
use crate::sidebar::SidebarTree;
use heca_config::programs::ProgramsConfig;
use heca_grid_ui::theme::Theme as GuiTheme;

pub use workspaces::WorkspacesContainerProvider;

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
    /// This container's share of its region's **main axis** — height in a sidebar, width in a
    /// bar — as a flex grow factor. Default `1.0` (an equal share); `0.0` is content-sized. See
    /// [`ContainerContribution::grow`](crate::chrome::ContainerContribution::grow).
    fn grow(&self) -> f32 {
        1.0
    }

    fn collapsible(&self) -> bool {
        true
    }

    /// Does this container do anything with chrome keyboard focus **beyond scrolling**?
    ///
    /// Every container can be focused and scrolled — that needs nothing from the provider. This
    /// answers the further question of whether the dock has its own keyboard navigation once it holds
    /// focus (the workspaces tree's cursor, a list's selection), which only the provider knows.
    ///
    /// It is what `sidebar_focus` looks for: with no navigable dock mounted there is nothing to
    /// navigate, so the action does nothing instead of expanding an empty container (F003/P011/T020).
    /// Default `false` — a container that merely displays and scrolls says nothing.
    fn keyboard_navigable(&self) -> bool {
        false
    }

    /// Build the contribution model. Called on mount and on each invalidation —
    /// the **render seam**.
    fn build_contribution(&self, ctx: &ChromeCtx<'_>) -> Contribution;

    /// Context-menu entries this provider contributes (context-menu-5). Declares *where*
    /// (`context_path`) and *what* (`build`) — never *when*: the host decides that, opening the menu
    /// on right-click / `prefix+>` and merging every provider for that path by `weight`, so these
    /// entries slot **between** the built-ins.
    ///
    /// Separate from [`build_contribution`](Provider::build_contribution) — which returns the one
    /// *mounted body* — because a provider commonly wants both a container **and** menu entries on
    /// its rows. Returns as many contributions as it likes, on as many paths as it likes.
    /// Default: none.
    fn context_menus(&self, _ctx: &ChromeCtx<'_>) -> Vec<ContextMenuContribution> {
        Vec::new()
    }

    /// Subscribe to events / register actions on activation. The returned
    /// [`ProviderHandles`] are held by the host while the provider is mounted and
    /// dropped (unsubscribing) on unmount. Default: no subscriptions.
    fn on_activate(&mut self, _ctx: &ChromeCtx<'_>) -> ProviderHandles {
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

/// The read-only host inputs a container body is projected from. Present only on a
/// context built for a render pass ([`ChromeCtx::for_build`]); a context built to
/// merely observe ([`ChromeCtx::new`], e.g. at [`Provider::on_activate`]) has none,
/// because there is no frame in flight to read a theme or a tree from.
struct RenderInputs<'a> {
    tree: &'a SidebarTree,
    programs: &'a ProgramsConfig,
    theme: &'a GuiTheme,
    emit: &'a ChromeIntentEmitter,
}

/// The provider/plugin-facing facade (contract §3.4.1). It *extends* the shipped
/// read/observe [`App`] host API (`heca/src/host.rs`) with the write/contribute
/// halves deferred to §3.5 rows 3–10. plugin-02 wired the read/observe half;
/// plugin-03 adds the render-pass selectors below; `actions` / `overlay` / `regions`
/// arrive in plugin-04/05.
///
/// Everything reachable here is **read-only**: a provider projects app state into
/// widgets and reports user actions as
/// [`InteractionIntent`](crate::app::interaction::InteractionIntent)s through
/// [`emit_intent`](ChromeCtx::emit_intent) — it never mutates app state (§2.3). The
/// registries a build *does* mutate are the separate, explicit
/// [`BuildCx`](crate::chrome::BuildCx).
pub struct ChromeCtx<'a> {
    app: App,
    /// `Some` during a render pass; `None` for an observe-only context.
    render: Option<RenderInputs<'a>>,
    // actions: ActionDispatch,   // plugin-04/05
    // overlay: OverlayHandle,    // plugin-05 / Phase 8 (§2.7.1)
    // regions: RegionHandle,     // plugin-05
}

impl<'a> ChromeCtx<'a> {
    /// An **observe-only** context over the app's host facade: state selectors and
    /// event subscriptions, no render inputs. Used outside a frame (activation).
    pub fn new(app: App) -> Self {
        Self { app, render: None }
    }

    /// A context for a **render pass**, carrying the frame's read-only inputs so a
    /// container's `build` closure can project them. Built by the chrome render path.
    pub fn for_build(
        app: App,
        tree: &'a SidebarTree,
        programs: &'a ProgramsConfig,
        theme: &'a GuiTheme,
        emit: &'a ChromeIntentEmitter,
    ) -> Self {
        Self {
            app,
            render: Some(RenderInputs {
                tree,
                programs,
                theme,
                emit,
            }),
        }
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

    /// The workspace/column/pane projection this frame renders, or `None` outside a
    /// render pass.
    pub fn tree(&self) -> Option<&SidebarTree> {
        self.render.as_ref().map(|r| r.tree)
    }

    /// The program catalog (icons + display names for running processes), or `None`
    /// outside a render pass.
    pub fn programs(&self) -> Option<&ProgramsConfig> {
        self.render.as_ref().map(|r| r.programs)
    }

    /// The resolved grid-ui [`Theme`](heca_grid_ui::theme::Theme) for this frame, or
    /// `None` outside a render pass. Every color/size a container paints comes from
    /// here — never a literal.
    pub fn theme(&self) -> Option<&GuiTheme> {
        self.render.as_ref().map(|r| r.theme)
    }

    /// The sink a container's widgets report user actions to, or `None` outside a
    /// render pass. Behaviour crosses this boundary as an intent, never as a state
    /// mutation.
    pub fn emit_intent(&self) -> Option<&ChromeIntentEmitter> {
        self.render.as_ref().map(|r| r.emit)
    }
}
