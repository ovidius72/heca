# Shared Tasks

## Purpose

This file is the coordination board for delegated work on the post-merge
terminal backlog.

Rules:

1. Only one active task at a time unless a task explicitly says it can be parallelized.
2. The assigned agent updates only the task they are working on.
3. When the assigned agent finishes, they append notes under `Agent Completion`.
4. The reviewer then marks the task:
   - `Accepted`
   - `Needs Edit`
   - `Rejected`
5. If review requests changes, the same task stays open and the assigned agent appends fixes under the same task.

Review policy:

- acceptance is based on code, verification, and architecture
- completion notes alone are not sufficient
- terminal-only shortcuts are not acceptable when the task requires shared host capability behavior
- no hardcoded feature-key behavior is acceptable for keyboard interactions; feature keys must route through `WmAction` + registry + keymaps + config

Status values:

- `Open`
- `In Progress`
- `Completed by Agent`
- `Accepted`
- `Needs Edit`
- `Rejected`

---

## Accepted Tasks

### Task 01 — Shared Selection State Model

**Status:** Accepted

**Delivered**

- introduced a host-owned shared selection model in
  [heca/src/app/selection_model.rs](/Users/antonio/projects/heca/heca/src/app/selection_model.rs)
- added shared concepts:
  - `SelectionOwner`
  - `SelectionSource`
  - `SelectionPhase`
  - `SelectionRenderMode`
  - `SelectionRegion`
  - `ActiveSelection`
  - `SelectionState`
- wired the shared state into:
  - [heca/src/app_state.rs](/Users/antonio/projects/heca/heca/src/app_state.rs)
  - [heca/src/app/startup.rs](/Users/antonio/projects/heca/heca/src/app/startup.rs)
  - [heca/src/app/mod.rs](/Users/antonio/projects/heca/heca/src/app/mod.rs)

**Acceptance Basis**

- shared model is host-owned, not terminal-only
- state invariants are encoded in types, not parallel flags
- render mode is derived from region shape
- verification passed:
  - `cargo check -p heca`
  - `cargo clippy -p heca --all-targets`

### Task 02 — Selection Actions and Input Mode

**Status:** Accepted

**Delivered**

- added shared selection actions to
  [heca/src/input.rs](/Users/antonio/projects/heca/heca/src/input.rs):
  - `EnterSelectionMode`
  - `ClearSelection`
  - `CopySelection`
  - `PasteClipboard`
- registered descriptors and handlers in:
  - [heca/src/actions.rs](/Users/antonio/projects/heca/heca/src/actions.rs)
  - [heca/src/handlers.rs](/Users/antonio/projects/heca/heca/src/handlers.rs)
  - [heca/src/app/registry.rs](/Users/antonio/projects/heca/heca/src/app/registry.rs)
- added `InputMode::Selection` and selection-mode keyboard plumbing in:
  - [heca/src/app_state.rs](/Users/antonio/projects/heca/heca/src/app_state.rs)
  - [heca/src/app/input.rs](/Users/antonio/projects/heca/heca/src/app/input.rs)
- wired RPC surface in:
  - [heca/src/rpc.rs](/Users/antonio/projects/heca/heca/src/rpc.rs)
- documented bindings in:
  - [heca-config/src/keys.rs](/Users/antonio/projects/heca/heca-config/src/keys.rs)
  - [keybindings.toml](/Users/antonio/projects/heca/keybindings.toml)

**Acceptance Basis**

- actions are generic, not terminal-only
- selection mode is represented in `InputMode`
- clear-selection goes through the action architecture
- prefix timeout behavior is preserved when entering prefix from selection mode
- selection actions are reachable from keyboard and RPC
- verification passed:
  - `cargo check -p heca`
  - `cargo clippy -p heca --all-targets`
  - `cargo test -p heca`

---

### Task 03 — Terminal Selection Adapter and Overlay

**Status:** Accepted

**Goal**

Make terminal panes the first concrete consumer of the shared host selection
capability.

This task must connect the accepted shared selection model and action surface to
the terminal pane path so a terminal pane can:

- begin a host-rendered selection
- update that selection from terminal cell coordinates
- confirm / clear that selection through the existing shared actions
- render a visible selection overlay inside terminal content bounds

This is the first real backend adapter task for Phase 9.

**Scope**

This task is limited to terminal panes as the first backend implementation of
the shared selection contract.

It should implement:

- terminal-side selection ownership using the shared `SelectionState`
- terminal cell hit conversion through the existing terminal host path
- explicit pointer entry gesture for terminal selection
- host-rendered selection overlay for terminal panes
- selection clearing consistency with `InputMode::Selection`

It should not yet implement:

- copying selected text
- clipboard integration
- paste / bracketed paste / `OSC 52`
- browser-native selection
- custom Neovim GUI selection
- final global mouse policy across every pane type

**Required Architecture**

The implementation must follow the shared selection contract in
[terminal-implementation.md](/Users/antonio/projects/heca/terminal-implementation.md):

- selection remains a host capability
- terminal is only the first backend consumer
- terminal selection must use the shared action/model layer already landed
- selection overlay must draw only inside terminal content bounds
- TUI mouse behavior must be preserved by using an explicit entry gesture
- keyboard feature behavior must not be hardcoded in the mode handler; movement and copy/clear semantics must resolve through actions and keymaps

For this task, use this interaction policy:

- terminal host selection entry path may remain `Shift + left-drag`
- plain terminal mouse input must continue to go to the terminal backend
- the gesture is only the entry path; the selection state itself must remain shared

**Suggested Files**

- [heca/src/app/terminal_host.rs](/Users/antonio/projects/heca/heca/src/app/terminal_host.rs)
- [heca/src/app/events.rs](/Users/antonio/projects/heca/heca/src/app/events.rs)
- [heca/src/app/render.rs](/Users/antonio/projects/heca/heca/src/app/render.rs)
- [heca-renderer/src/terminal.rs](/Users/antonio/projects/heca/heca-renderer/src/terminal.rs)
- [heca/src/app_state.rs](/Users/antonio/projects/heca/heca/src/app_state.rs)
- [heca/src/app/selection_model.rs](/Users/antonio/projects/heca/heca/src/app/selection_model.rs)

Avoid unless strictly necessary:

- sidebar / `heca-grid-ui` migration files
- browser-related code
- clipboard/platform files
- global chrome/shell refactors

**Implementation Requirements**

1. Terminal pane ownership
- terminal selections must be stored as shared host selections owned by the focused pane
- use `SelectionOwner::Pane(...)`

2. Cell-space integration
- selection begin/update must use terminal cell coordinates, not raw pixels
- use the existing terminal host geometry/cell conversion path
- do not introduce a second ad hoc terminal selection coordinate model

3. Overlay rendering
- selection overlay must be drawn in the terminal renderer path
- overlay must clip to `pane_content_rect`
- overlay must not paint into pane chrome
- overlay must read from shared selection state, not terminal-only temporary fields

4. Input policy
- `Shift + left-drag` begins host selection in terminal panes
- drag updates focus cell while selecting
- mouse release ends the selecting phase and leaves a selected region active
- plain left click should not break existing terminal mouse forwarding
- clearing selection must use the shared action/state path already landed

5. No architecture drift
- no new terminal-only selection state in `AppState`
- no new renderer-only selection cache that becomes the source of truth
- no clipboard behavior
- no hidden coupling to browser or future Neovim GUI assumptions

**Acceptance Criteria**

- terminal panes can create a host-owned selection using the shared model
- selection overlay is visible in terminal panes
- overlay stays inside terminal content bounds
- selection state is owned by the shared host model, not terminal-only state
- explicit terminal mouse forwarding still works when not using the selection gesture
- no `#[allow]` / `#[expect]` shortcuts are introduced to force it through
- code compiles and tests stay green

**Verification**

At minimum:

```bash
cargo check -p heca
cargo check -p heca-renderer
cargo clippy -p heca --all-targets
cargo clippy -p heca-renderer --all-targets
cargo test -p heca
```

If practical, also live-check:

1. `Shift + left-drag` selects terminal text visually
2. plain terminal mouse behavior in `nvim` still works
3. selection overlay does not bleed into pane borders or status chrome

**Delivered**

- Added `SelectionOverlay` struct and `render_selection_overlay` method to `TerminalRenderer` in `heca-renderer/src/terminal.rs`:
  - `SelectionOverlay` carries raw cell coordinates (`anchor_row`, `anchor_col`, `focus_row`, `focus_col`) and a `color: [f32; 4]`. The renderer is agnostic of the shared selection model — it receives plain coordinates from the app layer.
  - `render_selection_overlay` draws a semi-transparent rectangle over the cell range from `(min_row, min_col)` to `(max_row, max_col)` inclusive, clipped to the fitted grid. It is called after cell backgrounds and glyphs (so text remains readable) and before the cursor overlay (so the cursor is always visible on top).
- Modified `heca/src/app/terminal_host.rs` to intercept `Shift + left-drag` for host selection:
  - Added `cell_coords_at_position` helper that reuses the same geometry path as `build_mouse_event` (one coordinate model for both selection and terminal forwarding).
  - `forward_mouse_button`: on `Shift + LeftButton Pressed`, calls `state.selection.begin(SelectionOwner::Pane(pane_id), SelectionSource::MouseDrag, SelectionRegion::HostGrid { ... })` and skips terminal forwarding. On `Shift + LeftButton Released`, calls `state.selection.end()` if a selection is in progress.
  - `forward_mouse_move`: if `state.selection.is_selecting()` and the owner matches the hovered pane, calls `state.selection.update_focus(row, col)` and skips terminal forwarding.
  - Plain left-click (no Shift) continues to forward to the terminal backend as before — TUI mouse behavior is preserved.
- Wired the selection overlay into `render_terminal_mount` in `heca/src/app/render.rs`:
  - Added `selection_overlay_for_pane` helper that reads from `state.selection` (the shared host model) and returns `Some(SelectionOverlay)` only when the selection is active, owned by the given pane, and uses `HostGrid` render mode. The overlay color is derived from `theme.accent` with 0.25 alpha.
  - `render_terminal_mount` now takes an `Option<SelectionOverlay>` parameter and draws it between `render_snapshot` and `render_cursor_overlay`.
  - Both call sites (tiled panes and floating panes) compute the overlay via `selection_overlay_for_pane` and pass it in.
- No new terminal-only state in `AppState` — all selection state lives in the shared `AppState.selection` model.
- No new renderer-only selection cache — the overlay reads from the shared model each frame.
- No clipboard behavior, no browser/Neovim GUI coupling.
- No `#[allow]` / `#[expect]` shortcuts introduced.
- Did not touch sidebar / `heca-grid-ui`, browser, or clipboard files.

**Post-implementation review fixes applied**

- **rust-skills review (high):**
  - H1: `render_selection_overlay` now draws 1 rectangle instead of N per-row draw calls.
  - H2: `SelectionOverlay` uses `min_row`/`max_row`/`min_col`/`max_col` bounding box. The anchor/focus → min/max conversion moved to the app layer (`build_selection_overlay`); the renderer stays agnostic of selection-model semantics.
  - H3: Extracted shared `cell_coords_in_rect` helper. Both `build_mouse_event` (terminal forwarding) and `cell_coords_at_position` (host selection) call it — no more duplicated geometry logic.
- **rust-skills review (medium):**
  - M1: Shift+Left release now checks `state.selection.owner() == Some(SelectionOwner::Pane(target.pane_id))` before ending — won't incorrectly confirm a keyboard- or RPC-started selection.
  - M2: Shift+Left press checks `backend.terminal_snapshot().is_some()` before beginning a `HostGrid` selection — future non-terminal panes won't get false grid selections.
  - M4: Extracted pure `build_selection_overlay` function + 4 unit tests (inactive, owner mismatch, BackendNative, HostGrid overlay values).
- **rust-skills review (low):**
  - L2: Added `SelectionOverlay::new()` constructor.
  - L3: Doc comment reworded to clarify caller responsibility for ordering.
  - L4: Added comment explaining why `owner_id` is used (not hovered pane) for coordinate lookup during drag.
- **rust-best-practices review:**
  - Improved `build_selection_overlay` doc comment (was "Pure logic extracted for unit testing" — now describes what the function does independently).
- **Coordinator review fix:**
  - Fixed `Shift+drag` not ending when released outside the owning pane. The `Released` path was blocked by `terminal_target_at_position(state, pos)` which returns `None` outside any pane. Restructured: release now runs **before** the target lookup and uses `source() == Some(SelectionSource::MouseDrag)` as the guard (instead of `target.pane_id`). This correctly ends mouse-drag selections even when the pointer leaves the pane, while keyboard-/RPC-started selections are unaffected.
- **Deferred:** M3 (content-rect duplication between terminal_host.rs and render.rs) — pre-existing, not addressed in this pass.

**Verification**

```bash
cargo check -p heca                       # passes
cargo check -p heca-renderer              # passes
cargo clippy -p heca --all-targets       # passes
cargo clippy -p heca-renderer --all-targets  # passes
cargo clippy --workspace --all-targets --all-features  # passes
cargo build -p heca                       # passes
cargo test -p heca                        # 158 passed (+4 new)
cargo test -p heca-renderer               # 7 passed
```

**Warnings**

The selection model warnings dropped from 9 to 7 — `active()`, `owner`, `begin()`, `update_focus()`, `end()`, `clear()`, and `is_selecting()` are now consumed by the terminal host adapter and renderer. The remaining warnings are:
- `KeyboardMode` / `Rpc` never constructed (only `MouseDrag` is used)
- `SelectionPhase` / `SelectionRenderMode` never used directly (derived from region/enum variant)
- `BackendNative` never constructed (only `HostGrid` is used for terminal selection)
- `render_mode` methods on `SelectionRegion` / `ActiveSelection` never used
- `phase`, `source`, `render_mode`, `region`, `is_active`, `has_selection` accessors never used

These will resolve as future tasks add keyboard-driven selection mode, RPC-initiated selection, and backend-native selection paths.

**Acceptance Basis**

- terminal panes are the first concrete consumer of the shared host selection model
- selection ownership is stored in shared host state, not terminal-only fields
- the renderer consumes host-provided overlay spans and stays agnostic of the selection model
- `Shift + left-drag` is an explicit selection entry path without breaking ordinary TUI mouse forwarding
- selection mode movement is action-driven through registry + keymaps, not hardcoded raw feature keys
- move/swap gesture coexistence is preserved:
  - `Shift + drag` selects
  - interactive move modifier + drag moves
  - interactive move modifier + `Shift` + drag still reaches move/swap behavior
- release outside the pane still confirms mouse-drag selection
- verification passed:
  - `cargo check -p heca`
  - `cargo check -p heca-renderer`
  - `cargo clippy -p heca --all-targets`
  - `cargo clippy -p heca-renderer --all-targets`
  - `cargo test -p heca`
  - `cargo test -p heca-renderer`

---

## Task 04 — Shared Selection Text Extraction and Copy Action

**Status:** Open

**Goal**

Complete the next planned selection slice by making the active shared selection
actually copyable.

This task must:

- extract selected text from the active selection owner
- implement `CopySelection` end-to-end
- write the copied text to the system clipboard
- preserve the shared host-selection architecture

This is the first clipboard-facing task, but it is still scoped to **copy of
existing selection only**. It must not implement paste yet.

**Why this task exists**

Selection visuals are already implemented for terminal panes. The next missing
user-visible capability is converting that shared selection into real text and
copying it through the existing action system.

This task should finish the “selection is useful” milestone before we start
paste / bracketed paste / `OSC 52`.

**Scope**

This task should implement:

- shared selection text extraction for terminal panes
- system clipboard write for `CopySelection`
- action/registry/RPC integration for copy
- no-op / safe handling when there is no active selection

This task should not implement:

- paste
- bracketed paste
- `OSC 52`
- browser-native selection extraction
- Neovim GUI selection extraction
- selection search / scrollback search
- selection rendering changes unless strictly required by text extraction

**Required Architecture**

The implementation must preserve these rules:

1. Selection is still host-owned.
- Do not add terminal-only “copied text” state.
- Do not make the renderer the source of truth.
- Do not bypass the existing `SelectionState`.

2. Copy must go through the action system.
- `WmAction::CopySelection` must remain the action entry point.
- mouse/UI, keyboard bindings, and RPC should all reach the same action path.
- do not add a raw “copy this selection now” helper that bypasses the registry.

3. Extraction logic belongs at the pane/backend adapter layer.
- The terminal backend snapshot already has the visible cell grid.
- Selection text extraction should operate from snapshot/grid data and shared
  selection coordinates.
- The clipboard layer should receive a final `String`, not know terminal cell semantics.

4. Design for future multi-surface selection owners.
- This task may initially support `SelectionOwner::Pane(...)` for terminal panes only.
- But the API shape must allow future browser / Neovim GUI / backend-native owners.
- If a selection owner is unsupported for extraction, fail cleanly rather than
  forcing terminal assumptions onto every owner.

5. No hardcoded feature keys.
- If you touch bindings or selection mode behavior, keep them action/keymap/config-driven.

**Expected Behavior**

For terminal panes:

1. If there is an active host-grid selection owned by a terminal pane:
- `CopySelection` extracts text from that selection
- writes it to the system clipboard
- leaves the selection active unless there is an explicit reason to clear it

2. If there is no active selection:
- `CopySelection` is a safe no-op
- no panic
- no bogus clipboard write

3. If the owner is not yet extractable:
- do not panic
- either no-op or log in debug builds
- keep the contract future-friendly

**Terminal Extraction Contract**

The agent must implement and document the exact text semantics used for terminal selection extraction.

Use these rules unless the codebase already has a stronger established contract:

1. Host-grid selection coordinates are inclusive cell coordinates.

2. Row semantics:
- single-row selection copies cells from `start_col..=end_col`
- multi-row selection copies:
  - first row: `start_col..end_of_row`
  - middle rows: full logical selected row span
  - last row: `0..=end_col`

3. Text content comes from visible snapshot cells.
- use the visible `TerminalSnapshot`
- do not talk to the PTY directly for selection extraction
- do not invent a second shadow text model

4. Wide-character handling:
- do not duplicate trailing filler cells
- only copy the logical anchor cell text for a wide grapheme
- extraction must respect the existing terminal cell width model

5. Whitespace handling:
- preserve internal spaces exactly
- trim only row-end cells that are visually blank filler if the snapshot model
  clearly distinguishes them as empty trailing space
- be explicit in code comments about what is and is not trimmed

6. Newlines:
- join copied rows with `\n`
- do not append an extra trailing newline after the last selected row unless
  the selected content itself requires it

7. Backend-native selection:
- if `SelectionRegion::BackendNative`, do not try to reinterpret it as host-grid text
- return `None` / unsupported cleanly for now

**Clipboard Contract**

Clipboard integration must be:

- system clipboard, not an internal scratch buffer
- action-driven
- safe if clipboard write fails

Preferred behavior:

- write selected text to the clipboard
- if clipboard write fails, keep app state coherent and report only in debug logs unless
  the repo already has a user-facing error path for clipboard failures

Do not:

- clear selection automatically just because copy succeeded
- tie copy implementation to terminal paste implementation
- add terminal-protocol clipboard behavior here

**Suggested Files**

- [heca/src/handlers.rs](/Users/antonio/projects/heca/heca/src/handlers.rs)
- [heca/src/app/terminal_host.rs](/Users/antonio/projects/heca/heca/src/app/terminal_host.rs)
- [heca/src/app/selection_model.rs](/Users/antonio/projects/heca/heca/src/app/selection_model.rs)
- [heca-core/src/backend/snapshot.rs](/Users/antonio/projects/heca/heca-core/src/backend/snapshot.rs)
- [heca/src/rpc.rs](/Users/antonio/projects/heca/heca/src/rpc.rs)
- any existing clipboard/platform helper files if already present in the repo

Avoid unless strictly necessary:

- renderer files
- `heca-grid-ui` files
- paste/protocol files
- browser and future Neovim GUI code

**Implementation Requirements**

1. Extraction helper
- add one clear extraction helper for terminal selections
- it should take:
  - the shared selection state or active selection
  - the owning pane/backend snapshot
- it should return:
  - `Option<String>` or a small result type that can represent unsupported/empty/failure cleanly

2. Handler wiring
- `handle_copy_selection` must become real
- route through the registry path already established
- no direct bypass from input handlers

3. Owner/type checks
- only terminal-pane host-grid selection needs to work in this task
- explicitly guard other cases

4. Clipboard write
- use the project’s preferred platform clipboard path if one exists
- if no helper exists, implement the smallest correct cross-platform path in app code
- keep the clipboard write localized; do not spread platform branches through selection logic

5. Tests
- add pure logic tests for text extraction
- include:
  - single-line selection
  - multi-line selection
  - reverse anchor/focus selection
  - wide-character / filler-cell case if the snapshot model supports it
  - inactive selection
  - wrong owner / unsupported region

**Acceptance Criteria**

- `CopySelection` copies text from an active terminal host-grid selection
- copied text respects row semantics and does not duplicate wide-char filler cells
- no panic when there is no selection or when the owner is unsupported
- clipboard write is triggered only through the action path
- shared host-selection architecture remains intact
- no raw feature-key shortcuts are added
- no paste behavior is added in this task

**Verification**

At minimum:

```bash
cargo check -p heca
cargo check -p heca-core
cargo clippy -p heca --all-targets
cargo clippy -p heca-core --all-targets
cargo test -p heca
cargo test -p heca-core
```

If practical, also live-check:

1. select text in a terminal pane
2. trigger `CopySelection`
3. paste into another app and confirm the copied text matches the selected terminal text
4. reverse-direction selections copy correctly
5. a no-selection copy does nothing harmful

**Agent Completion**

- not started

**Reviewer Decision**

- pending

**Reviewer Notes**

- review must focus on extraction semantics first, not just “clipboard seems to work”
- reject if the implementation duplicates wide-character filler cells or invents a terminal-only parallel selection source of truth
  movement behavior yet.
  The shared selection mode currently does not move the selection focus with
  `h/j/k/l` or arrow keys, so the mode is not usable as a tmux-like keyboard
  selection mode yet.
- **Medium**: keyboard movement must not be implemented by matching raw keys
  directly inside `handle_selection_mode`.
  Selection movement must use `WmAction` variants resolved through the existing
  keybinding/mode-keymap infrastructure, consistent with the project-wide
  action architecture.
- **Medium**: the entry gesture/binding contract is not yet settled.
  `prefix+s` appeared as a delegated default binding, not as an approved final
  UX decision. Treat it as provisional and keep the implementation/config path
  flexible.
- **Medium**: `Shift + click/drag` must be verified against pane move/drag
  interactions.
  The selection entry gesture must not conflict with pane move, floating drag,
  resize handles, or normal terminal mouse forwarding.
- **Medium**: the current selection overlay does not follow text semantics.
  It paints a rectangular cell region, but terminal selection behavior needs to
  follow text/line semantics rather than only showing a coarse area overlay.
- **Medium**: there is currently no reliable exit path from mouse-started
  selection.
  The user reported that after `Shift + click` starts selection, `Esc`,
  clicking away, and double-clicking do not reliably clear/exit the selection
  state. The next fix pass must define and implement explicit exit behavior for
  cancel/clear/confirm.
- Verification rerun by reviewer:
  - `cargo check -p heca`
  - `cargo check -p heca-renderer`
  - `cargo clippy -p heca --all-targets`
  - `cargo clippy -p heca-renderer --all-targets`
  - `cargo test -p heca`
  - `cargo test -p heca-renderer`

---

## Task 04 — Terminal Surface Transparency

**Status:** Open

**Goal**

Add terminal pane transparency now, using the already-merged appearance
contract, without waiting for the future reusable in-app blur pass.

This task is specifically about **transparency**, not blur.

**Scope**

This task should:

- make terminal pane surfaces participate in the existing transparency model
- keep terminal text, cursor, borders, and symbols visually crisp
- keep the implementation reusable for later blur integration

This task must not:

- implement a terminal-only blur path
- implement clipboard or selection work
- change the shared selection model
- refactor sidebar / dock / chrome-region code

**Background**

Current merged behavior:

- the window can already be transparent via
  [heca-config/src/appearance.rs](/Users/antonio/projects/heca/heca-config/src/appearance.rs)
- OS vibrancy/backdrop is already applied at window creation in
  [heca/src/app/startup.rs](/Users/antonio/projects/heca/heca/src/app/startup.rs)
- chrome panels already use alpha-based translucent fills in
  [heca/src/app/render.rs](/Users/antonio/projects/heca/heca/src/app/render.rs)
  and [heca/src/chrome.rs](/Users/antonio/projects/heca/heca/src/chrome.rs)
- the current compositor in
  [heca-renderer/src/composite.rs](/Users/antonio/projects/heca/heca-renderer/src/composite.rs)
  is only a persistent scene texture + blit; it does **not** perform blur yet

So this task should reuse the existing transparency contract and avoid
pretending that blur is already available.

**Required Architecture**

- transparency must be applied as a **surface/background policy**
- terminal foreground content remains opaque:
  - glyphs
  - cursor
  - underline/undercurl
  - box-drawing
  - powerline symbols
- the solution must stay compatible with a future shared blur-surface pass
- do not put blur logic in
  [heca-renderer/src/terminal.rs](/Users/antonio/projects/heca/heca-renderer/src/terminal.rs)

**Suggested Files**

- [heca/src/app/render.rs](/Users/antonio/projects/heca/heca/src/app/render.rs)
- [heca/src/app/terminal_host.rs](/Users/antonio/projects/heca/heca/src/app/terminal_host.rs)
- [heca-renderer/src/terminal.rs](/Users/antonio/projects/heca/heca-renderer/src/terminal.rs)
- [heca-config/src/appearance.rs](/Users/antonio/projects/heca/heca-config/src/appearance.rs)
- [heca-config/src/theme.rs](/Users/antonio/projects/heca/heca-config/src/theme.rs)

Avoid unless strictly necessary:

- sidebar/chrome-region widget files
- `heca-grid-ui` structural widgets
- compositor blur implementation files

**Implementation Requirements**

1. Surface alpha only
- terminal pane background must become alpha-aware
- alpha should come from the existing appearance/theme policy, not a terminal-only literal

2. Opaque foreground
- text and cursor must remain fully readable
- do not globally fade terminal glyphs with the pane background

3. Correct clipping
- transparency must still respect terminal content rect clipping
- no bleed into pane chrome or neighboring panes

4. Future compatibility
- the shape of the code should make it easy to replace “alpha background only”
  with “blurred backdrop + alpha background” later

**Acceptance Criteria**

- terminal panes visually participate in transparency
- terminal text/cursor remain crisp
- no terminal-specific blur implementation is introduced
- code compiles and tests remain green

**Verification**

At minimum:

```bash
cargo check -p heca
cargo check -p heca-renderer
cargo clippy -p heca --all-targets
cargo clippy -p heca-renderer --all-targets
```

If practical, also live-check:

1. terminal pane background is visibly translucent
2. terminal text stays fully legible
3. transparent panes still clip correctly in float/zoom states

**Agent Completion**

- Pending.

**Reviewer Decision**

- Pending.

**Reviewer Notes**

- Pending.

---

## Task 05 — heca-grid-ui Pane Replacement and Pane Gap Config

**Status:** Open

**Goal**

Replace the current custom pane container presentation with the existing
`heca-grid-ui` pane widget/path, and make pane chrome geometry configurable
through `config.toml`.

This task is about the **pane surface/container**, not sidebar replacement.

**Source Material**

This task must be implemented against the real current widget API, not guessed.
Read first:

- [docs/widgets.md](/Users/antonio/projects/heca/docs/widgets.md)
- showcase usage in
  [heca-renderer/examples/showcase.rs](/Users/antonio/projects/heca/heca-renderer/examples/showcase.rs)

Relevant facts from the current implementation/docs:

- `heca-grid-ui` already has a `Pane` widget
- pane/card/widget border width and radius are theme-driven
- the showcase explicitly demonstrates runtime-configurable radius and border width
- layout gaps are part of the widget/layout vocabulary already

**Scope**

This task should:

- replace pane presentation/container chrome with the `heca-grid-ui` pane path
- keep terminal/content rendering mounted inside the pane content area
- make pane border width configurable through app config/theme
- make pane radius configurable through app config/theme
- add a user-configurable pane gap setting

This task must not:

- redesign sidebar shell behavior
- refactor workspace tree semantics
- change terminal backend semantics
- implement blur itself

**Required Architecture**

- pane presentation should move toward the future `heca-grid-ui` shell/container direction
- terminal content must remain mounted **inside** the pane surface, not become the owner of pane chrome again
- border/radius/gap must come from config/theme, not hardcoded render literals
- gap must be a layout/input to pane positioning, not just a visual fake margin

**Suggested Files**

- [heca/src/app/render.rs](/Users/antonio/projects/heca/heca/src/app/render.rs)
- [heca/src/chrome.rs](/Users/antonio/projects/heca/heca/src/chrome.rs)
- [heca-config/src/theme.rs](/Users/antonio/projects/heca/heca-config/src/theme.rs)
- [heca-config/src/loader.rs](/Users/antonio/projects/heca/heca-config/src/loader.rs)
- [heca-config/src/defaults.rs](/Users/antonio/projects/heca/heca-config/src/defaults.rs)
- `heca-grid-ui` pane-related widget files only if needed after reading docs

Avoid unless required:

- sidebar model/drag files
- browser-specific code
- terminal backend/core files

**Implementation Requirements**

1. Pane widget path
- use the existing `heca-grid-ui` pane/container capability rather than inventing a new custom pane chrome path
- keep terminal and other pane contents rendered inside the resulting content rect

2. Configurable border and radius
- pane border width must be configurable via `config.toml`
- pane radius must be configurable via `config.toml`
- values must flow through theme/config, not be hardcoded in render code

3. Configurable pane gap
- add a pane gap setting in config
- pane gap must affect the actual layout spacing between panes/columns
- it must not be simulated only by shrinking visuals after layout

4. Backward compatibility
- existing panes must still render correctly after the container swap
- focus and active-pane treatment must still work
- float/zoom states must still respect clipping and pane separation

**Acceptance Criteria**

- pane container presentation uses the `heca-grid-ui` path
- pane border width is configurable from `config.toml`
- pane radius is configurable from `config.toml`
- pane gap is configurable from `config.toml`
- terminal pane content still renders correctly inside the pane content rect
- code compiles and tests remain green

**Verification**

At minimum:

```bash
cargo check -p heca
cargo check -p heca-grid-ui
cargo clippy -p heca --all-targets
cargo clippy -p heca-grid-ui --all-targets
```

If practical, also live-check:

1. changing pane radius in config changes the pane chrome
2. changing border width in config changes the pane chrome
3. changing pane gap in config visibly changes pane spacing
4. terminal panes still clip/render correctly inside the new pane surface

**Agent Completion**

- Pending.

**Reviewer Decision**

- Pending.

**Reviewer Notes**

- Pending.
