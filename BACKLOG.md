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

> Source: terminal design, now consolidated into this backlog.
> Real PTY terminal using `portable-pty` + `wezterm-term` + `cosmic-text` is live. Core (Phases 0–5) is shipped.
> **Shipped**: dirty-region rendering, host scrollback, ligature policy, OSC 8 + linkify **hyperlink
> open** across all surfaces (mouse / keyboard / selection / context menu), configurable **bell**
> (attention / visual / audible), and **scrollback search** (`/` in selection mode). Phase
> `terminal-08` is complete.

> **RESUME HANDOFF (2026-07-01) — read first.**
> **Branch `feat/terminal-images`** — synced with `origin/main` (theming PR #211 merged in, commit
> `e3f33cb`); builds, `heca-core` 84/84, clippy 0. A PR to `main` is being opened.
> **Done this session (committed on the branch):**
> - **`terminal-09` inline images** — Sixel + iTerm2 `OSC 1337` + Kitty graphics, any tool. Capture
>   (`heca-core/src/backend/terminal/engine.rs` `collect_row_graphics` + decode cache), renderer
>   (`heca-renderer/src/image.rs` + `image.wgsl`), app integration + damage (`terminal_render.rs`).
> - **Yazi fixed** — two non-obvious fixes: PTY pixel size (`TIOCSWINSZ`) in `pty.rs`, and
>   **`TERM_PROGRAM=WezTerm` identity** in `pty.rs` so Yazi picks the iTerm2 protocol (no WezTerm
>   release implements Kitty Unicode placeholders — Yazi's fallback — so identity is the real fix).
> - **Retina-crisp images** — physical px (`cell × scale`) via `PaneBackend::set_scale_factor`.
> - **`terminal-07`** — OSC 52 clipboard write (`OscClipboard` in engine) + bracketed-paste-aware
>   `PaneBackend::paste`. Copy/paste already worked.
> - **`terminal-04`/`05`/`06`** — verified already covered/implemented, marked done.
> - **`terminal-task-25`** — `appearance.terminal.images` config toggle.
> - Kitty Unicode placeholders **deferred to the Neovim GUI** (see `neovim-plan.md`).
> **Method reminder (learned the hard way this session): VERIFY before concluding.** Don't generalize
> from one file/one tool; run the probe/test/build first. (Cost real time on Yazi + the worktree.)
> **Remaining terminal work (not blocking):**
> - **`terminal-10` per-pane + whole-app font zoom** — **DONE + merged (PR #213).** See the font-zoom
>   phase below for the full contract.
> - **Image polish** — **DONE (branch `feat/terminal-image-polish`):** per-image row-range damage
>   (`terminal-task-23`) + animated GIF/APNG/`AnimRgba8` (`terminal-task-24`). See the image phase.
> - **`terminal-task-07`** manual validation matrix — **DONE (2026-07-02), validated in-app by the user.**
> **Also pending (needs a human at the keyboard):** verify retina image sizing + animated-GIF playback
> in-app.

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

### [x] Phase: Dirty-region terminal rendering · `terminal-01`
Render only changed terminal rows instead of the full pane every frame. This phase assumes `terminal-00` has already made row damage visible and safe by preserving unchanged terminal content across frames.

> **DONE — delivered by the retained foundation (`terminal-00b/00c`); verified 2026-06-29.**
> The backend emits row damage during normal output (`take_terminal_damage` → `Rows`, coalesced;
> `Full` only on resize/viewport-move). `retained_damage_to_apply` passes `Rows` through,
> `render_terminal_layer_update` re-renders only the damaged rows into the scratch and
> `terminal_damage_copy_bands` copies only those bands into the per-pane retained layer (unchanged
> rows are never touched), and `render_terminal_lines` iterates only the dirty ranges. Covered by
> the `retained_damage_*` + `terminal_damage_copy_bands` tests in `terminal_render.rs`.

> **Deliberately deferred — SEPARATE from the scrollback feature.** This is a pure
> performance optimization, not a prerequisite for scrollback. The scrollback work
> (`terminal-01a`/`01b`) intentionally forces `TerminalDamage::Full` on every
> viewport movement (a full pane repaint while scrolling) and is fully usable that
> way; `terminal-01` only makes that movement cheaper. Do it AFTER the scrollback
> feature lands and the app is otherwise stable. Sibling of the frame-wide
> compositor damage optimization `app-task-22` (phase `app-07`) — same "repaint
> only what changed" spirit, different layer (terminal rows vs the whole scene).

- [x] **terminal-task-01** — Implement dirty-row rendering in `heca-renderer/src/terminal.rs`. **DONE.**
  Consume the already-plumbed `TerminalDamage` and redraw only dirty visible rows
  into the retained terminal content path. Fall back to full redraw when damage
  says `Full`. (Implemented as part of the retained-content foundation; see phase note.)
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

### [x] Phase: Backend and renderer tests · `terminal-04`
Close the test gap before selection/clipboard adds more moving parts.

- [x] **terminal-task-05** — Backend lifecycle tests. **DONE (already covered).**
  `heca-core/src/backend/terminal.rs` tests cover init→snapshot, resize→dimensions,
  initial-full-then-none + resize-forces-full damage, input→shell, output→row
  damage, grapheme preservation, exit detection (3 tests), nvim TUI bg cells, and
  bash/zsh shell-integration. Plus the new OSC52 / bracketed-paste / sixel /
  image-decode engine tests.

- [x] **terminal-task-06** — Renderer tests. **DONE (already covered).** 10 tests in
  `heca-renderer/src/terminal.rs` (color mapping via `run_push_cell`, color
  boundaries, multibyte/wide cells, default-bg match, font-family resolution,
  surface-alpha), the `clip` module tests (content-rect clipping/culling), the
  app-side row-invalidation policy (`retained_damage_*` + `terminal_damage_copy_bands`),
  plus the new image geometry + `image.wgsl` validation tests.

- [x] **terminal-task-07** — Manual validation matrix. **DONE (2026-07-02) — validated in-app by
  the user:** shell prompt, long output scroll, nvim, truecolor, Unicode fallback, pane resize,
  mouse-enabled TUI all confirmed working. Yazi image preview works (iTerm2 path — see `terminal-09`
  Yazi resolution); animated GIF/APNG playback verified via ranger + raw OSC 1337.

### [x] Phase: Pane-shell hosting contract · `terminal-05`
Formally mount the terminal as content inside a `heca-grid-ui` Pane shell.
The shell owns outer chrome (borders, title, focus ring, content rect, clip). The terminal host owns PTY/snapshot/render/input.
**DONE** (verified 2026-07-01 — was already implemented; this corrects the stale status).

- [x] **terminal-task-08** — Pane-shell contract. **DONE (embodied in code + this note).**
  Shell owns chrome (border/title/focus ring/content rect/clip), terminal host owns
  PTY/snapshot/render/input. Realized by `TerminalPaneShell` +
  `paint_terminal_pane_shell` (`terminal_render.rs`).

- [x] **terminal-task-09** — Render inside a grid-ui `Pane`. **DONE.**
  `paint_terminal_pane_shell` mounts the content inside `UiPane::new()` (grid-ui
  `Pane` widget); the terminal content is blitted into the shell's content rect.
  Files: `heca/src/app/terminal_render.rs`

- [x] **terminal-task-10** — Reflect `idle`/`running`/`error` from `PaneRuntime`.
  **DONE.** `heca/src/chrome/mod.rs` maps `ProcessStatus` to distinct `StatusDot`s
  (Idle→offline, Running/Success→online, Error→error) from the chrome store's
  `PaneRuntime`, decoupled from terminal rendering internals.
  Files: `heca/src/chrome/mod.rs`

### [x] Phase: Text selection · `terminal-06`
Shared host selection model — not terminal-only. Keyboard caret, actions, overlays.
**DONE** (verified 2026-06-30 — was already implemented; this corrects the stale status).

- [x] **terminal-task-11** — Shared selection state. **DONE**. `SelectionState` (+ `SelectionOwner`,
  `ActiveSelection`, caret-only/Selecting/Selected) on `AppState.selection`
  (`heca/src/app/selection_model.rs`).
- [x] **terminal-task-12** — Selection actions + mode-local keymap. **DONE**. `EnterSelectionMode`,
  `Selection{Left,Right,Up,Down}`, `BeginSelection`, `ClearSelection`, `ToggleSelectionEndpoint`,
  `CopySelection`, `PasteClipboard` wired through the registry; selection-mode bindings in
  `keybindings.default.toml` (`v`/`Space`, `o`, `y`, plus later `Shift+o`, `/`, `n`/`N`).
- [x] **terminal-task-13** — Host-rendered selection overlay. **DONE**. `selection_overlay_for_pane`
  (`heca/src/app/terminal_render.rs`); `Shift+left-drag` mouse entry; unmodified drags still reach the
  TUI.

### [x] Phase: Clipboard and paste · `terminal-07`
System clipboard on top of the shared selection model.
Source: terminal design phase 10 (now tracked in this backlog)

- [x] **terminal-task-14** — Copy selected text to system clipboard. **DONE.**
  `handle_copy_selection` extracts the host-grid selection and writes via `arboard`
  (`set_system_clipboard`). Reachable from keyboard / mouse / RPC.
  Files: `heca/src/handlers.rs`

- [x] **terminal-task-15** — Paste from system clipboard through the focused pane.
  **DONE.** `handle_paste_clipboard` reads `arboard` and forwards via the new
  `PaneBackend::paste`, which **wraps in bracketed-paste markers** (`ESC[200~ …
  ESC[201~`) when the program enabled DECSET 2004.
  Files: `heca/src/handlers.rs`, `heca-core/src/backend/terminal.rs`, `engine.rs`

- [x] **terminal-task-16** — `OSC 52` clipboard support. **DONE.** Register a wezterm
  `Clipboard` handler (`OscClipboard`); wezterm parses + base64-decodes the
  sequence, we capture the text into a queue the app drains
  (`take_clipboard_writes`) and pushes to the OS clipboard. **Writes only** —
  `OSC 52` read/query is intentionally unsupported (clipboard-exfiltration risk).
  Covered by `captures_osc52_clipboard_write_once` + `bracketed_paste_mode_tracks_decset_2004`.
  Files: `heca-core/src/backend/terminal/engine.rs`, `heca/src/app/lifecycle.rs`

### [x] Phase: Terminal UX and attention features · `terminal-08`
Bell, scrollback search, hyperlinks.
Source: terminal design phase 11 (now tracked in this backlog)

- [x] **terminal-task-17** — Bell handling. **DONE**. Backend capture (`BellHandler` → `take_alerts`
  → `BackendAlert::Bell`) already existed; added the **configurable policy** under
  `[appearance.terminal]`: `bell_attention` (OS attention cue when unfocused; default on),
  `bell_visual` (fading accent flash over the content area), `bell_audible` (system beep — macOS
  `NSBeep`, no-op elsewhere). Wired in `lifecycle.rs` (`apply_bell_policy` + `ring_system_bell`); flash
  state `AppState.bell_flash_until` + render via `chrome::paint_bell_flash` (shares
  `BELL_FLASH_DURATION`). Files: `heca-config/src/appearance.rs`, `heca/src/app/{lifecycle,render}.rs`,
  `heca/src/chrome/mod.rs`, `config.default.toml`, `README.md`. clippy 0 + tests green.

- [x] **terminal-task-18** — `OSC 8` hyperlink **open**. **DONE** (verified live). One
  `WmAction::OpenLink { url }` (handler → OS opener + scheme allowlist, policy `Global`, RPC
  `open-link`) behind **all surfaces**, all merged:
  - **Mouse Cmd+click** (#199) — link-first in `mouse.rs` `on_mouse_input` before interactive-move;
    `hyperlink_uri_at_position` + pure `hyperlink_at_cell` (start-incl / end-excl) in `terminal_host.rs`.
    Plus a hover **pointer cursor** when Cmd is held over a link (refreshed on `ModifiersChanged`).
  - **Keyboard `prefix+Shift+o`** (#200) — `WmAction::FollowLink` + `InputMode::FollowLink`
    (vimium-style a–z keycaps over visible links of the focused pane). Keycap visual reused from
    `KeyHint` via extracted `keycap_size`/`paint_keycap`, painted into the chrome scene by
    `chrome::paint_link_hints` (`terminal_host::cell_screen_pos`).
  - **Selection-mode `Shift+o`** (#201) — `WmAction::OpenLinkAtCaret`; `SelectionState::cursor_cell()`
    (caret in caret-only AND active-selection) + `hyperlink_uri_at_stable_cell` (stable→visible).
  - **Context menu right-click** (#202) — the app's first **stateful overlay**: `AppState.context_menu`
    + `context_menu_action` sink (closure→`WmAction` bridge); entries Open link / New column / Split
    down / Zoom / Float / Close. Right-click in the pane body opens it; `fallback_divider` now only
    resizes within a 24px band of a seam. Painted via `chrome::{layout_context_menu, paint_context_menu}`.
  - **Centralized action icons** (#202) — new `ActionDescriptor.icon: Option<Glyph>` is the single
    source; `ActionRegistry::icon(name)` reads it; both the context menu and the pane-action bar
    (`pane_action_spec`) resolve icons from it (foundation also serves `app-task-33`).

  **Follow-ups:**
  - ✅ **DONE** (#204) — glow **strength** is now config/theme-driven: `scaled_glow` applies
    `GlowLevel::strength_scale` (not just `radius_scale`), so `[appearance] glow_size` drives both
    halo size and intensity uniformly across all widgets (`medium` = 1.0×, no default regression).
  - ✅ **DONE** (#205) — FollowLink overlay now spans **all visible panes** (`LinkHint` carries its
    own `pane_id`; `collect_link_hints` iterates `pane_outer_frames`).
  - ~~SquareSplitHorizontal glyph for "New column"~~ — **decided: keep `Plus`** (user preference).

- [x] **terminal-task-26** — **URL auto-detection (linkify)**. **DONE** (#197). OSC 8 only marks links a program
  *explicitly* emits; the common case (`echo "https://google.com"`, log output, …) is **plain text**.
  Detect URL patterns in the visible terminal text and emit them as `HyperlinkSpan`s — the **same**
  pipeline as OSC 8 (`terminal-03`), so rendering (`terminal-03`) and every open surface
  (`terminal-task-18`) work identically for detected and explicit links.
  - Scan in the engine snapshot beside `collect_row_hyperlinks` (regex or a small hand-rolled
    scanner); match the open-allowlist schemes (`http(s)`, `ftp(s)`, `file`, `mailto`; optional bare
    `www.`). Trim trailing punctuation; per-row for v1 (wrapped URLs → two spans, acceptable).
  - **OSC 8 wins**: never double-link a cell that already carries an explicit OSC 8 link.
  - Configurable: `[appearance.terminal] link_detection` (default `true`).
  Files: `heca-core/src/backend/terminal/engine.rs`, `heca-config/src/appearance.rs`,
  `config.default.toml`, `README.md`.
  Related: `terminal-03` (HyperlinkSpan pipeline), `terminal-task-18` (open surfaces).

- [x] **terminal-task-19** — Scrollback search. **DONE**. Backend `search_scrollback(query, cols)`
  (engine, case-insensitive substring over the full scrollback stable range; `SearchMatch{stable_row,
  start_col,end_col}`). Entered with `/` in selection mode (`WmAction::SearchScrollback`); live query
  edit in `InputMode::Search` re-runs the search each keystroke and jumps the caret to the nearest
  match (scroll + `ensure_caret_visible`); `n`/`Shift+n` navigate (`SearchNextMatch`/`PrevMatch`);
  Enter keeps matches, Esc cancels; leaving copy-mode clears it. `AppState.search: Option<SearchState>`.
  Render: match highlights over the visible viewport (current bolder) + a `/query  n/total` bar at the
  pane's bottom-right (`chrome::paint_search`). v1 input is a status-style buffer (not the grid-ui
  `Input` widget) per user preference. Unit test on the backend search; clippy 0 + tests green.
  Files: `heca-core/src/backend/{mod,terminal,terminal/engine}.rs`, `heca/src/{input,handlers}.rs`,
  `heca/src/app/{terminal_host,input,render,registry,interaction}.rs`, `heca/src/chrome/mod.rs`,
  `heca/src/app_state.rs`, `keybindings.default.toml`, `README.md`.

### [x] Phase: Terminal image protocols (general inline images) · `terminal-09`
Source: terminal design phase 12 (now tracked in this backlog)
Gate: `terminal-task-03`/`terminal-task-04` (protocol hook stubs) — satisfied.

> **DONE — general inline-image rendering shipped on `feat/terminal-images`.**
> Scope grew from "Yazi preview" to **any image, any tool**: Sixel, iTerm2
> `OSC 1337`, and Kitty graphics all funnel through wezterm's per-cell
> `ImageCell` path, so one protocol-agnostic capture + one renderer blit covers
> them all. Verified live in the app (`wezterm imgcat` 4-colour PNG rendered in a
> pane; harness also drove `chafa -f sixel`). Three atomic commits:
> capture (stage 1) → renderer pipeline (stage 2) → app integration + damage (stage 3).

- [x] **terminal-task-20** — Capture image/graphics placements from `wezterm-term`
  through `TerminalSnapshot`. **DONE.** Enabled Kitty graphics in
  `HecaTerminalConfig`; decode each unique source image to RGBA once (cached by
  content hash); extended `GraphicsPlacement` (image id / source texcoords /
  z-index) + added a `TerminalImage` registry; coalesced per-cell slices into one
  block per `(image, placement, z)`. Also reports real pixel size to wezterm so
  Sixel attachment doesn't divide by zero and `CSI 14 t`/`16 t` queries (used by
  image tools to size previews) return non-zero.
  Files: `heca-core/src/backend/snapshot.rs`, `heca-core/src/backend/terminal/engine.rs`, `heca-core/src/backend/terminal.rs`

- [x] **terminal-task-21** — GPU renderer for image placements. **DONE.** New
  `heca-renderer/src/image.rs` + `image.wgsl`: textured-quad pipeline
  (premultiplied alpha, sRGB), per-image GPU texture cache keyed by image id with
  generation-based eviction, one quad per placement sampling the placement's
  source texcoords, flushed under-text (z<0) / over-text (z>=0).
  Files: `heca-renderer/src/image.rs`, `heca-renderer/src/image.wgsl`, `heca-renderer/src/lib.rs`

- [x] **terminal-task-22** — Wire image preview end-to-end. **DONE.** Draw into the
  terminal scratch around the glyph pass so the retained-layer/copy-band machinery
  carries images for free; image-aware retained damage (full repaint on placement
  change / when an image-bearing pane is touched; idle image panes still skip).
  Files: `heca/src/app/terminal_render.rs`, `heca/src/app_state.rs`, `heca/src/app/startup.rs`

> **Yazi resolution (verified live) — two extra fixes beyond rendering:**
> 1. **PTY pixel size (`TIOCSWINSZ`).** The PTY winsize reported `pixel_width/height
>    = 0`; image tools read `ioctl(TIOCGWINSZ)` to size previews — `kitten icat`
>    errors outright, Yazi spins. Now we report real pixel dims on the PTY (and the
>    wezterm model, so `CSI 14/16 t` answers + Sixel cell math agree). Files:
>    `heca-core/src/backend/terminal/pty.rs`, `terminal.rs`.
> 2. **Terminal identity (`TERM_PROGRAM=WezTerm`).** Tools pick their image protocol
>    by sniffing terminal identity. Unidentified, **Yazi falls back to Kitty Unicode
>    placeholders** (`U=1`) — a mode **no WezTerm release implements either**, so it
>    renders as boxes. Presenting as WezTerm makes Yazi (and others) use the **iTerm2
>    `OSC 1337`** protocol heca fully supports — exactly how it works in real WezTerm.
>    Verified: Yazi previews render, fast, in a release build. File: `pty.rs`.
> **Known gaps (not blocking):**
> - `kitten icat` direct transmission still fails: it emits **unpadded base64** and
>   wezterm-term's decoder requires canonical padding (`osc.rs` `base64_decode`).
>   Needs a lenient-padding patch (fork / `[patch]`) — low priority.
> - **Kitty Unicode placeholders** — **DEFERRED to the Neovim GUI** (user decision
>   2026-07-01). Verified: **no WezTerm release implements `U=1` placeholders**, so
>   tools that can use another protocol don't need them — Yazi works via iTerm2.
>   Implementing them in the terminal (snoop transmits → decode `U+10EEEE` cells →
>   placements) is a sizable feature whose main beneficiary is the editor, which the
>   **Neovim GUI** (`neovim-plan.md`) handles natively instead. Not planned for the
>   terminal pane.

> **Stage 4 follow-ups:**
> - **terminal-task-23 — DONE (2026-07-01, branch `feat/terminal-image-polish`).** Per-image
>   row-range damage: `retained_damage_to_apply` now damages the union of text-damage rows and the
>   image placement rows (new ∪ old) instead of forcing `Full`; a text change that doesn't overlap an
>   image leaves the image retained. Helpers `image_row_ranges`/`merge_row_ranges`/`ranges_overlap`
>   + `RetainedTerminalLayer.image_rows`; 14 policy/helper tests.
> - **terminal-task-24 — DONE (2026-07-01, same branch).** Animated GIF/APNG/`AnimRgba8`.
>   `TerminalImage` now holds `frames: Arc<[TerminalImageFrame]>` (pixels + per-frame delay);
>   `decode_encoded_frames` decodes all frames via `image::AnimationDecoder`, near-zero GIF delays
>   clamp to 100ms. `ImageRenderer` owns a per-image wall-clock, picks the frame for "now",
>   re-uploads it into the existing texture on change, and returns whether any advanced;
>   `sync_retained_terminal_layers` advances before the damage decision (→ per-image row damage via
>   task-23), and `AppState.has_animated_images` keeps the event loop ticking at frame cadence.
>   Tests: `frame_index_at` loop/clamp + end-to-end animated-GIF decode **with distinct frame
>   pixels**. heca-core 88, heca-renderer 25, heca 288, clippy clean. **VERIFIED LIVE (2026-07-01):**
>   plays correctly via `ranger` and a raw iTerm2 `OSC 1337` send.
>   **Bug found + fixed during live verification:** the animation froze on frame 0 because
>   `has_animated_images` was clobbered — `sync_retained_terminal_layers` runs twice per frame (tiled
>   + floating) and the second call overwrote the flag to `false`, killing the continuous-redraw
>   loop. Fix: reset once per frame in `render_frame`, OR into it from both calls. Added an optional
>   `HECA_DEBUG_IMAGES` env-gated decode diagnostic.
>   **Not our bug:** `yazi` previews don't animate (it sends a single flattened frame — confirmed on
>   other terminals too); `ranger` sends the raw GIF and animates. **Still open:** APNG not visually
>   spot-checked (decode path shared with GIF, so low risk); retina crispness of animations.
>   Shipped as PR #214 (branch `feat/terminal-image-polish`).
> - **terminal-task-25** — **DONE.** `config.toml` toggle
>   `appearance.terminal.images` (default `true`). When off, the engine skips
>   inline-image capture entirely (guarded `collect_row_graphics`). Wired via
>   `PaneBackend::set_image_capture` from `backend_factory`, mirroring
>   `link_detection`.
> - **terminal-task-26** — Yazi previews work via the iTerm2 path (see Yazi
>   resolution above). **DONE: crisp retina sizing** — the backend reports
>   *physical* px (`cell × scale`) to the model + PTY, threaded via
>   `PaneBackend::set_scale_factor` (pushed each frame in `sync_terminal_backend_size`
>   + on scale change in `refresh_terminal_cell_size`). Previews now render at the
>   real on-screen resolution instead of being upscaled. ⚠️ visible appearance
>   change on HiDPI (crisper + more correctly sized) — verify in-app.

### [x] Phase: Per-pane font zoom · `terminal-10`
> **DONE (2026-07-01) on `feat/terminal-font-zoom`** (branched off `feat/terminal-images`).
> Two explicit action variants: `GlobalTerminalFontZoom { step }` (app-wide base, policy `Global`)
> and `PaneTerminalFontZoom { pane_id: Option, step }` (focused/target pane, policy
> `FocusedPaneLocal`), with `FontZoomStep { In, Out, Reset }`. State: `terminal_font_zoom_global`
> (points) + `pane_font_zoom: HashMap<PaneId, f32>` (per-pane offset) + `pane_cell_override` cache on
> `AppState`; effective size = `(config + global + pane).clamp(6.0, 72.0)` via
> `AppState::effective_terminal_font_size`. Render loop builds `TerminalStyle.font_size` per pane and
> a per-pane render key in `sync_retained_terminal_layers`; `prepare_terminal_mount` fits each PTY to
> the pane's base cell. Keyboard: `prefix+Ctrl+=/-/0` (global), `prefix+Alt+=/-/0` (pane). Mouse:
> `Ctrl`/`Meta`+wheel intercepted in `events.rs` before terminal forwarding, pointer-resolved (pane →
> pane, chrome → global). RPC: `global-terminal-font <step>`, `pane-terminal-font <step> [pane_id]`.
> Step size is configurable: `[settings] terminal_font_zoom_step` (default 1.0pt). Maps pruned on
> pane close. Tests: `default_font_zoom_bindings_resolve_without_collision`, `test_font_zoom_commands`,
> `action_from_name`. heca 282/282, heca-core 84/84, heca-config 75/75, clippy clean.
> **Binding fix (2026-07-01 review):** pane branch moved from `Alt` to **`Ctrl+Shift`**
> (`prefix+Ctrl+Shift+=/-/0`). Root cause: `normalize_key_text` only applies the physical-key
> remap (`Equal`→`=`, `Digit0`→`0`) when **Ctrl** is held, so on macOS `Alt+=` delivers the
> Option-rewritten character (`≠`) and never matched — the Alt bindings were dead. Ctrl+Shift routes
> through the working Ctrl-remap path.
> **Scope change (2026-07-01, user request "l'app intera"):** the global branch is now a true
> **whole-app** zoom — it scales the chrome/UI font (sidebar, tabs, status bar) **and** every
> terminal pane together, not just terminals. Renamed `GlobalTerminalFontZoom`→`AppFontZoom`, config
> keys `global_terminal_font_*`→`app_font_*`, RPC `global-terminal-font`→`app-font`, state field
> `terminal_font_zoom_global`→`app_font_zoom`. Chrome side: `AppState::app_ui_font_size()` =
> `(config.ui + app_font_zoom).clamp(8,48)`, applied in `chrome_gui_theme` (font used both by the
> per-frame `paint_chrome_root` base font and the retained-tree rebuild); `app_font_zoom` added to
> `chrome_signature` so a zoom forces a chrome rebuild. Keyboard: `prefix+Ctrl+=/-/0` (whole app),
> `prefix+Ctrl+Shift+=/-/0` (focused pane). Mouse `Ctrl`/`Meta`+wheel: over pane → pane, over chrome
> → whole app. heca 282/282, heca-config 75/75, clippy clean.
> **Sticky font modes (2026-07-01, user request):** two `[[keys.mode]]` blocks (config-only, no new
> Rust — actions already exist, `build_modes` is generic): `app_font_size` (trigger `prefix+!` =
> Shift+1) and `pane_font_size` (trigger `prefix+@` = Shift+2), both `sticky=true`. Inside: `k`/`ArrowUp`
> bigger, `j`/`ArrowDown` smaller, `0` reset, `Esc`/`Enter` exit (auto via `handle_custom_mode`). Avoids
> re-pressing the chord. Test `default_font_size_modes_build_with_triggers_and_keys`. No collisions
> (`!`/`@` were free). heca 283/283, clippy clean.
> **Wheel toggle (2026-07-01, user request):** `[settings] mouse_wheel_change_font_size` (bool,
> default true) gates the `Ctrl`/`Meta`+wheel zoom in `handle_wheel_font_zoom`; false forwards the
> modified wheel normally (font zoom stays keyboard-only). Threaded through `AppState`, startup, and
> reload like `terminal_font_zoom_step`.
> **Still needs a human at the keyboard:** rebuild + restart, then confirm `Ctrl` scales chrome +
> all panes, `Ctrl+Shift` scales only the focused pane, and the `prefix+!` / `prefix+@` sticky modes
> work; retina visual check.

Zoom the terminal font **per pane**, with the same gesture also driving app-wide zoom when no
pane is targeted. Feasibility confirmed 2026-06-29: the per-pane plumbing mostly exists — cell
size is already per-backend (`backend.cell_size()`, used for mouse/selection mapping), the grid
is sized per-backend (`sync_terminal_backend_size`), retained layers are per `pane_id` with
`font_size` already in `terminal_layer_render_key`, and the glyph cache is size-keyed. The font
size is global today in only three spots: `resolve_terminal_cell_size`/`refresh_terminal_cell_size`
(`terminal_metrics.rs`) and the single `TerminalStyle.font_size` built in `render.rs`.

Design (agreed with user 2026-06-29):
- **Keyboard = two distinct bindings/actions** (explicit, no implicit scope resolution): one
  `prefix`+key for **global** font zoom (app-wide, `app-03`) and a separate `prefix`+key for the
  **focused pane** — the pane action always targets the currently focused pane. (Plus inc/dec and a
  reset for each.)
- **Mouse = `Ctrl`/`Meta`+wheel, pointer-resolved**: over a pane → that pane; over chrome/empty →
  global.
- **Composition with `app-03`**: app-wide zoom is the global base; per-pane is an offset/factor on
  top, so the global binding moves everything and the per-pane binding fine-tunes one pane. The
  global action IS `app-03`'s — design them together.

- [x] **terminal-task-23** — Per-pane font-size state (default = global config). Store per `pane_id`
  (AppState map or `PaneRuntime`); resolve cell size from the pane's size and `set_cell_size` per
  backend (re-fits cols/rows → PTY reflow, already handled).
  Files: `heca/src/app/terminal_metrics.rs`, `heca/src/app/terminal_host.rs`, `heca/src/app_state.rs`
- [x] **terminal-task-24** — Build `TerminalStyle.font_size` per pane in the render loop (today a
  single global style feeds all panes); retained layer + size-keyed glyph cache already cope.
  Files: `heca/src/app/render.rs`, `heca/src/app/terminal_render.rs`
- [x] **terminal-task-25** — Actions + input. A pane `WmAction` (`PaneFontZoom { delta }` + reset)
  for the focused pane, and the global zoom action shared with `app-03`. Two keyboard bindings
  (global vs pane) + mouse `Ctrl`/`Meta`+wheel resolved by pointer (over pane → pane action; over
  chrome → global action). Intercept the modified wheel at the WM level **before** terminal wheel
  forwarding. All through `ActionRegistry`, reachable from keyboard + mouse + RPC; follow the
  "Adding New Actions" checklist.
  Files: `heca/src/input.rs`, `heca/src/handlers.rs`, `heca/src/app/registry.rs`, `heca/src/mouse/`,
  `keybindings.default.toml`, `heca/src/rpc.rs`
  Related: `app-03` (the global branch is app-03's action — design together).

---

## Theming

> Source: archived `theming-plan.md`, `theming-documentation.md`
> **Status: CLOSED / DONE (historical track).** The active theming workstream is concluded; this backlog section is retained only as a historical mapping to the old theming plan.

### [x] Phase: Finish showcase visual verification · `theming-01`
- [x] **theming-task-01** — Run the showcase and verify all widgets react to theme cycling: colors, radius, border, glow, fonts.
- [x] **theming-task-02** — Verify the light theme (`latte`) renders correctly.

### [x] Phase: Migrate `heca-config` to `heca-theme` · `theming-02`
- [x] **theming-task-03** — Add `heca-theme` dep to `heca-config/Cargo.toml`.
- [x] **theming-task-04** — Remove `heca-config/src/color.rs` — re-export `heca_theme::Color` from `heca-config::color`.
- [x] **theming-task-05** — Remove `heca-config/src/defaults.rs` — move serde default helpers inline into `theme.rs` or callers.
- [x] **theming-task-06** — Update `heca-config/src/theme.rs` for `heca-theme` re-export/wrappers.
- [x] **theming-task-07** — Update `heca-config/src/loader.rs` — delegate `load_theme(name)` to `heca_theme::load_theme(name)`.
- [x] **theming-task-08** — Update all other `heca-config` files that import `Color` or `Theme` directly.
- [x] **theming-task-09** — Change `default_theme()` return value from `"mocha"` to `"grid_tron"`.
- [x] **theming-task-10** — `cargo check -p heca-config` + `cargo test -p heca-config` + clippy clean.

### [x] Phase: Migrate `heca-grid-ui` to `heca-theme` · `theming-03`
- [x] **theming-task-11** — Add `heca-theme` dep to `heca-grid-ui/Cargo.toml`.
- [x] **theming-task-12** — Remove `heca-grid-ui/src/color.rs` — re-export `heca_theme::Color`.
- [x] **theming-task-13** — Update `heca-grid-ui/src/theme.rs` for the composed/re-exported theme model.
- [x] **theming-task-14** — Update `heca-grid-ui/src/lib.rs` + `prelude` re-exports.
- [x] **theming-task-15** — Verify all widgets still compile.
- [x] **theming-task-16** — Update `heca-grid-ui/tests/phase_a.rs` for the new theme contract.
- [x] **theming-task-17** — `cargo check -p heca-grid-ui` + `cargo test -p heca-grid-ui` + clippy clean.

### [x] Phase: Migrate the main app (`heca`) to `heca-theme` · `theming-04`
- [x] **theming-task-18** — Fix `heca/src/chrome/mod.rs` / `chrome_gui_theme()` and remove hardcoded chrome background handling.
- [x] **theming-task-19** — Audit remaining app render theme callsites and remove stale theme-name/hardcoded color dependencies.
- [x] **theming-task-20** — Fix `heca/src/app/terminal_render.rs` for the composed GUI theme.
- [x] **theming-task-21** — Align sidebar/theme consumers with the unified theme path.
- [x] **theming-task-22** — Replace remaining mouse/render hover-preview theme mismatches.
- [x] **theming-task-23** — Wire `[settings].theme` into config loading with `grid_tron` default.
- [x] **theming-task-24** — Tokenize / align grid-ui widget alpha + effect usage to the unified theme model.
- [x] **theming-task-25** — Grep gate for literal color removal / intentional fallbacks.
- [x] **theming-task-26** — `cargo clippy --workspace --all-targets --all-features` clean + `cargo test --workspace` green/non-regression checked.

### [x] Phase: Migrate hand-drawn chrome to grid-ui widgets · `theming-05`
- [x] **theming-task-27** — Migrate tab bar (background + text) to a grid-ui widget.
- [x] **theming-task-28** — Migrate status bar to a grid-ui component.
- [x] **theming-task-29** — Migrate collapsed sidebar rail to grid-ui widgets.
- [x] **theming-task-30** — Migrate sidebar tree to grid-ui widgets.
- [x] **theming-task-31** — Migrate pane select/swap overlays to grid-ui overlay widgets.
- [x] **theming-task-32** — Decide on `grid_ares()` / final theme inventory.

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

- [x] **plugin-task-01** — Write the formal `ChromeHost` contract: what a region is, what it can host, allowed contribution types (container, toolbar group, status segment, panel, overlay request). Document in `pluggable-chrome-plugin-plan.md` §3.1. — **DONE 2026-07-02**, §3.1.1: `RegionId {LeftSidebar,RightSidebar,TopBar,BottomBar}` (widens the `Left/Right` event enum in `plugin-02`), 5-unit `Contribution` taxonomy + per-region allow-list, `ContainerContribution` metadata, `ChromeHost` responsibilities + skeleton, geometry (§5.7) + movement-as-action (§2.9) rules.

- [x] **plugin-task-02** — Write the formal provider lifecycle model: `id()`, `supported_regions()`, `default_region()`, `movable`, `collapsible`, `build_contribution(ChromeCtx)`. Document in `pluggable-chrome-plugin-plan.md` §3.4. — **DONE 2026-07-02**, §3.4.1: `Provider` trait (adds `default_order`/`title`), 6-step lifecycle state machine, `ChromeCtx` = shipped `App` facade + deferred write/contribute halves.

- [x] **plugin-task-03** — Write the overlay ownership and result-returning API shape: modal/dropdown lifecycle, focus trap, ESC, async result contract. Document in `pluggable-chrome-plugin-plan.md` §2.7/Phase 8. — **DONE 2026-07-02**, §2.7.1: host-owned `OverlayHost` z-stack (closes the "no central stack / no result" gaps in the shipped grid-ui overlay widgets), `open_modal`/`open_dropdown` → typed `OverlayFuture` (single-threaded one-shot; WASM marshals as request-id + resolve event).

- [x] **plugin-task-04** — Audit and fix geometry types in chrome-facing code. — **DONE (already satisfied) 2026-07-02**: verified there is no legacy `heca_core::types::Rect` — `heca-core` exposes only `backend`/`layout`/`runtime`, no `types` module and no `Rect` geometry type (only `Rectangle` in `layout::types`), and nothing in `heca/src` imports a bare `Rect`. Eliminated in an earlier refactor; §5.7/§7 of the plan were stale. New chrome/container APIs already use `Rectangle`/`Point`/`Size`.

### [x] Phase: ChromeHost and region hosts · `plugin-02` — **DONE 2026-07-02** (runtime core; no render, no app-side provider yet — those are plugin-03)
The central runtime that mounts/orders/moves containers across all 4 regions.
Source: `pluggable-chrome-plugin-plan.md` Phase 3

> **Scope landed:** pure runtime, no render change. The app still hand-paints its
> chrome and wires no provider; `ChromeHost` is constructed empty in `AppState`
> (`app/startup.rs`) and unit-tested. First visible payoff is plugin-03.
> Also: renamed the `ChromeRegion` event enum → `RegionId {LeftSidebar,RightSidebar,TopBar,BottomBar}`
> (+ `index()`/`ALL`) and added the `ContainerPlacementChanged` event.

- [x] **plugin-task-05** — Introduce `ChromeHost` struct in `heca/src/chrome/host.rs`. — **DONE**: 4-region array + `ContainerId→RegionId` placement index + event bus; `register`/`contributions`/`placement`/`move_container`/`reorder`/`set_region_visible` + `MoveError`. Seats containers from `Provider` *metadata* only (never calls the render seam `build_contribution`), so the host is App-free and unit-tested. Bridging to `App`/actions is plugin-03/04.

- [x] **plugin-task-06** — Region hosts. — **DONE**: chose the generic `RegionHost` (ordered `Vec<MountedContribution>` + visibility), one per `RegionId`, held in `ChromeHost.regions[4]`. (Not four named structs.)

- [x] **plugin-task-07** — Container registration, ordering, placement. — **DONE**: `register` seats at `default_region`/`default_order` (stable insert); `move_container` validates against `supported_regions`; `reorder` positions before a target. **Placement persistence to disk/config deferred** — in-memory only (one container today, no user-visible effect); `// TODO(plugin-07/config)` seam → plugin-06/07.

- [x] **plugin-task-08** — Host-level container move as a named action. — **DONE**: `WmAction::MoveContainerToRegion`/`ReorderContainerBefore`/`SetRegionVisible` (parameterized, `ActionPolicy::Global`), handlers in `handlers.rs`, registered in `build_registry`, RPC parity (`move-container-to-region`/`reorder-container-before`/`set-region-visible` + `parse_region_id`). Mouse/DnD path is plugin-03; dotted dynamic-action string ids (`chrome.container.move_to_region`) are plugin-04's dynamic registry.

### [ ] Phase: Built-in provider system and WorkspacesContainer migration · `plugin-03`
Prove the provider model with the first real built-in provider before loading external plugins.
Source: `pluggable-chrome-plugin-plan.md` Phases 4–5

> **`plugin-task-10a` (sidebar-nav highlight) — DONE 2026-07-03** on branch
> `feat/plugin-03-providers`. Cursor highlight (accent+glow, always-on, distinct from
> active) projected via `nav_selection` + `SidebarSelectionChanged`; cursor navigates
> **panes + workspaces only** (skips columns); `handle_sidebar_focus` no longer expands
> a contracted sidebar. `Row`/`MarkerGroup`/`DockFrame` gained `nav_selected`. 297 tests,
> clippy clean. Full detail: `handoff-sidebar-nav-task10a.md`.

**Sidebar follow-ups (arising from task-10a live testing — 2026-07-03):**
- [ ] **sidebar-fu-1** — Collapsed rail: pane initials not updated on rename.
  `collapsed_pane_label` (`heca/src/sidebar/render.rs`) uses the process name; prefer
  `custom_name` when set.
- [ ] **sidebar-fu-2** — KeyHint missing in the collapsed rail (old impl exists); bring
  the universal `KeyHint` into the hand-drawn rail. Folds into `app-task-21`.
- [ ] **sidebar-fu-3** — New `[settings]` bools `show_left_sidebar` / `show_right_sidebar`
  / `show_top_bar` / `show_bottom_bar`; `false` = fully hide that chrome widget. Touches
  `heca-config` schema + chrome region apply (`RegionMode::Hidden`) + startup/reload.
- [ ] **sidebar-fu-4** — Restore sidebar **add/remove buttons + context menu** for
  ws/cols/panes (lost in the grid-ui rebuild). Three surfaces: sidebar **buttons**
  (addPane→highlighted column, addCol→highlighted workspace, addWs→top), **sidebar-mode
  actions** on the highlighted item (handlers already exist:
  `handle_sidebar_create_workspace`/`create_column`/`split_in_column`/`delete_selected`,
  `handlers.rs:1471/1485/1504/1532` — verify targets), and a **mouse context menu** with
  the same ops (reuse the right-click overlay pattern in `chrome/mod.rs`).

- [~] **plugin-task-09** — Define the `Provider` trait: `id()`, `supported_regions()`, `default_region()`, `movable: bool`, `collapsible: bool`, `build_contribution(ChromeCtx) -> ContainerContribution`.
  Files: `heca/src/providers/mod.rs` (new)
  **Trait + `ChromeCtx` + `ProviderHandles` already landed in plugin-02** (`ChromeHost::register` needed them): `heca/src/providers/mod.rs` + the `Contribution`/`ContainerContribution` model in `heca/src/chrome/contribution.rs`. `ChromeCtx` currently wraps only the read/observe `App` half; its `actions`/`overlay`/`regions` halves are plugin-04/05. Remaining for plugin-03: the first real `impl Provider` (`plugin-task-10`) + calling `build_contribution`/`on_activate` on the render path.

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
>
> **PRUNING DECISIONS (2026-07-02) — verified against the code; these govern the tasks below.**
> The section stalled because it was full of speculative widgets with no consumer, plus items already
> covered by existing widgets. Verified findings:
> - **CUT — already built/covered:** scrollbar color token (`gridui-task-30` — `scroll_bar.rs` exists);
>   Pane header slot + CornerBrackets/Reticle (`gridui-task-02`/`04` — the terminal pane-shell already
>   draws header + focus-ring bracket; `terminal-05` DONE); HUD Frame (`gridui-task-17` — the bracketed
>   `Pane` covers it); Accordion (`gridui-task-20` — `DockFrame`/`ItemGroup` already collapse); Search
>   Input widget (`gridui-task-19` — the base `Input` **and** `CommandPalette`'s filter input already
>   provide search; sidebar search is just wiring, tied to `plugin-03`); Metric Row (`gridui-task-18` —
>   REMOVED: the "stat card" look already exists in the showcase via a local `card(label, value)` helper
>   composing bracketed `Pane` + `Label`; no dedicated widget needed, and no consumer in the app anyway).
> - **CUT — no consumer:** ScrollRegion horizontal scroll (`gridui-task-29` — the column strip already
>   scrolls horizontally via `ViewOffset`; ScrollRegion is vertical-list only); `DrawCommand::Custom`
>   (`gridui-task-14` — not present, YAGNI escape hatch).
> - **DEFER — until a real consumer:** full-scene bloom (`gridui-task-13`); visual-regression tests
>   (`gridui-task-12` — heavy infra, low ROI); nested-region wheel hit-testing (`gridui-task-32`).
> - **KEEP (real):**
>   - **Status bar REBUILD** (`gridui-task-05`) — NOT "done": today it's text-only; must gain
>     badges/tags/info + plugin extensions; ties to the Pluggable-Chrome **bottom-bar region**.
>   - **Tab-bar slot** (`gridui-task-03`) — for **stacked/tabbed pane layout** (niri/i3). `tabs.rs` is
>     the building block; the stacked layout itself is a SEPARATE heca-core layout feature (see App note).
>   - **Multi-select `Select`** (`gridui-task-16`) — NOT done (`select.rs` has no multi-selection).
>   - **Item DnD reorder** (`gridui-task-15`) — SMALL; the DnD framework (`heca-grid-ui/src/drag/`) is ready.
>   - **Sidebar scroll wiring** (`gridui-task-33`) — real; overlaps `plugin-03`.
>   - **Pick-a-scrollable-region** (`gridui-task-34`) — SMALL/optional, NOT the big task the prose implies:
>     `KeyHint` + the six pick `InputMode`s (PaneSelect/Swap/Take, WorkspacePick, ColumnPick, …) already
>     exist and are reused everywhere; only a `ScrollFocused` action + a scroll mode are new. Do it only
>     when multiple regions actually compete for `j/k`.
>   - **NF icons** (`gridui-03`), **showcase audit** (`gridui-task-11`), and all **`gridui-07` crate-debt**.
> - **Widget cleanup:** `gauge.rs` is reportedly unused — candidate for removal (verify no importers first).
> - **New feature surfaced:** stacked/tabbed pane layout (niri/i3) — a heca-core LAYOUT feature that would
>   consume `tabs.rs`; track under App/Chrome, not here.

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

- [~] **gridui-task-19** — ~~`Search Input` widget~~ **CUT (redundant): the base `Input` + `CommandPalette`'s
  filter input already provide search.** Remaining real work = wire search into the sidebar (integration,
  tied to `plugin-03`), not a new widget.

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

> **SIDEBAR MODE — regressions, root causes & DECISIONS (2026-07-02). Verified against code.**
> Three regressions found in `InputMode::SidebarNav`. **Fixes are DEFERRED** — they are done when we
> touch the owning tasks below (mostly inside `plugin-03`, since `plugin-03` migrates the sidebar into a
> `WorkspacesContainerProvider` and re-touching it now = double rework). Full write-up:
> `handoff-sidebar-and-plugin.md`.
>
> - **#1 — keys "don't work" in sidebar mode.** Most likely the keys FIRE but produce NO visible effect.
>   Routing is correct: `InputMode::SidebarNav` → `handle_sidebar_nav_mode` (`heca/src/app/input.rs:603`)
>   resolves via the `"sidebar"` mode keymap (j/k/arrows → `sidebar_up/down`); `handle_sidebar_up/down`
>   move the cursor. But the expanded sidebar highlights only `active_pane` (`chrome/mod.rs:1260`), NOT
>   the nav cursor (`sidebar_tree.current_item()`) → nothing changes on screen. **Owner: `plugin-task-10a`**
>   (already exists). Caveat: confirm at runtime the cursor actually moves (log `sidebar_tree.cursor`); if
>   it doesn't move, it's a keymap regression instead.
> - **#2 — collapsed rail: incoherent style, no `KeyHint`, no pane name (even after rename).** Root cause:
>   the collapsed rail is still LEGACY hand-drawn (`render_sidebar_collapsed` `heca/src/sidebar/render.rs:30`),
>   labels via `collapsed_pane_label` (`render.rs:353`) showing only the FIRST CHAR of `pane.name`; not
>   theme-coherent with the expanded side (`DockFrame`/`Card`/`MarkerGroup`). **Owner: `app-task-21`, with
>   EXTENDED scope (see below).**
> - **#3 — entering sidebar mode force-expands (should stay collapsed).** Root cause (certain):
>   `handle_sidebar_focus` (`heca/src/handlers.rs:1309`) does `set_left_mode(Expanded)` +
>   `set_left_size(DEFAULT_SIDEBAR_WIDTH)` on every entry. **Owner: NEW task `app-task-31` below.**
> Coupling: fixing #3 (stay collapsed) means the collapsed rail MUST render the nav cursor, or you
> navigate blind — so #2's scope must include collapsed-cursor rendering (the collapsed analog of `10a`).

- [ ] **app-task-20** — Wire sidebar buttons (`+w` workspace, `+c` column, `+p` pane) — `button_hitboxes` are defined but click handlers are not connected.
  Files: `heca/src/sidebar/hit_test.rs`, `heca/src/mouse/surface_left.rs`

- [ ] **app-task-21** — Migrate the collapsed sidebar rail from legacy hand-drawn + `sidebar_hit_test` to `RailCell`/`ChromeRegion` grid-ui widgets. **(EXTENDED SCOPE, 2026-07-02 — regression #2.)** Must also:
  (a) use `KeyHint` in collapsed mode (consistent with the expanded pick overlays);
  (b) surface the pane NAME (tooltip/label), updating live on rename — today only the first char shows;
  (c) render the sidebar-nav CURSOR/selection while in `InputMode::SidebarNav` (collapsed analog of `plugin-task-10a`);
  (d) be theme-coherent with the expanded side.
  Files: `heca/src/sidebar/render.rs`, `heca/src/mouse/render.rs`. Coordinate with `theming-task-29`.
  **Do this INSIDE `plugin-03`** (sidebar → `WorkspacesContainerProvider`) to avoid double rework.

- [ ] **app-task-31** — **(NEW, 2026-07-02 — regression #3.)** Enter `SidebarNav` WITHOUT force-expanding:
  `handle_sidebar_focus` must NOT call `set_left_mode(Expanded)` / `set_left_size(DEFAULT_SIDEBAR_WIDTH)` —
  keep the current collapsed/expanded state so sidebar mode works while collapsed.
  Small + standalone-able, but DEFERRED with the rest (pairs with `app-task-21`'s collapsed-cursor render).
  Files: `heca/src/handlers.rs` (`handle_sidebar_focus`, ~line 1309).

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

- [ ] **app-task-33** — Right-click context menu on the **terminal content pane** — the capstone of the
  terminal arc, done LAST (user request 2026-06-29). Reuses the `ContextMenu` widget from `app-task-32`.
  Entries route through `ActionRegistry` so mouse/keyboard/RPC stay one code path:
  **Copy selection** (gated by `terminal-06` text selection), **Paste** (gated by `terminal-07`
  clipboard), and **pane actions** (split/close/float — already existing `WmAction`s). Build only after
  `terminal-06` + `terminal-07` land so copy/paste are real and the menu is built once, complete.
  Files: `heca/src/mouse/`, `heca/src/app/terminal_host.rs`, `heca-grid-ui/src/widgets/` (reuse).

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
terminal-05   (DONE 2026-07-01 — no longer gated; pane-shell contract implemented)
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
