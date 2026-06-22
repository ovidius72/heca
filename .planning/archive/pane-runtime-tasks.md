# Pane Runtime State — Orchestration Board

> Coordination board for the **pane-runtime-state** initiative. **Full context lives in
> `pane-runtime-state-plan.md`** — read the referenced phase section before starting; this board is only
> assignment + status + responses. (Separate from `shared-tasks.md`, which tracks unrelated work — do not
> mix them.)

## Roles
- **Orchestrator** (me): assigns phases, reviews completions, accepts/rejects, advances the order.
- **Agent**: claims one *Open* phase, implements per the plan's task checklist, verifies, reports under
  *Agent Completion*. Updates **only** the phase it owns.

## Rules
1. Only active / not-yet-accepted phases stay here; accepted ones are struck or removed.
2. One agent per phase at a time. Set status to `In Progress` and put your id in *Assigned*.
3. Honor **Depends-on** — do not start a phase whose dependency isn't `Accepted`.
4. Obey the plan's **§2 cross-cutting rules** (grid-ui widgets; registry/keymap/RPC; event-on-mutation;
   `heca-core` UI-free; showcase+docs; tests; no `cargo fmt`; clippy clean).
5. Report with **code + verification + architecture notes** — completion notes alone are not acceptance.
6. Branch per phase off latest `origin/main`; open a PR; never merge without explicit OK.

## Status values
`Open` · `In Progress` · `Completed by Agent` · `Needs Edit` · `Accepted` · `Rejected`

---

> ## ✅ INITIATIVE COMPLETE — all phases 0–10 merged to `main` (verified 2026-06-22)
> Every phase below is implemented and on `origin/main`; the spun-out split-button chrome-flash
> known issue is also fixed + merged. Verified by presence of each phase's key artifact/symbols on
> `main` (git monitor, process catalog, host API, `mouse/resize.rs`, `ResizeColumnBy`/`ResizePaneHeightBy`,
> `PaneAction::Zoom`/`Float`, `Glyph::FrameCorners`/`Cards`, `action_allowed_when_floating`,
> `IconButton::active`). Nothing outstanding on this board. Per-phase detail kept below for the record.

---

## Phase 0 — Chrome event bus + finish SharedChromeState migration  ⟶ FOUNDATION
**Status:** Completed by Agent · **Assigned:** agent (dispatched 2026-06-18) · **Depends-on:** none · **Plan:** §4 Phase 0
**One-liner:** typed event bus + emit-on-mutation; consume the dead store fields; retire the `Rc<Cell>`
click mailbox; feed signal changes into the damage/repaint path.
**Agent Completion:** Completed on current branch.
Built:
- typed chrome event bus + event-emitting store setters
- expanded-sidebar interaction routing via `ChromeIntent` app events instead of the `Rc<Cell>` mailbox
- `chrome_state` mirroring for pick candidates + hovered pane, with a real retained-tree hover consumer
- retained chrome repaint invalidation narrowed from root-wide dirtying to per-widget repaint requests
- actual chrome damage forwarding into `GridRenderer`/`TextRenderer`
Verification:
- `cargo test -p heca --quiet`
- `cargo clippy -p heca --all-targets --quiet`
Notes:
- I intentionally extended Phase 0 slightly beyond the minimum wording to finish repaint granularity in the same slice; leaving damage collection wired but unused would have made the new invalidation path misleading and much less valuable.
**Reviewer Decision:** Accepted (merged to `main` via PR #129).

## Phase 1 — Pane runtime state model
**Status:** Completed by Agent · **Assigned:** agent · **Depends-on:** Phase 0 · **Plan:** §3 + §4 Phase 1
**One-liner:** `ProcessStatus`/`GitInfo`/`ContentKind`/`PaneRuntime` in core + reactive per-pane mirror in
the store with event-emitting setters.
**Agent Completion:** Completed on current branch.
Built:
- canonical pane runtime types in `heca-core` and attached `PaneRuntime` to core `Pane`
- per-pane reactive runtime mirror in `SharedChromeState.workspaces` with guarded setters for program/status/cwd/git/kind
- runtime event coverage for `pane.process.changed`, `pane.status.changed`, `pane.cwd.changed`, and `pane.git.changed`
- session→store runtime projection in `sync_chrome_state`, including stale-pane pruning when panes disappear
Verification:
- `cargo test -p heca-core --quiet`
- `cargo test -p heca --quiet`
- `cargo clippy -p heca --all-targets --quiet` (existing warnings only in `selection_model.rs` and `terminal_render.rs`)
Notes:
- I extracted the runtime projection into a dedicated helper so Phase 1 has a direct acceptance test for canonical session state mirroring into the reactive store, rather than only store-local setter tests.
**Reviewer Decision:** Accepted · **Reviewer Notes:** Rust-skill review on the merged diff (PR #130 / `1bc51ee`). All findings F1–F7 fixed in the review-fix PR:
- F1 `#[allow(dead_code)]` on `with_pane_runtime` now has an explanatory comment (AGENTS rule 7).
- F2 reactive hazard fixed: setters now decide inside the `panes` borrow and `.set()`+emit **outside** it (signal handles are `Copy`, captured into outer locals); `PaneRuntimeSignals` now `Copy`.
- F3 `sync_pane_runtime_state` per-frame push documented as tech debt (TODO) for the Phase 0 reactive damage-path reconciliation.
- F4 `set_pane_*`/`with_pane_runtime`/`retain_panes` tightened to `pub(crate)`.
- F5 `set_pane_runtime` collapsed to a single `panes.update` (was six per-field borrows).
- F6 `PaneRuntimeSignals` fields + struct documented; `Copy` rationale explained.
- F7 `HashSet` imported instead of fully-qualified.
Verification: `cargo test -p heca-core -p heca` 204+40 green; `cargo clippy -p heca-core -p heca --all-targets --all-features` — 0 new lints (same 8 pre-existing).
Note: the 6 per-field setters carry a documented `#[allow(dead_code)]` — they are the Phase 2 process-monitor write API, exercised by tests today; not removable dead code.

## Phase 2 — Process detection (OS-native foreground + exit; event-first, NO polling timer)
**Status:** Completed by Agent · **Accepted** (merged) · **Assigned:** agent · **Depends-on:** Phase 1 · **Plan:** §4 Phase 2 (see also §0.2, §0.6)
**One-liner:** exit+code captured from `try_wait` (event, reader EOF wake) + `pane.exited{code}`; foreground
program + running/idle via `tcgetpgrp` vs `process_group_leader()` (macOS libproc / Linux /proc, basename);
cwd OS-fallback (**Linux `/proc/<pid>/cwd` ✓; macOS OS-cwd deferred → Phase 3 OSC 7**, fragile FFI); **event-driven on output/EOF wakes + 250 ms debounce — NO periodic poll timer** (deferred);
auto-close stays (only fires on shell death — §0.6); **remove `ProcessStatus::Exit`** (dead pane closes);
`PaneBackend::runtime()` + per-wake monitor → `Pane.runtime` (Phase 1) → store. FakeBackend tests.
**Agent Completion:** PR #133 (commit `e8ac856`) — merged 2026-06-18. Code: `feat/phase2-process-detection`; design/deferral docs: PR #132.
**Reviewer Decision:** Accepted by merge · **Reviewer Notes:** FakeBackend + TerminalBackend unit tests (idle/running, exit-captures-code, debounce, monitor); clippy 0 new warnings. macOS cwd OS-fallback deferred → Phase 3 OSC 7 (tracked in plan §4 + one-liner).

## Phase 3 — Shell integration (OSC 133 / OSC 7)
**Status:** Accepted · **Assigned:** agent · **Depends-on:** Phase 2 · **Plan:** §4 Phase 3
**One-liner:** passive OSC snooper in heca-core before `advance_bytes` (§0.8) — parse OSC 133 (success/error+code) + OSC 7 (cwd, macOS preferred); re-sample foreground on markers; **hybrid shell wrap** (bash `--init-file` / zsh `ZDOTDIR` / fish `-C source`) auto-enabled by `settings.shell_integration` (bool, default true). **Implementation-shape decisions locked in §0.8** after an agent flagged them as pre-coding blockers.
**Agent Completion:** Completed on current branch.
Built:
- passive OSC snooper in `heca-core` before `engine.advance_bytes()` with fragmentation handling and `BEL`/`ST` terminators
- OSC 133 routing for `PromptStart` / `CommandStart` / `PreExec` / `CommandFinished(code)` into pane runtime status/exit-code updates
- OSC 7 cwd parsing with percent-decoding and runtime cwd updates
- semantic-status preservation so `Success`/`Error` is not immediately clobbered by a shell-foreground idle refresh
- shell-integration asset materialization under `~/.config/heca/runtime/shell-integration/` and shell-specific PTY wrapping for bash/zsh/fish
- new `[settings].shell_integration` bool plumbed through config reload/startup and centralized terminal-backend spawn helpers
- review cleanup: replaced the long shell-integration constructor with `TerminalBackendOptions`, preallocated the hot-path OSC event vec, added public API field docs, and broadened shell wrapper detection for suffixed shell names
Verification:
- `cargo test -p heca-config --quiet`
- `cargo test -p heca-core --quiet`
- `cargo test -p heca --quiet`
- `cargo clippy -p heca-core --all-targets --quiet`
- `cargo clippy -p heca --all-targets --quiet` (existing warnings only in `selection_model.rs` and `terminal_render.rs`)
Notes:
- Added a deterministic end-to-end bash PTY test that verifies `false` → `Error`, `true` → `Success`, and `cd /tmp` → OSC 7 cwd update without depending on a user shell rc file.
**Reviewer Decision:** Accepted by merge (PR #136, commit `0879b3c`) · **Reviewer Notes:** Review follow-ups addressed on branch: constructor naming/API cleanup, config-threading reduction at spawn call sites, OSC vec preallocation, public-field docs, and broader shell-kind matching. **Lead §0.8 compliance verified post-merge:** passive snooper forwards all bytes unchanged to wezterm-term (`observe` immutable → `advance_bytes` full); BEL + ST (ESC \\) + C1-ST (0x9c) terminators + cross-read fragmentation; routes 133 D→Success/Error+code, 7→cwd (percent-decoded, closes the Phase 2 macOS cwd deferral), A/B/C→force foreground re-sample; `semantic_status_active` correctly defers to OS detection between commands. Shell wrap = bash `--init-file` / zsh `ZDOTDIR`+`OLD_ZDOTDIR` / fish `-C source`, snippets source user RC first. `settings.shell_integration` (bool, default true) gates `Some`/`None` + refreshed on reload. Event-first + 250 ms debounce, no periodic timer. 51 heca-core tests, clippy clean (0 new). **Process nit:** PR #136 head branch was misnamed `feat/phase1-pane-runtime-state` (carried Phase 3 code) — harmless but rename future branches to match their phase.

## Phase 4 — Git integration (git2 behind a trait)
**Status:** Completed by Agent · **Assigned:** agent · **Depends-on:** Phase 3 (or Phase 2 cwd) · **Plan:** §4 Phase 4
**One-liner:** `GitProvider` trait + `git2` impl (branch, ahead/behind, +a/-d/Δ), debounced on cwd-change +
optional `.git` fs-watch; cache per repo.
**Agent Completion:** Completed on current branch.
Built:
- added `app/git_monitor.rs`: a provider-trait seam plus `git2` implementation that resolves repo root, branch, ahead/behind, and dirty counts (`added` / `modified` / `deleted` / `dirty`)
- added a host-owned `GitRuntimeCache` on `AppState`, keyed by pane cwd + shared repo root, so panes in the same repo reuse one cached snapshot
- wired a debounced git sync into `sync_chrome_state`: canonical `Pane.runtime.cwd` now drives canonical `Pane.runtime.git` before the chrome-store mirror runs, so existing `set_pane_runtime` change-guards emit `pane.git.changed` on real change only
- implemented non-repo behavior as `git = None`, with no per-frame churn for unchanged non-repo cwd values
- added tests for runtime projection, shared repo-root cache reuse across panes, timed refresh after the debounce window, and a temp-repo `git2` integration check for branch + dirty counts
**Reviewer Decision:** Accepted (merged to `main`; `git_monitor.rs` verified on `main` 2026-06-22).

## Phase 5 — Process catalog (`[program.<id>]` + `processes[]` aliases → {name, icon, description, color})
**Status:** Completed by Agent · **Assigned:** agent · **Depends-on:** Phase 1 · **Plan:** §0.7 + §4 Phase 5
**One-liner:** new `heca-config/src/programs.rs` (`ProgramMeta`/`ProgramsConfig`/`ProgramView`); seeded shell
defaults + user canonical app entries with raw-process aliases; semantic Phosphor icon names resolved through the existing `Icon` widget; optional
`color` pane tint; resolver with raw always available and terminal-icon fallback.
**Agent Completion:** Completed on current branch.
Built:
- added `heca-config/src/programs.rs` with documented `ProgramMeta`, `ProgramsConfig`, `ProgramView`, and a shared default terminal icon
- seeded built-in shell defaults (`sh`, `bash`, `zsh`, `fish`) and well-known app defaults, plus field-wise merge for partial same-key user overrides and `disabled = true`
- wired `programs` into top-level `Config` deserialization and added config/docs examples in `README.md`, `example.config.toml`, and `default-keybindings.toml`
- implemented `ProgramsConfig::resolve(raw)` with `raw` always preserved, explicit names/colors passed through, and terminal-icon fallback for both shells and unknown programs per the locked Phase 5 rule
- added tests for default shell resolution, partial override merging, unknown-program fallback, color parsing/pass-through, and top-level config parsing
Verification:
- `cargo test -p heca-config --quiet`
- `cargo clippy -p heca-config --all-targets --quiet`
**Reviewer Decision:** Accepted (merged to `main`; `heca-config/src/programs.rs` verified on `main` 2026-06-22).

## Phase 6 — Command spawn (run real programs + kind + float + close-policy)
**Status:** Completed by Agent · **Assigned:** agent · **Depends-on:** Phase 2 · **Plan:** §0.4 + §4 Phase 6
**One-liner:** real `CommandBuilder` spawn; extend `[[keys.command]]` (`kind`/`float`/`close_pane`/
`keep_on_error`/`keep_on_success`); close-policy on `pane.exited`; RPC parity; full action checklist.
**Agent Completion:** Completed on current branch.
Built:
- extended `[[keys.command]]` parsing with `kind`, `float`, `close_pane`, `keep_on_error`, and `keep_on_success` (default `kind = "terminal"`, plus `keys` alias support)
- extended `WmAction::SpawnCommand` payloads and config/RPC mapping to carry spawn kind, float mode, and pane close-policy
- added a real PTY command-spawn path in `TerminalBackend`/`PtyHandle`, using the user's shell as the command trampoline (`-ic` on Unix, `/C` on Windows) while keeping shell integration disabled for direct command spawns
- implemented tiled and floating command-pane creation in `handle_spawn_command`, including pane-owned close-policy storage
- applied close-policy exactly once from the drained exit-event path, while preserving shell-pane auto-close behavior
- documented the current `[[keys.command]]` contract in `README.md`, `example.config.toml`, and `default-keybindings.toml`
- review cleanup: documented `PaneClosePolicy` public fields, added config-load validation for invalid `[[keys.command]].kind`, clarified RPC `spawn-command` separator/empty-command errors, documented why shell integration stays off for direct command panes, and fixed the small process-monitor signature formatting artifact
Verification:
- `cargo test -p heca-core -p heca-config -p heca --quiet`
- `cargo clippy -p heca-core -p heca-config -p heca --all-targets --quiet` (existing warnings only in `selection_model.rs` and `terminal_render.rs`)
Notes:
- `kind = "terminal"` is implemented today; `app` and `plugin` currently report a clear "not yet implemented" message without forking a second spawn path.
**Reviewer Decision:** Accepted (merged to `main`). **Reviewer Notes:** Review follow-ups addressed on branch: interactive command shell mode (`-ic`), clearer RPC separator errors, documented public close-policy fields, config validation for invalid command kinds, explicit direct-command shell-integration rationale, and minor formatting cleanup.

## Phase 7 — Display: pane-info widgets (sidebar card + in-pane info bar)
**Status:** Completed by Lead (verified live) · **Assigned:** lead+user (interactive, branch `feature/phase-7`; Slice 1 merged via PR #147, rest in a fresh PR → main) · **Depends-on:** Phases 1–6 (degrades gracefully) · **Plan:** §0.3 + §4 Phase 7
**One-liner:** sidebar card (icon · name · status · flat git row) **+ an in-pane segmented info bar** — a `Tag` pill inside the pane top with **config-driven segments** (left) and **action buttons** (right), theme-driven grid-ui widgets, reactive; **configurable sidebar width**; showcase + docs.
**Agent Completion:** Partially implemented on `feature/phase-7` (uncommitted past the merge; not yet on PR).
Built (sidebar card):
- Row 1 resolves the catalog icon/name through `pane_info_view` → `Icon + Label + error-only indicator`; Row 2 a flat git row from `Pane.runtime.git`, hidden outside repos; pure projection tests; showcase + `docs/widgets.md`.
- git branch now **left-ellipsised** (keeps the meaningful tail, e.g. `…security-upgrade`); cap 22; full branch on hover.
Built (in-pane info bar — the chosen design):
- a self-contained **segmented `Tag`** rendered *inside* the pane top (`app::terminal_render` + `chrome::build_pane_info_bar`), reusing the sidebar's `pane_info_view` projection — no border-matching.
- **config (`[appearance]`):** `pane_title_segments = ["location","app_name","git_branch","git_status"]` (ordered; `[]` hides) + `pane_title_actions = ["split","move_left","move_right","close"]` (ordered; `[]` hides). Replaced the straddle config (`pane_title_style`/`color`/`background`, `pane_show_title` — all removed).
- distinguishable **header band** (theme `surface`) under the rounded frame; title **vertically centered**; width-aware **location truncation** (left-ellipsis) + **per-pane clip** so it never spills into a neighbor.
- **font:** UI = **Geist Mono** (configurable `[theme] font_family`; removed all `JetBrainsMono Nerd Font`); terminal stays **Maple Mono**; bar size = `theme.font_size` (matches the sidebar).
- **`[appearance] sidebar_width`** — configurable, clamped `160..=560` (default 300, wider); applied at startup + reload (left + right panels).
- empty segments+actions ⇒ **no bar, no reserved padding/margin** (gated on segments-only until buttons land).
Superseded (built then replaced): the `heca-grid-ui` `Pane.title` + `PaneTitleStyle` (`Cut`/`Filled`/`Boxed`) border-straddle widget — the in-pane `Tag` bar replaced it (border-matching fought the transparent pane over the terminal). Cleanup: remove the unused straddle widget + its showcase/docs in a later slice.
Done since the merge (committed on `feature/phase-7`, 2026-06-20):
- ✅ **Removed the superseded straddle title** (`6a5d3d6`): dropped `Pane.title`/`PaneTitleStyle`/`paint_title`/`paint_title_backing`/`title_reserved_height`/`truncate_to_width`(+tests)/`TITLE_*` consts + `Glyph::secondary_char`; replaced the showcase straddle demo with an in-pane-bar demo; updated `docs/widgets.md`. Dissolves review M1.
- ✅ **Decoupled the UI font from the color theme** (`344df73`): `[settings] font_family`/`font_size` (+ validator) → loader `apply_overrides` maps onto `Theme`; theme font fields made optional + stripped from color `.toml`s; `Theme.font_size` default normalized **32→15**; `chrome_gui_theme` now maps `state.theme.font_size` (landmine fixed). README + example.config.toml updated. (font_family reaches the renderer via `set_font_family`, so only size needed mapping.)
Done (Slice 2 — action buttons, committed `be12e46` on `feature/phase-7`, 2026-06-20):
- ✅ Interactive `IconButton`+`Tooltip` cluster (config `pane_title_actions`) via a **retained per-pane header + pointer dispatch**: `chrome::sync_pane_headers` builds-on-content-change (`pane_header_key`) + re-lays-out/positions every frame at the top of `render_frame` (before the `scene_view`/compositor borrow); `terminal_render` paints it read-only; `events.rs` intercepts a plain left-press on a button before the content/terminal paths and consumes it (empty band → focus pane, no selection); pointer-move feeds hover. Geometry via `terminal_host::pane_outer_frames` (tiled + floating).
- ✅ Icons `square-split-vertical`/`arrow-line-left`/`arrow-line-right`/`x-square` (Phosphor v2.1 codepoints). Actions: split→`AddPaneToColumn`, move_left/right→`MovePane*{pane_id: Option}`, close→`ClosePaneById` — all RPC-parameterized (+ `move-pane-left/right` RPC). Bar gate widened to segments **or** actions. Showcase full-header demo + `docs/widgets.md` + tests.
- ✅ **Verified live + polished** (feedback rounds): default bar = **split + close** (move dropped — mouse drag already moves panes; still config-available); tooltips show the **real configured keybind** with the prefix as a symbolized combo (`PaneActionHints::from_keys` + `format_binding`); retained headers are **ticked** each frame (press flash / hover / tooltip reveal no longer stick); **split re-bakes its column** (ws/col in the rebuild key — no more "adds to active column"); **softened-red close** (`danger.lerp(surface, 0.25)`); bigger buttons + tighter gap; **per-pane clip restored** so the bar can't spill into a neighbor on resize, while the Tooltip still escapes (it's on the overlay layer, rendered after the base PopClip). Keyboard resize gestures restored (only an actual button hit consumes the press).
- **Status: Phase 7 COMPLETE.** Slice 1 merged via PR #147; the remaining work (cleanup, font decouple, Slice 2 + polish) is a **fresh PR → main**. Bottom placement + full token/segment templates stay deferred; `NfIcon` → `PLAN.md` backlog.
**Reviewer Decision:** — · **Reviewer Notes:** review M1–M7 triaged in the RESUME doc (M7 fixed; M2/M3/M4 leave; M5/M6 minor; M1 moot after straddle removal).

## Phase 8 — Plugin event exposure + plan/docs updates
**Status:** Completed by Lead · **Assigned:** lead (`feature/phase-8`) · **Depends-on:** Phase 0 · **Plan:** §4 Phase 8
**One-liner:** first-party `app.on` + `app.state` selectors over the bus/store; document state access + event
model + the deferred token customization in `pluggable-chrome-plugin-plan.md`; point `PLAN.md` here.
**Completion:**
- `heca/src/host.rs`: `App` facade (cheap clone of `SharedChromeState`) — `App::on(event, handler)` over the event bus (RAII `ChromeSubscription`, `"*"` catch-all) + `App::state()` → `StateView` read selectors (`active_pane`, `pane_runtime`/`pane_status`, sidebar visibility, workspace collapsed). `AppState::host()` hands one out. New public `WorkspacesContainerState::pane_runtime` snapshot selector; `ChromeSubscription` exported. Methods/overlay/regions namespaces left to later phases (documented). Module-level `#![allow(dead_code)]` (seam, like the Phase 0 bus) until first-party providers consume it.
- Tests: typed + catch-all delivery, unsubscribe-on-drop, and an end-to-end "provider subscribes to `pane.status.changed` then reads `pane_status`" round-trip.
- Docs: `pluggable-chrome-plugin-plan.md` §3.3/§3.5/§5.4 marked foundation-landed; §8.1 records the deferred hybrid `${token}` customization shape (segment-list selection already shipped in Phase 7). `PLAN.md` refreshed.
- Verification: clippy clean; heca 226 / config 51 tests green.
**Reviewer Decision:** Accepted (merged to `main` via PR #149; `heca/src/host.rs` verified on `main` 2026-06-22).

## Phase 9 — Mouse pane/column resize (drag dividers)  ⟶ NEW (spun out of Phase 7 discussion)
**Status:** ✅ Accepted (MERGED to `main` via PR #157, 2026-06-21) · **Assigned:** lead (`feature/phase-9`) · **Depends-on:** niri-parity question #2 (RESOLVED) · **Plan:** §4 Phase 9
**One-liner:** drag the gap between **columns** (vertical divider) → resize that column; drag the gap between
**panes** in a column (horizontal divider) → resize pane height. **Fallback**: **hold right-button on a pane to
resize** along the nearer axis (for when the thin ~8px gap fights the terminal).
**Why standalone (not folded into Phase 7):** Phase 7 is the *display* slice; resize is a *layout/interaction*
feature overlapping the deferred DnD/cursor work and an open niri-parity question — bigger scope + different risk.
**Tasks (order matters):**
- [x] **Persistence landmine SETTLED** (foundation commit `e1f9e71`): `resize_*` mutate the canonical
      `ColumnWidth`/pane `preferred_height`; `update_all_column_widths` only recomputes the derived cache from it,
      so a manual resize persists. Test: `resize_column_persists_through_recompute_and_add`.
- [x] **Parameterized core resize** (foundation): `resize_column(col_idx, delta)` / `resize_pane_height(col_idx,
      pane_idx, delta)`; the active-only keyboard fns delegate. RPC parity = `WmAction::ResizeColumnBy` /
      `ResizePaneHeightBy` + RPC `resize-column` / `resize-pane-height`.
- [x] **Resize-drag gesture** — `heca/src/mouse/resize.rs`: left-press on a divider starts the drag (intercepted
      in `app/events.rs` after the header-button check, only an actual hit consumes), each cursor-move emits a
      parameterized resize action, release clears. Right-button fallback resizes the pane under the cursor.
- [x] **Divider hit-testing** — pure `divider_at()` over `pane_outer_frames` + `find_pane_location` (pane gaps
      first, then column gaps, ~6px grab; floats excluded).
- [x] **Resize cursor** — `ColResize`/`RowResize` via `mouse::update_cursor` (drag axis while resizing, divider
      axis on hover).
- [x] RPC parity + tests: `divider_at`/`fallback_divider` geometry tests, RPC parse tests, foundation resize-math tests.
**Agent Completion (Lead, `feature/phase-9`, uncommitted):**
Built:
- `app_state.rs`: `ResizeDivider{Column{col}|Pane{col,pane}}` + `ResizeDrag{divider,last_pos}`; `MouseState.resize`.
- `input.rs`: `WmAction::ResizeColumnBy{col_idx,delta}` + `ResizePaneHeightBy{col_idx,pane_idx,delta}` (priority + policy + exhaustiveness tests).
- `handlers.rs` + `registry.rs`: handlers calling the core resize methods, registered.
- `interaction.rs`: both actions `TiledOnly`.
- `mouse/resize.rs`: hit-test (`divider_at`), `fallback_divider`, `on_press`/`on_right_press`/`on_drag_move`/`on_release`/`cursor_for`; 6 unit tests.
- `mouse.rs`: wired into `on_cursor_moved` (resize takes priority), `on_mouse_input` (release + right-button), `update_cursor`; `is_resizing` helper.
- `app/events.rs`: left-press divider interception (after header-button block); guards chrome-hover + terminal move/button forwarding while resizing.
- `rpc.rs`: `resize-column <col> <delta>` / `resize-pane-height <col> <pane> <delta>` + parse tests.
- **Live-feedback fixes round 1 (2026-06-21):** (1) columns can now grow to the **full visible width** — `resize_column` cap raised `Proportion 0.95 → 1.0`; (2) **min sizes** so a pane can't become a thin line — `MIN_COLUMN_WIDTH = 150px` (scrolling.rs) + `MIN_PANE_HEIGHT = 100px` (column.rs), replacing the old 50px floors; (3) **vertical first-drag jump fixed** — `resize_pane_height` now bases the new height on the pane's actual current `pane_sizes[idx].h` instead of a hardcoded 200px, so a still-auto pane no longer snaps on the first delta. Keyboard resize benefits too (shared core).
- **Live-feedback fixes round 2 (2026-06-21):** (4) **single / rightmost column now resizable** — divider hit-test is edge-based: a column's RIGHT edge is its handle (between two columns it spans the gap; for the last/single column it's a band around the right edge); (5) **"resizes on the wrong side" fixed** — `resize_column` now anchors the view on the **resized** column's left edge (not the active column), so the dragged divider tracks the cursor; dropped the active-column recenter + per-move animation that fought a smooth drag.
Verification: `cargo check/test/clippy -p heca-core -p heca` — heca 233 + heca-core resize tests green, clippy 0 warnings (incl. changed files). (Pre-existing `terminal_backend_bash_integration_*` flake unrelated.)
**Live-verified by the user (2026-06-21):** divider drag, full-width growth, min sizes, no vertical jump, single/rightmost-column resize, and correct drag side all confirmed good.
**Reviewer Decision:** Accepted (merged via PR #157; symbols `ResizeColumnBy`/`ResizePaneHeightBy`/`MIN_COLUMN_WIDTH`/`MIN_PANE_HEIGHT`/`divider_at` + RPC `resize-column`/`resize-pane-height` verified on `main` 2026-06-22).

## Phase 10 — Pane action buttons: float + zoom  ⟶ NEW (requested 2026-06-21, after Phase 9)
**Status:** ✅ Accepted (MERGED to `main` via PR #157 + follow-up #158, 2026-06-21) · **Assigned:** lead (`feature/phase-9`) · **Depends-on:** Phase 7 (in-pane action bar; DONE) · **Plan:** §0.3 + this entry
**One-liner:** add **float** and **zoom** as pane info-bar action buttons, alongside the existing split/close,
each wired to its existing WM action + keybinding (`float` = `prefix+f`, `zoom_column` = `prefix+z`, both
already configured).
**Tasks:**
- [ ] Extend `heca-config` `PaneAction` enum (`heca-config/src/appearance.rs`) with `Zoom` + `Float` (snake_case
      `zoom`/`float`); they join the existing `split`/`move_left`/`move_right`/`close`.
- [ ] Map them in the pane-header builder (`heca/src/chrome/mod.rs` `build_pane_info_bar` / `sync_pane_headers`):
      `float → WmAction::Float`, `zoom → WmAction::ZoomColumn`; pick Phosphor icons; tooltips auto-show the real
      keybind via the existing `PaneActionHints`/`format_binding` path (bindings already exist — no new keymap).
- [ ] Per the action checklist: reachable from keyboard (already), mouse/UI (these buttons), and RPC (Float/
      ZoomColumn RPC parity — confirm/add).
- [ ] Update the showcase pane-header demo + `docs/widgets.md` (grid-ui rule), and `example.config.toml`/README
      docs for the new `pane_title_actions` values.
- [ ] Tests where pure (action-kind → WmAction mapping).
**Config note (confirmed 2026-06-21):** the pane action config IS implemented like segments, but the key is
**`pane_title_actions`** (NOT `pane_actions`) — parallel to `pane_title_segments`. Current valid values:
`split`/`move_left`/`move_right`/`close` (default `["split","close"]`). `zoom`/`float` become valid once this
phase lands. The user's recalled `pane_actions: ["close","zoom","float","split"]` should be written as
`pane_title_actions = ["close", "zoom", "float", "split"]`.
**Agent Completion (Lead, `feature/phase-9`):**
- `heca-config` `PaneAction` += `Zoom` + `Float` (snake_case `zoom`/`float`).
- `heca-grid-ui` `Glyph` += `FrameCorners` (Phosphor `frame-corners`, `\e626`) + `Cards` (`cards`, `\e0f8`) — codepoints cross-checked against the embedded Phosphor-Duotone v2.1 font (all 4 existing anchors matched the official CSS) and confirmed present in the font cmap.
- `chrome/mod.rs` `pane_action_spec`: `zoom → (FrameCorners, WmAction::ZoomColumn)`, `float → (Cards, WmAction::Float)`. These are **active-targeted** so the button does **focus-then-act** (sends `FocusPane{pane_id}` then `ActivateAction` — queued in order) so they land on the clicked pane, not whatever's active. Routed through the **ActionRegistry** like the others. Tooltips show the real keybind via `PaneActionHints` (`zoom_column`/`float` already bound to `prefix+z`/`prefix+f`). RPC parity already existed (`zoom-column`/`float`).
- Showcase icon strip + `docs/widgets.md` glyph list updated (grid-ui rule); README actions table + `example.config.toml` updated; mapping unit test added.
- Verification: heca 235 + grid-ui + showcase build green, clippy 0 warnings. (Pre-existing unrelated `heca-config loader::test_fallback_when_config_missing` theme default `latte`/`mocha` mismatch — resolved after syncing main / PR #156.)
**Toggled state + floating filter (2026-06-21, follow-up):**
- `IconButton::active(bool)` — persistent tone-tinted fill + firm border + held glow, mirroring the `Toggle` on-state (showcase strip + `docs/widgets.md` updated). The zoom button is active while its column is zoomed/full-width; float is active while the pane is floating.
- **Floating panes show only float + close**, driven by the **action policy** (not a hardcoded list): new `pub(crate) action_allowed_when_floating(&WmAction)` in `interaction.rs` (`FocusedPaneLocal | Global`); the header filters tiled-only buttons (split/zoom/move) when floating. Pane `zoomed`/`floating` state threaded into `PaneHeaderContent` + rebuild key + computed in `sync_pane_headers`.
- Tests: `pane_action_spec` mapping, `floating_pane_keeps_only_float_and_close`. heca 241 green, clippy 0, showcase builds.
- **Unfloat log fix:** float button no longer focus-firsts when the pane is floating (`needs_focus && !content.floating`) — a `FocusPane` from MouseContent is blocked in the floating domain and logged a spurious "blocked intent"; unfloat acts on the already-active floating pane directly.
- **Live-verified by the user (2026-06-21):** active toggled look + floating shows only float/close + clean unfloat (no log).
**Reviewer Decision:** Accepted (zoom/float buttons merged via #157; toggled-state + floating filter merged via #158; symbols `PaneAction::Zoom`/`Float`, `Glyph::FrameCorners`/`Cards`, `action_allowed_when_floating`, `IconButton::active` verified on `main` 2026-06-22).

---

## Suggested order
`0 → 1 → 2 → 3 → 6 → 4 → 5 → 7 → 8` (single-threaded). Parallel once Phase 1 is Accepted: **2 & 5** together,
then 3→4 and 6 alongside.

## Known issues
- ✅ **RESOLVED — Split-via-action-button flash** (reported 2026-06-21; fixed + merged 2026-06-22, `49a5cee`).
  **Correct root cause** (the first guess above was wrong): the compositor scene texture is **cleared every
  frame** while a `Tracked` chrome frame **scissored the chrome grid to the damage rect** — so chrome content
  *outside* it (sidebars + tab/status bars) wasn't repainted that frame, leaving bare background → the dark
  "re-render" flash. The split **button's press-flash animation** drove the `Tracked` (`RequestRedraw`) frames;
  keyboard split stayed `Full`, so it didn't flash (the button-only asymmetry). **Fix:** chrome now **always
  full-repaints** (`render.rs` `damage: None`) and the now-unsound `ChromeDamageMode` machinery was removed.
  The deferred damage-region/scene-preservation optimization that would make partial chrome sound is tracked
  in `PLAN.md` ("Foundation gaps").

## Activity log
- 2026-06-22 — **Initiative closed out.** Verified all phases 0–10 on `origin/main`; refreshed every phase status to Accepted/merged. **Phase 9 (mouse resize) merged via PR #157; Phase 10 (zoom/float buttons) via #157 + toggled-state/floating-filter follow-up #158.** **Split-button chrome flash RESOLVED + merged** (`49a5cee`): real cause = per-frame scene clear + `Tracked` partial chrome repaint (button press-flash drove the Tracked frames); fix = chrome always full-repaints + removed the unsound `ChromeDamageMode`; deferred scene-preservation optimization tracked in `PLAN.md`.
- 2026-06-21 — **Phase 10 added** (pane action buttons: float + zoom, `prefix+f`/`prefix+z`) per user request, after Phase 9. Confirmed the pane-action config key is `pane_title_actions` (like `pane_title_segments`), currently `split`/`move_left`/`move_right`/`close`; `zoom`/`float` land in Phase 10. Logged the split-via-button chrome flash as a known issue.
- 2026-06-18 — board created; plan locked (`pane-runtime-state-plan.md`); all phases `Open`.
- 2026-06-18 — docs landed (PR #127); **Phase 0 dispatched** to an agent (foundation).
- 2026-06-18 — **Phase 2 implemented + merged**: code PR #133 (`e8ac856`), design/deferral docs PR #132. Accepted by merge. macOS cwd OS-fallback deferred → Phase 3 OSC 7 (tracked).
- 2026-06-18 — **Phase 3 implementation-shape locked (§0.8)**: shell-hook mechanism (hybrid bash/zsh/fish wrap), config switch (`settings.shell_integration` bool), OSC parsing ownership (passive pre-parse snooper before `advance_bytes`). **Lesson:** an agent raised these as pre-coding blockers — they were implementation-shape decisions the lead left open after locking only the high-level Phase 3 design. The lead should lock implementation-shape up-front, not just design intent. Gaps now recorded so no agent has to guess.
- 2026-06-18 — **Phase 3 implemented + merged**: PR #136 (`0879b3c` — OSC snooper `osc.rs`, shell assets, `settings.shell_integration`, routing). Lead verified §0.8 compliance post-merge (no drift); Reviewer Decision = Accepted. Plan §4 Phase 3 tasks ticked.
- 2026-06-20 — **Phase 7 redesign (interactive, with the user) on `feature/phase-7`/PR #147**: started as a border-straddle `Pane.title` (Cut/Filled/Boxed), then **pivoted to an in-pane segmented info bar** (`Tag`) after the straddle's border-matching fought the transparent pane over the terminal. Landed: config segments/actions (replaced straddle config), distinguishable centered header band, width truncation + per-pane clip, UI font → **Geist Mono** (terminal stays Maple), configurable clamped `sidebar_width`, sidebar git branch left-ellipsis. **Remaining = Slice 2 action buttons** (interactive). Bar work uncommitted past the merge.
- 2026-06-20 — **Phase 7 cleanup + font decoupling landed** (`feature/phase-7`): removed the superseded straddle title widget (`6a5d3d6`, dissolves M1) and **decoupled the UI font from the color theme** into `[settings] font_family`/`font_size` (`344df73`, landmine fixed: default 32→15, mapped in `chrome_gui_theme`). **Slice 2 (action buttons) design locked** with the user: focus-then-act, `MouseContent` source, icons `square-split-vertical`/`arrow-line-left`/`arrow-line-right`/`x-square`, actions split→`AddPaneToColumn` + parameterized `MovePane*{pane_id: Option}` + `Close`, `IconButton`+`Tooltip`.
- 2026-06-20 — **Phase 9 created** (mouse pane/column resize) — spun out of the Phase 7 pane-action discussion; standalone because it's layout/interaction (not display) and gated on the niri column-width-persistence question. Recorded in `PLAN.md` + plan §4 Phase 9.
- 2026-06-21 — **Phase 7 Slice 2 (action buttons) landed + verified live**, then polished over two feedback rounds (default split+close, real keybind tooltips with symbolized prefix, per-frame tick fixing stuck press/tooltip, split column re-bake, softened-red close, bigger buttons, per-pane clip restored so the bar can't spill on resize while the Tooltip escapes via the overlay). **Phase 7 complete.** Slice 1 merged via #147; the rest opens as a **fresh PR → main** (#148).
- 2026-06-21 — **Phase 8 done** (`feature/phase-8`): first-party host API `app.on`/`app.state` (`heca/src/host.rs`) over the Phase 0 bus + store, end-to-end tests, plugin-plan docs (foundation-landed + deferred `${token}` shape). **The pane-runtime initiative (Phases 0–8) is complete.** Only the standalone Phase 9 (mouse resize) remains as a follow-up.
- 2026-06-21 — **Phase 9 gesture built** (`feature/phase-9`, lead, uncommitted): on top of the foundation (`e1f9e71` parameterized core resize + persistence landmine resolved), added the mouse divider-drag gesture — `mouse/resize.rs` hit-test (pane gaps first → RowResize, column gaps → ColResize, ~6px grab, floats excluded) + right-button fallback; left-press start intercepted in `app/events.rs` (after header-button, only a real hit consumes); each move emits parameterized `ResizeColumnBy`/`ResizePaneHeightBy` (registry + RPC `resize-column`/`resize-pane-height`); release clears; resize cursor on hover/drag; chrome-hover + terminal forwarding gated while resizing. 233 heca tests green, clippy clean. **Phase 9 = unit-complete; live-verify + commit/PR pending OK.**
