# Handoff — Sidebar follow-ups + confirm dialog + global KeyHint picker

## STATUS 2026-07-04 — committed + PR to main
GUI-verified by the user and **committed**; PR opened to `main`. This branch delivered:
confirm dialog keyboard + `ModalButton` (fu-13 S1–2), region show/hide actions (fu-6),
top-bar collapse toggles (fu-10→fu-14, headers removed, `WidgetSize::Small`), and grid-ui
foundation bits: theme `Spacing` token + `LayoutExt::pad_x/pad_y/pad_all` (containers take
padding from the theme), `Color::luminance()` + `Theme::on(fill)` (readable text on tonal
fills → danger button legible). All theme-driven, no hardcoded (AGENTS.md contract).
**Still open (dedicated branches, on proposal):** `gridui-styling-foundation` remaining —
interaction-alpha tokens + `WidgetSize` header-size coverage (migrate pane-header buttons off
`header_icon_size`/`.cell`); deferred sidebar items (fu-2, fu-9 design-first, fu-11 ViewNode,
fu-15 top-bar structure, app-task-21 collapsed-rail grid-ui, fu-13 Stage 3).

## ⚠️ RESUME — READ FIRST (orientation)
**This work lives in a WORKTREE, not the main checkout.** If you started in
`/Users/antonio/projects/myvim` you are in the WRONG place — nothing here is on
`docs/gridui-pruning`/`main`.

- **Worktree:** `/Users/antonio/projects/myvim-sidebar-fu`
- **Branch:** `feat/sidebar-followups`
- **Verify you're set up:** `git -C /Users/antonio/projects/myvim-sidebar-fu status`
  (should show branch `feat/sidebar-followups` + a dirty tree of uncommitted work).
- Operate via **absolute paths** / `git -C <worktree>` — the shell cwd resets to the
  main checkout between calls.

**ID legend (so "resume from fu-13 Stage 2" is unambiguous):**
- `fu-N` = "sidebar follow-up" task N. Full list + per-task `file:line` in this
  worktree's `BACKLOG.md` (search `fu-1`, `fu-13`, …). This is the granular source of truth.
- `fu-13` (confirm-dialog polish) was split into **Stage 1 / 2 / 3**. **Stages 1–2 DONE;
  Stage 3 DEFERRED** to `fu-11` (rich `Modal` `ViewNode` body). See BACKLOG `sidebar-fu-13`.

**Exact next action:** `fu-13` is COMPLETE for now — Stage 2 (host keyboard routing) landed,
Stage 3 (KeyHint over the dialog) is DEFERRED (it collides with Stage 2's "modal owns the
keyboard"; revive it when `fu-11` gives the modal a `ViewNode` body + widget vector — full
reasoning in BACKLOG `sidebar-fu-13` + `sidebar-fu-11`). `fu-6` (region show/hide actions) is
also DONE; `fu-10` (sidebar header toggle) DONE. **Next pending work** = `fu-9` (bottom-bar
restyle — **design to agree FIRST**, don't just build), `fu-2` (collapsed-rail KeyHint →
deferred to `app-task-21`). `fu-11` (rich `Modal` `ViewNode` body) is the registered
`plugin-task-ui-4`. All work UNCOMMITTED — the user reviews before any commit.

**Date:** 2026-07-03. **Synced with `origin/main`** (merge commit in history). **All work
is UNCOMMITTED** — the user reviews before any commit. Read this + `BACKLOG.md` before acting.

## Build / test state (all green)
- `cargo build -p heca`, `cargo clippy -p heca -p heca-grid-ui -p heca-config --all-targets`
  clean (only the pre-existing unrelated `block v0.1.6` future-incompat note).
- Tests: **heca 301**, **heca-grid-ui 52+127+1**, **heca-core 88**, **heca-config 77** — pass.
- Don't run `cargo fmt` (rustfmt churns unrelated files — repo rule).

## DONE this arc (detail + file:line in BACKLOG.md under each id)
- **fu-1** — collapsed rail initial prefers `custom_name` (`sidebar/render.rs`).
- **fu-3** — `[settings] show_left_sidebar/show_right_sidebar/show_top_bar/show_bottom_bar`
  (default true) fully hide a region. Hard "mounted" gate on `AppState` via
  `left_sidebar_width()`/`right_sidebar_width()`/`tab_bar_height()`/`status_bar_height()`
  (NOT `RegionMode`, which in this app renders Hidden as a 40px rail). + bottom-bar-hidden
  leftover-text bug fixed (`chrome_root` omits the bar when height 0).
- **fu-4** — sidebar right-click **context menu** (add/remove ws/col/pane), expanded
  sidebar only. Per user: **mouse = context menu only, NO inline "+" buttons**. Sidebar-mode
  keyboard actions (`w/c/v/d`) already existed.
- **fu-5** — **delete-column keyboard shortcut**: `WmAction::DeleteCurrentColumn` (mode-aware
  target) → `Shift+d` (sidebar) / `prefix+Shift+x` (normal). Bare `D`/`X` fold to `d`/`x` in
  the keymap, so `Shift+` is required.
- **fu-7 / fu-8** — **confirm dialog + centralized, config-gated destructive confirm**.
  `Modal` overlay (`state.confirm_dialog`); single chokepoint
  `handlers::request_destructive(raw_action, resume)` generates the message + reads
  `[settings] confirm_close_pane/confirm_delete_column/confirm_delete_workspace` (default
  true) → dialog or run raw. Wired at **every** entry incl. `prefix+x` + content-menu "Close
  pane" (`ClosePaneById{focused}`; removed dead `close_tiled_pane`/`close_floating_pane`),
  sidebar deletes, sidebar mouse-button delete, and the sidebar context-menu deletes
  (intercepted at the `settle_context_menu` drain). Both keyboard (y/Enter/n/Esc) + mouse
  resolve via `handlers::resolve_confirm_delete`. Bottom-bar "CONFIRM" prompt removed.
- **fu-12** — **global KeyHint picker** (`prefix+/`) = the documented "intent ⇒ hintable"
  host capability (`docs/plugin-authoring.md`, app-side first). grid-ui primitive
  (`HintTargetId`, `Base.hint_target`, `HintExt::hint_target`, `hint::collect_hint_targets`)
  + host `HintTargetRegistry` on `RetainedChrome` + `InputMode::HintPick` + `WmAction::HintPick`
  + `handle_hint_pick`/`handle_hint_pick_mode` + `paint_hint_targets`. Targets today: sidebar
  **pane cards → FocusPane**, **workspace docks → FocusWorkspace**. Keycap centered in a ~40px
  top band (fixed the "too high" alignment the user flagged).
- **last-pane/col/ws deletion unblocked** — pane/col already worked (leave empty ws). Removed
  the last-**workspace** guard in `handle_delete_workspace` + `Session::remove_workspace`, and
  **fixed the underflow panic** in `remove_workspace`'s clamp at len 0 (it was NOT harmless
  as-is). Deleting the last ws now leaves the session empty (blank, recoverable via
  create-workspace — `handle_create_workspace` handles `None`).
- **fu-13 Stage 1** — grid-ui `Modal` reworked to N `ModalButton`s (label + `shortcut(char)`
  + `danger` + `cancel` role) with a focused index + public `focus_next/focus_prev/
  activate_focused/activate_shortcut/request_cancel`, focus ring, `(x)` labels. `.confirm/
  .cancel/.danger/.open/.dismissible` kept as convenience (app + showcase still build). Initial
  focus = cancel. `ModalButton` exported.

## fu-13 — DONE (Stages 1–2) + Stage 3 DEFERRED
- **Stage 1 DONE** — grid-ui `Modal` reworked to N `ModalButton`s (label + `shortcut(char)` +
  `danger` + `cancel` role) with a focused index + `focus_next/prev`, `activate_focused/shortcut`,
  `request_cancel`, focus ring, `(x)` labels.
- **Stage 2 DONE** — host keyboard routing in `heca/src/app/events.rs`: when
  `confirm_dialog.is_some()`, Tab/Shift+Tab + ←/→ + Ctrl+h/l → `focus_prev/next`; Enter/Space →
  `activate_focused`; Esc → `request_cancel`; a bare letter → `activate_shortcut` (y/n); then
  drains `confirm_dialog_result` → `resolve_confirm_delete` (same as the pointer path).
  `handle_confirm_delete_input` retired (Modal owns the keyboard). `begin_confirm_delete` rebuilt
  as `[Cancel (n)] [Delete (y)]` `ModalButton`s (Cancel focused, `danger` on Delete).
- **Widget-change rule DONE** — `ModalButton` exported at crate + prelude; showcase demo
  (`heca-renderer/examples/showcase.rs`) switched to the new API; `docs/widgets.md` Modal section
  rewritten. heca 301 + grid-ui 52+127+1 tests green, clippy clean, showcase builds.
- **Stage 3 (KeyHint over the dialog) — DEFERRED (2026-07-03).** Collides with Stage 2's "modal
  owns the keyboard": (a) `prefix+/` can't reach the picker (the confirm block swallows keys
  before the prefix state machine runs); (b) `paint_hint_targets` runs BEFORE
  `paint_confirm_dialog` (`render.rs:1165` vs `1168`) so keycaps would paint under the dialog;
  (c) the pending action in `InputMode::ConfirmDelete { action }` would be overwritten by a
  hint-pick mode. The dialog is already fully keyboard-operable, so this waits. **Revive with
  `fu-11`** (rich `Modal` `ViewNode` body + widget vector → hinting arbitrary widgets becomes
  worthwhile). Full detail: BACKLOG `sidebar-fu-13` (Stage 3 note) + `sidebar-fu-11`.
- **Manual GUI check still owed** (can't automate keys here): `prefix+x` → dialog → Tab/←→ move
  the focus ring, `y`/`n` fire, Enter/Space activate focused, Esc cancels.

## Other pending (tracked in BACKLOG.md, not started)
- **fu-2** — collapsed-rail KeyHint → **deferred to `app-task-21`** (hand-drawn rail has no
  rounded-corner/glow primitive; do it during the rail→grid-ui `RailCell` migration).
- **fu-6 — DONE (2026-07-03).** 12 config-bindable unit actions (`show_`/`hide_`/`toggle_` ×
  4 regions) → `handlers::handle_set_chrome_region_shown` flips the `AppState.show_*` mounted-gate
  + reflows. Decisions: keep both visibility axes (expand/rail vs mounted), unbound by default,
  unit-variants (parameterized `WmAction`s aren't config-bindable). Full detail in BACKLOG
  `sidebar-fu-6`. GUI check owed (bind `toggle_bottom_bar`, press → mounts/unmounts).
- **fu-9** — bottom-bar restyle (badges + tabs), **design to agree**.
- **fu-10 + fu-14 — DONE (2026-07-03).** fu-10 fixed the right-sidebar click routing
  (`mouse::point_in_right_sidebar`) + slimmed the headers. **fu-14 then moved the collapse toggles
  to the TOP BAR** (`ArrowLineLeft`/`ArrowLineRight` at the top bar corners →
  `ActivateAction(SidebarLeft`/`SidebarRight)`), **removed both sidebar-header rows entirely**, and
  routed top-bar clicks via `mouse::point_in_top_bar`. Because the top bar is always visible, the
  toggle now works in **both** expanded + collapsed states — no rail migration needed for it.
  Detail: BACKLOG `sidebar-fu-14` / `sidebar-fu-10`. GUI check owed (arrows in the top-bar corners).
- **app-task-21 — still open, but NO LONGER owns the toggle.** Migrate the collapsed rail from
  legacy hand-drawn to grid-ui (`RailCell`/`ChromeRegion`) for the rail's cells/nav-cursor/KeyHint/
  pane-name/theming (a–d in the backlog). The collapse button is handled (fu-14). Large/atomic —
  dedicated effort. Full spec in BACKLOG `app-task-21`.
- **fu-11** — rich `Modal` `body: ViewNode` = the registered `plugin-task-ui-4` (coordinate,
  don't fork).
- **fu-12 polish** — more hint targets as buttons arrive; `docs/widgets.md` note for
  `HintExt`/`collect_hint_targets`; collapsed rail not covered by the picker (arrives with
  app-task-21).

## Manual GUI verification still pending (can't automate keyboard/mouse here)
`cd /Users/antonio/projects/myvim-sidebar-fu && cargo run -p heca`, then:
- `prefix+/` → keycaps on panes+workspaces → letter focuses.
- `prefix+x` / right-click "Close pane" → confirm dialog (per `confirm_close_pane`).
- Right-click a ws/col/pane in the expanded sidebar → add/remove menu.
- Toggle `[settings] show_*` + `confirm_*` in `config.toml`.
- Delete down to empty (last ws) → blank app → recreate a workspace.

## Rules for whoever resumes
- Do NOT `git commit`/push/PR without the user's explicit OK (work stays uncommitted).
- No `cargo fmt`. Fix warnings even if pre-existing. Keep `BACKLOG.md` updated as work lands.
- New UI = proper `heca-grid-ui` widget reading theme; update its showcase + `docs/widgets.md`
  in the same change.
