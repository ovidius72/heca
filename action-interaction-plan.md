# Action Interaction Architecture — declarative confirmation + response buttons

> Status: **PLANNED** (design locked with the user 2026-07-07). Owner: the plugin/grid-ui arc.
> Prerequisite already landed: the **central destructive gate** at the dispatch chokepoint
> (`interaction.rs`, `maybe_confirm_destructive` + `handlers::request_destructive`) — this plan
> generalizes that gate from 4 hardcoded variants to **data declared on each action**.

## 1. Goal

Every action declares, **as data**, whether it needs a prompt (confirmation / choice) and which
**response buttons** to show, plus what each button does (the *outcome*). One central gate reads that
data and drives the existing `OverlayHost::open_modal` pipeline. Built-in actions **and plugins**
declare this the same way, so a new surface (keyboard, header button, context menu, dropdown, RPC)
never re-implements confirmation — the guard lives on the **action**, not the call site.

Non-exhaustive cases to support:
- **Close / Delete** → yes/no confirm (2 buttons; danger).
- **3-way** → yes / no / cancel.
- Arbitrary N-response prompts declared by app or plugin.

## 2. Locked decisions

1. **Native callback outcome is included now** (`Outcome::Callback`). It is **native-only** and must
   be documented meticulously (see §6). Rationale (user): "if we don't do it now we never will."
2. **Runtime action registry now (2a).** Replace the compile-time `static ALL: &[ActionDescriptor]`
   catalog with a **runtime registry** so plugins (and native callbacks) can register actions with
   full metadata + interaction spec. Not an incremental const-first step.
3. **Config is a generic `[confirm]` table** keyed by action name (`[confirm].<name> = bool`), so a
   plugin action becomes user-configurable automatically without a new settings field.
4. **`ConfirmSpec` is a dedicated pure-data type** (serializable, plugin/RPC-safe, decoupled from the
   chrome/overlay layer). The gate converts `ConfirmSpec → ModalSpec` at open time.

## 3. What already exists (reuse, do not fork)

- **`OverlayHost::open_modal` + `ModalSpec`/`ModalAction`/`ModalResult` + `FnOnce` completion**
  (`heca/src/chrome/overlay.rs`): already opens a modal with N declared buttons, routes
  `SubmitOverlay`, and runs a completion. This is the render/route engine — the confirm feature is a
  **producer** of `ModalSpec`.
- **`ViewNode` / `Intent`** (`heca/src/chrome/view.rs`): serializable UI + action model (WASM/RPC).
  `ConfirmSpec.prompt` reuses `ViewNode`; declarative outcomes reuse the action-name + args `Intent`.
- **Central dispatch chokepoint** (`heca/src/app/interaction.rs`, `dispatch_intent`, the
  `Allow(ActivateAction)` arm): the single point every action executes through. The generic gate
  lives here.
- **`ActionDescriptor` metadata** (`heca/src/actions.rs`): `name/label/description/category/
  default_binding/icon`, resolved by name (`ActionRegistry::icon/label`). Today a `const` array;
  §5.1 moves it into the runtime registry.
- **Current destructive gate** (commit `<pending>`): `maybe_confirm_destructive` +
  `request_destructive`/`run_destructive_now`/`begin_confirm_delete`. This plan folds those into the
  generic path (the 3 destructive actions become 3 declared `ConfirmSpec`s).

## 4. The model (dedicated, pure data)

Lives in a **low layer** that both `heca` and plugins can depend on (candidate: a new
`heca-actions` crate or `heca-config`; decided in Phase A). Types are `serde`-serializable **except**
the native `Callback` outcome, which is represented opaquely across the WASM/RPC boundary.

```
ActionMeta {
    name: String,                 // config identity ("close", "delete_column", plugin ids)
    label: String,
    description: String,
    category: ActionCategory,
    default_binding: String,
    icon: Option<Glyph>,          // Glyph name across the boundary
    confirm: Option<ConfirmSpec>, // NEW — the interaction declaration
}

ConfirmSpec {
    prompt: ConfirmBody,          // Message(String) | View(ViewNode) — default: a one-line message
    buttons: Vec<ResponseButton>, // 2, 3, N
    default_button: usize,        // initial focus (safe default, e.g. Cancel)
    config_name: String,          // key under [confirm]; user override of on/off
    default_enabled: bool,        // value when [confirm].<config_name> is absent
}

ResponseButton {
    id: String,                   // comes back in ModalResult::Action { id }
    label: String,
    role: ButtonRole,             // Default | Cancel | Danger  (focus/tint/Esc mapping)
    outcome: Outcome,
}

Outcome {
    Proceed,                      // run the ORIGINAL gated action (the "yes, do it")
    Cancel,                       // do nothing
    Dispatch { action: String, args: PropMap },  // dispatch another named action (declarative)
    Callback(ActionCallback),     // NATIVE-ONLY closure (see §6); opaque across WASM/RPC
}
```

- `ButtonRole::Cancel` = the button Esc/scrim maps to; `Danger` = destructive tint; `Default` = the
  initially-focused safe choice.
- `ActionCallback = Rc<dyn Fn(&mut AppState, &ActionRegistry)>` (native handle; not `serde`).

## 5. Architecture

### 5.1 Runtime `ActionRegistry` (decision 2a)

Unify today's split (`handlers: HashMap<Discriminant, fn>` + `static ALL` descriptors) into **one
runtime registry**:

```
ActionEntry {
    meta: ActionMeta,                       // name/label/desc/category/icon/binding/confirm
    dispatch: Dispatch,                     // how to run it
}
Dispatch =
    Native(ActionHandler)                   // fn(&mut AppState, &WmAction) — built-ins
  | Declarative                             // name+args → routed via the Intent/ViewNode path (plugins)
```

- **Keying:** built-ins keep the **`Discriminant<WmAction>`** key for dispatch + confirm lookup
  (parameterized variants share a discriminant). A **name → entry** index preserves
  `icon/label/find/by_category/count`. Plugin actions have no `WmAction` variant → keyed by **name**
  and dispatched through the existing declarative `Intent` path (`dispatch_view_intent`).
- **Population:** built-ins **register at startup** (data moved out of `const ALL` into
  `register_builtins()` — same values, now runtime). Plugins register at load via the host API.
- **Back-compat:** `ActionRegistry::icon/label/find/by_category/count` keep working (now reading the
  runtime map). The `#[expect(dead_code)]` metadata catalog becomes **live** (command palette + RPC
  introspection consume it).
- **Confirm lookup:** `registry.confirm_spec(action) -> Option<&ConfirmSpec>` by discriminant/name.

> Note (name↔variant): `close` maps to both `ClosePane` and `ClosePaneById`. Both discriminants
> reference the **same** `ConfirmSpec` (config key `close`), so either dispatch confirms identically.

### 5.2 The generic gate

Replace `maybe_confirm_destructive` with a data-driven `maybe_confirm`:

```
fn maybe_confirm(state, registry, action) -> bool {
    let Some(spec) = registry.confirm_spec(action) else { return false };
    if !confirm_enabled(&state.config, &spec.config_name, spec.default_enabled) { return false }
    let resolved = resolve_target(state, action);           // e.g. ClosePane → ClosePaneById{focused}
    let modal = build_modal_spec(spec, state);              // ConfirmSpec → ModalSpec (§5.3)
    open_modal(state, modal, move |state, registry, result| {
        run_outcome(state, registry, spec, &resolved, result);
    });
    true
}
```

Placed in the `Allow(ActivateAction(act))` arm of `dispatch_intent` (where the current gate is):
`if !maybe_confirm(...) { registry.execute(&act, state); }`.

- **`resolve_target`**: pins the concrete action at prompt time (only `ClosePane`→`ClosePaneById
  {focused}` needs it today; every other action resolves to itself). Kept as a small, explicit map —
  NOT per-call-site logic.
- **Loop-safety**: `Outcome::Proceed` runs the resolved action via **`registry.execute(&resolved,
  state)`**, which calls the raw handler directly — the gate lives only in `dispatch_intent`, so
  `registry.execute` never re-enters it. (Generalizes today's `run_destructive_now`.) This removes
  the need for a `confirmed` flag.

### 5.3 `ConfirmSpec → ModalSpec` conversion (keeps layers clean)

The gate (in the `heca` chrome layer) converts the pure-data `ConfirmSpec` into a `ModalSpec`:
`prompt → body ViewNode`, each `ResponseButton{id,label,role}` → `ModalAction{id,label,danger =
role==Danger}`, `default_button`/`Cancel` role → the Dialog's initial focus + dismiss mapping.
`open_modal` already builds real hintable `Button`s with tooltips from the id (the centralized path).

### 5.4 `run_outcome`

On `ModalResult::Action{id}` find the `ResponseButton` by id and run its `Outcome`:
- `Proceed` → `registry.execute(&resolved, state)`.
- `Cancel` / `Dismissed` → nothing (Esc/scrim maps to the `Cancel`-role button).
- `Dispatch{action,args}` → `dispatch_action` (re-enters the gate normally for the new action).
- `Callback(cb)` → `cb(state, registry)`.

### 5.5 Config (decision B)

- New `[confirm]` table: `[confirm] close = true`, `delete_column = true`, `delete_workspace = true`,
  `<plugin_action> = false`, … Keyed by `ConfirmSpec.config_name`. Absent → `default_enabled`.
- **Migrate** `confirm_close_pane` / `confirm_delete_column` / `confirm_delete_workspace` out of
  `[settings]` into `[confirm]` (`close`/`delete_column`/`delete_workspace`). Document the move;
  decide read-old-keys back-compat in Phase B.
- `config.default.toml` gains the `[confirm]` section with all built-in confirmable actions
  (cardinal rule: every setting in the default file).

### 5.6 Register-action API (native + plugin)

- **Native dev**: one call declares dispatch + metadata + interaction:
  ```
  registry.register(ActionSpec {
      action: WmAction::ClosePaneById { .. },   // discriminant → handler
      handler: handle_close_pane_by_id,
      meta: ActionMeta { name: "close", label: "Close Pane", icon: Some(XSquare), .. ,
          confirm: Some(ConfirmSpec {
              prompt: Message("Close pane?".into()),
              buttons: vec![
                  ResponseButton::cancel("cancel", "Cancel"),
                  ResponseButton::new("confirm", "Close", Danger, Outcome::Proceed),
              ],
              default_button: 0, config_name: "close".into(), default_enabled: true,
          }),
      },
  });
  ```
  Native callbacks are available (`Outcome::Callback`) for app-internal logic.
- **Plugin (WASM)**: declares an action + `ConfirmSpec` as **serialized data**; outcomes limited to
  `Proceed / Cancel / Dispatch`. A first-party host adapter registers it as a **Declarative** action
  (name-based, dispatched via the `Intent` path). Plugins **cannot** use `Callback`.

## 6. `Outcome::Callback` — meticulous documentation (decision 1)

**Callback is a native-only escape hatch. Document all of this in code + `AGENTS.md` + `docs`:**

- **Signature:** `Rc<dyn Fn(&mut AppState, &ActionRegistry)>`. Runs once when its button is chosen.
- **Native-only, by construction:** a closure is not `serde`-serializable and cannot cross the WASM
  or RPC boundary. **Plugins must not use it** — the plugin-facing builder does not expose it. When an
  action's metadata is serialized for RPC/introspection, a `Callback` outcome is rendered **opaquely**
  (e.g. `{"outcome":"native"}`) — never silently dropped.
- **When to use it vs `Dispatch`:** prefer `Dispatch{action}` (portable, testable, RPC-drivable).
  Use `Callback` **only** when the logic genuinely cannot be a named action (rare). If you reach for
  `Callback`, first ask whether a small native action + `Dispatch` is cleaner.
- **No policy re-routing:** like the confirm completion, a `Callback` runs after the user already
  chose — it executes directly; it does not re-enter interaction policy. Document that it must not be
  used to smuggle un-gated destructive work (compose `Dispatch`/`Proceed` for that).
- **Lifetime/capture:** captured state must be owned/`'static` (`Rc`), consistent with `open_modal`
  completions.
- **Testing:** provide a unit test that a `Callback` outcome fires exactly once on its button and not
  on cancel/dismiss.

## 7. Migration of current work

- The 3 hardcoded destructive variants in `maybe_confirm_destructive` → **3 declared `ConfirmSpec`s**
  on `close` / `delete_column` / `delete_workspace`. The gate becomes generic `maybe_confirm`.
- `request_destructive` / `run_destructive_now` / `begin_confirm_delete` / `destructive_message`
  collapse into: `ConfirmSpec` data + `build_modal_spec` + `run_outcome` (`Proceed` =
  `registry.execute`). Net: less bespoke code, all data-driven.
- `[settings] confirm_*` → `[confirm]` table.

## 8. Phases (execution order)

- **Phase A — Runtime registry foundation.** Convert `ActionRegistry` to hold `ActionEntry`
  (metadata + dispatch) at runtime; move built-in descriptors from `const ALL` into
  `register_builtins()`; keep `icon/label/find/by_category/count` working; pick the crate for the
  shared types. Green + clippy, **no behavior change**. Tests: registry parity with the old catalog.
- **Phase B — `ConfirmSpec` + generic gate.** Add the dedicated types (`ConfirmSpec`/`ResponseButton`
  /`Outcome`/`ButtonRole`), attach `ConfirmSpec`s to the 3 destructive built-ins, generalize the gate
  (`maybe_confirm`), `ConfirmSpec→ModalSpec` conversion, `run_outcome` (incl. `Callback`), `[confirm]`
  config + `config.default.toml` + migration off `[settings] confirm_*`. Remove the hardcoded gate +
  fold `request_destructive`/`run_destructive_now`/`begin_confirm_delete`. Tests: gate opens on
  enabled, skips on disabled, `Proceed`/`Cancel`/`Dispatch`/`Callback` each run correctly, 3-way
  works. Docs.
- **Phase C — Plugin/dev API + docs.** The `register(ActionSpec)` API for native; the plugin/WASM
  declarative path + host adapter; RPC introspection of action metadata; **meticulous `Callback`
  docs** (§6). Update `AGENTS.md` "Adding New Actions" (now includes the confirm spec), `README`
  (`[confirm]`), `docs/`. Tests: a declarative plugin-style action with a `ConfirmSpec` confirms; a
  `Callback` action fires its closure.

## 8b. Phase A — precise execution notes (scoped 2026-07-07)

Concrete scope discovered by grep (start here):
- **New type `ActionCatalog`** (`heca/src/actions.rs`): runtime metadata store. Phase A holds
  **`&'static ActionDescriptor`** entries seeded from the existing `ActionRegistry::ALL` const
  (zero-copy; owned/`String` plugin entries are deferred to task-C to avoid a `&'static str`→`String`
  ripple now). Methods mirror today's statics: `find/icon/label/by_category/count`, **same `&'static`
  returns** so no caller type changes.
  ```
  ActionCatalog { by_name: HashMap<&'static str, &'static ActionDescriptor>, order: Vec<&'static ActionDescriptor> }
  ::with_builtins() -> seed from ActionRegistry::ALL
  ```
- **`AppState.action_catalog: ActionCatalog`** built in startup (`heca/src/app/startup.rs`).
- **Migrate the 6 static-metadata call sites** off `ActionRegistry::{icon,label,find}` → the catalog:
  - has `state`: `heca/src/app_state.rs:~239` (`self.action_catalog.find`), `heca/src/mouse.rs` ×3
    (`state.action_catalog.icon`).
  - **stateless helpers → thread `&ActionCatalog`**: `heca/src/chrome/mod.rs` `pane_action_spec`
    (icon lookup, callers `mod.rs:559,602` + tests `4149-4171`) and `action_tooltip` (label lookup,
    callers `overlay.rs:202`, `mod.rs:747,1884`). Pass the catalog down from where the chrome tree is
    built (it has `&AppState`).
- **Remove** the static `ActionRegistry::{find,icon,label,by_category,count}` methods once all callers
  use the catalog; keep `ActionRegistry::ALL` as the **built-in seed** (private-ish). Move the
  metadata unit tests to exercise `ActionCatalog`.
- **No behavior change**; gate: `heca` tests green (currently 311) + clippy clean.

## 9. Open details to settle during execution

- Which crate hosts the shared `ActionMeta`/`ConfirmSpec` types (new `heca-actions` vs `heca-config`).
- `resolve_target` map location + whether other actions need pinning beyond `ClosePane`.
- Back-compat for the old `[settings] confirm_*` keys (read + warn, or hard-migrate).
- `ConfirmBody::View` (rich prompt) — wire now or after a real consumer needs it.
