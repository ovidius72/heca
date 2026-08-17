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

mod actions;
pub(crate) mod workspaces;

pub(crate) use actions::{
    SEAT_ARG, bind_provider_keybindings, move_provider_cursor, owning_mount,
    register_provider_actions,
};

use crate::chrome::{
    ChromeEvent, ChromeIntentEmitter, ChromeSubscription, ContextMenuContribution, Contribution,
    RegionId, RegionSet,
};
use crate::host::{App, StateView};
use heca_grid_ui::theme::Theme as GuiTheme;

pub use workspaces::WorkspacesContainerProvider;

/// A built-in (later WASM-backed) contributor of chrome content.
pub trait Provider {
    /// Stable identity of this **placement** — also the `ContainerId` when it contributes a
    /// container.
    ///
    /// Everything that belongs to one seating keys off this: the cursor, the scroll offset, whether
    /// it holds chrome focus. Place the same container twice and there are two of these.
    fn id(&self) -> &str;

    /// The component **type** — the config namespace (`[keys.docker]`) and the identity its
    /// declared actions belong to.
    ///
    /// Distinct from [`id`](Provider::id) on purpose (user decision, 2026-07-29): bindings and
    /// declared actions belong to the *type*, so writing them once covers every placement, while
    /// scroll position and chrome focus belong to each *mount*. `Hero::new()` twice is two instances
    /// of one type, not two components.
    ///
    /// **The cursor is the component's to say.** The host keeps one per mount, which is right for a
    /// component that navigates each placement independently — but a component whose model has a
    /// single cursor (the workspaces tree has exactly one) must light the same row in every seating,
    /// or two views of one thing disagree. The host brings the seatings back into step after each
    /// provider call (`mirror_cursor_to_siblings`), so this follows the model rather than being
    /// asserted against it (F003/P086/T365).
    ///
    /// Defaults to `id()`, which is right for a container that is only ever seated once and keeps
    /// the single-placement case free of ceremony.
    fn kind(&self) -> &str {
        self.id()
    }

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

    /// Which context-menu path describes the row `key` — **the component naming its own row kinds**
    /// (F003/P086/T365).
    ///
    /// The host resolves *which row* was right-clicked (the container under the point, the
    /// `nav_key` on it) and *when* to open a menu; it cannot know that `pane:7` is a pane and
    /// `ws:2` a workspace, and must not learn. So it asks the component that wrote the key, gets
    /// back a path, and looks that path up in the registry exactly as it does for any other. A
    /// Docker dock answering `"docker.container"` gets a right-click menu with no host code.
    ///
    /// Answer by **matching the key against your own rows**, never by parsing it: a tiled pane and
    /// a floating one are both `pane:<id>`, and only the row knows which it is.
    ///
    /// Default `None` — a component whose rows have no menu, which is most of them. The key is one
    /// this component wrote, so an unrecognised one (a stale cursor, another placement's row) is
    /// also `None`, and no menu opens rather than a wrong one.
    fn context_path(&self, _key: &str, _ctx: &ChromeCtx<'_>) -> Option<String> {
        None
    }

    /// Subscribe to events / register actions on activation. The returned
    /// [`ProviderHandles`] are held by the host while the provider is mounted and
    /// dropped (unsubscribing) on unmount. Default: no subscriptions.
    fn on_activate(&mut self, _ctx: &ChromeCtx<'_>) -> ProviderHandles {
        ProviderHandles::default()
    }

    /// What this component **declares** — and nothing more.
    ///
    /// Only what nothing else can do: the selection-dependent semantics that are the component's
    /// own ("restart *this* container", "delete *this* note"). Everything else it wants is an
    /// action that already exists, which it *binds* rather than redeclares — a component's keymap
    /// can name any action id, built-in or its own.
    ///
    /// **The test for whether an action is yours** (user, 2026-07-29) — mechanical, and the clearest
    /// statement of the rule:
    ///
    /// > **Remove this component. Does the action still make sense?**
    /// > **Yes** → it is the app's. Bind it here; do not declare it.
    /// > **No** → it is yours. Declare it.
    ///
    /// `workspaces.cursor_up` fails the test — delete the dock and there is no cursor, so no
    /// meaning. `workspace_next` passes it: it switches the session's active workspace, which exists
    /// whether or not anything displays it, so the workspaces dock **binds** it rather than owning
    /// it. So do `create_workspace`, `zoom_column`, `close` and `next_pane`.
    ///
    /// The test also separates two actions that a name can hide: *"switch the active workspace"*
    /// survives the deletion and is the app's; *"move my cursor to the next workspace row"* does not
    /// and would be this component's. Same words, different owners.
    ///
    /// **There is no generic verb set** (user decision, 2026-07-29). An inherited
    /// up/down/select/toggle vocabulary would be dead weight for a container showing one number, so
    /// an empty list is a real and common answer — hence the default. Conventional ids (`up`,
    /// `down`, `activate`, `remove`, `add`) are encouraged so docks agree in practice, never
    /// enforced in code.
    ///
    /// The declaration alone buys the palette entry, the icon and label, `describe-action` and RPC,
    /// the `prefix+/` hint, and the central destructive-confirm gate — because these metas join the
    /// same [`ActionCatalog`](crate::actions::ActionCatalog) the built-ins do.
    fn actions(&self) -> Vec<crate::actions::ActionMeta> {
        Vec::new()
    }

    /// The host moved this placement's cursor to `key` — a click, an RPC call, a script.
    ///
    /// The key is one **this component wrote** (`ComponentExt::nav_key`), so only it can say which row
    /// that is; the host deliberately never parses it. Whatever the component keeps of its own — a
    /// positional index, a domain-typed selection — reconciles here.
    ///
    /// Default: nothing, which is right for a component whose only cursor **is** the host's generic
    /// one. A component that keeps no index of its own needs none of this.
    fn cursor_moved(&self, _key: &str, _cx: &mut ProviderCx<'_>) {}

    /// Keys this component asks for, as `(action id, combo(s))` — **the plugin path**
    /// (F003/P086/T366).
    ///
    /// **Empty for anything shipped with heca, and that is the point.** A core component writes its
    /// keys in `keybindings.default.toml` under `[[keys.component]]`, where they sit beside every
    /// other binding, `--keys-show` finds them, and the user edits them in one place. Declaring them
    /// in code as well would be a second copy of the file that nothing compares against — which is
    /// exactly what `ActionMeta::default_binding` was, and why it is gone.
    ///
    /// A **plugin** has no entry in that merge, so this is its only way in. Whatever the user's file
    /// says still wins: a registration never overwrites a combo the config already bound, and never
    /// adds a second key to an action the user has rebound.
    fn keybindings(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    /// Run one of this component's declared actions.
    ///
    /// **One entry point, not one closure per action** (user decision, 2026-07-29), for four
    /// reasons that all point the same way:
    ///
    /// - it is the shape a WASM component is *forced* into — an id plus args crosses a boundary, a
    ///   closure does not — so native and plugin components stay one design;
    /// - `&self` is available, because the host is calling **into** the component; a closure
    ///   registered at mount outlives `actions()` and has to hoist everything it needs into `Rc`s;
    /// - a raw [`DynHandler`](crate::actions::DynHandler) is `Fn(&mut AppState, …)` — full mutable
    ///   app state, which §2.3 forbids a provider. [`ProviderCx`] is read + emit, so the contract is
    ///   enforced by the type rather than by discipline;
    /// - "which placement?" is answered once, by the host, instead of captured per closure.
    ///
    /// Returning [`Handled::No`] is a legitimate **silent decline** — nothing selected, nothing to
    /// act on — not an error.
    fn perform(
        &self,
        _action: &str,
        _args: &crate::chrome::Intent,
        _cx: &mut ProviderCx<'_>,
    ) -> heca_grid_ui::Handled {
        heca_grid_ui::Handled::No
    }
}

/// RAII bundle a provider returns from [`Provider::on_activate`]: the host keeps
/// it alive for the provider's mounted lifetime and drops it on unmount, which
/// unsubscribes every held event subscription.
#[derive(Default)]
pub struct ProviderHandles {
    pub subscriptions: Vec<ChromeSubscription>,
    /// Dynamic-action handles for everything this mount **declared**, so its actions die with it.
    ///
    /// Unlike the subscriptions beside them these cannot retire themselves on `Drop`: retiring an
    /// action needs the registry *and* the catalog, which live on `AppState` and are not reachable
    /// from a destructor. The host retires them explicitly (`retire_provider_actions`) — the same
    /// lifetime, without wrapping the registry in a `RefCell`.
    pub actions: Vec<crate::actions::ActionHandle>,
}

impl ProviderHandles {
    /// Take ownership of an event subscription so it lives as long as the
    /// provider is mounted.
    pub fn keep(&mut self, sub: ChromeSubscription) {
        self.subscriptions.push(sub);
    }

    /// Take ownership of a registered action's handle, so unmounting retires it.
    pub fn keep_action(&mut self, handle: crate::actions::ActionHandle) {
        self.actions.push(handle);
    }
}

/// The context a component gets while its [`perform`](Provider::perform) runs: **read the store,
/// emit intents** — and nothing else.
///
/// This is §2.3 written as a type. A provider must not mutate app state directly, so it never sees
/// `&mut AppState`; what it can do instead is ask the store questions and *ask the host* to do
/// things, which the host applies after `perform` returns.
///
/// There is also a borrow reason the design could not avoid: the provider being called lives
/// **inside** `state.chrome_host`, so no `&mut AppState` may be alive while `perform` runs. This
/// holds only an `Rc` alias of the store and a queue, so the borrow ends before the call and the
/// queued intents are applied after it ([`drain`](ProviderCx::drain)).
pub struct ProviderCx<'a> {
    /// The **placement** this call is aimed at — whose cursor and scroll position are the answer.
    mount: &'a str,
    /// An `Rc` alias of the shared chrome store; reads are cheap signal `get`s.
    store: crate::chrome::SharedChromeState,
    /// What the component asked the host to do, in order, applied after `perform` returns.
    queued: Vec<crate::chrome::Intent>,
}

impl<'a> ProviderCx<'a> {
    /// Build a context aimed at `mount`, over an alias of the shared store.
    pub fn new(mount: &'a str, store: crate::chrome::SharedChromeState) -> Self {
        Self {
            mount,
            store,
            queued: Vec::new(),
        }
    }

    /// The placement this call is aimed at.
    pub fn mount(&self) -> &str {
        self.mount
    }

    /// Read-only state selectors — the same [`StateView`] a render pass reads through.
    pub fn state(&self) -> StateView<'_> {
        StateView::over(&self.store)
    }

    /// **This placement's** selected row, as the row declared it (`nav_key`, F003/P085/T354).
    ///
    /// Per mount, like the scroll offset and the focus beside it: two placements of one container
    /// have two cursors, so "the selection" is only ever a question about *one* seating. `None`
    /// means nothing is selected, which is the ordinary reason a `perform` declines.
    pub fn selected(&self) -> Option<String> {
        use heca_grid_ui::reactive::SignalGet as _;
        self.store.container_cursor(self.mount).get()
    }

    /// Move **this placement's** cursor to the row that declared `key` (`None` clears it).
    ///
    /// A component owns its own cursor, because only it knows what its rows are and what "next"
    /// means among them — the workspace tree steps workspaces, columns and panes; a label showing
    /// one number has no cursor at all. There is deliberately **no** host-side "step to the next
    /// `nav_key`": that would impose one notion of next on components it means nothing to.
    ///
    /// Writing goes to the **store**, not to app state — read-via-signals / write-via-actions, so
    /// the row outlines follow with no rebuild and §2.3 still holds.
    pub fn set_selected(&self, key: Option<String>) {
        self.store.set_container_cursor(self.mount, key);
    }

    /// Ask the host to dispatch an action by name — how a component reaches something it did not
    /// declare (`cx.dispatch("focus_pane", args)`).
    ///
    /// Queued, not run: the host applies it after `perform` returns, when `&mut AppState` is free
    /// again. It goes out through the ordinary intent path, so it is policy-routed and
    /// confirm-gated exactly like the same action from a key or a menu.
    pub fn dispatch(&mut self, action: impl Into<String>, args: crate::chrome::PropMap) {
        self.queued.push(crate::chrome::Intent {
            action: action.into(),
            args,
        });
    }

    /// Queue a fully-built intent. The general form of [`dispatch`](ProviderCx::dispatch).
    pub fn emit(&mut self, intent: crate::chrome::Intent) {
        self.queued.push(intent);
    }

    /// Take what the component asked for, so the host can apply it.
    pub fn drain(&mut self) -> Vec<crate::chrome::Intent> {
        std::mem::take(&mut self.queued)
    }
}


/// The read-only host inputs a container body is projected from. Present only on a
/// context built for a render pass ([`ChromeCtx::for_build`]); a context built to
/// merely observe ([`ChromeCtx::new`], e.g. at [`Provider::on_activate`]) has none,
/// because there is no frame in flight to read a theme from.
///
/// **Everything here must be true for ANY component** (F003/P086/T367). The frame's theme and the
/// intent sink are: every component paints in the current theme and reports what the user did. A
/// component's *model* is not, and neither is anything derived from one domain — those live on that
/// component's own state, where only it can see them. This carried the **workspace tree** and the
/// **program catalog** until 2026-07-30, so a Docker dock, a notes dock and a label showing one
/// number were each handed a workspaces model through the one context they all share. A new field
/// here has to answer the same question first.
struct RenderInputs<'a> {
    theme: &'a GuiTheme,
    emit: &'a ChromeIntentEmitter,
    /// The action catalog, so a container can ask what an action **looks like**.
    ///
    /// Carried rather than reached for because `ActionMeta.icon` is "the single source of an
    /// action's `Glyph`. Every surface that renders this action reads it from here instead of
    /// inventing its own" (`heca/src/actions.rs`) — and a container that declares a menu over
    /// built-in actions (`close`, `delete_column`, …) has no other way to honour that.
    catalog: &'a crate::actions::ActionCatalog,
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
        theme: &'a GuiTheme,
        emit: &'a ChromeIntentEmitter,
        catalog: &'a crate::actions::ActionCatalog,
    ) -> Self {
        Self {
            app,
            render: Some(RenderInputs { theme, emit, catalog }),
        }
    }

    /// **The action catalog** — the one place that owns what an action looks like and what it is
    /// called.
    ///
    /// A container declaring a context menu over an action names the *action*, never a glyph, so
    /// the sidebar's "Close pane" and the command palette's `close` cannot drift apart. It is the
    /// catalog rather than a single lookup because `menu_from_items` — the **one** way a menu is
    /// built, for every surface — takes a catalog, and the host has one too. `None` outside a
    /// render pass.
    pub fn action_catalog(&self) -> Option<&crate::actions::ActionCatalog> {
        self.render.as_ref().map(|r| r.catalog)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::ActionMeta;
    use crate::app::interaction::ActionPolicy;
    use crate::chrome::{Intent, SharedChromeState};
    use heca_grid_ui::Handled;

    /// A dock that declares one action and acts on its own selection — the shape the phase's
    /// worked Docker example has, minus the domain.
    struct Dock {
        id: String,
        /// What `perform` was asked to act on, so the test can see which placement's cursor it read.
        acted_on: std::cell::RefCell<Vec<String>>,
    }

    impl Dock {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
                acted_on: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl Provider for Dock {
        fn id(&self) -> &str {
            &self.id
        }
        fn kind(&self) -> &str {
            "dock"
        }
        fn title(&self) -> &str {
            "Dock"
        }
        fn supported_regions(&self) -> RegionSet {
            RegionSet::sidebars()
        }
        fn default_region(&self) -> RegionId {
            RegionId::LeftSidebar
        }
        fn build_contribution(&self, _ctx: &ChromeCtx<'_>) -> Contribution {
            unimplemented!("these tests read the action contract, not the render seam")
        }

        fn actions(&self) -> Vec<ActionMeta> {
            vec![ActionMeta {
                name: "dock.restart_selected".into(),
                label: "Restart".into(),
                description: "Restart the selected thing.".into(),
                category: crate::actions::ActionCategory::Chrome,
                // The host stamps the owner at registration; a component never writes it.
                owner: None,
                icon: None,
                policy: ActionPolicy::Global,
                args: Vec::new(),
                confirm: None,
            }]
        }

        fn perform(&self, action: &str, _args: &Intent, cx: &mut ProviderCx<'_>) -> Handled {
            // Nothing selected ⇒ decline. A silent decline is a real answer, not an error.
            let Some(selected) = cx.selected() else {
                return Handled::No;
            };
            match action {
                "dock.restart_selected" => {
                    self.acted_on.borrow_mut().push(selected);
                    // Reaching an action it did NOT declare goes out as a request, never a mutation.
                    cx.dispatch("focus_pane", crate::chrome::PropMap::new());
                    Handled::Yes
                }
                _ => Handled::No,
            }
        }
    }

    /// A container that declares nothing still compiles and behaves — the one-number-widget case,
    /// which is why `actions()` has a default and there is no generic verb set.
    struct Quiet;

    impl Provider for Quiet {
        fn id(&self) -> &str {
            "quiet"
        }
        fn title(&self) -> &str {
            "Quiet"
        }
        fn supported_regions(&self) -> RegionSet {
            RegionSet::sidebars()
        }
        fn default_region(&self) -> RegionId {
            RegionId::RightSidebar
        }
        /// A body built from **nothing but the shared context** — one label, no model, no catalog.
        fn build_contribution(&self, _ctx: &ChromeCtx<'_>) -> Contribution {
            Contribution::Container(crate::chrome::ContainerContribution {
                id: self.id().to_string(),
                title: self.title().to_string(),
                supported_regions: self.supported_regions(),
                default_region: self.default_region(),
                default_order: self.default_order(),
                movable: self.movable(),
                collapsible: self.collapsible(),
                grow: self.grow(),
                build: Box::new(|ctx: &ChromeCtx<'_>, _bx: &mut crate::chrome::BuildCx<'_>| {
                    // The theme is there for anyone; nothing else is needed to render a number.
                    let _theme = ctx.theme().expect("a render pass carries the frame's theme");
                    Box::new(heca_grid_ui::widgets::Label::new("42"))
                        as crate::chrome::WidgetModel
                }),
            })
        }
    }

    fn store() -> SharedChromeState {
        SharedChromeState::new(240.0, true, 240.0, true)
    }

    #[test]
    fn a_component_that_declares_nothing_is_a_real_answer() {
        let quiet = Quiet;
        assert!(quiet.actions().is_empty());
        assert_eq!(
            quiet.kind(),
            "quiet",
            "kind defaults to id, so a single-placement container needs no ceremony",
        );
        let s = store();
        let mut cx = ProviderCx::new("quiet", s);
        assert_eq!(
            quiet.perform("anything", &Intent::new("anything"), &mut cx),
            Handled::No,
        );
    }

    /// **A component that knows nothing about panes builds with the same context** (F003/P086/T367).
    ///
    /// `ChromeCtx::for_build` used to carry the workspace tree and the program catalog, so this
    /// dock — which shows one number — was handed another component's model through the contract
    /// they share. The context now carries only the frame's theme and the intent sink; a component's
    /// own model lives on its own state, where only it can see it.
    #[test]
    fn the_shared_context_carries_nothing_of_one_components_domain() {
        let store = store();
        let theme = heca_grid_ui::theme::Theme::default();
        let emit: crate::chrome::ChromeIntentEmitter = std::rc::Rc::new(|_| {});
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let ctx = ChromeCtx::for_build(crate::host::App::new(&store), &theme, &emit, &catalog);

        let Contribution::Container(c) = Quiet.build_contribution(&ctx) else {
            panic!("the quiet dock contributes a container");
        };
        let mut signals = crate::chrome::ChromeSignals::default();
        let mut drag = crate::chrome::DragItemRegistry::default();
        let mut bx = crate::chrome::BuildCx::new("quiet", &mut signals, &mut drag);
        let body = (c.build)(&ctx, &mut bx);

        assert!(body.base().children.is_empty(), "one label, no rows");
        assert!(drag.items().is_empty(), "…and it registered nothing host-side");
        assert!(heca_grid_ui::collect_hints(body.as_ref()).is_empty(), "…and declared no hint");
    }

    #[test]
    fn kind_is_the_type_and_id_is_the_placement() {
        let a = Dock::new("dock.left");
        let b = Dock::new("dock.right");
        assert_eq!(a.kind(), b.kind(), "two placements are one type");
        assert_ne!(a.id(), b.id(), "…and two seatings");
    }

    /// `perform` reads **this placement's** cursor, so the same component seated twice acts on two
    /// different rows.
    #[test]
    fn perform_acts_on_the_cursor_of_the_mount_it_was_aimed_at() {
        let s = store();
        s.set_container_cursor("dock.left", Some("alpha".into()));
        s.set_container_cursor("dock.right", Some("beta".into()));
        let dock = Dock::new("dock.left");
        let intent = Intent::new("dock.restart_selected");

        let mut left = ProviderCx::new("dock.left", s.clone());
        assert_eq!(
            dock.perform("dock.restart_selected", &intent, &mut left),
            Handled::Yes,
        );
        let mut right = ProviderCx::new("dock.right", s.clone());
        assert_eq!(
            dock.perform("dock.restart_selected", &intent, &mut right),
            Handled::Yes,
        );

        assert_eq!(
            dock.acted_on.borrow().as_slice(),
            ["alpha", "beta"],
            "one component, two placements, two selections",
        );
    }

    /// A component moves its **own** cursor, and only its own — the placement it was called on.
    #[test]
    fn a_component_moves_its_own_cursor() {
        let s = store();
        let cx = ProviderCx::new("dock.left", s.clone());
        cx.set_selected(Some("row-2".into()));

        assert_eq!(cx.selected(), Some("row-2".to_string()));
        assert_eq!(
            ProviderCx::new("dock.right", s).selected(),
            None,
            "the other placement of the same component is untouched",
        );
    }

    #[test]
    fn nothing_selected_is_a_silent_decline() {
        let s = store();
        let dock = Dock::new("dock.left");
        let mut cx = ProviderCx::new("dock.left", s);
        assert_eq!(
            dock.perform(
                "dock.restart_selected",
                &Intent::new("dock.restart_selected"),
                &mut cx
            ),
            Handled::No,
        );
        assert!(dock.acted_on.borrow().is_empty());
        assert!(cx.drain().is_empty(), "a decline asks the host for nothing");
    }

    /// What a component asks for is **queued**, not run: the host applies it after `perform`
    /// returns, when `&mut AppState` is free again — which is also what keeps the request on the
    /// policy-routed path instead of past it.
    #[test]
    fn what_a_component_asks_for_is_queued_for_the_host() {
        let s = store();
        s.set_container_cursor("dock.left", Some("alpha".into()));
        let dock = Dock::new("dock.left");
        let mut cx = ProviderCx::new("dock.left", s);
        dock.perform(
            "dock.restart_selected",
            &Intent::new("dock.restart_selected"),
            &mut cx,
        );

        let queued = cx.drain();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].action, "focus_pane");
        assert!(cx.drain().is_empty(), "draining takes them");
    }

    /// An unknown id declines rather than panicking — a component is asked about ids it may not own
    /// (a remount, a stale binding).
    #[test]
    fn an_unknown_action_id_declines() {
        let s = store();
        s.set_container_cursor("dock.left", Some("alpha".into()));
        let dock = Dock::new("dock.left");
        let mut cx = ProviderCx::new("dock.left", s);
        assert_eq!(
            dock.perform("someone.elses.action", &Intent::new("x"), &mut cx),
            Handled::No,
        );
    }
}