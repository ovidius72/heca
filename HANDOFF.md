# HANDOFF — Overlay pipeline + Dialog + surface-compositor overlays

> **How to resume:** a fresh session starts in the MAIN checkout `/Users/antonio/projects/myvim`.
> This work is in a **sibling worktree**. Say *"read HANDOFF.md in the worktree and resume."*
> Auto-loaded memory: `surface-compositor-viewnode-arc`, `every-setting-and-keybinding-in-default-files`.

## 0. Orientation (read FIRST)
- **Worktree:** `/Users/antonio/projects/myvim-gridui-styling` — operate here (`git -C … `, abs paths).
- **Branch:** `feat/gridui-styling-foundation`.
- **MANDATORY context to read before touching code** (the user enforces this — I got burned repeatedly this session by NOT reading them):
  - `AGENTS.md` — esp. the top **"⛔ STOP"** section: (1) use existing grid-ui widgets, (2) actions/keys through the registries + **EVERY setting → `config.default.toml`, EVERY keybound action → `keybindings.default.toml`**, (3) no "for now" fixes; **§ "Chrome buttons → action, tooltip, KeyHint (do NOT hand-roll)"** (line ~421); **§ "Creating new widgets"** (line ~704); **§ "Adding New Actions"** checklist.
  - `docs/surface-compositor.md` — the layered-surface model (bands, occlusion, **paint z-order = the "Later" phase**, overlays are a top band). Read before adding any layer/surface/overlay.
  - `docs/widgets.md` — every widget + the `Dialog` section (updated this session).
  - `pluggable-chrome-plugin-plan.md` — §2.6.2 (ViewNode), §2.7.1 (OverlayHost/ModalSpec/ModalResult), **§2.7.2** (intent/dispatch/overlay-control decisions).
  - `docs/overlay-design.md` — the older `with_overlay` infra proposal (context for the Scene overlay layer).
  - `BACKLOG.md` — the plugin arc: `plugin-ui` phase (`plugin-task-ui-*`), `sidebar-fu-13`, `sidebar-fu-11`, `plugin-task-15`. **This backlog is mine to keep consistent** (memory `plugin-gridui-backlog-is-mine-keep-it-consistent`). NEEDS UPDATING — see §6.
- **Never run `cargo fmt`** (rustfmt 1.9 churns unrelated files). Fix all warnings (even pre-existing). `cargo build -p heca` can exceed a 2-min tool timeout → use long timeouts.

## 1. ⏸ EXACT POINT WE ARE DISCUSSING NOW (resume here)
**DONE this turn (committed):**
- **Esc = default modal cancel** (`dialog.rs`). Esc **always** fires `on_dismiss` (universal cancel); `dismissible` gates ONLY the scrim/outside-click. A blocking modal simply sets no `on_dismiss` → Esc no-ops. USER RULE: **this is the default behaviour for all non-blocking modals.** The destructive confirm keeps `dismissible(false)` (scrim guarded) but Esc cancels it.
- **Focus-ring glow/size tuning** (`component.rs corner_brackets`): shorter arms `len 12→7` + subtler glow `radius 6→3`/`intensity 1.0→0.3`, still respects `glow=none`.
- Variant-coloured ring **restored** (`button.rs:378`, Destructive→`danger`, else `accent`).
- Keyboard-only focus was tried (`FocusManager::focus_first_quiet` + `Dialog::open` used it) — **the user then reversed it** (see below).

**⏸ NEXT / PENDING — FOCUS-STYLE REWORK (not started; the exact resume point):** the user wants the focus indicator to **show always** (not keyboard-only) and to look different:
1. `button.rs:378` — key the indicator off **`self.base.focused`** (not `focus_visible`) so it shows always; **replace `cx.corner_brackets(...)`** with a *"slightly different accent when focused"* — a subtle accent-tone treatment (thin full outline OR a faint tint), following the button tone (Destructive→`danger`, else `accent`). NO brackets.
2. **Revert the keyboard-only bits:** `Dialog::open()` back to `focus.advance(panel, true)`; **remove `FocusManager::focus_first_quiet`** (`focus.rs`) — becomes unused.
3. `corner_brackets` is the SHARED focus primitive (badge_button/tabs/icon_button/toggle/item/rail_cell/select/row/toast/input/checkbox all use it) — decide whether they adopt the new non-bracket style too, or only Button changes. (Ask the user.)
Then commit + visually verify in the showcase.

**I offered 3 options; the user has NOT chosen yet:**
1. **Full thin outline ring** *(I recommended)* — replace brackets with a full rounded-rect accent border + soft glow (no fill). Conventional focus ring; reads distinctly from heca's decorative bracket reticles (Pane/DockFrame/Dialog panel use brackets for *decoration*, so focus ≠ decoration is good).
2. **Soft glow only** — no brackets, just an accent halo.
3. **Toned-down brackets** — shorter + thinner + no glow (e.g. `len 12→6`, drop glow).

**NEXT ACTION:** get the user's choice, then make it a **theme-driven** change in that one `if` block (use theme tokens, no hardcoded sizes — the corner_brackets already reads `focus_border_width`). Update the showcase note + `docs/widgets.md` if the focus-ring description changes. This is a grid-ui-wide visual change → verify in the showcase (`cargo run -p heca-renderer --example showcase`).

## 2. WHAT SHIPPED THIS SESSION (all committed, on the branch, user-verified)
Commits (newest first): `e27ac58` (settings+rule docs) · `e9f174c` (overlay top band + layer tick + centralized modal buttons) · `fa6e3eb` (self-contained Dialog keyboard + re-entrant Scene overlay) · `5f80a10` (OverlayHost pipeline; confirm dialog = hintable overlay layer) · `47f1d4c` (Dialog on_dismiss + body_boxed) · `434c2b6` (docs §2.7.2) · `bebb141` (InteractionIntent::View + realize) · `fe18771` (Dialog widget).

**The full overlay pipeline (was step 3 of the arc) is DONE.** The destructive-confirm prompt is now a **`Dialog` layer** in the `LayerRegistry` (`Modal` band), so its buttons are **real components** → hint targets + focus-traversable. **This CLOSES `sidebar-fu-13` Stage 3** (KeyHint over a dialog). User verified live: both bugs fixed (modal no longer vanishes when a tooltip shows; pane-header tooltip floats above neighbours/sidebar).

Concretely:
- **grid-ui `Dialog`** (`heca-grid-ui/src/widgets/dialog.rs`) — centered panel over a scrim holding real children (`Modal`'s counterpart). **Self-contained keyboard**: Tab/Shift+Tab/←→/Ctrl+h·l/Enter/Space/Esc, tracking `Event::ModifiersChanged` itself (focus methods private). `on_dismiss` callback (Esc/scrim). `body_boxed()` for a `realize()`-produced body. Registered in `widgets/mod.rs` + `lib.rs`.
- **`realize()`** (`heca/src/chrome/realize.rs`, `plugin-task-ui-3`) — `&ViewNode → Box<dyn Component>`; actionable node → hint_target + `on_click→emit(View intent)`.
- **`InteractionIntent::View`** + `dispatch_view_intent` (`heca/src/app/interaction.rs`).
- **`OverlayHost`** (`heca/src/chrome/overlay.rs`) — `OverlayId`(=LayerId, pub), `ModalSpec`/`ModalAction`/`ModalResult`. `open_modal(state, spec, completion) -> OverlayId`: realizes body + builds one `Button` per action via the **centralized path** (`hints.register(SubmitOverlay)` + `action_tooltip(button, id, label, &state.action_shortcuts)`), wraps in a `Dialog`, inserts a Modal-band layer, stores the completion. `resolve()` pops the layer + runs the completion. `top_modal()`.
- **Actions** `WmAction::SubmitOverlay{overlay,action}` / `CloseOverlay{overlay}` (`input.rs`, parameterized) — intercepted in `dispatch_intent` (has the registry the completion needs), like `FocusPaneThenAction`. Classified in `action_policy` + `action_priority` for match completeness (never actually consulted).
- **Confirm dialog migration** (`heca/src/handlers.rs` `begin_confirm_delete`) — calls `open_modal` with a completion that dispatches the confirmed action; `dismissible(false)` (forced choice). Removed `AppState::confirm_dialog`/`confirm_dialog_result`, `resolve_confirm_delete`, the 4 bespoke paths; `InputMode::ConfirmDelete` is now a fieldless status marker.
- **Render** (`heca/src/app/render.rs`) — **overlay top band**: `render_chrome` collects each surface's overlay segments into an `overlay_sink` flushed once by `render_overlay_band` after ALL bases (panes→floats→chrome), so tooltips/popovers float above everything (surface-compositor paint z-order). Generic `layout_layers`/`paint_layers` (`chrome/mod.rs`) for dynamic layers.
- **`Scene::begin/end_overlay` re-entrant** (`heca-grid-ui/src/scene.rs`, depth counter) — nested `with_overlay` (Tooltip inside Dialog) no longer drops the parent's segment. +2 tests.
- **Layer tick** (`heca/src/app/lifecycle.rs`) — `state.layers.visible_roots_mut()` ticked each frame so widgets in a layer (tooltip reveal, hover flash) animate + request frames.
- **Config/docs** — 7 settings added to `config.default.toml` (`confirm_close_pane/column/workspace`, `show_left/right_sidebar`, `show_top/bottom_bar`). AGENTS.md ⛔STOP cardinal rule added. Memory `every-setting-and-keybinding-in-default-files`.

**Verification:** heca **311** tests, grid-ui **58+127+1**, clippy clean, showcase builds. User confirmed the two visual bugs are fixed.

## 3. KEY DECISIONS (with WHY — do not relitigate)
- **Modal buttons go through the ONE centralized path** (`action_tooltip` + `hints.register`), NEVER hand-rolled. WHY: AGENTS.md § "Chrome buttons → action, tooltip, KeyHint — do NOT hand-roll". The developer only declares `ModalAction{id,label,danger}`; tooltip + KeyHint + click intent come for free.
- **NO y/n (per-button) shortcuts on modals.** WHY: overlay-control actions are **parameterized** (carry the overlay id) → not config-bindable by design; `action_tooltip` resolves shortcuts from `config.keys.bindings`, so there's nothing to show. KeyHint (`prefix+/`) covers discoverability. The `id` doubles as the tooltip's action-name — if a modal button confirms a *config-bound* action, pass that action's name as the id and its keybinding shows for free.
- **Everything is an action (RPC works):** clicking a modal button emits `SubmitOverlay{overlay,action}`; Esc/scrim emits `CloseOverlay{overlay}`. RPC drives the SAME actions with the `OverlayId` returned from `open_modal` (or, normal flow, awaits `ModalResult`). The RPC *bridge* is Phase-8/9 (not built); the action shape that enables it is done.
- **Dialog is self-contained** (owns its keyboard, tracks `ModifiersChanged`); the host just forwards events. WHY: the widget should provide next/prev/keyhint/activation itself — the developer/host must not wire it. Rejected the host-side `handle_modal_key` + `Component::as_any_mut` downcast (removed).
- **Overlays are a top band** (`render_overlay_band`), not flushed per-surface. WHY: a pane-header tooltip was occluded by later-flushing surfaces (adjacent panes, sidebar); the surface-compositor's paint z-order says overlays paint above all bases.
- **`Scene` overlay layer must nest** (depth counter). WHY: a `Dialog` (overlay) containing a `Tooltip` (overlay) nests `with_overlay`; the non-re-entrant version dropped the Dialog's segment → modal vanished when the tooltip showed.
- **Destructive confirm = `dismissible(false)`** (forced choice) — user's call.
- **EVERY setting → `config.default.toml`, EVERY keybound action → `keybindings.default.toml`** (cardinal, now in AGENTS.md ⛔STOP + memory). Defaults living only in code are invisible/undiscoverable.
- **I did NOT create sidebar-toggle actions.** `Show/Hide/Toggle{Left,Right}Sidebar`, `*TopBar`, `*BottomBar` are pre-existing (`833c214`, sidebar-fu-6). The mounted-gate `show/hide/toggle_*` are **intentionally unbound + documented** in `keybindings.default.toml` (lines 97-107); `rename_column` intentionally unbound (line 121-122, `prefix+Shift+c` reused); `rename_pane` = `prefix+$`. My earlier "5 missing keybindings" was a bad read of the file — there are NONE missing.

## 4. WHAT TO DO NEXT (in order, after the focus ring)
1. **Focus ring** — §1 above (get choice → theme-driven change at `button.rs:378`).
2. **Migrate the context menu onto `OverlayHost`** — the LAST bespoke overlay. Today `AppState.context_menu` + `context_menu_action` sink + `layout_context_menu`/`paint_context_menu` (`chrome/mod.rs`) + dedicated input blocks in `events.rs`. Convert it to an `OverlayHost` overlay (dropdown-style) the same way the confirm dialog was migrated → removes the last parallel overlay path. (BACKLOG: relates to `plugin-task-ui-4` / dropdown.)
3. **Hint overflow > 52 labels** — a–z then A–Z is 52 max; overflow is NOT handled. Planned: reserved prefix `=` then `=a`,`=b`… Still queued.
4. **plugin-ui continuation** (`BACKLOG.md` `plugin-ui`): `plugin-task-ui-2` (SwiftUI-style ViewNode builder SDK), `plugin-task-ui-4` (rich `Modal`/overlay `body: ViewNode` with N arbitrary widgets — `open_modal` already realizes a `ViewNode` body, so this is mostly done for the modal case), `plugin-task-15` (`app.overlay.open_modal/open_dropdown` async plugin-facing API, Phase 8), OverlayHost dropdown + `OverlayFuture`.

## 5. GOTCHAS / hard-won facts (don't rediscover)
- Fresh session starts in MAIN checkout, not the worktree (§0). See main-checkout `WORKTREES.md` router.
- The confirm dialog is a **Modal-band `Dialog` layer** now — NOT `AppState.confirm_dialog` (removed). Its buttons emit `SubmitOverlay`; Esc/scrim emit `CloseOverlay`; both resolve in `dispatch_intent` → `overlay::resolve`.
- Overlay content (tooltips/popovers) is flushed in the **top band** (`render_overlay_band` at end of `render_frame`), NOT with its surface. Any new per-surface overlay just works via `with_overlay`.
- Dynamic layers are ticked in `lifecycle.rs` (add nothing per-layer).
- `Box<dyn Component>` is NOT itself `Component` → can't `.child(box)`; push onto `base_mut().children` or use `Dialog::body_boxed`.
- Overlay-control actions are intercepted BEFORE the registry (they need the registry to run their completion). Don't register them as `ActionHandler`s.

## 6. BACKLOG UPDATES STILL OWED (do these — memory `update-backlog-before-pr` / `plugin-gridui-backlog-is-mine`)
Mark in `BACKLOG.md`: `plugin-task-ui-3` (realize) **DONE**; `OverlayHost::open_modal` slice **DONE**; **`sidebar-fu-13` Stage 3 DONE** (confirm dialog = hintable Dialog layer); `Dialog` widget **DONE** (grid-ui catalog + `docs/widgets.md`); note `plugin-task-ui-4` largely delivered for the modal case (`open_modal` realizes a `ViewNode` body). Flip the actual checkboxes, not just prose.

## 7. STANDING RULES
No commit/push/PR/merge without explicit user OK (the user OK'd the commits in §2). Never `cargo fmt`. Use library widgets + only library-provided values (widget owns styling; caller picks variant). **Always generic, never patch the narrow problem.** Read AGENTS.md + docs BEFORE writing code. When marking a task DONE, grep the WHOLE backlog+plans for cross-references and flip the actual checkboxes in ONE pass.
