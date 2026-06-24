# Handoff — Terminal `terminal-01a` Host Scrollback Viewport

Last updated: 2026-06-24
Worktree: `/Users/antonio/projects/myvim-terminal-followups`
Branch: `feature/terminal-followups`
HEAD at handoff: `1f54d91` (Merge of `origin/main` into the branch; tree clean)
Previous commit: `dc2ac10` (the retained scratch-size fix — committed & review-clean)

> **Read this file FIRST**, then `AGENTS.md`, `BACKLOG.md`, `terminal-implementation.md`,
> then `handoff-terminal-00-foundation.md`. This handoff supersedes the `terminal-00`
> handoff for everything concerning the **host scrollback viewport** phase.

---

## 0. Why this handoff exists

A `/grill-me` session on 2026-06-24 locked the full design for the **host-managed
terminal scrollback viewport** phase (`terminal-01a` + `terminal-01b` in `BACKLOG.md`).
The session also discovered that `origin/main` had landed a big **config/keybinding
single-source refactoring** (PR #185 / commit `3dfc458`) that moves all default
keybindings + default settings out of Rust and into versioned TOML files. That
refactoring is NOT in the older `terminal-00` handoff and materially changes WHERE
the new scrollback bindings/config live. This handoff captures both: (a) every
locked grill decision, and (b) the new config/keybinding model + how the plan was
adapted to it.

The next agent should implement `terminal-01a`/`01b` starting from slice 1
(backend viewport model). **Do NOT start coding until you have read this file and
the four files named above.**

---

## 1. Non-negotiable workflow and coding rules (carry forward)

- Act as a strong Rust developer. Load `~/.agents/skills/rust/SKILL.md` and review
  against it before every commit (per AGENTS.md).
- Use `apply_patch` for edits; prefer `rg`/`sed`/`git diff` for inspection.
- **Do NOT run `cargo fmt`** over the dirty tree (no pinned rustfmt.toml).
- **Do NOT use destructive git commands.** No rebase that rewrites shared history.
- **Do NOT hardcode colors/styles** and **do NOT bypass the registries** if you
  touch UI/input. Every action goes through `ActionRegistry`; every keybinding
  through `KeymapRegistry`; both configurable in `config.toml`.
- Follow the **"Adding New Actions" 11-step checklist** in `AGENTS.md` for every
  new `WmAction` variant.
- New UI = a proper, generic, theme-driven `heca-grid-ui` widget (embed `Base`,
  impl `Component` + builder traits, read ALL styling from `Theme`). Never ad-hoc
  inline `Flex`/`Surface` with hardcoded sizes/colors. Add to catalog +
  `docs/widgets.md` + showcase, then compose app-side.
- Keep terminal-specific retained-content work scoped to terminal panes; do NOT
  silently broaden it into general compositor optimization.
- **GUI-first principle (user directive):** heca is a **native GUI/GPU terminal**,
  not a terminal inside a terminal. Always aim for the best GUI/UX and exploit the
  GPU render loop (animated viewport, scrollbar widget, scrolled-up indicator,
  click-to-jump, momentum) rather than replicating tmux's instant integer jumps.
- No commit until the user reviews and explicitly asks. Stop before commit and
  hand back to the user for review.
- When the session is about to run out of tokens (70/80%), write a detailed handoff.

---

## 2. Current git/worktree state

- Worktree: `/Users/antonio/projects/myvim-terminal-followups` (the main checkout
  `/Users/antonio/projects/myvim` is NOT the active worktree for this task).
- Branch: `feature/terminal-followups`. Remote tracking shows
  `origin/feature/terminal-followups [gone]` — do not assume the remote branch exists.
- `origin/main` was **synced/merged** into this branch on 2026-06-24 (merge commit
  `1f54d91`). The tree is **clean** (no uncommitted changes). Build is green
  (`cargo check -p heca-config -p heca` passes).
- HEAD chain:
  - `1f54d91` Merge origin/main (brings in the config/keybinding refactor + border docs)
  - `65fd40d` PR #185 merge (config single-source)
  - `3dfc458` config/keybing config refactor
  - `dc2ac10` fix(terminal): size retained scratch to exact pane physical size
    (the `terminal-00` scratch-size fix — REVIEW-CLEAN, committed, not yet PR'd)
- The `dc2ac10` fix is the only commit on this branch not yet on `origin/main`.
  The prior `terminal-00` work was already merged into `origin/main` via PR #183.

**If the user asks to start a fresh phase from synced `origin/main`:** that likely
needs a fresh clean worktree. Do not merge `origin/main` blindly into a dirty tree
(the tree is clean now, but if it gets dirty again, sync first).

---

## 3. Locked grill decisions (the design contract)

These were resolved one-by-one with the user via `/grill-me` on 2026-06-24. They are
**settled** — do not casually re-decide them.

### Q1 — Viewport offset ownership: HYBRID
- **Backend is the source of truth for *rendering***: `TerminalEngine` owns
  `viewport_offset` (0 = pinned to live bottom; N = N rows above bottom) and
  projects it through `TerminalSnapshot` (`viewport_offset`, `at_bottom`, and
  `scrollback_rows`). `visible_lines()` changes from "always bottom" to
  "bottom minus `viewport_offset`", clamped to `[0, scrollback_rows - visible_count]`.
- **AppState mirrors it for *observability***: each frame the terminal host reads
  `viewport_offset`/`at_bottom`/`scrollback_rows` from the snapshot and pushes them
  into the reactive chrome store as a per-pane signal, emits
  `ChromeEvent::TerminalViewportChanged { pane_id, offset, at_bottom, scrollback_rows }`,
  and exposes `host.terminal_viewport(pane_id)` selector. Pattern follows the
  existing `KeyHint` pending-pick + `pane_custom_name` reference impls (see
  BACKLOG.md "Architecture principles").
- **One writer** (the backend, via `scroll_viewport(...)` calls the app/host
  initiates through the action system) → **many readers** (renderer via snapshot;
  plugins/widgets via the AppState mirror).

### Q2 — Wheel routing policy
- Wheel → **host scrollback** when the terminal is NOT grabbing the mouse
  (`!is_mouse_grabbed()`, plain shell prompt).
- Wheel → **forward to PTY** when `is_mouse_grabbed()` is true (nvim/lazygit/yazi/fzf
  with mouse mode).
- **Shift+wheel → always host scrollback** (universal override; lets the user scroll
  heca scrollback even inside a mouse-enabled TUI).
- Default scroll amount: **3 rows per notch** (config-tunable via
  `terminal_wheel_scroll_lines`).
- `is_alt_screen_active()` is NOT needed for the wheel decision — mouse-grab already
  distinguishes the cases that matter; alt-screen apps that don't grab the mouse
  (less/man) simply have no scrollback (alt screen = `allow_scrollback = false`),
  so host scrollback there harmlessly clamps to bottom.

### Q3-revised — PageUp/PageDown routing: TMUX-STYLE (user override of an earlier answer)
- heca's prefix (Ctrl+B) is deliberately tmux-style, so the user wants the **tmux
  copy-mode model**, NOT a context-aware heuristic.
- **Plain PageUp/PageDown (no prefix) → forward to the PTY** (tmux passes them
  through; do NOT intercept). This drops the `application_cursor_keys` /
  `is_alt_screen_active` heuristics entirely — cleaner.
- **`prefix+PageUp` / `prefix+PageDown` → enter scrollback navigation mode** (tmux
  `prefix+PageUp` enters copy-mode scrolled up one page) and scroll one page.
- **The scrollback-nav mode IS the existing `InputMode::Selection`** (already
  tmux-copy-mode/vim-visual style with caret + `v`/`Space`/`o`/`y`/`Esc`). Entering
  via `prefix+PageUp` places the caret at the terminal cursor and scrolls the host
  viewport one page. "Scroll to read" = Selection mode with a caret but no active
  selection; `v`/`Space` turns it into a real selection (now over history via Q4).
- Inside the mode: PageUp/PageDown scroll by page; `h/j/k/l` + arrows move the
  caret (auto-scroll on edge per Q4); `v`/`Space` begin; `o` flip; `y` copy;
  `Esc`/`q` exit and snap to bottom.
- **Wheel also enters the mode** (Q3-revised wheel): wheel-up at a non-grabbed
  prompt enters `InputMode::Selection` + scrolls (tmux `mouse on` behavior);
  wheel-down reaching the bottom auto-exits + snaps to bottom. Inside a
  mouse-grabbed TUI wheel forwards; Shift+wheel always scrolls.

### Q4 — Selection edge movement + coordinate space: AUTO-SCROLL + STABLE-ROW
- In `InputMode::Selection`, moving the caret up/down past the top/bottom visible
  row **auto-scrolls the host viewport by one row** in that direction, caret
  pinned to the edge (tmux copy-mode / vim visual-mode feel).
- The selection caret/anchor/focus move from **visible-row** coordinates to
  **stable-row + col** (absolute scrollback coordinates, wezterm-term's
  `StableRowIndex`). This keeps selections **anchored to content** while the
  viewport scrolls (scrolling doesn't drag the selection along) and lets
  selections extend into history + copy history rows. Rendering computes the
  visible row as `caret_stable_row - viewport_top_stable_row`.
- This is a **real refactor of `SelectionRegion::HostGrid`** (it currently holds
  `anchor_row`/`focus_row` as visible indices). Required for the phase.

### Q5 — Snap-to-bottom triggers
- **Forwarded key input** (printable char, Enter, arrows in Normal mode, etc. that
  go to the PTY) → **snap to bottom** (scroll-on-keypress, default true).
- **New PTY output** → snap to bottom **only if already at bottom** (offset 0).
  If scrolled up to read history, new output does NOT yank the viewport; it just
  appends to scrollback (wezterm-style; respects reading history).
- **Explicit snap:** `ScrollToBottom` action, clicking the content area /
  scrolled-up indicator, and reaching the bottom via scroll all set offset 0.
- **Config knobs deferred** (`terminal_scroll_on_output`, default false;
  `terminal_scroll_on_keypress`, default true) — hardcode the recommended defaults
  now; make configurable in a follow-up to keep this phase focused.

### Q6 — Damage semantics during viewport motion: FULL (user reverted an earlier "incremental" choice)
- **Viewport motion produces `TerminalDamage::Full`** for that frame (full
  retained-layer re-render with the new viewport rows), then blit.
- The engine sets a `viewport_changed` flag on `scroll_viewport`; the next
  snapshot/damage is `Full`. Per-row `Rows(...)` stays reserved for ordinary PTY
  output only.
- **Incremental/dirty-row viewport optimization is a SEPARATE phase (`terminal-01`)** —
  the user explicitly chose to keep it split as originally planned. Do NOT fold the
  GPU-texture-shift dirty-row work into this phase. (See BACKLOG `terminal-task-01`.)
- This keeps `terminal-00b`/`00c` stable.

### Q7 — GUI-native viewport UX: ALL THREE affordances (user directive: exploit the GUI)
- **(1) Animated viewport offset** — `viewport_offset` animates smoothly to its
  target (easing, like the existing column `ViewOffset`), driven by `tick(dt)`.
  Wheel/PageUp set a target; the render loop eases toward it each frame.
- **(2) Scrollbar widget** — theme-driven `heca-grid-ui` widget on the right edge
  of each terminal pane: thin rail, thumb sized by `visible_rows / scrollback_rows`,
  **clickable/draggable to jump** (mouse surface), live-reflects the animated
  offset. Generic/domain-neutral widget (per AGENTS widget rules) — NOT a
  hardcoded inline overlay.
- **(3) "Scrolled up / N lines below" indicator** — small GUI badge near the pane
  bottom or in the pane info bar when `viewport_offset > 0`, with a
  click-to-snap-to-bottom affordance. Built as a generic catalog widget.
- All three subscribe to `ChromeEvent::TerminalViewportChanged` /
  `host.terminal_viewport(pane_id)`. Scrollbar + indicator must be added to the
  `heca-grid-ui` catalog + `docs/widgets.md` + the showcase, then composed app-side.

### Q8 — Action surface + default keybindings (tmux-faithful, conflict-checked)
New `WmAction` variants (full 11-step registry treatment each):

| Variant | Effect |
|---|---|
| `ScrollbackPage { direction: Up\|Down }` | If not in `Selection` mode → enter it (caret at cursor) + scroll one page; if already in mode → just scroll one page. |
| `ScrollbackLine { direction, amount: u32 }` | Enter mode + scroll `amount` lines (wheel fine-grained). |
| `ScrollbackToTop` | Jump viewport to top of scrollback, stay in mode. |
| `ScrollbackToBottom` | Snap to bottom, stay in mode. |
| `ExitScrollback` | Snap to bottom + exit `Selection` mode. |

Existing actions extended (no new variant): `SelectionLeft/Right/Up/Down` (Phase 9)
handlers gain edge-auto-scroll + the selection model moves to stable-row coords
(Q4). `EnterSelectionMode` (Phase 9) still enters mode at the cursor with viewport
at bottom (no scroll).

**Default keybinding map** (prefix = Ctrl+B, all configurable):

| Binding | Action | Mode |
|---|---|---|
| `prefix+PageUp` | `scrollback_page_up` (`ScrollbackPage{Up}`) | normal (NEW) |
| `prefix+PageDown` | `scrollback_page_down` (`ScrollbackPage{Down}`) | normal (NEW) |
| `prefix+s` | `enter_selection_mode` | normal (EXISTS — do not change) |
| `PageUp` | `scrollback_page_up` | selection (mode-local, NEW) |
| `PageDown` | `scrollback_page_down` | selection (mode-local, NEW) |
| `g` | `scrollback_to_top` | selection (mode-local, NEW) |
| `G` | `scrollback_to_bottom` | selection (mode-local, NEW) |
| `q` | `exit_scrollback` | selection (mode-local, NEW) |
| `Esc` | exit selection mode (hardcoded universal) — EXTEND to snap-to-bottom | selection (existing) |
| `h/j/k/l`+arrows | `selection_*` (now auto-scroll on edge) | selection (existing, enhanced) |
| `v`/`Space`/`o`/`y` | selection begin/flip/copy | selection (existing) |

**Conflict check done:** `prefix+[` is ALREADY bound to `prev_pane`
(`heca-config/src/keys.rs` legacy → now `keybindings.default.toml`). Do NOT use
`prefix+[` for selection mode. heca uses `prefix+s` for selection-mode entry (keep
it). `prefix+s` = `enter_selection_mode` already exists.

**Wheel (not a TOML binding; dispatches via the registry after the policy check):**
if `terminal_mouse && (!is_mouse_grabbed || shift_held)` →
`registry.execute(ScrollbackLine{ direction, amount: wheel_lines })` (which enters
mode + scrolls); else → `forward_mouse_wheel` to PTY.

**RPC:** all five scroll actions + `host.terminal_viewport(pane_id)` selector (read)
+ `ChromeEvent::TerminalViewportChanged` (observe).

**interaction.rs policy:** all five scroll actions are `FocusedPaneLocal` (operate
on the focused pane in either Tiled/Floating domain). Add spot-check assertions +
routing tests (tiled allows, floating allows — these are pane-local).

### terminal_mouse config toggle (tmux `set -g mouse on/off` analog, terminal-only)
- The existing global `settings.mouse` gates WM clicks/drags/resize AND is used by
  chrome (sidebar, contextual menus, checkboxes, DnD). heca has NO concept of
  "global mouse off" — turning it off would break the UI. So do NOT reuse it.
- Add a **terminal-only** `terminal_mouse: bool` (default `true`) to
  `[settings]` in `config.default.toml` + `SettingsConfig` Rust field with a serde
  backstop default. Gates only the terminal wheel-enters-scrollback behavior:
  - `true`: wheel-up at a non-mouse-grabbed prompt enters Selection mode + scrolls.
  - `false`: wheel always forwards to the PTY; scrollback still reachable via
    `prefix+PageUp` / keyboard (which doesn't need the mouse toggle).
- The global `mouse` stays unchanged (gates WM mouse only).

---

## 4. The new config/keybinding single-source model (PR #185, now merged)

> **The older `terminal-00` handoff does NOT mention this.** It materialised in
> `origin/main` as commit `3dfc458` (PR #185 `feat/config-single-source`) and was
> merged into this branch (`1f54d91`). The plan file
> `HANDOFF-config-single-source.md` is the *plan*; the *actual implementation* is
> what's in the tree now. **Read the actual files, not just that plan doc — the
> user warned the handoff may diverge from the actual code.**

### Where defaults live now
- **`keybindings.default.toml`** (repo root, embedded via
  `include_str!("../../keybindings.default.toml")` in `heca-config/src/loader.rs`):
  the single source for ALL default keybindings. Contains the `[keys]` section
  (prefix + ~50 flat bindings + `[keys.unbind]` + commented `[[keys.command]]`/
  `[[keys.mode]]` examples) and the active `[[keys.mode]]` blocks for resize /
  sidebar / selection. Parsed by `parse_default_keys()` → `KeysConfig::default()`.
- **`config.default.toml`** (repo root, embedded): single source for
  `[settings]` / `[appearance]` / `[font]` / `[program]` defaults. Renamed from the
  old `example.config.toml` (which is deleted).
- `default-keybindings.toml` (old) is **deleted** (renamed to `keybindings.default.toml`).

### Loader mechanism (`heca-config/src/loader.rs`)
- `CONFIG_DEFAULT = include_str!("../../config.default.toml")`,
  `KEYS_DEFAULT = include_str!("../../keybindings.default.toml")`.
- `embedded_base()` = `deep_merge(parse(CONFIG_DEFAULT), parse(KEYS_DEFAULT))`
  (the two files cover disjoint TOML sections — clean union).
- `Config::default()` = `embedded_base().try_into::<Config>()` (the files ARE the
  default; no separate Rust-seeded defaults).
- `load_config_file()` = `embedded_base()` then deep-merge user `config.toml` +
  user `keybindings.toml` (searched at `~/.config/heca/config.toml` and
  `~/.config/heca/keybindings.toml` via `config_file_paths()`/`keybindings_file_paths()`).
- `deep_merge(base, over)`: **tables merge per-key (recursively); scalars AND arrays
  are replaced by `over`** (arrays are NOT concatenated). So a user
  `[[keys.command]]`/`[[keys.mode]]` list **replaces** the default list wholesale,
  BUT the comment in `keybindings.default.toml` says built-in modes (resize/sidebar/
  selection) are "always merged back in at runtime by name" — verify this in the
  loader/app code before relying on it.

### Rust types (`heca-config/src/keys.rs`, now 204 lines — only types, no defaults)
- `BindingValue::Single(String) | Many(Vec<String>)` (flat binding values).
- `KeybindingMap = HashMap<String, BindingValue>` (flat bindings; **NO args field**).
- `CommandKeybindConfig` (`[[keys.command]]`).
- `ModeBindingConfig { action, keys, args: HashMap<String,String> }` — **mode
  bindings DO support `args`** for parameterized actions (e.g. resize uses
  `args = { target = "column", axis = "x", amount = "50" }`).
- `KeyModeConfig { name, trigger, sticky, bindings: Vec<ModeBindingConfig> }`.
- `KeysConfig { prefix, bindings (flatten), unbind, command, mode }`.
  `KeysConfig::default()` calls `parse_default_keys()` (no recursion).
- `KeybindingMap` flat bindings CANNOT carry args. So flat normal-mode bindings
  for scrollback must use **unit action names** (`scrollback_page_up` etc.), which
  `action_from_name` maps to the parameterized `WmAction::ScrollbackPage{Up}`.
  Mode-local bindings CAN use args, but for consistency use unit names too
  (`action = "scrollback_page_up"`, `keys = "PageUp"`).

### `SettingsConfig` (`heca-config/src/settings.rs`)
- Has `mouse: bool` (default true) — gates WM clicks/drags/resize (NOT wheel).
- Many `terminal_*` color overrides (`Option<Color>`).
- Add here: `terminal_mouse: bool` (default true, terminal-only wheel gate),
  `terminal_wheel_scroll_lines: u32` (default 3), `terminal_scrollback_lines: usize`
  (default 3500). Each with `#[serde(default = "default_…")]` backstop. Also add
  the active values to `config.default.toml [settings]`.

### Tests to keep green / extend
- `heca-config/src/keys.rs::test_keys_config_has_default_bindings` — add
  `scrollback_page_up` to the assertion list.
- `heca-config/src/keys.rs::test_keys_config_has_default_sidebar_mode` —
  optionally add a selection-mode assertion for the new bindings.
- `heca-config/src/loader.rs` `config_default_toml_parses` /
  `keybindings_default_toml_parses` / `deep_merge_*` tests — keep green.
- The `default_selection_mode_bindings_resolve` test in
  `heca/src/app/registry.rs` — extend with the new selection-mode bindings
  (PageUp/PageDown/g/G/q).

---

## 5. Code facts the grill grounded (do NOT re-ask the user about these)

- The vendored `wezterm-term` crate (at
  `~/.cargo/git/checkouts/wezterm-26cdb9e734a97642/891bed3/term/`) does **NOT**
  expose a scrollback viewport offset. heca must own the host viewport offset
  itself. Relevant Screen API (`term/src/screen.rs`):
  `scrollback_rows()`, `visible_row_to_stable_row(VisibleRowIndex) -> StableRowIndex`,
  `lines_in_phys_range(Range<PhysRowIndex>) -> Vec<Line>`, `physical_rows`,
  `get_changed_stable_rows(range, seqno)`.
- `TerminalState` (deref target of `Terminal`) exposes:
  `is_mouse_grabbed()` (mouse_tracking | button_event_mouse | any_event_mouse),
  `is_alt_screen_active()`, `application_cursor_keys` (private field, has accessor?),
  `current_seqno()`, `screen()`, `cursor_pos()`, `get_size()`, `palette()`,
  `focus_changed()`, `bracketed_paste_enabled()`.
- `TerminalEngine` (`heca-core/src/backend/terminal/engine.rs`) currently:
  `visible_lines()` ALWAYS projects the bottom
  (`visible_end = screen.scrollback_rows().max(visible_count)`,
  `visible_start = visible_end - visible_count`). This is the line to change for
  the viewport offset.
- `HecaTerminalConfig` (`engine.rs`) implements only `color_palette()` of
  `TerminalConfiguration`; `scrollback_size()` uses the trait default **3500**.
  Override `scrollback_size()` to read `terminal_scrollback_lines`.
- `move_focused_terminal_selection` (`heca/src/app/terminal_host.rs`) clamps to
  `snapshot.rows-1` / `cols-1` — confirmed it cannot reach history today. It calls
  `state.terminal_snapshot()` (via `backend.terminal_snapshot()`).
- Wheel: `heca/src/app/events.rs:216` calls `forward_mouse_wheel(...)` regardless of
  `state.mouse_enabled` (the `mouse` setting gates `on_mouse_input` button handling
  but NOT wheel). `forward_mouse_wheel` is in `terminal_host.rs:247` and forwards
  `BackendMouseButton::WheelUp/Down/Right/Left` to the PTY.
- PageUp/PageDown: `heca/src/app/keyboard.rs:243-244` encodes them as
  `\x1b[5~`/`\x1b[6~` raw bytes and `BackendKeyCode::PageUp/Down` (lines 269-270).
- `TerminalSnapshot` (`heca-core/src/backend/snapshot.rs`): has
  `cols, rows, cell_w, cell_h, default_fg, default_bg, cursor_color, cursor,
  lines: Vec<TerminalLine>`. Add `viewport_offset: usize`, `at_bottom: bool`,
  `scrollback_rows: usize`. `debug_assert_valid()` enforces one line per visible row
  and ≤ cols cells — extend for new fields.
- `TerminalDamage` (`snapshot.rs`): `None | Full | Rows(Vec<TerminalRowRange>)`.
  Add viewport-changed → `Full` (NOT a new variant; just force `Full` when
  `viewport_changed` flag is set, per Q6).
- Retained layer pipeline (already working post-`dc2ac10`):
  `sync_retained_terminal_layers` (terminal_render.rs) renders dirty rows into the
  per-pane retained layer → `blit_retained_terminal_layer` blits to the scene. The
  retained scratch is now exact-size-matched (`ensure_size`). Viewport motion →
  `Full` re-renders the retained layer with the new viewport rows. This is correct.

---

## 6. Adapted implementation plan (Rust vs TOML locations)

### Rust (heca crate) — full 11-step action treatment + backend + app mirror
- `heca/src/input.rs`: `WmAction` variants (`ScrollbackPage{direction}`,
  `ScrollbackLine{direction,amount}`, `ScrollbackToTop`, `ScrollbackToBottom`,
  `ExitScrollback`) + `action_from_name` (unit names → variants) + `action_priority`
  (explicit match, no wildcard) + `build_action` if needed.
- `heca/src/handlers.rs`: handlers — enter Selection mode + scroll / scroll / snap /
  exit+snap. Reuse `EnterSelectionMode` logic where possible.
- `heca/src/app/registry.rs`: register handlers; extend
  `default_selection_mode_bindings_resolve` test.
- `heca/src/actions.rs`: `ActionRegistry::ALL` descriptors (user-facing).
- `heca/src/rpc.rs`: parsers for the five scroll actions.
- `heca/src/app/interaction.rs`: classify all five as `FocusedPaneLocal`; add
  spot-check assertions + routing tests.
- `heca-core/src/backend/terminal/engine.rs`: `viewport_offset` (animated target
  stored here OR app-side; Q1 says backend is source of truth — store here),
  `visible_lines()` projects bottom-minus-offset, `scroll_viewport(delta)`,
  `scroll_to_top()`, `scroll_to_bottom()`, a `viewport_changed` flag forcing `Full`
  damage. Override `HecaTerminalConfig::scrollback_size()` for
  `terminal_scrollback_lines`.
- `heca-core/src/backend/terminal.rs`: `TerminalBackend::scroll_viewport` etc.
  delegating to engine; expose viewport fields in `terminal_snapshot()`.
- `heca-core/src/backend/mod.rs`: add `scroll_viewport`/`scroll_to_top`/
  `scroll_to_bottom` to the `PaneBackend` trait (default no-op for non-terminal
  backends like FakeBackend).
- `heca-core/src/backend/snapshot.rs`: add `viewport_offset`, `at_bottom`,
  `scrollback_rows` to `TerminalSnapshot`; update `debug_assert_valid`.
- `heca/src/app/terminal_host.rs`: read snapshot → mirror into chrome store signal
  + emit `ChromeEvent::TerminalViewportChanged`; wheel policy check
  (`terminal_mouse && (!is_mouse_grabbed || shift_held)` →
  `registry.execute(ScrollbackLine{…})` else `forward_mouse_wheel`);
  `move_focused_terminal_selection` → stable-row coords + auto-scroll on edge.
- `heca/src/app/selection_model.rs`: refactor `SelectionRegion::HostGrid` from
  visible-row to **stable-row + col** (Q4). Careful: this touches Phase 9 selection
  state; keep `BackendNative` variant unchanged.
- `heca/src/app/terminal_render.rs`: compose the scrollbar + indicator widgets;
  convert stable-row selection to visible-row for overlay rendering
  (`visible_row = stable_row - viewport_top_stable_row`).
- `heca/src/chrome/state.rs` + `heca/src/chrome/mod.rs`: per-pane
  `terminal_viewport` signal + `ChromeEvent::TerminalViewportChanged` emission.
- `heca/src/host.rs`: `terminal_viewport(pane_id)` selector.
- `heca-grid-ui`: new generic scrollbar + indicator widgets (catalog +
  `docs/widgets.md` + showcase), theme-driven.
- `heca/src/app_state.rs`: `AppState` gains the wheel-scroll config reads
  (`terminal_mouse`, `terminal_wheel_scroll_lines`) and the animated viewport
  target maybe (if animation lives app-side; Q1 keeps backend as source of truth,
  but the *animation tween* could be app-side driving backend targets — decide
  during implementation; the existing column `ViewOffset` animation is in
  `heca-core/src/layout/view_offset.rs` and is a good model).

### TOML (new single-source)
- `keybindings.default.toml` `[keys]`: add
  `scrollback_page_up = "prefix+PageUp"`, `scrollback_page_down = "prefix+PageDown"`.
- `keybindings.default.toml` `[[keys.mode]] name = "selection"`: add
  `PageUp`→`scrollback_page_up`, `PageDown`→`scrollback_page_down`,
  `g`→`scrollback_to_top`, `G`→`scrollback_to_bottom`, `q`→`exit_scrollback`.
- `config.default.toml` `[settings]`: add `terminal_mouse = true`,
  `terminal_wheel_scroll_lines = 3`, `terminal_scrollback_lines = 3500` (commented
  or active; document them).
- `heca-config/src/settings.rs`: add the three fields with serde backstop defaults.
- `heca-config/src/keys.rs` test: add `scrollback_page_up` to
  `test_keys_config_has_default_bindings`.

### Docs
- `README.md` (scrollback section + terminal_mouse), `src/keybindings.md`,
  `BACKLOG.md` (mark `terminal-task-01a`/`01b` in progress → done),
  `terminal-implementation.md` HANDOFF (update Current Status + add `terminal-01a`
  phase notes), `AGENTS.md` if it references the old config files (the refactor
  already updated AGENTS — verify).
- `docs/widgets.md` + showcase for the new scrollbar/indicator widgets.

---

## 7. Slice plan (small, behavior-preserving slices, stop for review between)

### [x] Slice 1 — Backend viewport model (no UI/actions yet) — DONE
- `TerminalEngine`: `viewport_offset: usize` (0 = bottom), animated target field,
  `scroll_viewport(delta)`, `scroll_to_top()`, `scroll_to_bottom()`,
  `viewport_changed` flag, `visible_lines()` projects bottom-minus-offset clamped
  to `[0, scrollback_rows - visible_count]`.
- `PaneBackend` trait: add `scroll_viewport`/`scroll_to_top`/`scroll_to_bottom`
  (default no-op). `TerminalBackend` delegates. `FakeBackend` no-op.
- `TerminalSnapshot`: add `viewport_offset`, `at_bottom`, `scrollback_rows`;
  update `debug_assert_valid`.
- `TerminalDamage::Full` on viewport motion (engine sets it).
- `HecaTerminalConfig::scrollback_size()` override for `terminal_scrollback_lines`
  (wire from `SettingsConfig`).
- Unit tests: viewport clamping, at_bottom logic, Full on motion, scrollback_size
  override.
- Gate: `cargo check` + `cargo test -p heca-core` green. Stop for review.

### [x] Slice 2 — Selection model stable-row refactor (Q4) — DONE
- `SelectionRegion::HostGrid`: `anchor_row`/`focus_row` → stable-row (isize or
  StableRowIndex alias); col stays visible (cols don't scroll).
- `move_focused_terminal_selection`: move caret in stable-row space; on edge
  (caret would go past top/bottom visible row), call `backend.scroll_viewport(±1)`
  instead of clamping; keep caret pinned to the edge.
- Rendering converts stable-row → visible-row via `viewport_top_stable_row`.
- Tests: selection survives viewport scroll; edge auto-scroll; copy of history rows.
- Gate: `cargo test -p heca app::selection_model::tests` green. Stop for review.

### Slice 3 — Actions + keybindings + RPC + interaction policy (Q8)
- Five `WmAction` variants + full 11-step treatment + `interaction.rs`
  (`FocusedPaneLocal`) + RPC.
- `keybindings.default.toml`: add the normal-mode + selection-mode bindings.
- `config.default.toml` + `SettingsConfig`: `terminal_mouse`, wheel lines,
  scrollback lines.
- Wheel policy in `events.rs`/`terminal_host.rs` (dispatch `ScrollbackLine` after
  the `terminal_mouse && (!is_mouse_grabbed || shift_held)` check).
- Snap-to-bottom on forwarded key input + on output-if-at-bottom (Q5).
- Tests: `default_selection_mode_bindings_resolve` extended; action dispatch;
  wheel routing both branches; snap policy.
- Gate: clippy clean + tests green. Stop for review.

### Slice 4 — AppState observability mirror (Q1)
- Chrome store per-pane `terminal_viewport` signal + `ChromeEvent::TerminalViewportChanged`
  + `host.terminal_viewport(pane_id)` selector. Mirror from snapshot each frame.
- Tests: signal updates on scroll; event emitted; selector returns correct values.
- Gate: tests green. Stop for review.

### Slice 5 — Animated viewport offset (Q7.1)
- Animate `viewport_offset` toward target (easing via `tick(dt)`), modeled on
  `heca-core/src/layout/view_offset.rs`. Wheel/PageUp set target; render loop eases.
- Tests: animation tween reaches target; instant fallback for `prefix+PageUp`
  entry if desired.
- Gate: runtime validation. Stop for review.

### Slice 6 — GUI widgets: scrollbar + scrolled-up indicator (Q7.2/7.3)
- New generic theme-driven `heca-grid-ui` widgets (catalog + `docs/widgets.md` +
  showcase), composed in `terminal_render.rs`. Scrollbar clickable/draggable to
  jump; indicator click-to-snap-to-bottom.
- Tests: widget unit tests + showcase builds.
- Gate: showcase runs; runtime validation. Stop for review.

### Slice 7 — Docs + Rust-skill review + commit
- Update `README.md`, `src/keybindings.md`, `BACKLOG.md`,
  `terminal-implementation.md` HANDOFF, `config.default.toml`/`keybindings.default.toml`
  comments.
- Run `~/.agents/skills/rust/SKILL.md` review + `cargo clippy --workspace
  --all-targets --all-features` clean + `cargo test --workspace` green.
- Stop before commit; hand back to user for review.

---

## 8. Validation to run after each slice

- `cargo check -p heca` / `cargo check -p heca-core` / `cargo check -p heca-config`
  as relevant.
- `cargo test -p heca-core` (backend damage + new viewport tests).
- `cargo test -p heca app::terminal_render::tests`, `app::selection_model::tests`,
  `app::interaction::tests`, `app::registry` (default bindings test).
- `cargo test -p heca-config` (loader/keys/deep_merge tests).
- `cargo clippy --workspace --all-targets --all-features` (0 warnings) before commit.
- `cargo run -p heca-renderer --example showcase` after slice 6 (widget showcase).
- Runtime validation (user): shell-prompt wheel scrollback, PageUp/PageDown in less/
  nvim (forwarded, not intercepted), `prefix+PageUp` enters scroll mode, selection
  into history + copy, scrollbar drag, indicator click-to-snap.

---

## 9. Resume protocol for the fresh session

1. Confirm you are in `/Users/antonio/projects/myvim-terminal-followups`.
2. `git status` — tree should be clean at `1f54d91`. If dirty, sync `origin/main`
   first (the user explicitly synced it on 2026-06-24).
3. Read in this order: this handoff → `AGENTS.md` → `BACKLOG.md` →
   `terminal-implementation.md` → `handoff-terminal-00-foundation.md`.
4. Read the actual config/keybinding files (NOT just the plan doc):
   `keybindings.default.toml`, `config.default.toml`, `heca-config/src/loader.rs`,
   `heca-config/src/keys.rs`, `heca-config/src/settings.rs`.
5. Start slice 1 (backend viewport model). Do NOT start UI/actions until slice 1 is
   green and reviewed.
6. After each slice: run the focused validation, update `BACKLOG.md` +
   `terminal-implementation.md` HANDOFF status, stop for user review.
7. No commit until the user explicitly asks. No `cargo fmt`. No hardcoded colors.
   No registry bypasses. No `#[allow(dead_code)]` without a comment.

### What NOT to do
- Do NOT re-decide the locked grill decisions (Q1-Q8 + terminal_mouse + GUI-first).
- Do NOT fold incremental/dirty-row viewport damage into this phase (Q6 = Full;
  `terminal-01` is separate).
- Do NOT use `prefix+[` for selection mode (it's `prev_pane`). Use `prefix+s`.
- Do NOT reuse the global `settings.mouse` for terminal wheel gating — use the
  new terminal-only `terminal_mouse`.
- Do NOT keep viewport offset host-side only without the AppState observability
  mirror (Q1 hybrid requires BOTH).
- Do NOT store selection in visible-row coords (Q4 = stable-row).
- Do NOT build the scrollbar/indicator as inline hardcoded overlays — they are
  proper `heca-grid-ui` widgets (Q7 + AGENTS widget rules).
- Do NOT patch blur/compositor ad hoc (that's `app-07` / compositor plan, separate).
- Do NOT touch the main checkout `/Users/antonio/projects/myvim` for this task.