# heca — Development Backlog

> **How to use this file:**
> Follow the phase/tasks you have been assigned. Read the status. Take the next open task.
>
> **Before starting any task:**
> 1. Pull/sync `main`: `git fetch origin && git merge origin/main`
> 2. Create a new feature branch off `main` and start working using the Rust skill (`~/.agents/skills/rust/SKILL.md`)
> 3. When done, wait for a review
> 4. The user will give you the result of the review
> 5. If it's OK: commit and push, open PR → main
> 6. If it's not OK: fix the issues and wait for user feedback
> 7. Follow all rules in `AGENTS.md`
> 8. Act as a Rust expert developer — no shortcuts, no partial implementations, no silent deferrals
>
> **Status key:** `[ ]` = not started · `[~]` = in progress · `[x]` = done · `[⏸]` = deferred/parked

---

## Architecture principles (apply to every task)

- **All state that fires an action or updates content MUST flow through `AppState` / the
  reactive chrome store — never be local-only.** Any behavior that changes app state or
  triggers an action has to be **dispatched through `AppState`** (mirrored into the reactive
  store + emitted on the `ChromeEvent` bus + exposed via the host API) so that **plugins and
  other components can listen and react** (e.g. render their own custom widget). Make it
  **signal-aware**: store the value in a `Signal`, guard the setter (emit only on real change),
  and add a `ChromeEvent` + a `StateView` selector.
  - Reference implementation: the **KeyHint pending-pick description** — `InputMode::pending_pick()`
    → `WorkspacesContainerState.pending_pick` signal → `ChromeEvent::PendingPickChanged` →
    `host.pending_pick()`. Also the **pane custom name** (`PaneCustomNameChanged` / `host.pane_custom_name`).
  - Anti-pattern: computing a value only for an internal render path (the way the pick prompt
    used to live only in `status_mode_parts`). If a plugin can't observe it, it's not done.

---

## Terminal

> Source: `terminal-implementation.md`
> Real PTY terminal using `portable-pty` + `wezterm-term` + `cosmic-text` is live. Core (Phases 0–5) is shipped.
> Remaining: advanced fidelity, test coverage, selection/clipboard, pane-shell integration.

### [x] Phase: Terminal damage-preservation foundation · `terminal-00`
Dirty-row rendering depends on retained terminal content. The app currently clears the frame each redraw and the terminal host currently drains damage before render uses it, so skipping unchanged rows today would erase them instead of optimizing redraw cost.

- [x] **terminal-task-00** — Preserve terminal damage through the app path.
  Stop draining-and-dropping terminal damage in the mount path. Carry `TerminalDamage`
  alongside `TerminalSnapshot` through the terminal host/render boundary so the
  renderer can consume real pending damage for the pane it is about to draw.
  Files: `heca-core/src/backend/mod.rs`, `heca-core/src/backend/snapshot.rs`,
  `heca/src/app/terminal_host.rs`, `heca/src/app/terminal_render.rs`
  Status: done on `feature/terminal-followups`; terminal mount/render prep now carries damage through to the pane render path.

- [x] **terminal-task-00a** — Produce visible row-range damage from the terminal backend.
  Replace the current `dirty: bool -> Full|None` behavior with `Rows(...)` where
  possible, using `wezterm-term` / `termwiz` viewport line invalidation
  information. Coalesce adjacent rows. Fall back to `Full` on resize,
  alternate-screen transitions, viewport-shape changes, or any uncertain state.
  Files: `heca-core/src/backend/terminal.rs`,
  `heca-core/src/backend/terminal/engine.rs`,
  `heca-core/src/backend/snapshot.rs`
  Status: done on `feature/terminal-followups`; backend now derives/coalesces changed visible rows and falls back conservatively to `Full` for uncertain structural transitions.

- [x] **terminal-task-00b** — Add retained terminal-content foundation.
  Introduce the minimum retained-content mechanism required so unchanged rows stay
  visible while only dirty rows are redrawn. Keep this scoped to terminal panes;
  do not silently broaden it into general compositor optimization in the same
  task.
  Files: `heca/src/app/render.rs`, `heca/src/app/terminal_render.rs`,
  `heca-renderer/*` only if a terminal-specific retained surface is needed
  Status: done. Runtime validation (2026-06-24) confirmed all five scenarios pass —
  resize keeps glyphs crisp, typed text appears immediately, idle scrollback stays
  correct, panes don't bleed, and style reload repaints fully. See
  `runtime-validation-terminal-00b.md` for the scenario list that was run.

- [x] **terminal-task-00c** — Verify the prerequisite itself.
  Add focused tests for backend row-range damage production, app-path damage
  propagation, and retained-content correctness when only dirty rows are
  redrawn.
  Files: `heca-core/src/backend/terminal.rs`, `heca-renderer/src/terminal.rs`,
  `heca/src/app/terminal_render.rs`
  Status: done. Backend row-range damage (`terminal_backend_output_produces_row_damage`) is green. App-path retained-presentation coverage is now solid: the damage policy was extracted into the pure `retained_damage_to_apply(...)` seam (skip on `None`, upgrade to `Full` on resize/style change, passthrough `Rows`/`Full` otherwise) with 6 policy tests, plus `retained_terminal_texture_size` (scale/ceil/min-1) and `terminal_layer_render_key` (stability + font-size/alpha/family change detection) tests, plus the existing `terminal_damage_copy_bands` band-conversion/clamp tests. `cargo test -p heca app::terminal_render` = 23/23, `-p heca-core` 67/67, clippy 0 warnings. Runtime validation of the retained presentation itself is the only `00b` sign-off left.

### [ ] Phase: Dirty-region terminal rendering · `terminal-01`
Render only changed terminal rows instead of the full pane every frame. This phase assumes `terminal-00` has already made row damage visible and safe by preserving unchanged terminal content across frames.

> **Deliberately deferred — SEPARATE from the scrollback feature.** This is a pure
> performance optimization, not a prerequisite for scrollback. The scrollback work
> (`terminal-01a`/`01b`) intentionally forces `TerminalDamage::Full` on every
> viewport movement (a full pane repaint while scrolling) and is fully usable that
> way; `terminal-01` only makes that movement cheaper. Do it AFTER the scrollback
> feature lands and the app is otherwise stable. Sibling of the frame-wide
> compositor damage optimization `app-task-22` (phase `app-07`) — same "repaint
> only what changed" spirit, different layer (terminal rows vs the whole scene).

- [ ] **terminal-task-01** — Implement dirty-row rendering in `heca-renderer/src/terminal.rs`.
  Consume the already-plumbed `TerminalDamage` and redraw only dirty visible rows
  into the retained terminal content path. Fall back to full redraw when damage
  says `Full`.
  Files: `heca-renderer/src/terminal.rs`, `heca-core/src/backend/snapshot.rs`
  Related: compositor damage-region optimization (`app-task-22`)

### [x] Phase: Host terminal scrollback viewport · `terminal-01a`
Host-managed terminal scrollback viewport is now shipped: backend viewport state,
stable-row selection model, scrollback actions/keybindings/RPC, chrome mirror,
animated viewport jumps, and GUI widgets all landed. Final runtime review found
one residual alt-screen wheel-routing issue, tracked separately in
`terminal-task-01h`.

- [x] **terminal-task-01a** — Add host-managed terminal viewport state and snapshot projection. ✅ DONE (2026-06-27)
  Introduce terminal viewport/scrollback state so the host can render historical
  rows instead of always projecting the live bottom viewport. Damage semantics:
  viewport motion produces `TerminalDamage::Full` for now (incremental viewport
  damage is a SEPARATE phase, `terminal-task-01` — do NOT fold it in).
  Files: `heca-core/src/backend/terminal.rs`,
  `heca-core/src/backend/terminal/engine.rs`,
  `heca-core/src/backend/snapshot.rs`,
  `heca/src/app/terminal_host.rs`,
  `heca/src/app/render.rs`
  Status: design LOCKED via `/grill-me` on 2026-06-24 (see
  `handoff-terminal-01a-scrollback.md` for the full decision contract). Hybrid
  ownership: `TerminalEngine` owns `viewport_offset` + projects via `TerminalSnapshot`
  (adds `viewport_offset`/`at_bottom`/`scrollback_rows`); AppState mirrors into the
  chrome store + `ChromeEvent::TerminalViewportChanged` + `host.terminal_viewport(pane_id)`.
  Selection model moves to stable-row coords (refactor of `SelectionRegion::HostGrid`).
  Implementation not yet started — slice 1 (backend viewport model) is the next step.
  **Slice 1 DONE (2026-06-24):** backend viewport model landed (no UI/actions yet).
  - `TerminalEngine` owns `viewport_offset` (0 = live bottom) + `viewport_changed` flag;
    `scroll_viewport(delta)`/`scroll_to_top()`/`scroll_to_bottom()`/`take_viewport_changed()`;
    `visible_lines()` projects bottom-minus-offset clamped to
    `[0, scrollback_rows - visible_rows]`; `resize()` re-clamps after wezterm reflow.
  - `TerminalSnapshot` gained `viewport_offset`/`at_bottom`/`scrollback_rows` (+ `debug_assert_valid` checks).
  - `PaneBackend` trait gained `scroll_viewport`/`scroll_to_top`/`scroll_to_bottom` (default no-op);
    `TerminalBackend` delegates + `take_terminal_damage` forces `Full` on viewport motion (Q6).
  - `HecaTerminalConfig::scrollback_size()` overridden from `terminal_scrollback_lines`; wired
    `SettingsConfig` field + `config.default.toml` + `AppState.terminal_scrollback_lines` →
    `backend_factory` → `TerminalBackendOptions.scrollback_size` → engine.
  - Tests: viewport clamping, at_bottom, snap-to-top/bottom, history projection, resize re-clamp,
    scrollback_size override, plus a `reconcile_viewport_offset` unit test for the alt-screen shrink case.
    Review fixes (round 1): stored-offset drift fixed via `reconcile_viewport_offset()` called at the
    end of `TerminalBackend::update()` (write-back clamp + arms `viewport_changed` so the correction
    surfaces as `Full` damage); the read-clamp in `visible_lines()` stays as defense. Test helper
    `max_offset()` now calls the engine's own `max_viewport_offset()` (no snapshot round-trip / magic
    cell size). `scrollback_size_override` assertion tightened to the formula `scrollback_size + rows`.
    Gates: `cargo test -p heca-core` 67/67 (single-threaded; the parallel `terminal_backend_process_input_reaches_shell`
    failure is the known pre-existing flaky PTY-timing test, passes 3/3 in isolation), `-p heca-config` 70/70,
    `-p heca` 247/247, `cargo clippy --workspace --all-targets --all-features` 0 warnings.
  **Slice 2 DONE (2026-06-24):** selection model stable-row refactor (Q4) landed.
  - Renamed `SelectionRegion::HostGrid anchor_row/focus_row` → `anchor_stable_row/focus_stable_row`
    (`usize` → `isize`) so selections survive viewport scroll in scrollback history.
  - Renamed `Caret::row` → `Caret::stable_row` (`isize`).
  - Added `visible_row_to_stable_row()` helper in `terminal_host.rs`.
  - `build_selection_overlay` takes `&TerminalSnapshot` instead of `cols`, uses stable→visible
    row conversion via `viewport_top_stable_row`.
  - `enter_selection_mode_for_focused_terminal` and `move_focused_terminal_selection` convert
    cursor row to stable row.
  - `forward_mouse_move` converts mouse coords to stable rows.
  - Added `lines_in_stable_range` to `PaneBackend` trait + `TerminalBackend` for fetching
    arbitrary scrollback rows by stable range.
  - `handle_copy_selection` fetches lines via `lines_in_stable_range` and passes `base_stable`
    to `extract_selection_text`.
  - Caret rendering: both caret-only and selection-endpoint draw at the LEFT edge of the cell,
    eliminating the visual bar-position jump when pressing `v`/Space.
  - Block cursor reverted to thin 2px bar (user preference).
  - `render.rs` call sites updated to pass snapshot instead of `cols`.
  - Gates: `cargo test -p heca` 259/259, `-p heca-core` 67/67, `cargo clippy` 0 warnings.
    Commit `bc66d31`, pushed to `feature/terminal-followups`, rebased onto `origin/main`.
  **Slice 3 DONE (2026-06-25):** actions, wheel routing, keybindings, interaction policy, and RPC.
  - 7 `WmAction` variants added (`PageUp`/`PageDown`/`LineUp`/`LineDown`/`ToTop`/`ToBottom`/`ExitScrollback`).
  - Config: `terminal_mouse` (bool, default true), `terminal_wheel_scroll_lines` (usize, default 3).
  - Wheel routing: `is_mouse_grabbed()` on `PaneBackend` trait; gating: `shift_held || (terminal_mouse && !grab)`.
  - Q3: wheel-up at live bottom enters `InputMode::Selection`.
  - Q5 snap-to-bottom on key input (always) + new output (only if already at bottom).
  - RPC: 13 new scrollback commands with tests.
  - Default bindings: `prefix+s` enters Selection mode; direct scroll bindings use `prefix+Shift+Up/Down`, `prefix+Shift+g/End`; selection-mode keys include `u`/`d`/`g`/`G`/`Esc`.
  - Interaction policy: all scrollback actions → `FocusedPaneLocal`.
  - Review fixes: `is_mouse_grabbed()` API (🔴), RPC (🟠), amount=notches consistency, dedup, flaky toast test.
  - Gates: `heca` 262/262, `heca-core` 70/70, `heca-config` 73/73, `heca-grid-ui` 125/125, clippy 0.

- [x] **terminal-task-01b** — Route wheel, PageUp/PageDown, and selection-mode edge movement through the host scrollback policy.
  **DONE + reviewed 2026-06-25** (slice 3). All locked decisions met after review fixes:
  `is_mouse_grabbed()` added through `PaneBackend`/engine so the wheel gate is
  `shift_held || (terminal_mouse_enabled && !is_mouse_grabbed)` (TUIs keep their wheel);
  wheel-up enters `InputMode::Selection` (Q3); RPC commands added; snap-to-bottom on key
  input (Q5). Gate: `heca` 262, `heca-core` 70, `heca-config` 73, clippy 0. Residual nits
  (non-blocking): RPC line-amount default (1) vs flat-binding default (3); snap-on-output
  branch is a no-op when already at bottom.
  **Before starting: read `handoff-terminal-scrollback.md` — a DETAILED WORKING NOTE
  (file inventory, slice-by-slice guide) kept only until scrollback lands, then deleted.
  It is NOT authoritative: the durable plan + decisions + deferrals live in THIS backlog
  (the policy below + the `terminal-01` deferral note above). If the two ever disagree,
  the backlog wins.**
  Define the policy boundary between host scrollback navigation and backend/TUI
  mouse forwarding. Normal shell/history use must scroll the host viewport;
  mouse-enabled TUIs must still receive raw wheel input when appropriate;
  keyboard selection must be able to move beyond the currently visible rows by
  scrolling the viewport.
  Files: `heca/src/app/events.rs`,
  `heca/src/app/terminal_host.rs`,
  `heca/src/handlers.rs`,
  `heca/src/app/interaction.rs`
  Status: design LOCKED via `/grill-me` on 2026-06-24. Policy:
  • Wheel → host scrollback unless `is_mouse_grabbed()`; Shift+wheel → always scrollback; default 3 rows/notch (`terminal_wheel_scroll_lines`).
  • Plain PageUp/PageDown → forward to PTY (tmux pass-through; do NOT intercept).
  • `prefix+s` → enter `InputMode::Selection` (caret-only). Wheel-up at a non-grabbed prompt also auto-enters Selection mode.
  • Wheel-up at a non-grabbed prompt also enters `InputMode::Selection` + scrolls (tmux `mouse on`).
  • New terminal-only `terminal_mouse` setting (default true) gates the wheel-enters-scrollback behavior (NOT the global `mouse`, which chrome depends on).
  • Selection-mode edge movement auto-scrolls the viewport (stable-row coords).
  • Five new `WmAction` variants: `ScrollbackPage{direction}`, `ScrollbackLine{direction,amount}`, `ScrollbackToTop`, `ScrollbackToBottom`, `ExitScrollback` — all `FocusedPaneLocal`, full 11-step treatment + RPC + default bindings in `keybindings.default.toml` (NOT `keys.rs` — defaults moved to TOML in PR #185).
  • Three GUI affordances (animated viewport offset + scrollbar widget + scrolled-up indicator) as generic `heca-grid-ui` widgets.
  See `handoff-terminal-scrollback.md` §6 for the detailed slice 3 implementation guide.

- [x] **terminal-task-01c** — Mirror terminal viewport state into the reactive chrome store (slice 4).
  **Slice 4 DONE (2026-06-25):** terminal viewport state mirrored into the chrome store.
  - Added `viewport_offset`/`at_bottom`/`scrollback_rows` signals to `PaneRuntimeSignals`.
  - Added `WorkspacesContainerState::set_pane_viewport()` — idempotent, emits
    `TerminalViewportChanged` only on real change.
  - Added `ChromeEvent::TerminalViewportChanged { pane, viewport_offset, at_bottom,
    scrollback_rows }`.
  - Viewport sync in `render.rs` from each `prepare_terminal_mount` result (tiled +
    floating).
  - Host API: `TerminalViewport` struct + `StateView::terminal_viewport(pane_id)` read
    selector.
  - 3 new tests: defaults, emit-on-change (4 events not 8), preinit-without-runtime.
  - Gates: `heca` 265/265, `heca-core` 73/73, `heca-config` 73/73, `heca-grid-ui` 125/125,
    clippy 0.
  Next: slice 6 (GUI widgets — scrollbar + scrolled-up indicator).
  **Plumbing only — no user-visible behavior.** Prepares slices 5 (animated offset) and 6
  (scrollbar + "N lines above" badge), which read this state. Per-pane, mirror the
  `TerminalSnapshot` viewport fields (`viewport_offset` / `at_bottom` / `scrollback_rows`)
  into `SharedChromeState` so chrome/GUI can react.
  Deliverables:
  • Add the per-pane viewport fields to `SharedChromeState` — INTEGRATE into the existing
    per-pane `PaneRuntime` mirror, do NOT add a parallel structure.
  • Sync them from where the snapshot is read each frame (`terminal_host.rs` mount) via the
    store's `set_*` chokepoint, **only on real change** (guard the setter).
  • Emit `ChromeEvent::TerminalViewportChanged { pane_id, … }` (`chrome/events.rs`) from the
    mutation chokepoint.
  • Add host read accessor `host.terminal_viewport(pane_id)` (`host.rs`), consistent with the
    `app.state.*` / `app.on(...)` pattern (plugins read via selector/event, never the signals).
  • Tests (update + event fire ONLY on change; no-op emits nothing) + clippy 0. No GUI check.
  Constraints: follow the `SharedChromeState` contract (signal-backed, write via `set_*`
  chokepoint that emits events, read via selector); container/pane-namespaced. Decision Q1 is
  locked — engine owns `viewport_offset`, snapshot projects, THIS task only mirrors into the
  store (do not move ownership).
  Files: `heca/src/chrome/state.rs`, `heca/src/chrome/events.rs`, `heca/src/host.rs`,
  `heca/src/app/terminal_host.rs`
  Ref: `handoff-terminal-scrollback.md` §6b + §9 (working note only — this backlog is authoritative).

- [x] **terminal-task-01d** — Animated viewport offset (slice 5).
  **Slice 5 DONE (2026-06-25):** discrete scroll jumps now glide via easing.
  - Added `viewport_anim: Option<Animation>` to `TerminalEngine` (state in engine, Q1).
  - Reused `Animation`/`AnimationConfig::default()` (250ms ease_out_cubic) from `layout/animation.rs`.
  - Per-pane: each `TerminalEngine` owns its animation.
  - Animated variants: `scroll_viewport_animated`, `scroll_to_top_animated`, `scroll_to_bottom_animated` on `PaneBackend` trait (default delegates to immediate).
  - Wheel path (`scroll_viewport`) applies immediately and **clears** any ongoing animation.
  - Re-target (no queue): new jump bases delta on the current animation's **target**, creates a new Animation from the current animated value.
  - Approach A (integer-row step): `advance_animation()` rounds f64 → nearest usize, updates `viewport_offset` on change.
  - Drive: `tick_animation()` called per-frame in `poll_backends` (lifecycle.rs); `terminal_animating` flag gates `needs_frame` + `ControlFlow::WaitUntil`.
  - `reconcile_viewport_offset` clears animation when scrollback shrinks past the target.
  - 3 new tests: re-target (lands at 10 not 5), settles at target, wheel clears animation.
  - Gates: `heca` 265/265, `heca-core` 76/76, clippy 0.
  Next: slice 6 (GUI widgets — scrollbar + scrolled-up indicator).
  
  Make discrete scroll jumps glide instead of snapping. **Design locked 2026-06-25** (answers to
  the agent's open questions):
  • **State location:** in `TerminalEngine` (heca-core), consistent with Q1 (engine owns
    `viewport_offset`). NOT in `SharedChromeState` (that store is plugin-observable derived state;
    the animation is transient/per-frame). Engine holds a *target* offset + an animated *current*.
  • **Curve:** easing, **reuse the existing `Animation`/`AnimationConfig`** from
    `heca-core/src/layout/animation.rs` (the same primitive `ViewOffset` uses). No spring physics
    here — spring is a separate future overhaul (niri-parity).
  • **Duration:** reuse `AnimationConfig::default()` (the one `activate_column`/`ViewOffset` use)
    so terminal scroll feels like column scroll. Do not invent a new number.
  • **Scope:** per-pane (each `TerminalEngine` animates its own offset).
  • **What animates:** only the **discrete jumps** — page up/down, line-key, to-top, to-bottom.
    The **wheel applies immediately** (no per-notch animation; per-notch easing fights the next
    notch — same rule as the column-resize drag).
  • **Rapid consecutive scroll:** **interrupt / re-target** (never queue) — a new jump re-creates
    the `Animation` from the current animated value toward the new target, exactly like
    `activate_column`.
  • **Approach (locked): (A) integer-row step animated** — `visible_lines()` keeps picking whole
    rows; the animated value rounds to the nearest row each frame. Zero renderer changes, Q6-safe
    (`Full` damage during the animation is fine). **(B) sub-row pixel-smooth** (renderer shifts
    content by a fractional row + clips partial rows) is **DEFERRED to a follow-up** — nicer but
    needs renderer work. Build A now; revisit B as polish.
  • **Drive:** the app per-frame update must call `tick(dt)` on the terminal backend and request a
    redraw while the animation is ongoing (mirror how layout animations are driven); viewport
    motion still emits `Full` damage (Q6).
  Tests: animation re-targets on a new jump (no queue), settles at the target, wheel does NOT
  animate. Gate: clippy 0 + tests.
  Files: `heca-core/src/backend/terminal/engine.rs` (+ `terminal.rs` if `update`/`tick` drives it),
  the app per-frame update path (`heca/src/main.rs` / `app/render.rs`).
  Ref: handoff Q7 (working note only — this backlog is authoritative).

- [x] **terminal-task-01e** — Scrollback GUI widgets (slice 6). ✅ DONE (2026-06-25)
  Delivered two **generic** `heca-grid-ui` widgets wired to the slice-4 terminal viewport store:
  - **`ScrollBar`** — standalone vertical scrollbar widget (click track = jump, drag thumb = jump);
    app drives `content_extent` / `viewport_extent` / `offset` from
    `host.terminal_viewport(pane)`.
  - **`BadgeButton`** — clickable badge/chip used for the scrolled-up indicator (`N lines above`).
  App wiring:
  - retained per-pane viewport widgets synced each frame (`state.pane_viewport_widgets`), painted
    in the terminal pane shell, pointer-dispatched from `events.rs`.
  - badge sits flush-right **below** the pane info-bar header; scrollbar hugs the pane's right edge
    while using the terminal content rect for its vertical span.
  New config / actions:
  - `[appearance.terminal] show_scrollbar = "always" | "when_needed" | "never"`
  - `[appearance.terminal] show_scrolled_up_badge = true | false`
  - `WmAction::ScrollToOffset { rows }` with full registration: action registry, interaction
    policy, RPC (`direct-scroll-to-offset <rows>`), docs + commented keybinding example.
  - `[settings] terminal_scroll_animations = true | false` — disables backend-side animated
    viewport jumps by degrading animated APIs to immediate scroll.
  Docs / catalog:
  - updated `docs/widgets.md`, showcase, `config.default.toml`, `README.md`,
    `keybindings.default.toml`.
  Verification:
  - `cargo clippy --workspace --all-targets --all-features` ✅
  - `cargo test --workspace --all-targets --all-features` ✅
  - targeted animation tests (`animated_scroll_settles_at_target`,
    `animated_scroll_re_targets_on_new_jump_without_queueing`) ✅
  Ref: handoff Q7.

- [x] **terminal-task-01f** — Docs + final review + commit (slice 7). ✅ DONE (2026-06-27)
  Completed:
  • final user-facing docs pass across README/config for scrollback settings,
    `prefix+s` Selection-mode entry, direct `Shift+...` bindings, and GUI scrollbar/badge behavior
  • final runtime review of the merged feature: selection/caret-follow ✅, copy UX ✅,
    GUI widgets ✅, snap-to-bottom on key input ✅, `terminal_scroll_animations` restored to a
    visible effect via local fixes ✅
  • review leftovers split out instead of blocking closure of the main feature:
    alt-screen wheel-routing / `Shift+wheel` behavior in `nvim`/`less` is now tracked as
    follow-up `terminal-task-01h`
  • working-note cleanup: `terminal-01a` marked done and
    `handoff-terminal-scrollback.md` removed
  Validation during review:
  • `cargo clippy --workspace --all-targets --all-features` ✅
  • `cargo test --workspace --all-targets --all-features` ✅

- [x] **terminal-task-01g** — BUG: selection highlight does not follow the text while scrolling. ✅ DONE (2026-06-25)
  Found in review 2026-06-25 (latent slice-1 bug, exposed once slice-3 scrolling worked).
  **Fixed:** `visible_top_stable_row()` now subtracts `viewport_offset` from wezterm's live
  viewport top (`screen().visible_row_to_stable_row(0) - self.viewport_offset as isize`).
  Additional scrollback work bundled in this task:
  - **Direct bindings (6 new `Scroll*` actions):** `ScrollLineUp/Down`, `ScrollPageUp/Down`,
    `ScrollToTop/Bottom`. Immediate, repeatable, stay in Normal mode.
    Global keybindings: `Shift+{Up/Down/PageUp/PageDown/Home/End}`.
  - **Copy UX:** `y` copies + clears selection + stays in Selection mode with caret at focus
    position. Shift+drag mouse release auto-copies on release.
  - **Caret-follow:** `ensure_caret_visible()` in `move_focused_terminal_selection()` —
    immediate edge-by-edge scroll (tmux style).
  - **Caret bounds** clamped to absolute scrollback range `[0, scrollback_rows-1]`.
  - **Cursor hidden** during Selection mode (only `SelectionOverlay.caret` visible).
  - **PageUp/PageDown/Home/End** bindings in Selection mode.
  - **Q5 snap fix:** skip snap-to-bottom for modifier-only keys (Shift/Ctrl/Alt alone).
  - All handlers rewritten: move caret + `ensure_caret_visible`, no `scroll_viewport_animated`.
  - Entrance guards: scrollback handlers guard `enter_selection_mode_for_focused_terminal`
    to prevent resetting caret when already in Selection mode.
  - `scrollback_to_bottom` rewritten: calls `scroll_to_bottom()` before snapshotting so
    caret lands on the live cursor position. Stays in Selection mode (Esc exits).
  Gate: `heca` 265/265, `heca-core` 73/73, clippy 0. PR #189 → main.
  Files: `heca-core/src/backend/terminal/engine.rs` (`visible_top_stable_row`),
  `heca/src/handlers.rs`, `heca/src/app/input.rs`, `heca/src/app/interaction.rs`,
  `heca/src/input.rs`, `heca/src/app/registry.rs`, `heca/src/app/terminal_host.rs`,
  `heca/src/rpc.rs`, `keybindings.default.toml`, `README.md`.

- [x] **terminal-task-01h** — BUG: wheel routing is ineffective in alt-screen TUIs (`nvim`, `less`). ✅ DONE (2026-06-27)
  Found in final runtime validation after the scrollback merge. Product decision locked via
  `/grill-me`:
  - Plain wheel + host has no scrollback room (`max_offset == 0`, e.g. alt-screen with no
    retained history) → gracefully forward the wheel to the terminal backend so non-grabbed
    TUIs (`less`) can still react.
  - `Shift+wheel` + host cannot scroll → documented no-op. Shift's contract is "bypass the
    TUI, host-only", so we never silently scroll the TUI.
  - Alt-screen with pre-existing main-screen history: documented as inherent limitation —
    wezterm preserves the pre-alt history but does not expose it while the alt screen is
    active (fields are private on `TerminalState`). Exposing it would require forking
    wezterm or a heca-level frozen snapshot, both fragile for a marginal win.
  Earlier review-local fixes (already on main via PR #191):
  - `Shift+wheel` on the host path uses the dominant wheel axis (`y`, fallback `x`) because
    many platforms remap `Shift+wheel` into horizontal delta.
  - Selection mode only auto-enters on wheel-up if the host viewport actually moved.
  Implementation: `forward_mouse_wheel` peeks `scrollback_rows > rows` to decide
  host-can-scroll; a new `forward_wheel_to_terminal` helper dedups the terminal-forward path.
  README documents the alt-screen wheel limitation.
  Runtime-validated 2026-06-27: `less` plain wheel now scrolls `less`; `nvim` plain wheel
  scrolls nvim; `Shift+wheel` in alt-screen is a no-op as documented.
  Files: `heca/src/app/terminal_host.rs`, `README.md`.
  Gate: clippy 0; `cargo test -p heca` 266/266; `heca-core` flaky PTY tests pass in isolation.

### [x] Phase: Terminal ligature policy · `terminal-02`
Ligatures in coding fonts (Fira Code `->` `!=` `>=`, Maple Mono, …) are **multi-character** and span multiple terminal cells. heca's terminal renderer currently shapes text **one cell at a time** (`terminal.rs` calls `queue_text_in_line_box_with_style(&cell.text, …)` per cell, and the shaper only ever sees a single cell's text). Because the shaper never sees `->` as one run, **multi-cell ligatures cannot form today** regardless of the `calt`/`liga` OpenType features. So a `terminal_ligatures` setting would be a no-op until the renderer shapes whole row runs together. This phase is re-scoped into two steps: first enable run-level shaping (so ligatures can form), then add the on/off setting.

- [x] **terminal-task-02a** — Enable row-level (run) text shaping in the terminal renderer. **DONE.**
  The terminal renderer now groups consecutive cells sharing a font face into one shaped run
  (`render_terminal_lines` in `heca-renderer/src/terminal.rs` builds `TerminalTextRun`s and calls
  `TextRenderer::queue_terminal_run`); `build_run_emission` in `heca-renderer/src/text.rs` shapes
  the whole run (`Shaping::Advanced`, default `calt`/`liga`/`clig`) then **snaps every glyph back
  to its originating cell column** (`run_x + col*cell_w + bearing`) so the monospace grid stays
  exact even when a ligature spans cells. New `run_layout_cache` mirrors the existing eviction/
  scale/font-reload invalidation. Backgrounds/underlines/box-drawing symbols stay per-cell.
  **Decision (changed from original plan):** grouping is by font face only — **NOT** foreground
  color. Ligatures must form across color boundaries (e.g. a syntax-highlighted operator); the
  emission colors each glyph by its own cell (per-column `col_colors`, hashed into `EmitKey`).
  This matches kitty/WezTerm. (Original plan said group by `fg/bg` too — that broke ligatures on
  any color change, e.g. a red "command-not-found" `=>` at the prompt.)
  **Shaper bug found + fixed (cosmic-text upgrade 0.14 → 0.19):** with the old `cosmic-text 0.14`
  (rustybuzz 0.14.1) only `=`-initiated ligatures formed (`=>` `==` `>=` `<=` `::`); `->` `!=` `|>`
  did NOT — for EVERY font (Maple Mono, JetBrains Mono), proven by shaping the files through four
  shapers directly: real HarfBuzz forms all; rustybuzz 0.14 fails `->`/`!=`/`|>`; rustybuzz 0.20
  and cosmic-text 0.19 (now `swash`/`harfrust`) form all. So it was a rustybuzz-0.14 bug, not the
  font and not the terminal code. Upgraded `heca-renderer` to `cosmic-text 0.19` (contained: API
  migration in `text.rs` `set_size`/`set_text`/`get_font`, `atlas.rs` unchanged). Verified the
  embedded default Maple Mono Normal NF now ligates the full set under the project dependency.
  (Earlier wrong guess that "the font lacks `->`" — corrected by direct multi-shaper testing.)
  Files: `heca-renderer/src/terminal.rs`, `heca-renderer/src/text.rs`, `heca-renderer/Cargo.toml`,
  `README.md`.
  Gate: clippy 0 ✅ + 22 renderer tests ✅ + 74 heca-core ✅ (incl. byte→column mapping, wide
  cells, color-across-boundary). Visual: full arrow ligatures render in a terminal pane.

- [x] **terminal-task-02** — Add `terminal_ligatures: bool` and wire it to the shaping path. **DONE.**
  `[appearance.terminal] ligatures` (default **`true`** — decided with user; ligatures stay on out
  of the box). `TerminalAppearance.ligatures` → `TerminalStyle.ligatures` (render.rs, all 4 sites)
  → `queue_terminal_run` → `TerminalRun.ligatures`. When `false`, `build_run_emission` shapes with
  `Attrs::font_features` disabling `CONTEXTUAL_ALTERNATES`/`STANDARD_LIGATURES`/`CONTEXTUAL_LIGATURES`
  so each char stands alone. `ligatures` is part of `LabelKey` + `EmitKey` (so toggling re-shapes)
  and of `terminal_layer_render_key` (so the retained layer re-renders); `reload_config` already
  clears `terminal_layers`, so `prefix+Shift+r` applies it live. Scope: terminal font path only.
  Files: `heca-config/src/appearance.rs`, `heca-renderer/src/{terminal,text}.rs`,
  `heca/src/app/{render,terminal_render}.rs`, `config.default.toml`, `README.md`.
  Gate: clippy 0 ✅ + tests ✅ (`terminal_ligatures` shaping test: disabling features changes
  glyphs to standalone; `terminal_layer_render_key_changes_with_ligatures`).

### [x] Phase: Richer terminal protocol hooks · `terminal-03`
Extension points for hyperlinks and inline graphics without redesigning the core render contract.

- [x] **terminal-task-03** — `OSC 8` hyperlink extension point on `TerminalSnapshot`. **DONE.**
  Added `HyperlinkSpan { row, start_col, end_col, uri }` + `hyperlinks: Vec<HyperlinkSpan>` (real
  path: `heca-core/src/backend/snapshot.rs`), captured in the engine (`collect_row_hyperlinks`
  merges contiguous same-URI cells). **Plus nice rendering** (user request 2026-06-29, beyond the
  original capture-only scope): link cells are recolored + decorated, theme/config-driven via
  `[appearance.terminal] hyperlink_style` (`none|color|underline|undercurl`, default `underline`)
  and `hyperlink_color` (→ `theme.accent`). Renderer enum `HyperlinkDecor`, `TerminalStyle` carries
  color+style, `render_terminal_lines` recolors/decorates link spans. Click-to-open stays
  `terminal-task-18`.
  Files: `heca-core/src/backend/{snapshot,terminal/engine}.rs`, `heca-config/src/appearance.rs`,
  `heca-renderer/src/terminal.rs`, `heca/src/app/{render,terminal_render}.rs`,
  `config.default.toml`, `README.md`.
  Gate: clippy 0 ✅ + tests ✅ (`snapshot_captures_osc8_hyperlink_spans`).

- [x] **terminal-task-04** — Inline graphics/image placement stub on `TerminalSnapshot`. **DONE.**
  Added `GraphicsPlacement { row, col, cols, rows, image_id }` + `graphics: Vec<GraphicsPlacement>`
  (always empty for now) — the contract stub for `terminal-09`. No capture/rendering yet.
  Files: `heca-core/src/backend/snapshot.rs`
  Related: `terminal-task-20` (full image rendering for Yazi)

### [ ] Phase: Backend and renderer tests · `terminal-04`
Close the test gap before selection/clipboard adds more moving parts.

- [ ] **terminal-task-05** — Backend lifecycle tests: init → resize → snapshot, dirty rows after output, exit detection.
  Files: `heca-core/src/backend/terminal/tests.rs` (or inline)

- [ ] **terminal-task-06** — Renderer tests: content-rect clipping, row invalidation logic, color mapping.
  Files: `heca-renderer/src/terminal.rs` (inline test module)

- [ ] **terminal-task-07** — Manual validation matrix: shell prompt, long output scroll, nvim, truecolor, Unicode fallback, pane resize, mouse-enabled TUI. Document results in `terminal-implementation.md` HANDOFF.
  Note: verify Yazi image preview (currently shows infinite spinner — see `terminal-task-21`)

### [ ] Phase: Pane-shell hosting contract · `terminal-05`
Formally mount the terminal as content inside a `heca-grid-ui` Pane shell.
The shell owns outer chrome (borders, title, focus ring, content rect, clip). The terminal host owns PTY/snapshot/render/input.
Note: border/radius visual blocker is already FIXED (#121/#122).

- [ ] **terminal-task-08** — Define the pane-shell hosting contract in `terminal-implementation.md` — what the shell owns vs what the terminal host owns. No code yet.

- [ ] **terminal-task-09** — Adapt the terminal host to render inside a `heca-grid-ui` `Pane` widget via an explicit content-slot API.
  Starting point: `heca/src/app/terminal_host.rs` `Rectangle`-based mount step.
  Files: `heca/src/app/terminal_host.rs`, `heca/src/app/terminal_render.rs`
  Gate: the ChromeHost/pane-shell boundary must be defined first (`plugin-task-05`)

- [ ] **terminal-task-10** — Make the pane shell reflect `idle`/`running`/`error` from `PaneRuntime` (already in chrome store) without coupling to terminal rendering internals.
  Files: `heca/src/chrome/mod.rs`, `heca/src/app/terminal_render.rs`

### [ ] Phase: Text selection · `terminal-06`
Shared host selection model — not terminal-only. Keyboard caret, actions, overlays.
Source: `terminal-implementation.md` Phase 9

- [ ] **terminal-task-11** — Define shared selection state in app state, keyed by pane/surface owner.
  Files: `heca/src/app_state.rs` or new `heca/src/selection.rs`

- [ ] **terminal-task-12** — Add selection actions through `WmAction` + `ActionRegistry` + mode-local keymap.
  Actions needed: `EnterSelectionMode`, `BeginSelection`, `ClearSelection`, `ToggleSelectionEndpoint`, `CopySelection`, `PasteClipboard`, `SelectionLeft/Right/Up/Down`.
  Note: several variants already exist in `heca/src/app/interaction.rs` — verify completeness.
  Mode-local bindings (no prefix required while IN selection mode): `v`/`Space` = begin selection, `o` = flip endpoint, `y` = copy, `Esc` = exit.
  Follow "Adding New Actions" checklist in `AGENTS.md` (11 steps).
  Files: `heca/src/input.rs`, `heca/src/app/interaction.rs`, `heca/src/handlers.rs`, `heca/src/app/registry.rs`

- [ ] **terminal-task-13** — Implement host-rendered selection overlay for terminal panes.
  Mouse entry: `Shift+left-drag` (unmodified drags must still go to TUI mouse mode unaffected).
  Files: `heca/src/app/terminal_render.rs`, `heca-renderer/src/terminal.rs`

### [ ] Phase: Clipboard and paste · `terminal-07`
System clipboard on top of the shared selection model.
Source: `terminal-implementation.md` Phase 10

- [ ] **terminal-task-14** — Copy selected text to system clipboard (not terminal-specific — uses the shared selection owner).
  Files: `heca/src/handlers.rs`, add clipboard crate (`arboard` or platform equivalent)

- [ ] **terminal-task-15** — Paste from system clipboard through the focused pane/backend.
  Terminal paste must respect bracketed-paste mode when active.
  Files: `heca/src/handlers.rs`, `heca-core/src/backend/terminal/engine.rs`

- [ ] **terminal-task-16** — Add `OSC 52` terminal protocol clipboard support.
  Files: `heca-core/src/backend/terminal/engine.rs`

### [ ] Phase: Terminal UX and attention features · `terminal-08`
Bell, scrollback search, hyperlinks.
Source: `terminal-implementation.md` Phase 11

- [ ] **terminal-task-17** — Bell handling: capture backend alert, window attention signal, audible/visual bell policy via config.
  Files: `heca-core/src/backend/terminal/engine.rs`, `heca/src/app/lifecycle.rs`, `heca-config`

- [ ] **terminal-task-18** — `OSC 8` hyperlink open-link action: capture links in snapshot, add `WmAction::OpenLink { url }` + handler.
  Files: `heca-core/src/backend/terminal/snapshot.rs`, `heca/src/input.rs`, `heca/src/handlers.rs`

- [ ] **terminal-task-19** — Scrollback search: entry-point action, search overlay (grid-ui `Input` widget), results highlighting.
  Files: `heca/src/input.rs`, `heca-core/src/backend/terminal/engine.rs`, overlay UI via `heca-grid-ui`

### [ ] Phase: Terminal image protocols (Yazi preview) · `terminal-09`
Source: `terminal-implementation.md` Phase 12
Gate: `terminal-task-03`/`terminal-task-04` (protocol hook stubs) must exist first.

- [ ] **terminal-task-20** — Design image/graphics protocol surface: how Kitty graphics protocol + sixel placements map from `wezterm-term` through `TerminalSnapshot` to the renderer.
  Files: `heca-core/src/backend/terminal/snapshot.rs`, `heca-renderer/src/terminal.rs`

- [ ] **terminal-task-21** — Implement GPU renderer for image placements: texture upload + blit at correct cell coordinates.
  Files: `heca-renderer/src/terminal.rs`, new `heca-renderer/src/terminal_graphics.rs`

- [ ] **terminal-task-22** — Wire Yazi image preview end-to-end. Confirm no infinite spinner.

---

## Theming

> Source: `theming-plan.md`, `theming-documentation.md`
> `heca-theme` crate is live (grid_tron/mocha/latte bundled). Showcase cycles themes.
> Active work: finish visual verify, then migrate all consumers (heca-config, heca-grid-ui, heca app).

### [~] Phase: Finish showcase visual verification · `theming-01`
Phase 2.5 and 2.6 from `theming-plan.md` are not yet ticked.

- [ ] **theming-task-01** — Run the showcase and verify all widgets react to theme cycling: colors, radius, border, glow, fonts.
  Run: `cargo run -p heca-renderer --example showcase`
  Files: `heca-renderer/examples/showcase.rs` (fix if needed)

- [ ] **theming-task-02** — Verify the light theme (`latte`) renders correctly. Glow is now optional — renderer supports light-theme glow; confirm no visual regressions with `intensity = "off"` and `glow_size = "none"`.

### [ ] Phase: Migrate `heca-config` to `heca-theme` · `theming-02`
Replace duplicate Theme/Color in `heca-config` with re-exports from `heca-theme`.
Source: `theming-plan.md` Phase 3A

- [ ] **theming-task-03** — Add `heca-theme` dep to `heca-config/Cargo.toml`.

- [ ] **theming-task-04** — Remove `heca-config/src/color.rs` — re-export `heca_theme::Color` from `heca-config::color`.

- [ ] **theming-task-05** — Remove `heca-config/src/defaults.rs` — move serde default helpers inline into `theme.rs` or callers.

- [ ] **theming-task-06** — Update `heca-config/src/theme.rs`:
  Remove `Theme` struct — re-export `heca_theme::Theme`.
  Remove `catppuccin_mocha()`/`catppuccin_latte()` constructors — use `heca_theme::load_theme()`.
  Keep `Theme::terminal_cell_size()` (app-specific helper, not in heca-theme).
  Keep `Theme::load(name)` as thin wrapper around `heca_theme::load_theme(name)`.

- [ ] **theming-task-07** — Update `heca-config/src/loader.rs` — delegate `load_theme(name)` to `heca_theme::load_theme(name)`.

- [ ] **theming-task-08** — Update all other `heca-config` files that import `Color` or `Theme` directly.

- [ ] **theming-task-09** — Change `default_theme()` return value from `"mocha"` to `"grid_tron"`.
  This flip is safe only after `theming-task-07` delegates loading to `heca-theme` (which bundles grid_tron).

- [ ] **theming-task-10** — `cargo check -p heca-config` + `cargo test -p heca-config` + clippy clean.

### [ ] Phase: Migrate `heca-grid-ui` to `heca-theme` · `theming-03`
Replace duplicate Theme/Color/Intensity/GlowLevel in `heca-grid-ui`.
Source: `theming-plan.md` Phase 3B

- [ ] **theming-task-11** — Add `heca-theme` dep to `heca-grid-ui/Cargo.toml`.

- [ ] **theming-task-12** — Remove `heca-grid-ui/src/color.rs` — re-export `heca_theme::Color`.

- [ ] **theming-task-13** — Update `heca-grid-ui/src/theme.rs`:
  Remove `Theme`, `Intensity`, `GlowLevel` struct definitions.
  Re-export `heca_theme::Theme`, `heca_theme::Intensity`, `heca_theme::GlowLevel`.
  Remove `grid_tron()`/`grid_ares()` constructors.

- [ ] **theming-task-14** — Update `heca-grid-ui/src/lib.rs` + `prelude` — re-export `Theme`/`Intensity`/`GlowLevel` from `heca-theme` instead of local `theme` module.

- [ ] **theming-task-15** — Verify all 30+ widgets still compile (they call `cx.theme()` — should work via re-export). Run a `cargo check -p heca-grid-ui`.

- [ ] **theming-task-16** — Update `heca-grid-ui/tests/phase_a.rs` — replace `Theme::grid_tron()` with `heca_theme::load_theme("grid_tron")`.

- [ ] **theming-task-17** — `cargo check -p heca-grid-ui` + `cargo test -p heca-grid-ui` + clippy clean.

### [ ] Phase: Migrate the main app (`heca`) to `heca-theme` · `theming-04`
Remove all hardcoded colors, theme-name branches, and semi-hardcoded alpha magic from the app's render/chrome paths.
Source: `theming-plan.md` Phase 3C
Hard requirement: zero literal `Color::new(...)` / `[0.xxx, ...]` / `*_ALPHA` constants in active render/widget paths after this phase.

- [ ] **theming-task-18** — Fix `heca/src/chrome/mod.rs`:
  Make `chrome_gui_theme()` a direct pass-through of `heca_theme::Theme` (no manual 5-field patching).
  Replace `chrome_colors()` hardcoded `Color::new(17, 17, 27, 255)` with `theme.background`.

- [ ] **theming-task-19** — Fix `heca/src/app/render.rs`:
  Replace any remaining `if theme.name == "Catppuccin Mocha"` guards.
  Replace `pane-select` overlay literal color `[1.0, 0.9, 0.3, 0.9]` with a theme token.
  Audit and replace any other literal colors in `primitive_renderer.draw_rect()`/`queue_text()` calls.

- [ ] **theming-task-20** — Fix `heca/src/app/terminal_render.rs` — update `terminal_pane_gui_theme()` to use `heca_theme::Theme` directly.

- [ ] **theming-task-21** — Fix `heca/src/sidebar/render.rs` — replace `RenderColors` hardcoded arrays with values derived from `theme`.

- [ ] **theming-task-22** — Fix `heca/src/mouse/render.rs` — replace hardcoded `[0.118, 0.118, 0.180, 0.7]`, white overlay text, and any literal hover/preview colors with theme tokens.

- [ ] **theming-task-23** — Wire `[settings].theme` into config loading (the field already exists in `SettingsConfig`). Coordinate with `theming-task-09` — the default flips to `"grid_tron"` once loading delegates to `heca-theme`.

- [ ] **theming-task-24** — Tokenize `heca-grid-ui` widget hardcoded alphas:
  Promote `IconButton` `HOVER_FILL_ALPHA`/`HOVER_BORDER_ALPHA` to theme tokens.
  Replace the white press flash (`cx.flash = rgb(255,255,255)` in `component.rs`) with a theme token.
  Replace `Tag` `FILL_ALPHA`/`BORDER_ALPHA` with theme tokens.
  Replace the close-red `danger.lerp(surface, 0.25)` hack with a proper `muted_danger` theme token.
  Files: `heca-grid-ui/src/widgets/icon_button.rs`, `heca-grid-ui/src/component.rs`, `heca-grid-ui/src/widgets/tag.rs`, `heca-theme/src/theme.rs`

- [ ] **theming-task-25** — Grep gate: run `rg 'Color::new\(|Color::rgb\(|\[0\.' --glob '!*.md' --glob '!*test*'`
  Confirm no literal colors remain in active render/widget paths.
  Any intentional protocol-fallback literal must have a comment explaining why.

- [ ] **theming-task-26** — `cargo clippy --workspace --all-targets --all-features` clean + `cargo test --workspace` green.

### [ ] Phase: Migrate hand-drawn chrome to grid-ui widgets · `theming-05`
Replace remaining `primitive_renderer.draw_*()` calls with proper `heca-grid-ui` components.
Source: `theming-plan.md` Phase 4
Gate: `theming-02`, `theming-03`, `theming-04` must be complete.

- [ ] **theming-task-27** — Migrate tab bar (background + text) to a grid-ui widget.
  Files: `heca/src/app/render.rs` (tab bar section)

- [ ] **theming-task-28** — Migrate status bar to a grid-ui `StatusBar`/`ChromeRegion` component.
  Files: `heca/src/app/render.rs` (status bar section)

- [ ] **theming-task-29** — Migrate collapsed sidebar rail from legacy hand-drawn + `sidebar_hit_test` to `RailCell`/`ChromeRegion` grid-ui widgets.
  Files: `heca/src/sidebar/render.rs`, `heca/src/mouse/render.rs`
  Coordinate with `app-task-21` (collapsed rail wiring)

- [ ] **theming-task-30** — Migrate sidebar tree (`heca/src/sidebar/render.rs`, ~349 lines) to grid-ui `DockFrame`/`ItemGroup`/`Row` widgets.

- [ ] **theming-task-31** — Migrate pane select/swap overlays (`heca/src/mouse/render.rs`) to grid-ui overlay widgets.

- [ ] **theming-task-32** — Decide on `grid_ares()` — keep as a 4th bundled theme or drop. Update `heca-theme/src/loader.rs` accordingly.

---

## Compositor / Blur

> Source: `compositor-blur-refactor-plan.md`
> Replace the tiled-tint + OS-vibrancy approach with a heca-owned z=0 blurred gradient background layer.
> Fixes cross-platform frost AND the floating-pane text collision defect (5% sharp-content leak).
> Gate: run AFTER `theming-04` so the new `background_*` config knobs live in the unified theme system.
> **Status:** **CLOSED — not passed (2026-06-22).** Phases 1–3 done and merged (PR #165 = Phase 1, PR #167 = Phase 2, PR #169 = Phase 3). Phase 4 (`compositor-04`) docs shipped folded into `compositor-04b`. `compositor-04b` (intensity/glow override + glow/scanline separation) ✅ merged (PR #172). `compositor-04c` (font settings separation) ✅ merged (PR #175). **`compositor-06` quality gates ✅ done (2026-06-22, on `feature/compositor-05-06-finalize`):** rust-skill review clean, clippy 0, tests green in isolation (heca-core 57/57, heca-config 61/61, heca-renderer 241/241, heca 61/61; the bash-test fix `b42c016` isolated the test shell from `~/.bashrc`). The two non-green tests in a full workspace run are both pre-existing/unrelated, not regressions: the `heca-grid-ui` white-press-flash toast test fails on plain `main`, and `terminal_backend_exit_captures_code_via_take_exit` is a flaky PTY-timing test under workspace-parallel load (passes 5/5 in isolation).
> **`compositor-05` (visual tuning with the user) DID NOT PASS — closed not-passed.** The frost never reached a user-approved look. Root cause: the frosted-desktop-BEHIND-the-window look the user wanted is an OS capability (macOS `vibrancy`), which an app cannot do cross-platform (an app cannot sample/blur the OS desktop). The within-heca frost approaches tried (gradient tint, embedded background image, procedural gradient noise) only frost heca's own z=0 gradient, not the desktop, and none got user sign-off. `vibrancy` remains the only path to blurred-desktop-behind and is left for the user to enable on macOS. **`compositor-06` exit (ship) did not pass** — it was gated on Phase 5 sign-off (AGENTS.md gate). All uncommitted tuning experiments (procedural frost noise + `is_transparent()` change) were discarded; working tree clean at `243f875`. The merged z=0 GPU pipeline (Phases 1–4c) stays as the cross-platform frost surface.

### [x] Phase: GPU plumbing — gradient layer and cached blur · `compositor-01`
New isolated GPU primitives in `heca-renderer`. No app wiring yet.
Source: `compositor-blur-refactor-plan.md` Phase 1

- [x] **compositor-task-01** — Create `heca-renderer/src/gradient.rs` — `GradientRenderer` that fills a render-target texture with a 2-color vertical gradient via a fullscreen-triangle WGSL pipeline.
  API: `new(device, format)` + `render(queue, encoder, target, top: [f32;4], bottom: [f32;4])` (the `device` param was dropped during review — `render` borrows `&self` only).
  Colors are linear f32x4 (match the existing primitive renderer convention).
  Files: `heca-renderer/src/gradient.rs`, `heca-renderer/src/lib.rs`

- [x] **compositor-task-02** — Create `heca-renderer/src/background.rs` — `BackgroundLayer` owning z=0 layer state with a static cached blur.
  Fields: `z0_tex`/`z0_view` (gradient render target), `cache_tex`/`cache_view` (blurred result), `dirty: bool`, cached params (top/bottom/blur_radius/size/format).
  `render(&mut self, device, queue, encoder, blur: &Blur) -> &TextureView` — re-runs gradient+blur only when dirty; returns cached view when clean.
  CRITICAL: copy blurred view → cache (`encoder.copy_texture_to_texture`) BEFORE any subsequent `blur.process` call in the same frame.
  Files: `heca-renderer/src/background.rs`, `heca-renderer/src/lib.rs`

- [x] **compositor-task-03** — Unit test for the dirty-cache logic: call `render()` twice with no `set_params`/`resize` change between them; assert the blur path ran only once.
  If no headless GPU device: factor out `fn params_changed(&self, ...) -> bool` and test that instead.

### [x] Phase: Config — new background knobs · `compositor-02`
Add `background_gradient_top/bottom`, `background_blur`, `background_transparency` to config. Additive only — no removals yet.
Source: `compositor-blur-refactor-plan.md` Phase 2

- [x] **compositor-task-04** — Add fields to `heca-config/src/appearance.rs`:
  `background_gradient_top: Option<Color>`, `background_gradient_bottom: Option<Color>`,
  `background_blur: u8`, `background_transparency: u8`.
  Serde defaults: `background_blur = 0`, `background_transparency = 0` (opaque by default).

- [x] **compositor-task-05** — Add resolvers in `heca-config/src/appearance.rs`:
  `effective_background_gradient_top(&self, theme) -> Color` (config → `theme.background_gradient_top` → `theme.background`).
  `effective_background_gradient_bottom(&self, theme) -> Color` (config → `theme.background_gradient_bottom` → shifted derivation → `theme.background`).
  `background_blur_radius() -> f32` (`background_blur` 0..100 → 0..`MAX_BLUR_PX`).
  `background_alpha() -> f32` (`(1.0 - background_transparency as f32 / 100.0).clamp(0.0, 1.0)`).

- [x] **compositor-task-06** — Add `background_gradient_top`/`background_gradient_bottom` fields to `grid_tron.toml`, `mocha.toml`, `latte.toml` in `heca-theme/src/themes/`.
  Bonus: `Theme::grid_tron()` now parses `include_str!("themes/grid_tron.toml")` — single source of truth, all hardcoded `Color::rgb` removed from the Rust constructor.

- [x] **compositor-task-07** — Unit tests: defaults, config overrides, clamping, fallback chains. `cargo test -p heca-config` green (50 tests).

### [x] Phase: Pipeline integration — z=0 blit and tint removal · `compositor-03`
Wire the background layer into the render pipeline, remove the old tiled tint, fix floating backdrop to 100%.
Source: `compositor-blur-refactor-plan.md` Phase 3
Merged in PR #169.

- [x] **compositor-task-08** — Add `BackgroundLayer` field to `AppState` (`heca/src/app_state.rs`) — construct alongside `blur`/`compositor`.
  Done: `pub background: BackgroundLayer` field + constructed in `heca/src/app/startup.rs` sized to the physical framebuffer; import added to both `app_state.rs` and `startup.rs`.

- [x] **compositor-task-09** — Add resize hook in `heca/src/app/events.rs` — call `state.background.resize(...)` alongside the existing `blur.resize` and `compositor.resize` calls.
  Done: `state.background.resize(&state.device, phys.width, phys.height)` in `WindowEvent::Resized`, right after `blur.resize`.

- [x] **compositor-task-10** — Add z=0 blit in `heca/src/app/render.rs` — AFTER the scene clear and BEFORE any pane stencil/content passes:
  `state.background.set_params(top, bottom, blur_radius * scale_factor)` → `let bg_view = state.background.render(...)` → blit `bg_view` at `alpha = background_alpha()`.
  Reuse `Backdrop::draw` (fullscreen dst) or add a minimal alpha-blit pipeline to `backdrop.rs`.
  Done: reused `Backdrop::draw` (fullscreen dst, `src_uv = [0,0,1,1]`, `opacity = background_alpha()`, `stencil = None`) — no new pipeline. Reads `effective_background_gradient_top/bottom(theme)` + `background_blur_radius() * scale`. Drawn pre-stencil. An `// Order invariant:` comment documents the shared-`state.blur` snapshot contract (z=0 must render before any other blur user this frame).

- [x] **compositor-task-11** — Remove the tiled frost tint (Pass A) in `heca/src/app/render.rs`:
  Delete the `needs_frosted_backdrop` gate and the frosted-tint `draw_rect` block.
  Tiled panes now reveal z=0 through their `surface_alpha` — the frost IS z=0 showing through.
  Keep the stencil content-clip (Pass 2) intact.
  Done: removed the Pass A block, `needs_frosted_backdrop`, `frost_opacity`, and the "Frosted terminal backdrop" comment block. Stencil content-clip (Pass 2) kept intact. Also removed `content_canvas_fill()` + `FillRect`/`subtract_fill_rect`/`uncovered_fill_rects` + their pass + 3 unit tests (the PR #160 stopgap that sat in z=0's slot — folding it now avoids two competing background layers).

- [x] **compositor-task-12** — Remove old tint config in `heca-config/src/appearance.rs`:
  Delete `terminal_frost_color`, `terminal_frost_opacity()`, `effective_terminal_frost_color()`.
  Run `rg "terminal_frost" --glob '!*.md'` — confirm zero remaining code references.
  Done: removed `terminal_frost_color` (AppearanceConfig **and** `Theme` fields), `terminal_frost_opacity()`, `terminal_floating_frost_opacity()`, `effective_terminal_frost_color()`. Also removed `terminal_blur` / `terminal_blur_radius()` / `default_terminal_blur` (dead after Pass A removal — grill-me Q3 mandates dropping tiled `terminal_blur`; leaving it would break the 0-warning baseline). Fixed `latte.toml` `terminal_background #e6e9ef00 → #e6e9ef` (Q1 opaque-theme fix). Stripped `example.config.toml`. `rg "terminal_frost|terminal_blur"` (code, excl docs) → clean.

- [x] **compositor-task-13** — Fix floating backdrop opacity in `heca/src/app/render.rs`:
  Change floating-pane `backdrop.draw` opacity from `floating_frost_opacity` → `1.0`.
  Drop `floating_frost_opacity` accessor. Floating frost visibility is now driven by `terminal_floating_transparency` (cell surface alpha) only.
  Done: floating `backdrop.draw` opacity arg → `1.0`; removed the `floating_frost_opacity` local + `terminal_floating_frost_opacity()`.

- [x] **compositor-task-14** — Update showcase/examples for any changed `Backdrop::draw` or compositor signatures.
  Files: `heca-renderer/examples/showcase.rs`
  Done: no `Backdrop::draw` signature changed (reused as-is); `cargo build --workspace --all-targets` green (showcase builds). The showcase has no `backdrop.draw` call site, so nothing to update there.

- [x] **compositor-task-15** — Critical pipeline review: confirm the full render order is:
  clear → z=0 blit → tiled stencil → tiled content (translucent over z=0) → chrome borders → floating blur capture → floating stencil → floating backdrop(1.0) + content → grid-ui chrome → sidebar.
  The z=0 blit MUST be pre-stencil. Confirm no stencil state leaks between tiled and floating passes.
  Done: pipeline order walked and confirmed in the PR #169 body; z=0 is pre-stencil (`stencil = None`) so the tiled content-clip never clips it; the floating pass uses its own stencil. User review passed — 2 doc-only findings applied (z=0 ordering comment; `latte.toml` bug-fix inline comment).

### [x] Phase: Documentation update · `compositor-04`
Source: `compositor-blur-refactor-plan.md` Phase 4
Merged in this PR.

- [x] **compositor-task-16** — Update `keybindings.toml` (or `example.config.toml`) — document new `background_*` knobs; add a migration note that `terminal_blur`/`terminal_frost_color` are removed (use `background_blur` instead).
  Done: `example.config.toml` `[appearance]` — added the z=0 block (`background_blur`, `background_transparency`, optional `background_gradient_top/bottom`) + a migration note that `terminal_blur`/`terminal_frost_color` are removed. Also cleaned `theming-documentation.md` (removed 4 `terminal_frost_color` entries + fixed the stale `#e6e9ef00` latte `terminal_background` → opaque `#e6e9ef`) and `theming-plan.md` (both `content_canvas_fill` stopgap notes now point to Phase 3 Task 3.4b as the folding target, marked done). Grep gate: `rg "terminal_frost_color" --glob '!compositor-blur-refactor-plan.md'` → only historical/migration notes remain; `theming-documentation.md` clean of `terminal_frost_color`/`content_canvas_fill`/`#e6e9ef00`.

- [x] **compositor-task-17** — Update `README.md` — revise the appearance/blur section to describe the z-layer model.
  Done: added a new `### Appearance & Frost (z=0 background layer)` section after the Theme section — documents the z=0 gradient, `background_blur`/`background_transparency`/gradient knobs, tiled-vs-floating frost production, and the `terminal_blur`/`terminal_frost_color` migration note.

- [x] **compositor-task-18** — Update `AGENTS.md` — add z-layer frost model to rendering/appearance notes; reinforce "no hardcoded color — read from theme" rule with the gradient as the canonical example.
  Done: added a `### z=0 background frost model (heca-owned, cross-platform)` subsection after the Stack table — documents `BackgroundLayer` as a heca-renderer GPU primitive (NOT a `heca-grid-ui` widget), the render order, the grill-me Q1 opaque-theme translucency rule, the removed knobs, and reinforces the no-hardcoded-color rule with the gradient as the example. Also updated `.planning/research/ARCHITECTURE.md` §3 (Compositor/Renderer) with the z=0 pipeline + `BackgroundLayer` static blur cache + the app-vs-compositor cross-platform rationale.

> **Note (2026-06-22):** Phase 4 docs were committed to `docs/compositor-phase-4` (`75c1857`) and **shipped folded into the `compositor-04b` PR (#172)** rather than getting their own PR. ✅

### [x] Phase: `intensity` / `glow_size` `[appearance]` override + glow/scanline separation · `compositor-04b`
On-the-fly phase added during the compositor refactor when a problem was found: the `intensity` token was leaking into glow rendering (via `Intensity::glow_scale()`), and the two effect tokens (`glow_size` = glow, `intensity` = scanlines/CRT overlay) had no `[appearance]` override. `intensity` is tied to the compositor's z=0.5 CRT scanline overlay pass (Q2-extra in `compositor-blur-refactor-plan.md`), so this is compositor-track work even though it touches `Theme`/`AppearanceConfig`.
Source: `handoff-intensity-glow.md`, discussion 2026-06-22.
Gate: ships together with `compositor-04` docs (one PR).
**Status (2026-06-22):** ✅ DONE — merged in PR #172 (`feat(compositor): add intensity/glow appearance overrides`).

- [x] **compositor-task-23** — Add `[appearance]` override fields `intensity: Option<Intensity>` and `glow_size: Option<GlowLevel>` to `AppearanceConfig` (`heca-config/src/appearance.rs`) with resolvers `effective_intensity(&Theme)` / `effective_glow_size(&Theme)` (unset → theme wins, set → overrides). Unit tests: unset → theme value; set → override wins; snake_case parse.
- [x] **compositor-task-24** — Clean the glow/scanline separation: remove `Intensity::glow_scale()` from both `heca-theme::Intensity` and `heca-grid-ui::theme::Intensity`; add `GlowLevel::strength_scale()` to both crates (`none=0.0, thin=0.5, medium=1.0, large=1.6` — preserves the old `Intensity::glow_scale()` curve exactly so MarkerGroup's glow strength is unchanged). `GlowLevel::radius_scale()` stays `0.0/0.5/1.0/2.0`. Fix the single caller `heca-grid-ui/src/widgets/marker_group.rs:156` to read `t.glow_size.strength_scale()`; update its comment. Remove the `intensity_glow_scales` test; add `glow_level_strength_scales` + `intensity_scanline_opacity_is_separate_from_glow`.
- [x] **compositor-task-25** — Wire the override into the app at the single choke point `chrome_gui_theme(state)` (`heca/src/chrome/mod.rs`): after `app_theme_to_gui_theme`, override `theme.glow_size`/`theme.intensity` from `state.appearance.effective_*(state.theme)`. `terminal_pane_gui_theme` inherits via `chrome_gui_theme` — no extra wiring.
- [x] **compositor-task-26** — Fix the misleading `Intensity` doc comments in both `heca-theme/src/theme.rs` and `heca-grid-ui/src/theme.rs`: `intensity` controls scanline/CRT overlay opacity only, not glow; glow is `glow_size` (presence + radius + strength).
- [x] **compositor-task-27** — Document the two knobs in `example.config.toml` (commented, with value lists + the "unset → theme" + separation notes).
- [x] **compositor-task-28** — Full token doc audit across `theming-documentation.md` (every `Theme` field + every `[appearance]` field; remove stale `terminal_frost_color` references; corrected `intensity`/`glow_size` semantics; `[appearance]`-overridable note), `README.md`, `AGENTS.md`, `BACKLOG.md`.
- [x] **compositor-task-29** — Verification gates: `cargo build --workspace --all-targets` green; `cargo clippy --workspace --all-targets --all-features` → 0 warnings; `cargo test --workspace` green except the known pre-existing env-dependent `heca-core::terminal_backend_bash_integration...` and the pre-existing `heca-grid-ui toast_action_press_flashes...` test (both fail on plain `main`); `rg "\.glow_scale\(\)"` → no remaining callers.
- [x] **compositor-task-30** — Fold the `docs/compositor-phase-4` doc changes (`compositor-task-16/17/18`) into this PR; resolve the rebase on shared doc files (README, theming-documentation, AGENTS, example.config, BACKLOG). Show the user the full diff for review. **Do not commit until the user reviews.**

### [x] Phase: Font settings separation (extract fonts from `Theme`) · `compositor-04c`
Discovered during `compositor-04b`: fonts are **system-local, not theme-portable** — a theme that ships `font_family = "Maple Mono Normal NF"` breaks on a system without that font. Colors/palette are portable; fonts aren't. The terminal font handling is also messy (`Theme` owns `terminal_font_family`/`terminal_italic_font_family`/`terminal_font_size`, `[settings]` has Option-overrides applied via `loader::apply_overrides`). Move font family + size out of `Theme` into a dedicated structured `[font]` block in `config.toml` with per-style family slots.
Source: discussion 2026-06-22 (during `compositor-04b`).
Gate: after `compositor-04b` merges.
**Status (2026-06-22):** ✅ DONE — merged. Implementation complete on `feature/compositor-04c-font-settings`; build + clippy clean; tests green. Per-style family slots (normal/bold/italic/bold_italic) supported end-to-end (config → resolver → renderer `TerminalFontFamilies`); unset slots fall back to `normal` + weight/synthesized-oblique, so no new font files are needed. `normal` itself is optional — omitting it falls back to the **surface-correct** embedded font (Geist Mono for UI, Maple Mono Normal NF for terminal). Cross-AI review passed; review fixes landed (consts for fallback names, `tf` binding in the renderer bridge, 3 new `TerminalFontFamilies::resolve` edge-case tests).

- [x] **compositor-task-31** — Add a `FontConfig` struct to `heca-config` with this schema:
  ```toml
  [font.family.ui]
  normal = "..."
  bold = "..."        # optional → fall back to normal
  italic = "..."
  bold_italic = "..."

  [font.family.terminal]
  normal = "..."
  bold = "..."
  italic = "..."
  bold_italic = "..."

  [font.size]
  terminal = 12
  ui = 14
  ```
  Defaults live in `FontConfig::default()` (system-safe fonts) — **not** in the theme. Optional style slots fall back to `normal`.
  Files: `heca-config/src/font.rs` (new), `heca-config/src/lib.rs`, `heca-config/src/settings.rs` (remove the font Option-overrides)

- [x] **compositor-task-32** — Remove font fields from `heca-theme::Theme`: `font_family`, `font_size`, `terminal_font_family`, `terminal_italic_font_family`, `terminal_font_size`. Remove them from the 3 bundled TOMLs (`grid_tron.toml`, `mocha.toml`, `latte.toml`). Remove the font arms from `loader::apply_overrides()`.
  Files: `heca-theme/src/theme.rs`, `heca-theme/src/themes/*.toml`, `heca-config/src/loader.rs`

- [x] **compositor-task-33** — Wire consumers to read fonts from `FontConfig` instead of `theme.font_*` (~45 call sites, 11 files):
  `heca/src/main.rs`, `heca/src/app/startup.rs`, `heca/src/chrome/mod.rs` (6 sites), `heca/src/app/render.rs` (6 sites), `heca/src/app/terminal_render.rs`, `heca/src/app/terminal_metrics.rs`, `heca-grid-ui/src/layout.rs` + tests, `heca-renderer/examples/showcase.rs`.
  Pass the resolved font config down at the same choke points that today read `theme.font_*` (e.g. `chrome_gui_theme`, `TextRenderer::set_font_family`, terminal render pass context).

- [x] **compositor-task-34** — Renderer: support per-style family slots (normal/bold/italic/bold_italic) for both UI and terminal surfaces. `heca-renderer/src/text.rs` currently has one `font_family` slot + a `bold: bool` weight toggle; to honor distinct named families per style, load 4 font collections per surface (ui + terminal = 8) and select by (weight, style) via cosmic-text. Terminal already passes `italic_font_family` separately — extend to bold + bold_italic.
  Files: `heca-renderer/src/text.rs`, `heca/src/app/render.rs`
  Note: if full per-style families are deferred, `bold`/`italic`/`bold_italic` slots fall back to `normal` and the renderer keeps weight-based bold — the config schema is still future-proof.

- [x] **compositor-task-35** — Update `example.config.toml` (document the `[font]` block), `theming-documentation.md` (remove font tokens from the Theme field list; add a `[font]` config section), `README.md`, `AGENTS.md`.

- [x] **compositor-task-36** — `cargo clippy --workspace --all-targets --all-features` clean + `cargo test --workspace` green; grep gate `rg "theme\.font_|\.font_family|\.font_size" --glob '*.rs'` confirms no remaining theme-font reads in consumers.

### [—] Phase: Visual tuning with the user · `compositor-05` — CLOSED NOT PASSED (2026-06-22)
Source: `compositor-blur-refactor-plan.md` Phase 5
This phase is interactive — cannot be done without the user running the app.
**Closed not-passed:** the frost never reached a user-approved look. The frosted-desktop-BEHIND-the-window look the user wanted is an OS capability (macOS `vibrancy`), not cross-platform app code; within-heca frost approaches (gradient tint / image / procedural noise) only frost heca's own z=0 gradient, not the desktop. `vibrancy` is left for the user to enable on macOS. Tasks below were not completed; retained for reference if a cross-platform within-heca frost is revisited.

- [ ] **compositor-task-19** — User sets `background_blur`, `background_transparency`, gradient colors, `terminal_transparency`; confirms tiled frost looks like real frosted glass.

- [ ] **compositor-task-20** — Tune `BLUR_PASSES`/`MAX_BLUR_PX`/`background_blur_radius()` mapping if frost is too weak or too strong. Re-test until user confirms blur strength is right.

- [ ] **compositor-task-21** — User sets `terminal_floating_blur` + `terminal_floating_transparency`; confirms floating-pane text does NOT collide with tiled content behind (the 100% backdrop fix works).

- [ ] **compositor-task-22** — User resizes the window; confirms z=0 recomputes cleanly with no stale-resolution artifact.

---

## Pluggable Chrome / Plugin

> Source: `pluggable-chrome-plugin-plan.md`, `grid-ui-chrome-plan.md`
> Foundations landed: `SharedChromeState`, typed event bus, `app.on`/`app.state` host API (read/observe half),
> grid-ui widget vocabulary, shell compositing primitives, modal/dropdown overlays.
> Remaining: the architectural core — ChromeHost, providers, dynamic actions, WASM runtime.

### [ ] Phase: Formal architecture contracts · `plugin-01`
Write and ratify the formal chrome-host + provider + plugin contracts before any implementation.
Source: `pluggable-chrome-plugin-plan.md` Phase 1

- [ ] **plugin-task-01** — Write the formal `ChromeHost` contract: what a region is, what it can host, allowed contribution types (container, toolbar group, status segment, panel, overlay request). Document in `pluggable-chrome-plugin-plan.md` §3.1.

- [ ] **plugin-task-02** — Write the formal provider lifecycle model: `id()`, `supported_regions()`, `default_region()`, `movable`, `collapsible`, `build_contribution(ChromeCtx)`. Document in `pluggable-chrome-plugin-plan.md` §3.4.

- [ ] **plugin-task-03** — Write the overlay ownership and result-returning API shape: modal/dropdown lifecycle, focus trap, ESC, async result contract. Document in `pluggable-chrome-plugin-plan.md` §2.7/Phase 8.

- [ ] **plugin-task-04** — Audit and fix geometry types in chrome-facing code.
  All new chrome/container/overlay contracts must use `heca-core/src/layout/types.rs` `Rectangle`/`Point`/`Size`.
  Remove remaining legacy `heca_core::types::Rect` from chrome-facing code.
  Files: `heca/src/chrome/mod.rs`, `heca/src/sidebar/`, grep `heca_core::types::Rect`

### [ ] Phase: ChromeHost and region hosts · `plugin-02`
The central runtime that mounts/orders/moves containers across all 4 regions.
Source: `pluggable-chrome-plugin-plan.md` Phase 3

- [ ] **plugin-task-05** — Introduce `ChromeHost` struct in `heca/src/chrome/host.rs`:
  Owns registries for all 4 regions, tracks container placement and ordering, owns host-level container move/reorder, bridges plugins with the app state and action system.

- [ ] **plugin-task-06** — Introduce region hosts (`LeftSidebarHost`, `RightSidebarHost`, `TopBarHost`, `BottomBarHost`) or a single generic `RegionHost<Orientation>`. Each tracks its ordered list of mounted containers and their visibility.

- [ ] **plugin-task-07** — Implement container registration, ordering, and placement persistence.

- [ ] **plugin-task-08** — Implement host-level container move between compatible regions as a named action (not mouse-only).
  New `WmAction` variants: `MoveContainerToRegion { container_id, region }`, `ReorderContainerBefore { container_id, before_id }`.
  Must be reachable from keyboard + RPC. Follow "Adding New Actions" checklist in `AGENTS.md`.
  Files: `heca/src/input.rs`, `heca/src/handlers.rs`, `heca/src/app/registry.rs`

### [ ] Phase: Built-in provider system and WorkspacesContainer migration · `plugin-03`
Prove the provider model with the first real built-in provider before loading external plugins.
Source: `pluggable-chrome-plugin-plan.md` Phases 4–5

- [ ] **plugin-task-09** — Define the `Provider` trait: `id()`, `supported_regions()`, `default_region()`, `movable: bool`, `collapsible: bool`, `build_contribution(ChromeCtx) -> ContainerContribution`.
  Files: `heca/src/providers/mod.rs` (new)

- [ ] **plugin-task-10** — Implement `WorkspacesContainerProvider` as the first built-in provider.
  Migrates the current `heca/src/sidebar/` workspace-tree logic into the provider shape.
  The sidebar shell (already a `ChromeRegion` widget) hosts it; the provider owns tree semantics, search, DnD, row actions.
  Files: `heca/src/providers/workspaces.rs` (new), `heca/src/sidebar/` (reshape as the provider's impl)

- [ ] **plugin-task-10a** — Bridge sidebar-nav selection into shared chrome/workspaces state.
  Today the expanded sidebar highlights only `active_pane`, while sidebar navigation mutates
  `AppState.sidebar_tree.cursor/current_item()`; result: `prefix+e` → `j/k` moves the nav model
  internally but **nothing visibly changes** in the expanded sidebar. Add a shared
  `selected_row` / `selected_item` projection for `SidebarNav` mode and make the expanded
  `WorkspacesContainer` render **both**:
  - real session focus (`active_pane`)
  - sidebar-nav selection (`current_item()` / cursor)
  Preserve the settled contract: sidebar mode is **selection-driven**; main focus does **not**
  auto-follow `j/k`, but the selected workspace/column/pane must highlight visibly while in
  `InputMode::SidebarNav`.
  Source: `pluggable-chrome-plugin-plan.md` §3.3 shared UI/chrome state (`selected row ids`,
  `focus/selection`) + `plugin-task-10` WorkspacesContainer migration.
  Files: `heca/src/chrome/state.rs`, `heca/src/chrome/mod.rs`, `heca/src/sidebar/model.rs`,
  future `heca/src/providers/workspaces.rs`

### [ ] Phase: Dynamic action registry · `plugin-04`
Make the action system capable of hosting plugin actions and config-bindable dynamic actions.
Source: `pluggable-chrome-plugin-plan.md` Phase 6

- [ ] **plugin-task-11** — Evolve `ActionRegistry` to support dynamic string-based registration alongside the existing `WmAction` enum dispatch.
  Add: stable string action IDs, action metadata descriptors, `register(id, metadata, handler)`, `dispatch(id, args)`, `unregister(id)`.
  Preserve full compatibility with the existing `WmAction`-enum-based dispatch.
  Files: `heca/src/actions.rs`, `heca/src/app/registry.rs`

- [ ] **plugin-task-12** — Ensure future `config.toml` keybindings can target dynamic string action IDs (not just `WmAction` names).
  Files: `heca/src/keymap.rs`, `heca-config/src/keys.rs`

- [ ] **plugin-task-13** — Route the chrome container placement actions (`MoveContainerToRegion`, etc. from `plugin-task-08`) through the new dynamic registry as string IDs so plugins can also dispatch them.

### [ ] Phase: Write/contribute half of the host API · `plugin-05`
Add `app.actions.*`, `app.overlay.*`, `app.regions.*` to `heca/src/host.rs`.
Source: `pluggable-chrome-plugin-plan.md` §3.5 (rows 3–10)

- [ ] **plugin-task-14** — Add `app.actions.dispatch(id: &str, args: ActionArgs)` — routes through the dynamic action registry.
  Files: `heca/src/host.rs`

- [ ] **plugin-task-15** — Add `app.overlay.open_modal(spec)` + `app.overlay.open_dropdown(spec)` with async result-returning flows.
  Backed by the existing `Modal`/`Select` grid-ui widgets. The host owns the async plumbing.
  Files: `heca/src/host.rs`, `heca/src/chrome/mod.rs`

- [ ] **plugin-task-16** — Add `app.regions.left_sidebar.add_container(contribution)` + analogues for right/top/bottom + `move_container(container_id, target_region)`.
  Files: `heca/src/host.rs`

### [ ] Phase: Placeholder token system · `plugin-06`
tmux-style `${var}` tokens for use in config values, keybinding labels, and simple plugins.
Source: `pluggable-chrome-plugin-plan.md` Phase 8.1

- [ ] **plugin-task-17** — Define the token syntax (`${var}` form) and a token registry.
  Initial tokens: `${paneIndex}`, `${prevPaneIndex}`, `${paneTitle}`, `${prevPaneTitle}`, `${panesCount}`, `${paneProgram}`, `${paneCwd}`, `${columnIndex}`, `${columnTitle}`, `${columnsCount}`, `${workspaceTitle}`, `${workspaceIndex}`, `${workspacesCount}`, `${leftSidebarStatus}`, `${rightSidebarStatus}`, `${pid}`.
  Files: new `heca/src/tokens.rs` or `heca-config/src/tokens.rs`

- [ ] **plugin-task-18** — Wire token resolution into the pane info bar segment rendering — a segment text with `${paneProgram}` resolves at render time from the current `PaneRuntime`.
  Files: `heca/src/chrome/mod.rs`

- [ ] **plugin-task-19** — Wire token resolution into config/keybinding label paths where applicable.
  Files: `heca-config/src/loader.rs`

### [ ] Phase: Simple config.toml plugins · `plugin-07`
Let users define simple status/segment plugins directly in `config.toml` without Rust code.
Source: `pluggable-chrome-plugin-plan.md` Phase 8.2
Gate: `plugin-06` (placeholder tokens)

- [ ] **plugin-task-20** — Define and parse the `[[plugins]]` TOML schema:
  ```toml
  [[plugins]]
  name = "myplugin"
  placement = "bottomBar"
  weight = 100
  text = "pane: ${paneProgram}"
  ```
  Files: `heca-config/src/plugins.rs` (new), `heca-config/src/lib.rs`

- [ ] **plugin-task-21** — Implement renderer in `heca/src/chrome/` — mount config plugins as simple text contributions in the target region.

- [ ] **plugin-task-22** — Document in `README.md` + `example.config.toml`.

### [ ] Phase: WASM plugin runtime · `plugin-08`
External code-based plugins using WASM as the plugin format.
Source: `pluggable-chrome-plugin-plan.md` Phase 9
Gate: `plugin-02`, `plugin-03`, `plugin-04`, `plugin-05` must all be complete.

- [ ] **plugin-task-23** — Design the WASM host API/facade — the same `app.on`/`app.state`/`app.actions`/`app.overlay`/`app.regions` surface marshalled across the WASM boundary via a defined ABI.

- [ ] **plugin-task-24** — Add plugin discovery and loading lifecycle: scan `~/.config/heca/plugins/*.wasm`, lifecycle hooks: `activate`, teardown, optional reload.

- [ ] **plugin-task-25** — Add event bus bridge — marshal typed `ChromeEvent`s across the WASM boundary to plugin handlers.

- [ ] **plugin-task-26** — Add region contribution API — WASM plugin returns a container description; the host mounts it via `ChromeHost`.

- [ ] **plugin-task-27** — Add plugin action registration API — WASM plugin registers string action IDs; host dispatches back to the plugin on invocation.

### [ ] Phase: Multi-region proof and config integration · `plugin-09`
Prove the architecture is genuinely general-purpose with a second provider and config-bindable plugin actions.
Source: `pluggable-chrome-plugin-plan.md` Phases 10–11

- [ ] **plugin-task-28** — Add a second built-in provider (e.g. git status in the right sidebar or bottom bar) to prove multi-container coexistence, ordering, and collapse state.

- [ ] **plugin-task-29** — Ensure plugin-provided actions participate fully in `config.toml` keybindings, command palette, and RPC.

- [ ] **plugin-task-30** — Add action discovery UX — a way to list/browse all registered actions (built-in + plugin) for config authoring.

---

## Grid UI Widget Library

> Source: `grid-ui-chrome-plan.md`, `PLAN.md` grid-ui backlog
> Core widget vocabulary is largely built. Remaining: scroll primitive, Pane shell header, more widgets, Nerd-Font icons, showcase coverage, bloom effects, crate debt.

### [x] Phase: Scroll / list primitive · `gridui-01`
An embeddable scroll region for sidebar docks and list views.
Note: renderer `PushClip`/`PopClip` is ALREADY implemented in `heca-renderer/src/scene.rs` — this gate is closed.
**Status (2026-06-22):** ✅ DONE — on `feature/gridui-01-scroll-region` (PR #177). Reuses the whole-page scroll pattern (shift subtree bounds + clip) inside a widget: bakes `-scroll_offset` into the children's bounds so paint, hit-testing, and DnD all see the visual position (bounds === drawn), and clips to the viewport via `PushClip`. A new post-order `Component::on_layout` hook (layout engine) resets the baked offset on a fresh layout so the shift never compounds — this is what lets an embeddable scroll viewport reuse the page-scroll mechanism without owning the layout/scroll cycle. v1 is vertical-only, multi-child column: wheel (~10% of viewport/notch, viewport-proportional so a small sidebar doesn't overshoot) + draggable thumb (theme-accent grip that brightens on hover/drag, wider 16px grab lane, thumb radius from the `Theme::control_radius()` token), offset exposed as `Signal<f32>`; `Event::Scroll` has no position so the region hover-gates the wheel (tracks hover via `PointerMoved`) so an inline region doesn't swallow every wheel event in the tree. **Focus-gated keyboard scroll** (focusable; `Event::Key` goes to the focused component only, so the gate is just `focused`): `ArrowUp`/`ArrowDown` + `j`/`k` (with/without `Ctrl`) step, `Home`/`End` jump to top/bottom, with a focus ring; a focused child (e.g. `Input`) keeps its keys. **Scroll-into-view API** for keyboard cursor following: `ensure_visible(visual_rect)` (minimal scroll, recovers natural position internally via the baked shift so the host never tracks the offset) + `scroll_to_child(index)` convenience — the widget-side prep for mounting the sidebar tree and having `SidebarNav` keep the cursor on screen. Showcase demo + full `docs/widgets.md` reference added. Same radius-token fix applied to `MarkerGroup`'s marker bar (`control_radius()` instead of hardcoded `bar_w/2`). Grid-ui 47 tests pass, clippy 0. (An earlier draft used a renderer `Translate` primitive; pivoted to bounds-shift per review — no second scroll mechanism, DnD works while scrolled.) **Follow-ups are tracked as open tasks below** (`gridui-task-29`…`gridui-task-34`): horizontal scroll, a dedicated scrollbar color token, PageUp/PageDown keys, nested-region hit-testing, the sidebar scroll wiring, and the pick-a-scrollable-region mode — so the deferral is tracked, not buried in prose.

- [x] **gridui-task-01** — Build `ScrollRegion` widget in `heca-grid-ui/src/widgets/scroll_region.rs`.
  Uses `PushClip`/`PopClip` for content clipping + bounds-shift (the page-scroll pattern) for the offset.
  Exposes `scroll_offset: Signal<f32>` (vertical) + `scroll_to(v)` (clamps + bakes the shift into bounds).
  Wheel handling (`Event::Scroll`, viewport-proportional step) + draggable theme-colored (`muted`) scrollbar thumb, auto-shown on overflow; hover-gated so it doesn't swallow the page wheel.
  Files: `heca-grid-ui/src/widgets/scroll_region.rs`, `heca-grid-ui/src/widgets/mod.rs`, `heca-grid-ui/src/lib.rs` (exports + prelude), `heca-grid-ui/src/component.rs` (`Component::on_layout` hook), `heca-grid-ui/src/layout.rs` (call `on_layout` post-order in `assign`), `heca-renderer/examples/showcase.rs` (demo).
  Update `docs/widgets.md` + add showcase demo section.

**Follow-ups (carved out of v1 — tracked, not deferred-to-prose):**

- [ ] **gridui-task-29** — `ScrollRegion`: add **horizontal scroll** (axis-aware offset + thumb; reuse the bounds-shift mechanism). The widget is vertical-only today. Files: `heca-grid-ui/src/widgets/scroll_region.rs`.
- [ ] **gridui-task-30** — `ScrollRegion`: add a **dedicated scrollbar color token** to `Theme` (config-overridable, `heca-theme/src/themes/*.toml` + `heca-config/src/settings.rs` `[appearance]` override + `heca-grid-ui/src/theme.rs`); the thumb currently reuses `theme.accent`. Alphas stay widget-internal (consistent with `MarkerGroup`).
- [ ] **gridui-task-31** — `ScrollRegion`: add **PageUp/PageDown** keys (page = one viewport). Blocked on `GridKey` having no page keys — add `PageUp`/`PageDown` to the `GridKey` enum (`heca-grid-ui/src/component.rs`) + the host key mapping (`heca-renderer/examples/showcase.rs` and the app's input path), then handle them in `ScrollRegion::event`.
- [ ] **gridui-task-32** — `ScrollRegion`: **nested-region wheel hit-testing**. Today the wheel is hover-gated for a single inline region; with multiple nested scroll regions the host must find the innermost scrollable under the cursor and route the wheel to it. Likely a host-side helper walking the tree. Files: `heca-grid-ui/src/widgets/scroll_region.rs` + host wiring.
- [ ] **gridui-task-33** — **Sidebar scroll wiring** (integration, separate phase): mount the sidebar workspace tree inside a `ScrollRegion`; the `SidebarNav` cursor handler (`j`/`k`, selection-driven) calls `ScrollRegion::ensure_visible(selected_row.bounds)` (or `scroll_to_child`) after moving the cursor to keep it on screen. Selection stays container-owned via `Item::marker`/`state`. The widget API (`ensure_visible`/`scroll_to_child`) is already in place from `gridui-01`. Files: `heca/src/sidebar/*`, `heca/src/chrome/*`.
- [ ] **gridui-task-34** — **Pick-a-scrollable-region mode** (app-level, larger): `prefix+<key>` → a `KeyHint` overlay enumerating every scrollable region → pick one → enter a sticky scroll/nav mode bound to it (`j`/`k`/arrows/PgUp/PgDn/Home/End → `WmAction::ScrollFocused`). Needs the full "Adding New Actions" checklist: `WmAction::EnterScrollSelect` + `WmAction::ScrollFocused`, `InputMode::ScrollSelect` + `InputMode::Scroll`, action-registry registration, default binding + descriptor, RPC, `action_policy()` classification, `default-keybindings.toml`/`README.md`. Only needed once multiple scrollable regions compete for `j`/`k`. Run `/grill-me` first (key choice, sticky vs one-shot, pick→focus vs pick→mode). Files: `heca/src/input.rs`, `heca/src/actions.rs`, `heca/src/app/registry.rs`, `heca/src/handlers.rs`, `heca/src/app_state.rs`.

### [ ] Phase: Pane shell widget — header and tabs · `gridui-02`
The `Pane` widget is today a bracket container without a header. Add HUD header, tab bar, and expose the inner content rect properly.

- [ ] **gridui-task-02** — Add optional header slot to `Pane` — a title/status bar area above the content rect. Header can contain: title `Label`, `StatusDot`, `IconButton` actions.
  Files: `heca-grid-ui/src/widgets/pane.rs`

- [ ] **gridui-task-03** — Add optional tab bar slot to `Pane` — for multi-document pane types.
  Files: `heca-grid-ui/src/widgets/pane.rs`

- [ ] **gridui-task-04** — Build `CornerBrackets`/`Reticle` component for the focused-pane indicator.
  Files: `heca-grid-ui/src/widgets/corner_brackets.rs`
  Update `docs/widgets.md` + showcase.

- [ ] **gridui-task-05** — Build `StatusBar` component (bottom chrome band: mode label, git info, notifications).
  Files: `heca-grid-ui/src/widgets/status_bar.rs`
  Update `docs/widgets.md` + showcase.

### [ ] Phase: Nerd-Font icon widget · `gridui-03`
Add `NfIcon` for program/language logos (nvim, docker, lazygit, python, rust) that Phosphor Duotone doesn't cover.

- [ ] **gridui-task-06** — Verify and record the license for "Symbols Nerd Font Mono" glyph subset before embedding. Confirm OFL/MIT and note it in a code comment.

- [ ] **gridui-task-07** — Embed the Symbols Nerd Font Mono glyph ranges (~1–2 MB) in `heca-renderer/src/font.rs` as a named font family/role. Do NOT embed a full patched font (that would conflict with the user's terminal font).
  Files: `heca-renderer/src/font.rs`

- [ ] **gridui-task-08** — Build `NfIcon` widget in `heca-grid-ui/src/widgets/nf_icon.rs`.
  Single-layer (flat/monochrome) widget — contrast with `Icon` which is Phosphor Duotone.
  Reference by codepoint (`NfIcon::from_codepoint(u32)`) + a small curated `NfGlyph` enum for the commonly-used ones.
  Files: `heca-grid-ui/src/widgets/nf_icon.rs`, `heca-grid-ui/src/widgets/mod.rs`

- [ ] **gridui-task-09** — Wire `NfIcon` into the program catalog: let `[program.<id>].icon` optionally name an NF glyph.
  Pane title + sidebar card pick it up through the shared `pane_info_view` path.
  Files: `heca-config/src/appearance.rs` (`ProgramConfig`), `heca/src/chrome/mod.rs`

- [ ] **gridui-task-10** — Update `docs/widgets.md` + add showcase icon strip entries for NF glyphs.

### [ ] Phase: Showcase coverage and visual regression tests · `gridui-04`

- [ ] **gridui-task-11** — Audit the showcase — identify any widgets lacking a showcase demo section. Add missing demos.
  Files: `heca-renderer/examples/showcase.rs`

- [ ] **gridui-task-12** — Add snapshot/visual tests for key widget states (normal, hover, active, disabled, focused).
  If headless GPU is unavailable: use `DrawCommand` scene comparison as a proxy.

### [ ] Phase: Bloom and custom draw effects · `gridui-05`

- [ ] **gridui-task-13** — Implement offscreen bloom pipeline in `heca-renderer`: bright-pass filter → Gaussian blur → additive composite over the scene.
  Files: `heca-renderer/src/bloom.rs` (new), `heca-renderer/src/lib.rs`

- [ ] **gridui-task-14** — Add `DrawCommand::Custom(Box<dyn CustomDraw>)` escape hatch for one-off GPU effects that don't fit the standard scene model.
  Files: `heca-grid-ui/src/scene.rs`

### [ ] Phase: Additional widgets · `gridui-06`

- [ ] **gridui-task-15** — `Item` DnD reorder: wire the existing `DragExt` framework to allow items within `ItemGroup` to be reordered by drag. Add `description` field + `custom_background` option to `Item`.
  Files: `heca-grid-ui/src/widgets/item.rs`, `heca-grid-ui/src/widgets/item_group.rs`

- [ ] **gridui-task-16** — Multi-select `Select`: extend the `Select` widget to allow multiple simultaneous selections; expose a `Vec<usize>` value signal.
  Files: `heca-grid-ui/src/widgets/select.rs`

- [ ] **gridui-task-17** — `HUD Frame`: a floating HUD-style bordered container for overlays/panels.
  Files: `heca-grid-ui/src/widgets/hud_frame.rs`

- [ ] **gridui-task-18** — `Metric Row`: a compact key/value row for status displays (git stats, process metrics, etc.).
  Files: `heca-grid-ui/src/widgets/metric_row.rs`

- [ ] **gridui-task-19** — `Search Input`: a search/filter text input with clear button and debounced `on_change` callback.
  Files: `heca-grid-ui/src/widgets/search_input.rs`

- [ ] **gridui-task-20** — `Accordion`: collapsible section with animated open/close transition.
  Files: `heca-grid-ui/src/widgets/accordion.rs`

### [ ] Phase: Grid-UI crate-review debt · `gridui-07`
Fix all known code-quality issues from the two Rust crate reviews.

- [ ] **gridui-task-21** — Fix `badge.rs` `unreachable!()` in a reachable match arm — replace with `debug_assert!` or proper error handling.

- [ ] **gridui-task-22** — Add `[workspace.lints]` / per-package lints to `heca-grid-ui/Cargo.toml`.

- [ ] **gridui-task-23** — Replace `#[allow]` with `#[expect]` in `component.rs` (Rust 1.81+).

- [ ] **gridui-task-24** — Add `#![deny(missing_docs)]` to `heca-grid-ui/src/lib.rs` and fill all doc-comment gaps.

- [ ] **gridui-task-25** — Add `#[non_exhaustive]` to public enums that will grow over time (e.g. `DrawCommand`, `GlowLevel`, `Intensity`).

- [ ] **gridui-task-26** — Eliminate hot-path allocations: `Input::chars_vec`, `CommandPalette::results`, scene `to_vec()`/`clone()`. Use pre-allocated buffers or slice references.

- [ ] **gridui-task-27** — Extract a shared hover/flash/animation helper to eliminate ~200 lines duplicated across `Button`/`Toggle`/`Checkbox`/`IconButton`/`Item`/`Row`/`RailCell`.
  Files: new `heca-grid-ui/src/widgets/animation_helpers.rs` or inline module

- [ ] **gridui-task-28** — Increase widget test coverage from ~3% to meaningful coverage of all widget state transitions (normal → hover → pressed → disabled → focused).

---

## App / Chrome

> Source: `PLAN.md` (near-term active + deferred + foundation gaps)
> App-level work: niri parity, render.rs refactor, appearance/zoom/font controls, sidebar wiring, pane numbering, damage-region optimization.

### [ ] Phase: niri layout parity audit · `app-01`

- [ ] **app-task-01** — Catalog every animation niri supports (open, close, move, resize, workspace-switch, view-scroll, overview). Note spring/easing config and the off-switch. Map each to what heca does today. Write a gap-list document.
  Goal: the catalog IS the deliverable — no implementation yet.
  Files: `niri-compatibility-review.md` (update status table), new `docs/animation-gap-list.md`

- [ ] **app-task-02** — Confirm live whether adding/removing a column reflows existing column widths.
  Test: resize a column manually → add another column → resize the window → does the first column keep its size?
  If it reflows: switch `ScrollingSpace` to store each column's width and only recompute the changed one (the niri behavior).
  Files: `heca-core/src/layout/scrolling.rs` (if fix needed)

- [ ] **app-task-03** — Re-audit remaining unverified niri compat items: prefix-mode timeout, modifier read from event vs cache, forwarding the literal prefix key on double-press, viewport/chrome coordinate math, single-animation model, column-width double-caching.
  Run each against the live app. Update `niri-compatibility-review.md` status table.

### [ ] Phase: Split `render.rs` into a `render/` folder · `app-02`
Mechanical refactor — own PR, do not mix with feature work. Target: `render_frame` shrinks to ~250 lines.

- [ ] **app-task-04** — Extract geometry helpers into `heca/src/app/render/geometry.rs` — pane/scissor/textbox math.
- [ ] **app-task-05** — Extract the terminal pass into `heca/src/app/render/terminal.rs` — `TerminalRenderPassContext` + `render_terminal_mount`. (Partly done in `terminal_render.rs` — re-scope remainder.)
- [ ] **app-task-06** — Extract selection overlay into `heca/src/app/render/selection.rs` — `build_selection_overlay`, `selection_overlay_for_pane`, `status_mode_parts` + their tests.
- [ ] **app-task-07** — Extract chrome flush into `heca/src/app/render/overlays.rs` — collapsed rails, drag ghost, pane-select labels, `render_chrome` call.
- [ ] **app-task-08** — Extract tiled + floating pane passes into `heca/src/app/render/panes.rs`. Use a granular-field context struct (the `TerminalRenderPassContext` pattern) — never `&mut AppState` inside this file, because `scene_view` borrows `state.compositor` across the function body.
- [ ] **app-task-09** — Keep `heca/src/app/render/mod.rs` as the frame orchestrator + `update_session_viewport`. Verify by running the app.

### [ ] Phase: App-wide zoom and font-size controls · `app-03`
User-facing appearance controls. Deferred but must not be forgotten.

- [ ] **app-task-10** — App-wide zoom: whole-UI zoom increase/decrease (like the showcase `nudge_zoom` mode).
  New `WmAction` variants: `ZoomIn`, `ZoomOut`, `ZoomReset`. Follow "Adding New Actions" checklist (11 steps) in `AGENTS.md`.
  Files: `heca/src/input.rs`, `heca/src/handlers.rs`, `heca/src/app/registry.rs`, `heca-config/src/theme.rs` (default binding)

- [ ] **app-task-11** — Chrome/app font-size increase/decrease — adjusts `Theme.font_size`.
  New `WmAction` variants: `IncreaseFontSize`, `DecreaseFontSize`. Prefix-bound per the tmux-prefix keybinding rule.
  Follow "Adding New Actions" checklist.

- [ ] **app-task-12** — Terminal font-size increase/decrease — adjusts `Theme.terminal_font_size` independently of the UI font.
  New `WmAction` variants: `IncreaseTerminalFontSize`, `DecreaseTerminalFontSize`.
  Follow "Adding New Actions" checklist.

### [ ] Phase: Pane numbering · `app-04`
Deterministic `prefix+<ws>+<pane>` jump-to-pane. Agreed and spec'd — see memory `heca-pane-numbering-spec`.

- [ ] **app-task-13** — Add pane-number computation per workspace: deterministic 1–9 assignment within each workspace's visible panes.
  Files: `heca-core/src/layout/workspace.rs` or `heca/src/chrome/mod.rs`

- [ ] **app-task-14** — Display pane numbers on sidebar cards.
  Files: `heca/src/chrome/mod.rs` (sidebar card rendering), `heca-grid-ui` if a number badge widget is needed

- [ ] **app-task-15** — Add `WmAction::FocusPaneByNumber { ws: usize, pane: usize }` + handler + chord binding (`prefix+<ws-digit>+<pane-digit>`).
  Follow "Adding New Actions" checklist.
  Files: `heca/src/input.rs`, `heca/src/handlers.rs`, `heca/src/app/registry.rs`, `heca-config/src/theme.rs`

### [ ] Phase: Workspace drag-to-reorder in the sidebar · `app-05`
Complete sidebar DnD — workspaces can be dragged to reorder. Panes and columns already drag.

- [ ] **app-task-16** — Add `AppDragPayload::Workspace(ws_idx)` to the DnD payload type.
  Files: `heca/src/mouse/` (wherever `AppDragPayload` is defined)

- [ ] **app-task-17** — Make workspace rows in the sidebar `.draggable(Workspace(idx))` using the existing `DragExt` framework.
  Files: `heca/src/chrome/mod.rs` (workspace row building)

- [ ] **app-task-18** — Add drop-target logic: dropping a workspace row above/below another reorders them.
  Files: `heca/src/mouse/surface_left.rs` (or equivalent drag dispatch)

- [ ] **app-task-19** — Add `WmAction::MoveWorkspace { from_idx: usize, to_idx: usize }` + handler.
  Follow "Adding New Actions" checklist.

### [ ] Phase: Sidebar wiring and collapsed rail · `app-06`

- [ ] **app-task-20** — Wire sidebar buttons (`+w` workspace, `+c` column, `+p` pane) — `button_hitboxes` are defined but click handlers are not connected.
  Files: `heca/src/sidebar/hit_test.rs`, `heca/src/mouse/surface_left.rs`

- [ ] **app-task-21** — Migrate the collapsed sidebar rail from legacy hand-drawn + `sidebar_hit_test` to `RailCell`/`ChromeRegion` grid-ui widgets.
  Files: `heca/src/sidebar/render.rs`, `heca/src/mouse/render.rs`
  Coordinate with `theming-task-29`.

### [ ] Phase: Damage-region render optimization · `app-07`
Let the compositor's preserved scene texture actually preserve things — partial repaints instead of full-frame clears.
Note: the chrome flash is already fixed; this is the deferred optimization.
Risk: rendering correctness (transparent-pane double-blend, stale pixels). Do AFTER the app is otherwise stable.

- [ ] **app-task-22** — Stop the unconditional per-frame scene clear in `heca/src/app/render.rs` — preserve the compositor's scene texture between frames.
- [ ] **app-task-23** — Make pane rendering damage-aware — only re-render changed pane regions using dirty signals from `SharedChromeState`/terminal snapshots.
- [ ] **app-task-24** — Scissor the clear + each renderer to the union damage rect against the preserved scene.
- [ ] **app-task-25** — Re-introduce an app-side damage gate using `GridRenderer::set_damage` + the compositor preserved scene.
- [ ] **app-task-26** — Verify rendering correctness by running the app: check for transparent-pane double-blend artifacts and stale pixels.

### [ ] Phase: Fix NSWindow vibrancy console warning · `app-08`
Benign but noisy — spam from vibrancy adding `NSVisualEffectView` to `NSThemeFrame`.

- [ ] **app-task-27** — Investigate and apply the correct macOS approach to silence this warning without disabling vibrancy.
  Files: `heca/src/main.rs` or macOS platform init code
  Do NOT disable vibrancy as a workaround.

---

### [~] Phase: Sidebar/chrome small leftovers (F4.4 / F4.5) · `app-10`
Small remaining pieces from the old F4.4 (marker/rail widget) and F4.5 (sidebar drag-and-drop) work.
The widgets and the drag framework already exist; these are the leftover hook-ups.

- [x] **app-task-29** — F4.4 widget migration. DONE 2026-06-22. Most was already merged (`column_view`
  is `MarkerGroup`; `pane_card` is a signal-driven `Row`). The remaining piece — the active-workspace
  wash, still structural (rebuild-only) — is now signal-driven: `DockFrame` gained `.active(bool)` +
  `.active_state()` and paints a theme-driven accent wash; `ChromeSignals.ws_active` is synced in
  `sync_chrome_signals` like `col_active`. Also tokenized the two baked-in alphas into new theme tokens
  `active_wash_alpha` (0.11) + `card_background_alpha` (0.02) across `heca-theme`, `heca-grid-ui` Theme,
  and `app_theme_to_gui_theme`. Showcase + `docs/widgets.md` + `theming-documentation.md` + `README.md`
  updated.
  Files: `heca/src/chrome/mod.rs`, `heca-grid-ui` (`DockFrame`, `Theme`), `heca-theme`

- [ ] **app-task-30** — F4.4 column-level pick keycaps: pane pick keycaps already work; columns have no
  pick candidates today, so this needs NEW candidate computation in `heca/src/app/input.rs`, then project
  the candidates onto a per-column hint signal each frame. `KeyHint` stays universal — do NOT make it
  column-specific (memory `grid-ui-keyhint-universal`).
  NOTE (2026-06-23): the per-column `KeyHint` + candidate infra now exists (`ChromeSignals.col_hint`,
  `WorkspacesContainerState.col_pick_candidates`, built in `app-12`). This task is now just adding the
  *column-as-pick-target* candidate computation for move/swap/take that act ON a column.
  Files: `heca/src/app/input.rs`, `heca/src/chrome/mod.rs`

- [x] **app-task-31** — F4.5 drop-onto-workspace. DONE 2026-06-22. Scope narrowed with the user: the only
  real gap was **dropping a pane onto a workspace → move it into that workspace** (the one way to reach an
  *empty* workspace, since empty columns can't exist and pane-on-pane already covers column moves).
  `target_accepted_by` now lets a pane drag accept `Workspace` targets, and `accept_drop` routes a workspace
  target through `place_pane_at_sidebar_target`'s `Workspace` arm, which creates a **new column** at the end
  of the workspace (a pane dropped on a workspace starts its own column; appending into an existing column
  is what dropping on a column/pane card is for). The drop indicator already handled workspace targets
  (column-drag path), so it lights up for pane drags too.
  Files: `heca/src/mouse/surface_left.rs`, `heca/src/chrome/mod.rs`

### [x] Phase: Keyboard move-to-target picks + rename override + plugin-observable state · `app-12`
DONE 2026-06-23 (PR #178). Keyboard counterparts to the sidebar drag moves, plus making picks/renames
observable by plugins.

- [x] **app-task-33** — Keyboard "move to" picks via universal `KeyHint`: move active **column → workspace**
  (`prefix+c`), active **pane → workspace** (`prefix+g`), active **pane → column** (`prefix+Shift+c`, freed
  from `rename_column`). New `InputMode::WorkspacePick`/`ColumnPick`, candidate collectors (exclude the
  current workspace for the workspace picks; pane→column spans **all** workspaces and stacks into the target
  column), per-target `KeyHint` projection (`ws_hint`/`col_hint`), and resolve handlers dispatching the
  existing `MoveColumnToWorkspace`/`MovePaneToWorkspace`/`MovePaneToColumn`. `KeyHint` gained `.color()`,
  `.offset_y()`, `TopRight`; `Color::with_alpha_f32()` added; `DockFrame` active wash is signal-driven.
  Cross-workspace move-pane-to-column now stacks (`join_existing`) instead of making a new column.
- [x] **app-task-34** — Rename **custom-name override**: `Pane.custom_name` wins over the process-derived
  title everywhere (sidebar card, reactive sync, in-pane info bar/header); icon still tracks the process;
  empty rename clears it.
- [x] **app-task-35** — Plugin-observable state (per the Architecture principle): custom name → store signal
  + `PaneCustomNameChanged` + `host.pane_custom_name()`; in-progress pick → `PendingPick` (kind + label +
  prompt, text sourced from each action's `ActionDescriptor`) + `PendingPickChanged` + `host.pending_pick()`.
  Files: `heca/src/{app_state,handlers,actions,host}.rs`, `heca/src/app/{input,selection,render}.rs`,
  `heca/src/chrome/{mod,state,events}.rs`, `heca-grid-ui/src/{color,widgets/key_hint,widgets/dock_frame}.rs`

### [ ] Phase: Right-click context menu · `app-11`
Mouse-driven action menu — the pointer counterpart to the keyboard pick/rename actions.

- [ ] **app-task-32** — Right-click contextual menu for chrome actions. A new `ContextMenu` widget in
  `heca-grid-ui` (a floating, keyboard-navigable list of action entries, reusing `Surface`/`Item`/the
  overlay/scissor plumbing), opened on right-click hit-test over a sidebar pane / column / workspace (and
  later a content pane). Entries route through the existing `ActionRegistry` (rename, move-to-workspace,
  move-to-column, close, delete, …) so mouse + keyboard + RPC stay one code path. Needs: the widget +
  showcase + `docs/widgets.md`; right-click hit-testing in `heca/src/mouse/`; an open/close `InputMode` or
  overlay state; per-target entry sets. Design first (scope the widget + menu model) before building.
  Files: `heca-grid-ui/src/widgets/` (new), `heca/src/mouse/`, `heca/src/chrome/`

---

## AI Agent Integration

> Source: `agent-integration/agent-integration-plan.md`, `agent-integration/agent-integration-tasks.md`
> **Status: PARKED** — research is complete, design is locked, but implementation is gated on the Pluggable Chrome / Plugin arc (`plugin-01` through `plugin-05`) being done first.
> Do NOT start this track until those phases are complete.

### [⏸] Phase: Agent status tracking and sounds · `agents-01`
Per-pane structured `AgentStatus` sourced from each AI agent's lifecycle hooks, emitted on the existing event bus, displayed in the pane info bar and sidebar cards.

- [⏸] **agents-task-01** — Define `AgentDriver` trait + registry — the strategy-pattern seam for per-agent transports (in-band OSC vs AF_UNIX side-channel).

- [⏸] **agents-task-02** — Built-in driver for Claude Code: parse `terminalSequence` OSC sequences to derive `AgentStatus` (Working / WaitingForInput / WaitingForPermission / Finished / Error / Compacting / SubagentRunning).

- [⏸] **agents-task-03** — Built-in driver for Codex: parse native OSC 9 status emission.

- [⏸] **agents-task-04** — Built-in driver for pi: read from an AF_UNIX side-channel socket.

- [⏸] **agents-task-05** — Wire `PaneRuntime.agent: Option<AgentStatus>` into `SharedChromeState`. Emit `pane.agent.changed` on the existing typed event bus — plugins see this via `app.on("pane.agent.changed", handler)`.

- [⏸] **agents-task-06** — Display agent status in the pane info bar (as a segment) and on sidebar cards.

- [⏸] **agents-task-07** — Transition sounds via `rodio` — configurable audio cues for status transitions (e.g. WaitingForInput → play attention chime). Must be opt-in via config.

---

## Dependency order summary

```
theming-01 → theming-02 → theming-03 → theming-04 → theming-05
                                     ↓
                              compositor-01 → compositor-02 → compositor-03 → compositor-04 → compositor-04b → compositor-04c → compositor-05

plugin-01 → plugin-02 → plugin-03 → plugin-04 → plugin-05 → plugin-08
                ↓               ↓
          plugin-06 → plugin-07    plugin-09

terminal-01   (independent)
terminal-02a → terminal-02        (run-level shaping gates the ligature setting)
terminal-03 → terminal-09        (protocol hooks gate image rendering)
terminal-04   (independent, do soon)
terminal-05   (gate: plugin-02 for the formal pane-shell boundary)
terminal-06 → terminal-07        (selection gates clipboard)

gridui-01     (independent — clip is already done)
gridui-02     (independent)
gridui-03     (independent)

app-01        (independent — just run the app)
app-02        (independent — own PR)
app-07        (do LAST — high risk, do after app is stable)
app-10        (independent — F4.4/F4.5 leftovers; app-task-29 done, 30/31 open)

agents-01     (gated on plugin-01 through plugin-05)
```
