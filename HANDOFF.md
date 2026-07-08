# HANDOFF — action-interaction architecture + focus-ring polish

> **How to resume:** a fresh session starts in the MAIN checkout `/Users/antonio/projects/myvim`.
> This work is in a **sibling worktree** `/Users/antonio/projects/myvim-gridui-styling` — operate
> there (`git -C …`, abs paths). Say *"read HANDOFF.md in the worktree and resume."*
> **Branch:** `feat/gridui-styling-foundation`.

## 0. Orientation — read FIRST
- **MANDATORY before touching code** (enforced): `AGENTS.md` (esp. the ⛔ STOP section: use existing
  grid-ui widgets; actions/keys through registries + **EVERY setting → `config.default.toml`, EVERY
  keybound action → `keybindings.default.toml`**; no "for now" fixes; the FUNDAMENTAL styling/layout
  contract), `docs/widgets.md` (widget catalog + focus primitives), `docs/surface-compositor.md`
  (layers/overlays), `action-interaction-plan.md` (the architecture this session executed).
- **Never run `cargo fmt`** (rustfmt 1.9 churns unrelated files). **Fix all warnings** (even
  pre-existing). `cargo build -p heca` can exceed a 2-min tool timeout → use long timeouts.
- **No commit/push/PR/merge without explicit user OK** (this session's commits were all OK'd).

## 1. Git state
- **Pushed:** up to `8548b92` (`focus_ring` is the universal focus indicator — all grid-ui widgets).
- **Local, UNPUSHED** (9 commits, oldest→newest) — the whole action-interaction arc + fixes:
  - `d918b4d` fix: **central destructive-confirm guard** at the dispatch chokepoint (bug: header
    close button bypassed confirm) — verified in-app by the user.
  - `a4fe71c` docs: action-interaction **plan** (`action-interaction-plan.md`) + backlog phase.
  - `ba9f938` docs: precise Phase A execution notes.
  - `b0d13ce` refactor: **Phase A** — runtime `ActionCatalog` on `AppState`.
  - `ae5b6dc` feat: **Phase B** — declarative `ConfirmSpec` + generic confirm gate.
  - `e8d60a3` feat: **B2** — generic `[confirm]` config table replaces `[settings] confirm_*`.
  - `a92ebdd` docs: README `[confirm]` section.
  - `7519fa7` refactor: rename confirm key `close` → **`delete_pane`**.
  - `8dae1d1` fix: **Modal focus ring** tone-follows like Button/Dialog.
- **Whole workspace green** at HEAD: heca **313**, heca-config **78**, heca-theme **25**,
  grid-ui **58+127+1**. Clippy clean (only the unrelated upstream `block v0.1.6` note).
- **When ready:** the user may ask to **push** these 9 commits.

## 2. THE BIG ARC — Declarative action interaction (confirm + response buttons)
Goal: an action declares **as data** whether it needs a prompt (confirm/choice) and which response
buttons + outcomes; ONE central gate reads it; built-ins **and plugins** declare it the same way, so
no surface re-implements confirmation (the guard is on the **action**, not the call site). Full
design: **`action-interaction-plan.md`**.

### Locked decisions (do NOT relitigate)
1. **Native `Outcome::Callback` included now** — native-only, documented meticulously. Not
   serializable (never crosses WASM/RPC); plugins use declarative outcomes only.
2. **Runtime action registry now** (`ActionCatalog`), replacing the static `ALL` lookup path.
3. **Generic `[confirm].<action>` config table** (not per-scope settings fields).
4. **Dedicated pure-data `ConfirmSpec`** (decoupled from chrome/overlay), converted to `ModalSpec`
   at open time — NOT reusing `ModalSpec` directly.

### Phase A — runtime `ActionCatalog` (commit `b0d13ce`)
- `heca/src/actions.rs`: new `ActionCatalog { by_name, order, confirm }`, `with_builtins()` seeds
  from the built-in `ActionRegistry::ALL` (`&'static ActionDescriptor`; owned plugin entries deferred
  to Phase C). Methods `find/icon/label/by_category/count` (instance) replaced the **removed** static
  `ActionRegistry::{find,icon,label,by_category,count}`.
- `AppState.action_catalog` (`app_state.rs`), built in `startup.rs`.
- Migrated the 6 metadata call sites: `mouse.rs` context-menu builders (`entry` takes
  `&ActionCatalog`), `InputMode::pending_pick(&catalog)` + `render::status_mode_parts(_, &catalog)`,
  and the stateless chrome helpers `pane_action_spec` / `sidebar_toggle_button` (threaded
  `&ActionCatalog` via `PaneHeaderContent.catalog`, alongside the existing `shortcuts`).
- **No behavior change.** `by_category`/`count`/`category`/`default_binding` kept for the command
  palette + RPC introspection (some `allow(dead_code)` — the `expect(dead_code)`↔liveness interaction
  is finicky; use `allow` for those).

### Phase B — `ConfirmSpec` + generic gate (commit `ae5b6dc`)
- `heca/src/actions.rs`: `ConfirmSpec { message, buttons, dismissible, config_name, default_enabled }`,
  `ResponseButton { id, label, role, outcome }` (+ `cancel`/`proceed`/`new` ctors), `ButtonRole
  {Default,Cancel,Danger}`, `Outcome {Proceed, Cancel, Dispatch(WmAction), Callback(ConfirmCallback)}`,
  `ConfirmCallback = Rc<dyn Fn(&mut AppState, &ActionRegistry)>` (native-only, meticulously documented
  on the type + variant). `builtin_confirm_specs()` declares the 3 destructive specs.
- `heca/src/handlers.rs`: the central gate `maybe_confirm_destructive(state, action) -> bool` now
  **reads the catalog spec** (resolves `ClosePane`→`ClosePaneById{focused}` pinned at prompt time,
  looks up `confirm_config_name(action)` → `catalog.confirm_spec(name)`, checks `confirm_enabled`),
  then `open_confirm` converts `ConfirmSpec→ModalSpec` and runs the chosen button's `Outcome` via
  `run_outcome`: **Proceed = `registry.execute(resolved)`** (bypasses the gate → NO loop), Cancel =
  nothing, Dispatch = `dispatch_action`, Callback = the native closure. Dismiss (Esc/scrim) → the
  `Cancel`-role button's outcome.
- **Folded/removed:** `begin_confirm_delete` (gone); `request_destructive` + `run_destructive_now`
  reimplemented on the spec path (`request_destructive` still exists for the sidebar/keyboard
  resolvers that compute a target); `destructive_message` → `confirm_title` (the dynamic,
  target-specific title stays code — a static spec can't hold the pane/ws name).
- Gate lives at `interaction.rs` `dispatch_intent`'s `Allow(ActivateAction)` arm:
  `if !maybe_confirm_destructive(state, &act) { registry.execute(&act, state); }`.

### B2 — generic `[confirm]` config (commit `e8d60a3`)
- `heca-config/src/confirm.rs`: `ConfirmConfig { actions: HashMap<String,bool> }` (`#[serde(flatten)]`)
  + `enabled(name, default)`. On `Config.confirm` (`loader.rs`). Removed
  `confirm_close_pane/column/workspace` from `SettingsConfig`.
- `AppState.confirm` holds it (built at `startup.rs` + reload in `main.rs`); `handlers::confirm_enabled`
  → `state.confirm.enabled(name, spec.default_enabled)`. `config.default.toml` gains `[confirm]`.
  README documents it (`### Confirmation prompts`).
- **BREAKING config change:** old `[settings] confirm_*` keys are gone → users move them to
  `[confirm]` as `delete_pane` / `delete_column` / `delete_workspace` (documented).
- Also fixed a stale `heca-config` test (`bundled_latte_is_a_light_theme` asserted the old
  `show_focus_border=false`, flipped by the focus-ring commit `8548b92`).

### Rename (commit `7519fa7`)
- Confirm key **`close` → `delete_pane`** (was too generic; now consistent with
  `delete_column`/`delete_workspace`). The confirm key is **decoupled** from the action's descriptor
  name (which stays `close` for keybinding/tooltip/icon). Changed in `builtin_confirm_specs`,
  `confirm_config_name`, `config.default.toml`, README, plan/backlog/app_state docs.

### How a dev/plugin declares a confirm now
```rust
ConfirmSpec {
    message: "This action cannot be undone.".into(),
    buttons: vec![
        ResponseButton::cancel("cancel", "Cancel"),
        ResponseButton::proceed("confirm", "Delete", /*danger*/ true),
        // ResponseButton::new("discard", "Discard", Default, Outcome::Dispatch(some_action)),
        // Outcome::Callback(rc_closure)  // native-only
    ],
    dismissible: false, config_name: "delete_pane".into(), default_enabled: true,
}
```

## 3. Focus-ring work (this session + yesterday)
- **Yesterday (pushed, `bc8779d`→`8548b92`):** replaced the corner-bracket focus reticle with
  `PaintCx::focus_ring` — a **CSS-style thin accent outline drawn just OUTSIDE** the widget (visible
  on borderless Ghost/Link), shown whenever `focused`. Color is **theme-aware** (shifted toward
  `foreground`, so light/dark-correct) via `heca_theme::Theme::effective_focus_ring()` /
  `focus_ring_tone(base)`, plus the optional `focus_ring` theme token. **Tone-following:** a
  `Destructive` button rings in `danger`, else `accent` (`button.rs:385`). Migrated all 12 other
  widgets onto `focus_ring`; removed the glowing `corner_brackets`. `latte.toml` `show_focus_border`
  flipped to `true`.
- **This session (`8dae1d1`):** the `Modal` widget drew its focused button's ring **manually** with
  hardcoded `accent` regardless of `danger` → its danger button rang blue while `Dialog`/`Button`
  tone-follow. **Fixed** `modal.rs` to use the shared `cx.focus_ring` + tone-following +
  `show_focus_border` gate — identical to `Button`. **User confirmed focus now looks OK in both
  light and dark themes.**

## 4. OPEN / NEXT STEPS (in priority order)
1. **[BUG] Overlay click clears button focus (Dialog + Modal).** Clicking the modal **body** (not a
   button) clears the focused button's ring. The overlay panel isn't focusable — a background click
   should be a **no-op for focus** (keep current focus), not a clear. Likely the pointer path calls
   `FocusManager::focus_at`, which clears when the click misses every focusable. Fix in
   `heca-grid-ui/src/widgets/dialog.rs` (app-used) + `modal.rs`. Tracked in `BACKLOG.md` under the
   focus-ring entry. **Not started (context budget).**
2. **[polish] The 2 showcase confirm demos render buttons differently.** The `Modal` widget draws its
   buttons **manually** (`modal.rs` paint) while `Dialog` uses **real `Button`** widgets → different
   button *look* (fill/border), even though focus is now consistent. `Modal` is legacy (the app's
   confirm uses `Dialog` via `OverlayHost::open_modal`). Options: (a) make `Modal` render real
   `Button`s, or (b) deprecate the `Modal` widget + its showcase demo in favour of `Dialog`. Decide
   with the user.
3. **Phase C** (`action-interaction`): native `register(ActionSpec)` API + plugin/WASM declarative
   path + host adapter + RPC introspection of action metadata; update AGENTS.md "Adding New Actions"
   to include the confirm spec. The `Callback` meticulous doc already lives in the code.
4. **Context-menu → OverlayHost migration** (the LAST bespoke overlay, `mouse.rs`
   `open_context_menu`/`open_sidebar_context_menu` + `AppState.context_menu` + `settle_context_menu`).
   Now trivial: menus just **dispatch the plain action** and the central gate confirms — no per-menu
   destructive special-case (already removed). Add `OverlayHost::open_dropdown` mirroring `open_modal`
   (design in `action-interaction-plan.md` context + the earlier discussion) + an `on_dismiss` on the
   `ContextMenu` widget.
5. **Push** the 9 local commits when the user OKs.

## 5. KEY DECISIONS (with WHY — do not relitigate)
- **Confirm guard on the ACTION, at the dispatch chokepoint** (`interaction.rs`), not per call site —
  so every surface (keyboard, header button, menu, RPC) confirms identically. `Proceed` runs via
  `registry.execute` (bypasses the gate) → loop-safe.
- **`ConfirmSpec` is pure data + dedicated** (not `ModalSpec`); the gate converts it → keeps the
  action-metadata layer decoupled from chrome and serializable for plugins (Phase C).
- **Confirm key `delete_pane`** ≠ the action descriptor name `close` (decoupled).
- **`[confirm]` is a generic name-keyed table** so plugin actions are configurable without new fields.
- **Focus ring tone-follows** (danger→danger, else accent), theme-aware (shift toward `foreground`),
  drawn OUTSIDE the widget. The user explicitly wants the danger button's focus near `danger`, NOT
  accent — the Modal manual-draw was the only violator (now fixed).
- **`ActionCatalog` holds built-ins as `&'static`** for Phase A (owned plugin entries = Phase C) to
  avoid a premature `&'static str`→`String` ripple.

## 6. STANDING RULES
No `cargo fmt`. Fix all warnings. Library widgets + only library-provided values (widget owns
styling; caller picks variant). Always generic, never patch the narrow problem. Read AGENTS.md + docs
before writing code. When marking a task DONE, grep the whole backlog for cross-refs and flip the
actual checkboxes in one pass. No commit/push/PR/merge without explicit user OK.
