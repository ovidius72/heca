# Rule

This file is the source of truth for the terminal implementation. The `HANDOFF` section at the end of this file **must** be updated every time a phase is completed, whenever scope or architecture changes, whenever a blocker or regression is found, and on demand when another agent or session needs current context.

After any task is completed, the completed Rust code must be reviewed against the Rust skill rules before the task is considered done. If issues are found, they must be fixed and reviewed again until no issues remain. Do not use `#[allow(dead_code)]`, `#[expect(dead_code)]`, or similar suppression just to force code through review. When the task passes review, create a commit and ask the user to review it. If the user accepts, pull `origin/main`, sync, open a PR, and then start the next task.

---

# Terminal Implementation

## Purpose

This document defines the target architecture and execution plan for heca's terminal subsystem.

The chosen stack is:

- `portable-pty` for cross-platform PTY/process management
- `wezterm-term` for terminal emulation, escape parsing, cell state, scrollback, input encoding, and modern terminal protocol support
- `cosmic-text` for font shaping, fallback, rasterization, and user-configurable terminal font rendering

This keeps heca fully GUI-native:

- heca owns the window
- heca owns pane composition
- heca owns borders, overlays, animations, and clipping
- heca owns the GPU render loop
- terminal state is provided by a reusable terminal engine, not by an embedded widget

## Requirements

- Terminal font must be configurable by the end user through `config.toml`
- Terminal rendering must be GPU-native and pane-aware
- Full RGB/truecolor must be supported
- The architecture must be future-ready for richer glyph coverage, font fallback, inline images/graphics, hyperlinks, and advanced terminal attributes
- The implementation must avoid the current failure mode:
  - laggy rendering
  - text overlapping pane borders/content chrome
  - unstable redraw behavior
  - per-frame full-grid text reconstruction

## Non-Goals

- No embedded foreign terminal widget
- No second compositor or layout engine
- No renderer that treats terminal rows as generic UI labels
- No direct reuse of the current custom `vte` grid as the long-term model

## Design Principles

1. Separate terminal semantics from terminal rendering.
2. Render terminal content into a pane content rect, never the full pane rect.
3. Clip terminal drawing to the content rect.
4. Rebuild only changed terminal content, never the entire visible grid by default.
5. Keep input, PTY, model, renderer, and pane chrome as separate layers.
6. Keep terminal font configuration independent from UI font configuration, even if both use `cosmic-text`.
7. Preserve a clean path for Neovim and future pane types to reuse the renderer and pane plumbing where possible.

---

## Architecture Sketch

## Layer Model

### 1. PTY Layer

Owned by `portable-pty`.

Responsibilities:

- Create the PTY
- Spawn the shell or command
- Resize the PTY
- Provide a reader/writer pair
- Report process exit

This layer knows nothing about terminal escapes, fonts, colors, or panes.

### 2. Terminal Engine Layer

Owned by `wezterm-term`.

Responsibilities:

- Parse terminal output bytes
- Maintain the terminal model
- Track cursor state
- Track attributes and colors
- Track scrollback
- Encode keyboard and mouse input back to the terminal
- Support richer protocol state over time:
  - RGB colors
  - hyperlinks
  - image/graphics protocols supported by the engine

This layer knows nothing about `wgpu`, pane borders, or theme chrome.

### 3. Terminal Backend Adapter

New heca-owned adapter between `portable-pty` and `wezterm-term`.

Proposed location:

- `heca-core/src/backend/terminal/`

Responsibilities:

- Hold the PTY handle and child process state
- Feed PTY output bytes into `wezterm-term`
- Forward encoded keyboard and mouse input to the PTY writer
- Expose a cheap terminal snapshot interface for the renderer
- Track dirty state for redraw scheduling
- Track visible-region changes separately from scrollback/model changes

This layer is where `PaneBackend` integration happens.

### 4. Terminal Snapshot / Render Contract

New heca-owned render boundary between the backend adapter and the renderer.

Responsibilities:

- Expose viewport dimensions
- Expose terminal cell metrics
- Expose changed rows/ranges
- Expose cursor state
- Expose selection state later
- Expose graphics/image placements later
- Avoid `Vec<TerminalLine>` full copies each frame

This contract must be incremental by default.

### 5. Terminal Renderer

New dedicated renderer in `heca-renderer`.

Responsibilities:

- Draw cell backgrounds
- Draw glyphs
- Draw cursor
- Draw selection later
- Draw inline images/graphics later
- Respect pane content clipping
- Respect pane content inset from borders and overlays

This renderer should use `cosmic-text` for shaping/fallback/rasterization, but must not create and shape a fresh generic text buffer per terminal row every frame.

### 6. Pane Chrome Layer

Owned by heca app/rendering code.

Responsibilities:

- Pane borders
- Pane title area
- Focus ring
- Overlay badges
- Non-terminal UI text

This layer must never paint inside the terminal content rect except by deliberate overlay design.

---

## Proposed Code Structure

### `heca-core`

- `heca-core/src/backend/mod.rs`
  - `PaneBackend` trait remains the stable backend abstraction
- `heca-core/src/backend/terminal/mod.rs`
  - terminal backend public surface
- `heca-core/src/backend/terminal/pty.rs`
  - `portable-pty` integration
- `heca-core/src/backend/terminal/engine.rs`
  - `wezterm-term` state wrapper
- `heca-core/src/backend/terminal/input.rs`
  - keyboard/mouse encoding helpers
- `heca-core/src/backend/terminal/snapshot.rs`
  - incremental render snapshot types
- `heca-core/src/backend/terminal/config.rs`
  - terminal runtime settings derived from app config

### `heca-renderer`

- `heca-renderer/src/terminal.rs`
  - dedicated terminal render path
- `heca-renderer/src/text.rs`
  - remains general-purpose text renderer for UI/chrome
- future:
  - `heca-renderer/src/terminal_atlas.rs`
  - `heca-renderer/src/terminal_graphics.rs`

### `heca`

- `heca/src/app/render.rs`
  - computes pane chrome rect and pane content rect
  - calls terminal renderer with content rect
- `heca/src/main.rs`
  - schedules redraws from backend dirty events
- `heca/src/mouse.rs`
  - pane hit testing and pointer routing
  - delegates terminal mouse encoding to backend/input helpers

### `heca-config`

- Extend terminal-specific config:
  - `terminal_font_family`
  - `terminal_font_size`
  - `terminal_font_features` or equivalent shaping flags later
  - `terminal_line_height`
  - `terminal_letter_spacing` later if needed
  - `terminal_ligatures` as explicit opt-in or opt-out policy
  - terminal cursor style config later

---

## Rendering Model

## Pane Rect Separation

Every terminal pane must compute three rects:

1. `pane_outer_rect`
- full pane geometry

2. `pane_chrome_rect`
- title/border/header/footer chrome if present

3. `pane_content_rect`
- actual drawable terminal content area

Rules:

- Terminal backgrounds, glyphs, cursor, selection, and inline graphics draw only in `pane_content_rect`
- Border and pane title draw outside or around `pane_content_rect`
- GPU scissor/clipping must enforce the content boundary

This is the direct fix for the current overlap bug.

## Rendering Passes

Recommended render order for terminal panes:

1. pane background
2. terminal cell backgrounds
3. terminal glyphs
4. terminal cursor
5. terminal overlay layers later:
   - selection
   - IME composition
   - inline images/graphics
   - hyperlinks/hover accents
6. pane chrome and focus ring

Do not center the pane name inside terminal content.

## Text Strategy

Use `cosmic-text` for:

- font discovery
- font fallback
- glyph shaping
- rasterization

Do not use the generic UI `queue_text(...)` row batching strategy as the terminal renderer.

Instead:

- shape terminal runs through a terminal-specific path
- cache glyph rasterization aggressively
- avoid recreating full line buffers every frame
- group work by changed cells/runs, not by entire pane snapshots

## Color Strategy

Terminal colors must support:

- ANSI 16
- 256-color indexed mode
- 24-bit RGB truecolor
- future alpha/blending only where protocol semantics permit

Renderer colors should remain in linearized or consistent RGBA form for the GPU pipeline.

---

## Font Strategy

## User Configuration

The terminal font is user-configurable through `config.toml`.

The terminal font must be independent from the UI font even if both use the same backend.

Initial config targets:

```toml
[terminal]
font_family = "JetBrains Mono"
font_size = 14
line_height = 1.15
ligatures = false
```

Later expansion:

```toml
[terminal]
font_family = "JetBrains Mono"
font_size = 14
line_height = 1.15
ligatures = false
font_features = ["calt=0", "liga=0"]
fallback_families = ["Symbols Nerd Font", "Noto Color Emoji"]
cursor_style = "block"
```

## Ligature Policy

Terminal ligatures must be configurable.

Default recommendation:

- terminal ligatures off by default
- UI text shaping unrestricted

Reason:

- terminals are cell-based
- some ligatures can visually conflict with cursor motion, selection, or column reasoning

This does not block full Unicode support, fallback fonts, or richer glyph coverage.

---

## Why This Stack Has Three Libraries

### `portable-pty`

Needed because a terminal emulator must spawn and control a PTY-backed process.

Without it:

- no shell process
- no PTY resizing
- no cross-platform reader/writer lifecycle

### `wezterm-term`

Needed because PTY bytes are not directly renderable.

This layer translates bytes into terminal state:

- characters
- cursor movement
- color changes
- mouse modes
- scrollback
- hyperlinks
- richer protocols

### `cosmic-text`

Needed because terminal state is not directly drawable.

This layer turns text and attributes into rasterizable glyph output with:

- shaping
- fallback
- font selection
- user-configurable fonts

These are three separate concerns and should stay separate.

---

## Detailed Execution Plan

## Phase 0 — Decision Lock and Scope Reset

Goal:

- freeze the architecture decision
- stop further work on the old custom-grid path except for removal/migration

### Task 0.1 — Lock stack decision

Details:

- record `portable-pty + wezterm-term + cosmic-text` as the selected stack
- explicitly mark the old custom `vte` grid backend as transitional only

Files:

- `terminal-implementation.md`
- optional ADR later

Done when:

- no ambiguity remains about the chosen stack

### Task 0.2 — Identify migration boundaries

Details:

- inventory every place that assumes `BackendRenderData::Terminal { lines: Vec<TerminalLine>, ... }`
- inventory all places that assume full-pane rendering instead of content-rect rendering

Files:

- `heca-core/src/backend/mod.rs`
- `heca/src/app/render.rs`
- `heca-renderer/src/text.rs`

Done when:

- all incompatible assumptions are listed in the handoff

## Phase 1 — Backend Interface Redesign

Goal:

- redesign the terminal backend contract before integrating a real engine

### Task 1.1 — Define terminal snapshot types

Details:

- create terminal-specific snapshot structs
- support visible rows, dirty row ranges, cursor, dimensions, and future extension fields
- avoid `Vec<TerminalLine>` as the primary long-term transport

Files:

- `heca-core/src/backend/terminal/snapshot.rs`
- `heca-core/src/backend/mod.rs`

Done when:

- the renderer can depend on snapshots without needing the old full-copy model

### Task 1.2 — Extend `PaneBackend` without leaking renderer details

Details:

- add terminal-specific accessors or typed render data variants
- keep trait coherent for future Neovim/browser backends
- preserve clean separation between backend and GPU code

Files:

- `heca-core/src/backend/mod.rs`

Done when:

- terminal backends can expose incremental state safely

### Task 1.3 — Define redraw/dirty semantics

Details:

- distinguish:
  - PTY data arrived
  - viewport changed
  - cursor blink/state changed
  - terminal exited
- define what marks a pane as needing redraw

Files:

- `heca-core/src/backend/terminal/mod.rs`
- `heca-core/src/backend/terminal/snapshot.rs`

Done when:

- redraw policy is explicit and testable

## Phase 2 — PTY and Terminal Engine Integration

Goal:

- replace the custom terminal model with a `wezterm-term` based adapter

### Task 2.1 — Add PTY wrapper

Details:

- introduce a small wrapper around `portable-pty`
- encapsulate child, reader, writer, and resize logic

Files:

- `heca-core/src/backend/terminal/pty.rs`

Done when:

- PTY lifecycle is isolated from terminal parsing logic

### Task 2.2 — Add terminal engine wrapper

Details:

- wrap `wezterm-term`
- define initialization and screen sizing behavior
- feed output bytes into the engine

Files:

- `heca-core/src/backend/terminal/engine.rs`

Done when:

- terminal state updates from PTY bytes without renderer involvement

### Task 2.3 — Implement backend adapter

Details:

- assemble PTY wrapper + engine wrapper into `TerminalBackend`
- support process exit
- support resize
- support title changes if available later

Files:

- `heca-core/src/backend/terminal/mod.rs`

Done when:

- a backend instance can run a real shell end-to-end

### Task 2.4 — Add unit tests for backend lifecycle

Details:

- initialization
- resize propagation
- input write path
- exit detection

Files:

- `heca-core/src/backend/terminal/tests.rs` or inline test module

Done when:

- lifecycle behavior is covered without needing the full app

## Phase 3 — Dedicated Terminal Renderer

Goal:

- stop using the general UI text pipeline as the terminal renderer

### Task 3.1 — Create terminal renderer module

Details:

- add a dedicated renderer entrypoint in `heca-renderer`
- keep terminal rendering separate from generic chrome text rendering

Files:

- `heca-renderer/src/terminal.rs`
- `heca-renderer/src/lib.rs`

Done when:

- there is a terminal-specific render API

### Task 3.2 — Render cell backgrounds and cursor

Details:

- draw backgrounds from terminal cell attributes
- draw cursor using terminal state
- keep this independent from glyph rendering

Files:

- `heca-renderer/src/terminal.rs`

Done when:

- colored cells and cursor render correctly in isolation

### Task 3.3 — Add glyph rendering path using `cosmic-text`

Details:

- shape terminal runs with terminal font settings
- honor fallback fonts
- honor RGB foreground colors
- avoid per-frame full-pane generic text buffer recreation

Files:

- `heca-renderer/src/terminal.rs`
- `heca-renderer/src/text.rs` only if shared glyph cache helpers are extracted

Done when:

- visible text draws through the terminal renderer without using the old row-label path

### Task 3.4 — Add clipping/scissor support

Details:

- enforce clipping to pane content rect
- verify that glyphs and cursor cannot paint into border/title regions

Files:

- `heca-renderer/src/terminal.rs`
- `heca/src/app/render.rs`

Done when:

- overlap bug is structurally prevented

### Task 3.5 — Add dirty-region rendering strategy

Details:

- render only changed rows/ranges where possible
- define fallback for full invalidation

Files:

- `heca-renderer/src/terminal.rs`
- terminal snapshot types

Done when:

- redraw cost scales with changed content rather than full pane size

## Phase 4 — App Integration and Pane Geometry

Goal:

- integrate the new backend and renderer into live panes safely

### Task 4.1 — Compute pane content rect

Details:

- split pane outer rect from content rect
- reserve border/title space explicitly

Files:

- `heca/src/app/render.rs`
- any chrome helpers involved in pane geometry

Done when:

- terminal render calls receive content rect, not pane outer rect

### Task 4.2 — Remove center title overlay from terminal content

Details:

- stop drawing pane name in the middle of terminal panes
- move title rendering to chrome/header if needed

Files:

- `heca/src/app/render.rs`

Done when:

- no terminal text is visually obstructed by pane labels

### Task 4.3 — Replace fake startup backend path

Details:

- use the new real terminal backend at startup
- fail clearly if startup terminal creation is impossible

Files:

- `heca/src/app/startup.rs`

Done when:

- first pane is a real shell

### Task 4.4 — Replace pane creation sites

Details:

- use the real terminal backend for split/new-pane creation
- retain explicit fallback behavior only if the product wants degraded panes

Files:

- `heca/src/handlers.rs`
- `heca/src/app/mutations.rs`

Done when:

- new panes consistently open with the real backend

### Task 4.5 — Wire redraw scheduling

Details:

- trigger redraws from backend dirty notifications
- avoid redundant redraw floods

Files:

- `heca/src/main.rs`
- `heca/src/app/lifecycle.rs`
- backend adapter

Done when:

- terminal output appears promptly without pathological redraw loops

## Phase 5 — Input Fidelity

Goal:

- make the live terminal behave correctly as a terminal, not just as a renderer

### Task 5.1 — Keyboard input mapping

Details:

- encode normal keys, modifiers, arrows, function keys, and control sequences through the terminal engine/input layer

Files:

- `heca-core/src/backend/terminal/input.rs`
- app input plumbing

Done when:

- shell apps and TUIs receive correct keyboard input

### Task 5.2 — Mouse protocol forwarding

Details:

- use terminal engine aware mouse encoding
- support press/release/move/scroll as terminal modes require

Files:

- `heca-core/src/backend/terminal/input.rs`
- `heca/src/mouse.rs`
- `heca/src/main.rs`

Done when:

- mouse-aware terminal applications behave correctly

### Task 5.3 — Resize propagation

Details:

- convert pane content rect to terminal cols/rows
- resize PTY and engine consistently

Files:

- terminal backend adapter
- `heca/src/app/render.rs` or viewport sync path

Done when:

- resizing panes updates applications immediately and correctly

## Phase 6 — Font Config, Unicode, and Future-Rich Content

Goal:

- complete the terminal’s user-facing text behavior and future-ready hooks

### Task 6.1 — Add terminal font config surface

Details:

- add terminal-specific font family and size config
- load from `config.toml`
- support runtime reload if already available for config

Files:

- `heca-config/src/theme.rs` or split config module
- runtime reload plumbing

Done when:

- user can set terminal font independently of chrome/UI font

### Task 6.2 — Add fallback font support

Details:

- verify missing glyphs can fall through to fallback fonts
- test symbols and broader Unicode coverage

Files:

- renderer font setup
- config parsing if fallback lists are added

Done when:

- terminal text remains readable across mixed scripts/symbols

### Task 6.3 — Define ligature behavior

Details:

- add explicit terminal ligature policy
- default configurable behavior
- ensure no silent mismatch with cell semantics

Files:

- config
- terminal renderer/font setup

Done when:

- ligature behavior is intentional and documented

### Task 6.4 — Add hooks for richer protocols

Details:

- define snapshot/render extension points for:
  - hyperlinks
  - inline graphics
  - image surfaces

Files:

- terminal snapshot types
- renderer interfaces

Done when:

- future protocol work does not require redesigning the core render contract

## Phase 7 — Verification, Cleanup, and Documentation

Goal:

- close the migration with confidence

### Task 7.1 — Add backend tests

Details:

- engine integration
- snapshot generation
- dirty tracking semantics

Files:

- terminal backend tests

Done when:

- core terminal logic has repeatable coverage

### Task 7.2 — Add renderer tests where feasible

Details:

- content-rect clipping
- row invalidation logic
- color mapping

Files:

- renderer tests or snapshot tests

Done when:

- major regressions are catchable without manual testing alone

### Task 7.3 — Manual validation matrix

Details:

- shell prompt responsiveness
- long output scroll
- vim/neovim inside terminal
- truecolor output
- Unicode fallback
- pane resize behavior
- mouse-enabled TUI behavior

Files:

- this document
- handoff

Done when:

- the terminal is subjectively and objectively usable

### Task 7.4 — Remove obsolete custom-grid code

Details:

- delete superseded custom terminal grid logic once replacement is proven

Files:

- old terminal backend files

Done when:

- dead architecture is not left behind to confuse future work

---

## Progress Checklist

## Phase 0

- [x] 0.1 Lock stack decision
- [x] 0.2 Identify migration boundaries

## Phase 1

- [x] 1.1 Define terminal snapshot types
- [x] 1.2 Extend `PaneBackend` without leaking renderer details
- [x] 1.3 Define redraw/dirty semantics

## Phase 2

- [ ] 2.1 Add PTY wrapper
- [ ] 2.2 Add terminal engine wrapper
- [ ] 2.3 Implement backend adapter
- [ ] 2.4 Add unit tests for backend lifecycle

## Phase 3

- [ ] 3.1 Create terminal renderer module
- [ ] 3.2 Render cell backgrounds and cursor
- [ ] 3.3 Add glyph rendering path using `cosmic-text`
- [ ] 3.4 Add clipping/scissor support
- [ ] 3.5 Add dirty-region rendering strategy

## Phase 4

- [x] 4.1 Compute pane content rect
- [x] 4.2 Remove center title overlay from terminal content
- [ ] 4.3 Replace fake startup backend path
- [ ] 4.4 Replace pane creation sites
- [ ] 4.5 Wire redraw scheduling

## Phase 5

- [ ] 5.1 Keyboard input mapping
- [ ] 5.2 Mouse protocol forwarding
- [ ] 5.3 Resize propagation

## Phase 6

- [ ] 6.1 Add terminal font config surface
- [ ] 6.2 Add fallback font support
- [ ] 6.3 Define ligature behavior
- [ ] 6.4 Add hooks for richer protocols

## Phase 7

- [ ] 7.1 Add backend tests
- [ ] 7.2 Add renderer tests where feasible
- [ ] 7.3 Manual validation matrix
- [ ] 7.4 Remove obsolete custom-grid code

## Overall Acceptance

- [ ] Real shell renders in panes
- [ ] No pane-border text overlap
- [ ] No center title overlay inside terminal content
- [ ] No per-frame full-grid reconstruction as the default path
- [ ] Redraws are prompt and bounded
- [ ] Terminal font loads from `config.toml`
- [ ] Truecolor output works
- [ ] Unicode fallback works
- [ ] Mouse-aware TUIs work
- [ ] Architecture is documented and handoff is current

---

## HANDOFF

This section must be updated:

- when a phase is completed
- when a task is partially completed and work stops
- when architecture changes
- when blockers are discovered
- when verification reveals regressions
- when another agent asks for current status

### Current Status

- Stack decision: `portable-pty + wezterm-term + cosmic-text`
- Execution state: groundwork and render-geometry cleanup started
- Active implementation phase: Phase 1 complete, Phase 4.1/4.2 complete out of order, Phase 2 pending
- Last completed phase: Phase 1

### Latest Decisions

- heca remains fully GUI-native
- terminal font must be end-user configurable via `config.toml`
- terminal rendering must move to a dedicated renderer path
- pane content rect separation is mandatory
- ligatures are configurable terminal behavior, not assumed default behavior
- completed Rust tasks must go through a no-suppression review loop before they are considered done
- terminal backends now expose a renderer-agnostic `terminal_snapshot()` contract alongside the legacy `render_data()` path
- damage semantics are now modeled explicitly as `None | Full | Rows(...)`
- tiled and floating pane content now render inside an inset content rect instead of the full pane rect
- centered pane-title overlays were removed from live pane content rendering

### Current Known Risks

- old render path still treats terminal content like generic UI text
- current backend contract is too full-copy oriented for the target design
- content inset now exists, but there is still no dedicated GPU scissor/clipping path
- the legacy renderer still ignores per-cell backgrounds and other richer terminal attributes
- live pane creation still uses `FakeBackend` in startup, handlers, and mutations

### Migration Boundary Inventory

- `heca-core/src/backend/mod.rs`
  - legacy `BackendRenderData::Terminal { lines: Vec<TerminalLine>, ... }`
  - new `terminal_snapshot()` hook added for migration
- `heca-core/src/backend/fake.rs`
  - now exposes `TerminalSnapshot`
  - still adapts snapshots back into legacy `BackendRenderData`
- `heca-core/src/backend/terminal.rs`
  - still custom `vte` grid backend
  - now exposes `TerminalSnapshot`
  - still adapts snapshots back into legacy `BackendRenderData`
- `heca/src/app/render.rs`
  - still consumes legacy `BackendRenderData`
  - now paints terminal content in an inset content rect
  - no longer paints the pane name centered over content
- `heca-renderer/src/text.rs`
  - still generic UI text pipeline
  - not yet a dedicated terminal renderer
- `heca/src/app/startup.rs`
  - startup pane still uses `FakeBackend`
- `heca/src/handlers.rs`
  - split/new-pane paths still use `FakeBackend`
- `heca/src/app/mutations.rs`
  - placeholder pane creation still uses `FakeBackend`

### Next Recommended Task

- Start Phase 2.1 and 2.2:
  - introduce a PTY wrapper boundary
  - introduce a `wezterm-term` engine wrapper
  - keep the new snapshot contract as the public terminal renderer boundary

### In-Flight Work

- Snapshot contract groundwork landed in `heca-core`
- Pane content-rect separation landed in `heca/src/app/render.rs`

### Blockers

- None at planning level

### Verification State

- `cargo test -p heca-core` passes after the snapshot contract changes
- `cargo check -p heca` passes after the pane content-rect cleanup
- `cargo clippy --workspace --all-targets --all-features` passes after the Rust review loop

### Update Template

Use this template whenever updating the handoff:

```md
### Current Status
- Stack decision:
- Execution state:
- Active implementation phase:
- Last completed phase:

### Latest Decisions
- ...

### Current Known Risks
- ...

### Next Recommended Task
- ...

### In-Flight Work
- ...

### Blockers
- ...

### Verification State
- ...
```
