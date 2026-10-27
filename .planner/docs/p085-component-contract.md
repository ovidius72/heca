# Why

F003/P011/T020 landed chrome focus (a container id, a pick, a focus ring) and it is **inert**: nothing delivers keys to a focused container, so `prefix+Shift+e` focuses a dock and then you still need `prefix+e` to do anything with it. Verdict from the user 2026-07-29: *"it has no value if I have to do prefix+e to interact... it's only more keybindings."*

And the thing it would generalise — `InputMode::SidebarNav` — is not generalisable as written: every verb in it is app code that knows the workspace tree. `handle_sidebar_up/down` drive `state.sidebar_tree` (a workspace/column/pane projection, `heca/src/handlers.rs:1562+`) and `handle_sidebar_right_nav` calls `handle_focus_pane` directly (`handlers.rs:1586`). A second container inherits none of it.

# ✅ DECIDED BY THE USER 2026-07-29

**The rule the design hangs on: widgets answer gestures; components declare semantics.**

- Wheel, click, drag, hover, and keyboard **scrolling** are answered by the widgets a container composes. `ScrollRegion` already answers the eight `WidgetIntent::Scroll*` and *declines* the axis it cannot move (shipped `096f06e`). Nothing is declared for these — a container that nests a scroll area scrolls; one showing a single number declines.
- What only the component knows is **what its selection means**: rename *this*, delete *this*, restart *this*. That is what it declares.

**No generic verb set.** An inherited up/down/select/toggle vocabulary is dead weight for a container showing one number — "a component knows exactly what it needs" (user). Conventional *ids* (`up`, `down`, `activate`, `remove`, `add`) are encouraged so docks agree in practice, never enforced in code.

**Focus is the mode.** No `InputMode` variant for it: chrome focus already answers "where do the keys go", and a mode beside it would be a second fact that can disagree (the defect `nav_selection` vs `sidebar_tree.cursor` already paid for). Key routing, top to bottom, in every focus state:

1. prefix key → prefix mode → mode triggers → the global `normal` map → **falls through to the focused component's layer** → drop
2. global direct bindings (`Alt+Enter`, `Shift+PageUp`, …)
3. if a component holds chrome focus → its `[keys.<kind>]` layer → **swallow** if unbound
4. otherwise → forward to the focused pane's backend

`state.focused_pane` is NOT cleared by chrome focus — only the keyboard is redirected, so `prefix+Enter` still splits the pane you last worked in, exactly as sidebar mode behaves today.

**Components declare, mostly bind.** A component's keymap maps keys to **any** action id — an existing built-in (`next_pane`, `zoom_column`, `spawn_command` with args) or one of its own registered ids. It declares only what does not exist yet, which is the selection-dependent handful.

**`perform`, not one closure per action.** `Provider` gains `perform(id, args, cx) -> Handled`: it is the shape a WASM component is forced into (an id + args crosses a boundary, a closure does not), it keeps `&self` available, and `ProviderCx` (read + emit) enforces §2.3 — a raw `DynHandler` hands a component `&mut AppState`, which is more power than a provider may have.

**A row's `nav_key` is its one identity.** Not a keyboard concern: the keyboard cursor, the right-click target, and (later) drag are three readers of the same declaration. The host derives the context path from the owning component's `kind()`.

**kind vs mount.** Bindings and declared actions belong to the component **type** (`kind()`, the config namespace `[keys.docker]`); cursor, scroll offset and focus belong to each **placement** (`id()`, the mount). `Hero::new()` twice = two instances of one type. `WorkspacesContainerProvider::placed(id, region)` currently makes `id()` the mount id with no `kind()`, so this must land before a second placement exists.

# The worked example — a Docker containers dock, end to end

Two actions only it can do, three keys (one reusing a built-in with args), right-click firing one of them. ✦ marks API this phase adds.

## The component

```rust
// heca/src/providers/docker.rs — app-side. A WASM plugin is the same shape across the boundary.

pub struct DockerProvider {
    /// This PLACEMENT (`docker`, `docker.right`). Cursor/scroll/focus key off this.
    id: String,
    region: RegionId,
    /// The component's own domain state — not app state.
    service: DockerService,
}

impl Provider for DockerProvider {
    fn id(&self) -> &str { &self.id }          // instance
    fn kind(&self) -> &str { "docker" }        // ✦ TYPE → the config namespace `[keys.docker]`
    fn title(&self) -> &str { "Containers" }
    fn supported_regions(&self) -> RegionSet { RegionSet::sidebars() }
    fn default_region(&self) -> RegionId { self.region }
    fn keyboard_navigable(&self) -> bool { true }

    fn build_contribution(&self, ctx: &ChromeCtx<'_>) -> Contribution {
        Contribution::Container(ContainerContribution { /* … */ build: Box::new(build_body) })
    }
}

fn build_body(ctx: &ChromeCtx<'_>, bx: &mut BuildCx<'_>) -> WidgetModel {
    let mut list = Flex::column().gap(4.0);
    for c in ctx.state().containers::<DockerRow>() {
        list = list.child(
            Row::new()
                .nav_key(&c.id)        // ✦ the row's ONE identity: cursor + right-click target (+ drag later)
                .on_activate(/* click = attach */)
                .child(Label::new(&c.name)),
        );
    }
    // Scrolling is the widget's: nested here, it answers PageUp/Down/Home/End with nothing declared.
    Box::new(ScrollRegion::new().grow(1.0).child(list))
}
```

## Its two actions ✦

```rust
impl ProviderActions for DockerProvider {
    /// ONLY what nothing else can do. Everything else it wants is an existing action it binds.
    fn actions(&self) -> Vec<ActionMeta> {
        vec![
            ActionMeta {
                name: "docker.restart_selected".into(),
                label: "Restart Container".into(),
                description: "Restart the selected container.".into(),
                category: ActionCategory::Chrome,
                default_binding: "r".into(),      // the field already exists — unused today
                icon: Some(Glyph::ArrowClockwise),
                policy: ActionPolicy::Global,
                args: vec![],                      // acts on the selection; nothing to pass
                confirm: None,
            },
            ActionMeta {
                name: "docker.stop_selected".into(),
                label: "Stop Container".into(),
                description: "Stop the selected container.".into(),
                category: ActionCategory::Chrome,
                default_binding: "s".into(),
                icon: Some(Glyph::Stop),
                policy: ActionPolicy::Global,
                args: vec![],
                // Destructive → the CENTRAL confirm gate prompts on every surface. Free.
                confirm: Some(ConfirmSpec::destructive("docker_stop", "Stop")),
            },
        ]
    }

    /// One entry point. The selection is the component's own, keyed by THIS mount.
    fn perform(&self, action: &str, _args: &Intent, cx: &mut ProviderCx<'_>) -> Handled {
        let Some(id) = cx.selected() else { return Handled::No };   // nothing selected ⇒ decline
        match action {
            "docker.restart_selected" => { self.service.restart(id); Handled::Yes }
            "docker.stop_selected"    => { self.service.stop(id);    Handled::Yes }
            _ => Handled::No,
        }
    }
}
```

`perform` never touches `AppState`: its own service, or `cx.dispatch("focus_pane", args)` to reach an app action.

## Its three keybindings — shipped defaults, live only while it has focus

```toml
# keybindings.default.toml
[keys.docker]                 # ✦ merges PER KEY: override one, keep the rest
restart_selected = "r"        # its own action (namespace implied by the table)
stop_selected    = "s"

[[keys.docker.bind]]          # ✦ the arg-carrying form; merges by `keys`
action = "spawn_command"      # ← an EXISTING built-in, not redeclared
keys   = "t"
args   = { command = "lazydocker", float = "true" }

[keys.docker.unbind]          # ✦ explicit removal (no null/empty-string semantics)
"s" = true
```

## Right-click → the same action

```rust
fn context_menus(&self, _ctx: &ChromeCtx<'_>) -> Vec<ContextMenuContribution> {
    vec![ContextMenuContribution {
        context_path: "container.docker".into(),   // derived from kind() on the resolution side
        weight: vec![1],
        build: Box::new(|_ctx, target| vec![
            MenuEntry::action("Restart", Intent::new("docker.restart_selected"))
                .enabled(target.row_key().is_some()),
            MenuEntry::action("Stop", Intent::new("docker.stop_selected")),
        ]),
    }]
}
```

Same id, same `perform`, same confirm gate — one path for key, menu and RPC. The hook exists today (`Provider::context_menus`); `row_key()` on the target is new.

## The one host-side piece

```rust
// on mount, once — the HOST, not the component (a provider never mutates app state)
for meta in provider.actions() {
    let kind = provider.kind().to_string();
    let default_keys = meta.default_binding.clone();
    let id = meta.name.clone();
    handles.keep(register_dynamic(registry, catalog, meta, Some(Rc::new(route_to_owner))));
    keymaps.bind_component_default(&kind, &default_keys, &id);   // ✦ user config wins
}

// and the bridge, written once for every component that will ever exist
fn route_to_owner(state: &mut AppState, intent: &Intent) {
    let Some(mount) = owning_mount(state, &intent.action) else { return };  // focused, else last-focused of that kind
    let store = state.chrome_state.clone();        // Rc alias — no borrow of AppState held
    let mut cx = ProviderCx::new(&mount, store, state.intent_sink());
    if let Some(p) = state.chrome_host.provider(&mount) {
        p.perform(&intent.action, intent, &mut cx);  // &self available; no &mut AppState alive
    }
    cx.drain_into(state);                          // emitted intents applied by the host
}
```

## What the author wrote vs what they got

**Written:** `kind()`, two `ActionMeta`s, a five-line `perform`, `.nav_key()` per row, three TOML lines, one menu hook.

**Free:** the palette entry with its icon and label; `describe-action` + RPC; the destructive confirm on every surface; `prefix+/` hint targets on the rows; wheel, click, drag and PageUp/Down/Home/End from the widgets; the focus ring; a cursor that survives a tree rebuild.

**Written nowhere:** up/down/select verbs — for this component they are the cursor over its own `nav_key`ed rows; for a one-number component they do not exist.

# What already exists and is unused

- `register_dynamic(registry, catalog, meta, handler)` (`heca/src/actions.rs:409`) — name-keyed actions joining the same catalog as built-ins: label, icon, `policy`, declared `args`, `confirm`. Carries `ActionMeta.default_binding` (`actions.rs:2072`) which is **`#[allow(dead_code)]` — declared, never consumed by anything**.
- `DynHandler = Rc<dyn Fn(&mut AppState, &Intent)>` (`actions.rs:377`); `handler: None` = `Dispatch::Declarative`, already how a WASM action is modelled.
- `ProviderHandles` (`heca/src/providers/mod.rs:100`) — documented as where dynamic-action unregister handles join, so actions dying with the mount is the intended lifetime.
- `Provider::context_menus` (`providers/mod.rs:83`) is live: right-click / `prefix+>` → `open_context_menu_for` (`chrome/context_menu.rs:232`) → `plugin_providers_for` (`:271`) → `host.mounted_providers().flat_map(|p| p.context_menus(ctx))` filtered by `context_path`.
- Config bindings already resolve to `ActionRef::Builtin` **or** `ActionRef::Dynamic` at press (`app/registry.rs`), so binding a component's id needs no new resolution.
- `FocusRing` + `focused_container` + per-mount `keyboard_target` signals + the `focus_dock` pick (F003/P011/T020, landed).

# The gap, precisely

- Nothing turns `ActionMeta.default_binding` into a binding (the keymap is built from config at load, before any provider activates).
- Nothing scopes a binding to "while this component has focus".
- Right-click resolves its `(ContextPath, ContextTarget)` by **matching a closed domain enum**: `mouse.rs:143` matches `ChromeDragItem::{Pane,Column,Workspace}`, and `ContextTarget` is closed too. A plugin row cannot be expressed, so a `container.docker` path could never be produced.
- `resolve_active_context` (`context_menu.rs:308`) reads `InputMode::SidebarNav` + the sidebar cursor — a second, keyboard-specific resolution of the same fact.

# Decisions, dated (do not re-open without the user)

| Decision | Date |
|---|---|
| Widgets answer gestures; components declare semantics | 2026-07-29 |
| No generic verb set — a component declares exactly what it needs; conventional ids encouraged, never enforced | 2026-07-29 |
| Focus is the mode — no `InputMode` variant beside it | 2026-07-29 |
| An unbound key while a component is focused is **swallowed**, never forwarded to the backend | 2026-07-29 |
| Prefix **falls through** to the focused component's layer when the global map misses | 2026-07-29 |
| `perform(id, args, cx)`, not one closure per action | 2026-07-29 |
| `kind()` (type → bindings, declared actions) is distinct from `id()` (mount → cursor, scroll, focus) | 2026-07-29 |
| `nav_key` is the row's one identity: cursor, right-click target, later drag | 2026-07-29 |
| `[keys.<kind>]` is a TABLE (merges per key); `[[keys.<kind>.bind]]` merges by `keys`; removal via an explicit `unbind` table, never null/empty-string | 2026-07-29 |
| The palette/RPC show every registered action; choosing one focuses its component then performs (`FocusContainerThenAction`), with the last-focused instance of that kind as the target | 2026-07-29 |
| An `ActionDispatched` event is a **notification**, not a veto | 2026-07-29 |
| Keyboard scrolling is the widget's — no declaration; plain PageUp/Down/Home/End while focused, superseding T012's prefix-gated route | 2026-07-29 |

# Acceptance for the phase

- A container declaring nothing still scrolls, clicks and drags; a container declaring two actions gets keys, RPC, palette, context menu and the confirm gate from that declaration alone.
- No generic verb set anywhere; no `InputMode::SidebarNav`; no `sidebar_*` built-ins.
- Nothing in the focus, menu or nav path matches on a workspace/column/pane enum.
- Scoped changed-crate tests and Clippy pass, and Antonio drives the interaction checks in the app.


# Approved ownership clarification — 2026-10-24
T357 reuses the existing ContainerFocusChanged and ContainerCursorChanged events and adds accepted action-dispatch notifications; no second selection-event path. T360 owns named per-mount values, their change events, writes through ProviderCx and ViewNode.bindings. T506 owns the public read-only namespace facade and consumes those capabilities; it does not reimplement them. No new hand-written StateView selectors belong in T357/T360. The concrete namespace builder syntax still requires agreement before implementation. Legacy notes suggesting growing StateView are historical and superseded by the canonical accepted decision on P085.

Use the scoped checks ./scripts/lint-changed.sh test and ./scripts/lint-changed.sh. Prior whole-workspace wording is superseded. P097 is done; T357/T359/T360 remain planned. Old readiness counters, branch distances and warning counts are not current evidence.

