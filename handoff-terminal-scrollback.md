# heca — Terminal Scrollback Handoff

> **Single source of truth for the host-managed terminal scrollback viewport.**
> Covers: `terminal-00` foundation (prerequisite), `terminal-01a` (viewport + stable-row refactor).
> Created: 2026-06-24 · Supersedes: `handoff-terminal-00-foundation.md`,
> `handoff-terminal-01a-scrollback.md`, `terminal-implementation.md`,
> `runtime-validation-terminal-00b.md`.

---

## 1. What has been done

### Phase: `terminal-00` — Terminal damage-preservation foundation ✅ DONE

Four tasks delivered the prerequisite for scrollback: terminal damage must survive
the app render path so viewport motion (which forces `Full` damage) triggers a
complete repaint.

| Task | Status | Key deliverable |
|------|--------|----------------|
| `terminal-task-00` | ✅ | Damage preserved through mount/render boundary; `TerminalSnapshot` carries `damage` alongside lines. |
| `terminal-task-00a` | ✅ | Backend produces real `Rows(...)` damage instead of `dirty: bool`; coalesces adjacent rows; falls back to `Full` on resize/alt-screen/scroll-region changes. |
| `terminal-task-00b` | ✅ | Retained terminal-content foundation — unchanged rows stay visible; only dirty rows redrawn. Runtime-validated 2026-06-24: 5 scenarios passed (crisp resize, live text, correct idle scrollback, no pane bleed, full repaint on style reload). |
| `terminal-task-00c` | ✅ | Tests: `retained_damage_to_apply` pure seam (skip/passthrough/upgrade-to-Full), `retained_terminal_texture_size`, `terminal_layer_render_key` stability. `terminal_render::tests` 23/23, `heca-core` 67/67, clippy 0. |

### Phase: `terminal-01a` — Host terminal scrollback viewport 🟡 IN PROGRESS

Design locked via `/grill-me` on 2026-06-24. Split into 7 implementation slices.

| Slice | Status | Files | Gates |
|-------|--------|-------|-------|
| **1 — Backend viewport model** | ✅ DONE | `engine.rs`, `terminal.rs`, `snapshot.rs`, `backend/mod.rs`, `settings.rs`, `config.default.toml`, `app_state.rs`, `backend_factory.rs`, `startup.rs`, `main.rs`, `fake.rs`, `terminal_render.rs` (test snapshot) | `heca-core` 67/67, `heca-config` 70/70, `heca` 247/247, clippy 0 |
| **2 — Stable-row selection refactor** | ✅ DONE | `selection_model.rs`, `terminal_host.rs`, `terminal_render.rs`, `handlers.rs`, `backend/mod.rs`, `terminal.rs`, `fake.rs`, `snapshot.rs`, `render.rs` | `heca` 259/259, `heca-core` 67/67, clippy 0 |
| **3 — Actions + wheel + keybindings** | ⬜ NEXT | — | — |
| **4 — AppState chrome mirror** | ⬜ | — | — |
| **5 — Animated viewport offset** | ⬜ | — | — |
| **6 — GUI widgets** | ⬜ | — | — |
| **7 — Docs + review + commit** | ⬜ | — | — |

---

## 2. Architecture decisions (design contract)

All decisions locked via `/grill-me` (2026-06-24). Do NOT re-decide without a
new `/grill-me` session.

### Q1 — Viewport state ownership (hybrid)

- **`TerminalEngine`** owns `viewport_offset: usize` (0 = live bottom), projects
  into `TerminalSnapshot` each frame.
- **`TerminalSnapshot`** is a value struct (clone, no references). Carries:
  `viewport_offset: usize`, `at_bottom: bool`, `scrollback_rows: usize`,
  `viewport_top_stable_row: isize`.
- **AppState mirrors** into the reactive chrome store later (slice 4) — not yet.
- `viewport_offset()` and `at_bottom()` engine accessors are `#[cfg(test)]` only
  for now. Production reads viewport state from `TerminalSnapshot`.

### Q2 — Wheel policy

- Wheel → host scrollback UNLESS `is_mouse_grabbed()`.
- Shift+wheel → always host scrollback (bypasses grab).
- Default 3 rows/notch (`terminal_wheel_scroll_lines`).
- **Not yet implemented** (slice 3).

### Q3 — PageUp/PageDown (tmux-style, user override)

- Plain `PageUp`/`PageDown` → forward to PTY (do NOT intercept). This lets
  tmux-by-muscle-memory users keep their pass-through.
- `prefix+PageUp`/`prefix+PageDown` → enter `InputMode::Selection` + scroll one
  page (tmux copy-mode model).
- Wheel-up at a non-grabbed prompt also enters `InputMode::Selection` + scrolls
  (tmux `mouse on`).

### Q4 — Selection model (stable-row coords)

- `SelectionRegion::HostGrid` uses `isize` for rows (`anchor_stable_row`,
  `focus_stable_row`). Cols stay `usize` (columns don't scroll).
- Caret: `Caret::stable_row: isize`.
- `visible_row_to_stable_row(snapshot, visible_row) = snapshot.viewport_top_stable_row + row`.
- `stable_to_visible` converts back, clamps to `[0, snapshot.rows)`.
- Selection movement at viewport edge auto-scrolls the viewport (paved for slice 3).

### Q5 — Snap-to-bottom policy

- Forwarded key input → snap to bottom.
- New terminal output → snap ONLY if already `at_bottom`.
- Explicit `ScrollToBottom` action / indicator-click / reaching bottom → snap.
- Config knobs deferred.

### Q6 — Damage semantics

- Viewport motion → `TerminalDamage::Full`. This is the only correct choice until
  `terminal-01` (incremental viewport damage) is implemented.
- `reconcile_viewport_offset()` called at end of `update()`: write-back clamp that
  corrects stored offset when scrollback shrinks (alt-screen / `ESC[2J`), arming
  `viewport_changed` so the correction surfaces as `Full` damage.

### Q7 — GUI/UX ("exploit the GUI" directive)

Three generic `heca-grid-ui` widgets:
| Widget | Behavior | Slice |
|--------|----------|-------|
| Scrollbar (clickable/draggable) | Jump to any viewport position | 6 |
| Scrolled-up indicator badge | Shows "N lines above"; click = snap to bottom | 6 |
| Animated viewport offset | Easing via `tick(dt)` | 5 |

### Q8 — Action design

Five `WmAction` variants, all `FocusedPaneLocal`:
- `ScrollbackPageUp` / `ScrollbackPageDown`
- `ScrollbackLineUp` / `ScrollbackLineDown` (parameterized: `amount: usize`)
- `ScrollbackToTop` / `ScrollbackToBottom`
- `ExitScrollback` (exits selection mode + snaps to bottom)

Full 11-step treatment (AGENTS.md checklist) + RPC + default bindings.
**Not yet implemented** (slice 3).

### Q9 — Config

New `SettingsConfig` fields:
| Field | Type | Default | Alias |
|-------|------|---------|-------|
| `terminal_scrollback_lines` | `usize` | 3500 | `terminal-scrollback-lines` |
| `terminal_mouse` | `bool` | `true` | `terminal-mouse` (slice 3) |
| `terminal_wheel_scroll_lines` | `usize` | 3 | `terminal-wheel-scroll-lines` (slice 3) |

`terminal_mouse` is terminal-ONLY (NOT `settings.mouse` which chrome depends on).

### Q10 — Config bindings go in TOML

PR #185 / commit `3dfc458`: default keybindings moved from `keys.rs` to
`keybindings.default.toml`. New scrollback bindings (slice 3) go in
`keybindings.default.toml` under `[[keys.mode]]` for selection-mode bindings
and flat `[keys]` for prefix bindings. NEVER add them to `keys.rs`.

---

## 3. File inventory

### Slice 1 — Backend viewport model

| File | Changes |
|------|---------|
| `heca-core/src/backend/terminal/engine.rs` | `viewport_offset: usize`, `viewport_changed: bool`; `scroll_viewport(delta)`, `scroll_to_top()`, `scroll_to_bottom()`, `max_viewport_offset()`, `take_viewport_changed()`, `reconcile_viewport_offset()`; `visible_lines()` uses offset; `resize()` re-clamps after reflow |
| `heca-core/src/backend/terminal.rs` | `TerminalBackendOptions::scrollback_size`; `HecaTerminalConfig::scrollback_size()` override; delegates scroll methods; `take_terminal_damage` forces Full on motion; `update()` calls `reconcile_viewport_offset()`; `lines_in_stable_range()` |
| `heca-core/src/backend/snapshot.rs` | `viewport_offset`, `at_bottom`, `scrollback_rows`, `viewport_top_stable_row` fields; `debug_assert_valid` invariants |
| `heca-core/src/backend/mod.rs` | `PaneBackend` trait: `scroll_viewport`/`scroll_to_top`/`scroll_to_bottom`/`lines_in_stable_range` (default no-op); `TerminalSnapshot` fields |
| `heca-core/src/backend/fake.rs` | Snapshot construction with new fields |
| `heca-config/src/settings.rs` | `terminal_scrollback_lines: usize` (default 3500, alias `terminal-scrollback-lines`) |
| `config.default.toml` | `terminal_scrollback_lines = 3500` |
| `heca/src/app_state.rs` | `terminal_scrollback_lines: usize` field |
| `heca/src/app/backend_factory.rs` | Wires scrollback_size to `TerminalBackendOptions` |
| `heca/src/app/startup.rs` | Passes scrollback config |
| `heca/src/main.rs` | Wires settings |
| `heca/src/app/terminal_render.rs` | Test snapshot updated with new fields |
| `heca-renderer/src/terminal.rs` | `start_col`/`end_col` rendering (for overlay spans) |

### Slice 2 — Stable-row selection refactor

| File | Changes |
|------|---------|
| `heca/src/app/selection_model.rs` | `HostGrid`: `anchor_row`→`anchor_stable_row` (isize), `focus_row`→`focus_stable_row` (isize); `Caret::row`→`stable_row` (isize); `with_focus`/`update_focus`/`begin_selection_from_caret` updated; test constructions |
| `heca/src/app/terminal_host.rs` | `visible_row_to_stable_row()` helper; `forward_mouse_move` fetches snapshot + converts; `enter_selection_mode_for_focused_terminal` converts cursor row; `move_focused_terminal_selection` uses stable rows; `begin_terminal_selection_at` converts |
| `heca/src/app/terminal_render.rs` | `build_selection_overlay` takes `&TerminalSnapshot`; `stable_to_visible` closure; Caret/HostGrid convert stable→visible; `selection_overlay_for_pane` takes snapshot |
| `heca/src/app/render.rs` | Call sites pass `&mount.snapshot` instead of `cols` |
| `heca/src/handlers.rs` | `handle_copy_selection`: extracts `start_stable`/`end_stable` from HostGrid; fetches lines via `lines_in_stable_range`; passes `base_stable` to `extract_selection_text` |
| `heca-core/src/backend/mod.rs` | `PaneBackend::lines_in_stable_range` trait method (+ default no-op) |
| `heca-core/src/backend/terminal.rs` | Delegates `lines_in_stable_range` to engine |
| `heca-core/src/backend/terminal/engine.rs` | `lines_in_stable_range` reads from `TerminalScreen` visible/history lines |
| `heca-core/src/backend/snapshot.rs` | `viewport_top_stable_row: isize` field |
| `heca-core/src/backend/fake.rs` | Snapshot construction updated |

---

## 4. Test coverage

| Suite | Tests | Status |
|-------|-------|--------|
| `heca-core` engine tests | 6 viewport tests (clamping, at_bottom, snap, history, resize, scrollback_size) + reconcile test | ✅ 67/67 |
| `heca` app tests | `retained_damage_to_apply` (6 policy + size + render-key), `build_selection_overlay` (4), selection model (20+), terminal_render (23) | ✅ 259/259 |
| `heca-config` | Deep-merge / loader tests | ✅ 70/70 |
| Clippy | `--workspace --all-targets --all-features` | ✅ 0 warnings |

### Pre-existing flaky tests (not regressions)

- `heca-grid-ui` toast white-press-flash test — fails on plain `main`.
- `terminal_backend_exit_captures_code_via_take_exit` — PTY timing flake under
  parallel workspace load, passes 5/5 in isolation.

---

## 5. Key design decisions inline

- **Drift fix (review round 1):** `reconcile_viewport_offset()` write-back clamp
  at end of `update()` corrects stored offset when scrollback shrinks. Read-clamp
  in `visible_lines()` stays as belt-and-suspenders defence. The alternative
  (re-clamp at snapshot time only) would leave stale offsets between frames.
- **`with_launch_target` takes `cell_size: (f32, f32)`:** avoids clippy
  `too_many_arguments` (8→7) without `#[allow]`.
- **`TerminalBackendOptions` manual `Default`:** `scrollback_size` defaults to
  3500 via `DEFAULT_SCROLLBACK_SIZE` const.
- **`OnceLock` dropped from `HecaTerminalConfig`:** a shared cache would pin the
  first-seen `scrollback_size` and silently ignore later overrides.
- **Caret both left-aligned:** Both modes (caret-only + selection-endpoint) draw
  at LEFT edge of the cell (`x = rect.x + col * cell_w`), eliminating the visual
  bar-position jump when pressing `v`.
- **Block cursor → thin line:** User preference (2026-06-24). Default caret is a
  thin 2px bar, not a block cursor.
- **`first_movement` experiment removed (2026-06-24):** tmux-style boundary
  exclusion was attempted but rolled back — it introduced subtle bugs and the
  user decided to remove it. The selection model is now standard inclusive
  (anchor cell always included). This can be revisited as a separate feature.

---

## 6. Slice 3 — What to do next

### Actions + wheel + keybindings + interaction policy

**1. Five `WmAction` variants** in `heca/src/input.rs`:
- `ScrollbackPageUp` / `ScrollbackPageDown` (unit)
- `ScrollbackLineUp { amount: usize }` / `ScrollbackLineDown { amount: usize }`
- `ScrollbackToTop` / `ScrollbackToBottom`
- `ExitScrollback`

**2. Implement handlers** in `heca/src/handlers.rs`:
- Each calls `backend.scroll_viewport(...)` on the focused pane's backend.
- `ScrollbackToBottom` also clears selection + exits selection mode.
- `ExitScrollback` snaps to bottom + exits `InputMode::Selection`.

**3. Register in `build_registry`** (`heca/src/app/registry.rs`) — full 11-step
checklist per AGENTS.md.

**4. Classify in `action_policy()`** (`heca/src/app/interaction.rs`):
- All scrollback actions → `FocusedPaneLocal`.
- `ExitScrollback` → `FocusedPaneLocal`.

**5. Default bindings** in `keybindings.default.toml`:
- `prefix+PageUp` / `prefix+PageDown` → scroll one page + enter Selection mode.
- Prefix bindings for line scroll, top, bottom (assign keys).
- Selection-mode bindings (mode `selection`): `u`/`d` = half-page, `Ctrl+u`/`Ctrl+d`
  = full page, `g`/`G` = top/bottom, `Esc` = exit scrollback.
- DO NOT bind to plain PageUp/PageDown (forwarded to PTY).
- DO NOT bind to `prefix+s` (already selection-mode entry; use `prefix+PageUp`).

**6. Config** (`heca-config/src/settings.rs` + `config.default.toml`):
- `terminal_mouse: bool` (default `true`) — gates wheel-enters-scrollback.
- `terminal_wheel_scroll_lines: usize` (default 3).

**7. Wheel routing** (`heca/src/app/events.rs` or `terminal_host.rs`):
- Dispatch `ScrollbackLine` after `terminal_mouse && (!is_mouse_grabbed || shift_held)` check.
- Reset prefix timeout on scroll events like keyboard input.

**8. Snap-to-bottom policy** (Q5):
- On forwarded key input → `backend.scroll_to_bottom()`.
- In update loop, on new output → `backend.scroll_to_bottom()` only if `at_bottom`.

**9. Tests:**
- `action_policy` assertion for each new variant.
- Default binding resolution.
- Wheel routing both branches.
- Snap-to-bottom on input/output.

**10. Gate:** clippy clean + all tests green.

---

## 7. Git log

```
f63284b docs: update backlog + handoff for slice 2 (stable-row refactor)
50c2daf (origin/main) feat(terminal): slice 2 — stable-row selection model + cursor rendering
03cf599 feat(terminal): slice 1 — host-managed scrollback viewport model
649d18d test(terminal): close terminal-00 foundation — 00c tests + 00b runtime sign-off
```

Branch: `feature/terminal-followups`, upstream: `origin/feature/terminal-followups`.
All slices are on a single feature branch (NOT merged to main yet).

---

## 8. Rust conventions to follow

- `cargo clippy --workspace --all-targets --all-features` — 0 warnings mandatory.
- No `#[allow(dead_code)]` without a clear comment; prefer removing dead code.
- No `cargo fmt` — no pinned `rustfmt.toml`.
- Run `~/.agents/skills/rust/SKILL.md` review before every commit.
- `saturating_add_signed` / `saturating_sub` for arithmetic that might underflow.
- Prefer `f64` for layout coordinates, convert to `f32` at GPU boundary.
- All WM actions go through `ActionRegistry` — no direct state mutation from input code.

---

## 9. Key files to read before starting slice 3

| File | Why |
|------|-----|
| `heca/src/input.rs` | `WmAction` enum — add new variants here |
| `heca/src/handlers.rs` | Action handlers — add scrollback handlers here |
| `heca/src/app/interaction.rs` | `action_policy()` — classify new actions |
| `heca/src/app/registry.rs` | `build_registry()` — register handlers |
| `heca/src/app/terminal_host.rs` | `move_focused_terminal_selection` — add auto-scroll |
| `heca/src/app/events.rs` | Wheel input routing |
| `keybindings.default.toml` | Default bindings — add scrollback bindings here |
| `config.default.toml` | Default config — add terminal_mouse/wheel_lines here |
| `heca-config/src/settings.rs` | `SettingsConfig` — add new fields |
| `BACKLOG.md` | Track progress |
| `AGENTS.md` | All project rules (mandatory read) |
