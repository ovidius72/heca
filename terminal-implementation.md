# Rule

This file is the source of truth for the terminal implementation. The `HANDOFF` section at the end of this file **must** be updated every time a phase is completed, whenever scope or architecture changes, whenever a blocker or regression is found, and on demand when another agent or session needs current context.

At the end of each completed implementation phase, the Rust code for that phase must be reviewed against the Rust skill rules before the phase is considered done. Do not run the Rust-skill subagent review after every individual task. If issues are found at phase review time, they must be fixed and reviewed again until no issues remain. Do not use `#[allow(dead_code)]`, `#[expect(dead_code)]`, or similar suppression just to force code through review. When the phase passes review, create a commit and ask the user to review it. If the user accepts, pull `origin/main`, sync, open a PR, and then start the next task or phase.

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

- encode normal keys, modifiers, arrows, and function keys through the terminal engine/input layer
- prefer structured backend key events over ad hoc raw-byte escape guesses
- keep raw-byte forwarding only as a fallback path for events not yet represented structurally

Files:

- `heca-core/src/backend/mod.rs`
- `heca-core/src/backend/terminal/engine.rs`
- `heca/src/app/keyboard.rs`
- `heca/src/app/input.rs`

Done when:

- shell apps and TUIs receive correct keyboard input

### Task 5.2 — Mouse protocol forwarding

Details:

- use terminal engine aware mouse encoding
- support press/release/move/scroll as terminal modes require
- route pointer forwarding through the terminal host adapter, not through pane-render-specific code
- keep pane-shell chrome interactions separate from content-area terminal forwarding

Files:

- `heca-core/src/backend/mod.rs`
- `heca-core/src/backend/terminal/engine.rs`
- `heca/src/app/terminal_host.rs`
- `heca/src/app/events.rs`

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

## Phase 8 — Grid UI Pane Hosting and Process-Aware Shell

Goal:

- host the terminal inside the future `heca-grid-ui` pane shell without
  coupling terminal semantics to a specific pane implementation

Integration note:

- this phase is the convergence point with `pluggable-chrome-plugin-plan.md`
- the terminal host should become a mounted content provider/consumer of the
  future pane shell or ChromeHost contract, not a parallel pane architecture
- future pane shells and ChromeHost-owned regions should treat the terminal as
  inner content rendered into a host-provided content rect and clip rect
- process/global metadata surfaced by the pane shell must be derived from
  backend/runtime state through shared host state, not from terminal rendering

### Task 8.1 — Define pane-shell hosting contract

Details:

- treat the terminal as content rendered inside a pane shell/container
- keep the pane shell responsible for:
  - outer layout
  - borders/title/focus treatment
  - content rect computation
  - clipping/scissor ownership
- keep the terminal host responsible only for:
  - snapshot consumption
  - terminal content rendering
  - input/backend process integration

Files:

- `terminal-implementation.md`
- future `heca-grid-ui` pane widget integration surface
- app/render integration points

Done when:

- the terminal can be mounted into a pane shell via an explicit content-slot style contract

### Task 8.2 — Adapt terminal host to `heca-grid-ui` pane shell

Details:

- make the existing terminal host render inside a new `heca-grid-ui` pane widget
- do not let the legacy pane host retain ownership of:
  - outer pane chrome
  - title rendering
  - focus visuals
  - shell layout
- use this as a migration bridge, not as permanent double-pane ownership

Files:

- future `heca-grid-ui` pane widget integration
- `heca/src/app/render.rs` or its eventual successor

Done when:

- the terminal renders correctly inside the new pane shell while the shell owns chrome/layout

### Task 8.3 — Add process-aware pane state model

Details:

- make the pane host aware of terminal process lifecycle and current status
- define at least:
  - `idle`
  - `running`
  - `error`
- ensure status is not inferred from chrome alone; it must come from runtime/backend state

Files:

- backend adapter state model
- app-level pane/view-model state
- future pane-shell presentation layer

Done when:

- the pane shell can reflect process state without peeking into terminal rendering internals

### Task 8.4 — Define future global process metadata channel

Details:

- prepare a shared/global state path for process-derived metadata such as:
  - process status
  - git branch
  - git dirty/changes state
  - AI agents running status
  - future backend-specific activity summaries
- this phase does not require full implementation of every metadata source
- it must define ownership and flow so pane widgets can subscribe to stable state instead of querying ad hoc

Files:

- future global state/store design
- pane host integration contract
- planning docs / follow-up implementation tasks

Done when:

- there is a defined path from runtime process state to pane shell/global UI state

### Task 8.5 — Keep terminal host pane-agnostic

Details:

- verify terminal backend/renderer do not become tightly coupled to a specific pane widget implementation
- require the terminal host to depend only on:
  - content rect
  - clip rect
  - focus/state inputs
  - terminal style inputs
- prevent `heca-grid-ui` shell migration from forcing PTY/engine redesign

Files:

- terminal renderer interface
- pane-shell adapter layer
- planning/verification notes

Done when:

- pane-shell replacement can happen without redesigning PTY, terminal engine, or terminal snapshot contracts

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

- [x] 2.1 Add PTY wrapper
- [x] 2.2 Add terminal engine wrapper
- [x] 2.3 Implement backend adapter
- [x] 2.4 Add unit tests for backend lifecycle

## Phase 3

- [x] 3.1 Create terminal renderer module
- [x] 3.2 Render cell backgrounds and cursor
- [x] 3.3 Add glyph rendering path using `cosmic-text`
- [x] 3.4 Add clipping/scissor support
- [ ] 3.5 Add dirty-region rendering strategy

## Phase 4

- [x] 4.1 Compute pane content rect
- [x] 4.2 Remove center title overlay from terminal content
- [x] 4.3 Replace fake startup backend path
- [x] 4.4 Replace pane creation sites
- [x] 4.5 Wire redraw scheduling

## Phase 5

- [x] 5.1 Keyboard input mapping
- [x] 5.2 Mouse protocol forwarding
- [x] 5.3 Resize propagation

## Phase 6

- [x] 6.1 Add terminal font config surface
- [x] 6.2 Add fallback font support
- [ ] 6.3 Define ligature behavior
- [ ] 6.4 Add hooks for richer protocols

## Phase 7

- [ ] 7.1 Add backend tests
- [ ] 7.2 Add renderer tests where feasible
- [ ] 7.3 Manual validation matrix
- [x] 7.4 Remove obsolete custom-grid code

## Phase 8

- [ ] 8.1 Define pane-shell hosting contract
- [ ] 8.2 Adapt terminal host to `heca-grid-ui` pane shell
- [ ] 8.3 Add process-aware pane state model
- [ ] 8.4 Define future global process metadata channel
- [ ] 8.5 Keep terminal host pane-agnostic

## Review Backlog

These issues are not guaranteed to be solved automatically by the next renderer
and integration tasks. They must be tracked explicitly and closed before Phase 3
and the terminal migration are considered complete.

- [x] RB1 Replace `PtyError` string payloads with richer typed/source errors in `heca-core/src/backend/terminal/pty.rs`
- [x] RB2 Add text-label cache tests in `heca-renderer/src/text.rs`
  - hit/miss behavior
  - pruning behavior
  - invalidation on scale/font changes
- [x] RB3 Add config-loader precedence tests for terminal font overrides in `heca-config/src/loader.rs`
- [x] RB4 Add terminal backend regression coverage for child reaping / exit lifecycle invariants in `heca-core/src/backend/terminal.rs`
- [x] RB5 Remove the legacy `render_data()` terminal fallback after all pane backends use snapshots

Recommended ownership:

- RB1 and RB4 should be closed during backend hardening
- RB2 should be closed alongside the dedicated terminal renderer work
- RB3 should be closed during config/verification cleanup
- RB5 should be closed at the end of Phase 3 / Phase 4 integration

## Overall Acceptance

- [ ] Real shell renders in panes
- [x] Real shell renders in panes
- [ ] No pane-border text overlap
- [x] No pane-border text overlap
- [ ] No center title overlay inside terminal content
- [x] No center title overlay inside terminal content
- [ ] No per-frame full-grid reconstruction as the default path
- [x] No per-frame full-grid reconstruction as the default path
- [ ] Redraws are prompt and bounded
- [x] Redraws are prompt and bounded
- [ ] Terminal font loads from `config.toml`
- [x] Terminal font loads from `config.toml`
- [ ] Truecolor output works
- [x] Truecolor output works
- [ ] Unicode fallback works
- [ ] Mouse-aware TUIs work
- [x] Mouse-aware TUIs work
- [ ] Architecture is documented and handoff is current
- [ ] Terminal host can mount cleanly inside future `heca-grid-ui` pane shell
- [ ] Pane shell can reflect process/global-state metadata without coupling to terminal rendering internals

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
- Execution state: real PTY-backed terminal panes are live by default; dedicated terminal rendering, structured input, redraw wakeups, and color-space fixes are all landed
- Active implementation phase: Phase 3 remains open for terminal visual correctness hardening and richer terminal protocol work
- Last materially advanced areas:
  - terminal color fidelity and cursor behavior
  - box-drawing geometry rendering for border-heavy TUIs
  - backend/config hardening backlog (`RB1`, `RB3`, `RB4`)
- Last completed phase: Phase 2

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
- transitional terminal cells now carry grapheme text plus cell width instead of a single `char`
- `portable-pty` now owns child lifecycle, reader thread, writer, and resize behavior
- `wezterm-term` now owns terminal parsing, visible screen state, title state, and palette resolution inside `heca-core`
- `TerminalBackend` now composes PTY + engine wrappers instead of maintaining a custom `vte` grid
- app lifecycle now closes panes whose backends report `should_close()`
- the app render loop now prefers `terminal_snapshot()` and only falls back to legacy `render_data()` when a backend does not expose snapshots
- terminal pane content now flushes through renderer scissor rectangles on a per-pane basis
- app pane creation now uses `TerminalBackend` by default with `FakeBackend` retained only as a startup fallback
- the generic text renderer now caches shaped/rasterized labels across frames to reduce terminal redraw stalls while the dedicated terminal renderer is still pending
- terminal pane drawing now enters `heca-renderer` through a dedicated `terminal` module instead of keeping terminal cell rendering logic inside `heca/src/app/render.rs`
- terminal cell backgrounds and cursor drawing are now owned by the dedicated renderer module
- terminal glyph runs now use a terminal-specific line-box placement path instead of the generic UI centered-box path
- app-side terminal mounting now goes through a dedicated host adapter using `Rectangle` content geometry so future pane shells can mount terminals without depending on the current pane render loop shape
- current live behavior is "fast but visually off": typing latency is now acceptable again, but terminal text placement is still slightly misaligned/off and not yet at final visual quality
- future pane migration should treat the current terminal host as inner content inside a `heca-grid-ui` pane shell, not as the permanent outer pane implementation
- future pane shells must be able to surface process/global metadata such as idle/running/error state, git status/branch/changes, and AI-agent activity
- terminal font settings are now separated from the UI theme font, with an embedded Maple Mono Normal NF fallback for terminal text
- `config.toml` can now override terminal font family and terminal font size on top of the selected theme
- both `terminal_font_family` / `terminal_font_size` and `terminal-font-family` / `terminal-font-size` are accepted from `config.toml`
- pane render now resizes terminal backends to match the live pane content rect before taking terminal snapshots
- PTY reader threads now wake the winit event loop through a user-event proxy so terminal output can trigger redraws without waiting for user input
- active-pane highlighting during rendering now derives focus from the session active pane, not only from `AppState.focused_pane`
- sidebar/chrome rendering now happens after floating panes so float/zoom states do not visually erase the sidebars
- pane backends now expose structured keyboard and mouse event hooks in addition to raw byte input
- `TerminalBackend` now encodes structured key and mouse events through `wezterm-term` instead of relying only on app-side raw byte guesses
- app keyboard forwarding now prefers structured terminal key events and only falls back to raw bytes when needed
- terminal pointer forwarding now routes through `heca/src/app/terminal_host.rs`, which converts content-area pointer events into terminal cell coordinates without depending on the current pane widget implementation
- pane focus changes and window focus changes now notify the focused terminal backend so terminal focus-tracking sequences can work
- terminal cells now carry italic/underline style flags in addition to `fg/bg/bold`
- terminal snapshots now carry resolved terminal default foreground/background colors from `wezterm-term`
- terminal rendering now respects reverse-video cell colors, italic text, underline decoration, and uses the terminal's resolved default background instead of inferring pane fill from visible cells or using a hardcoded black backdrop
- terminal style config now includes a separate `terminal_italic_font_family` path so italic rendering can use a different family than regular terminal text when needed
- bold ANSI foreground colors now follow wezterm-style brightening semantics for palette indices `0..7`
- when italic text does not have a separate terminal italic family, terminal rendering now keeps the same family and applies a faux-italic slant instead of falling back to an unrelated generic italic face
- terminal engine now supports explicit terminal default foreground/background overrides without tying terminal defaults to the outer app chrome theme
- terminal snapshot generation now seeds per-column blank cells from the full wezterm line state before overlaying visible grapheme anchors, so TUIs like `nvim` can preserve background-colored blank space instead of collapsing back to the terminal default background
- terminal palette configuration now supports explicit terminal defaults plus ANSI/brights/cursor/selection colors through config/theme plumbing
- `PtyError` now preserves operation context and typed/source error chains instead of flattening PTY failures into strings
- config loader now has explicit precedence coverage proving `config.toml` terminal font/color overrides beat bundled theme terminal defaults without clobbering unspecified theme values
- terminal exit-state handling now has focused regression coverage for:
  - reader disconnect before child reap
  - close-on-reap after disconnect
  - conservative close behavior on `try_wait()` error
- shader color handling now converts UI/theme colors consistently into linear space, which materially improved live nvim colorscheme fidelity
- right-edge border artifacts in Telescope/FzfLua/lazygit were reduced by grid fitting and then fixed by rendering common box-drawing characters as deterministic GUI geometry instead of relying on font glyph joins

### Current Known Risks

- terminal rendering now has a dedicated renderer module, but glyph shaping still routes through shared `TextRenderer` internals rather than a fully independent terminal atlas/path
- background rendering and text shaping are still transitional and may still lag under dense terminal workloads even with renderer-side label caching
- terminal text is still visually misaligned/off because glyph placement is adapted from a generic text path rather than a cell-native terminal renderer
- terminal default-family naming is tied to the embedded font metadata (`Maple Mono Normal NF`), not the shorter marketing name
- terminal drawing now uses box-based placement for better vertical centering, but final cell-native alignment still needs validation in the live app
- `nvim`/alt-screen redraw behavior still needs live verification after PTY wakeup wiring
- sidebar visibility after zoom/float and multi-pane focus highlighting still need live verification after the latest render ordering/session-focus fixes
- structured keyboard forwarding is landed, but live verification is still needed for modifier-heavy terminal apps and function-key behavior
- terminal mouse forwarding is landed in the app/backend path, but live verification is still needed for `nvim` mouse mode, wheel behavior, and drag/move interaction boundaries
- terminal style fidelity is improved, but live verification is still needed for colorscheme parity with other terminals, especially palette/default-color semantics and italic-heavy themes
- terminal background/default-color fidelity is still wrong in practice because the backend still relies on wezterm's stock terminal palette unless explicit terminal colors are configured; full terminal palette/theme support is not implemented yet
- italic styling is supported, but the embedded terminal fallback currently includes only Maple Mono Normal NF regular/bold assets; without an installed italic face or a configured `terminal_italic_font_family`, italic runs may fall back to a different family
- underline is implemented as a straight underline; undercurl is not implemented yet
- the legacy `render_data()` fallback still exists and should be removed once all pane backends expose snapshots
- `FakeBackend` still exists as an error fallback and testing backend, not as the normal pane path
- future work must avoid coupling terminal backend/renderer to a specific pane widget implementation while pane shells evolve
- current app integration still lives in `heca/src/app/render.rs`, but terminal sizing/snapshot acquisition now sits behind a dedicated terminal-host adapter rather than being inlined into pane drawing loops
- Yazi currently exposes new terminal-fit issues:
  - row spacing/line height is still off
  - item columns are too narrow
  - some right-edge geometry still looks slightly off in complex TUI layouts
- Yazi image preview is not supported yet; selecting an image currently triggers an infinite loading spinner because richer graphics/image protocol handling is still unimplemented

### User-Verified TODOs

- Yazi:
  - fix line-height / row-spacing mismatch
  - widen effective item-cell geometry so filenames/icons do not look squeezed
  - inspect remaining right-edge visual drift in complex split layouts
  - treat image preview support as a separate richer-protocol task; current behavior is an infinite spinner
- Font sensitivity follow-up:
  - test 2-3 terminal Nerd Fonts inside heca, not just Maple Mono NF
  - compare Yazi alignment across fonts to separate font-specific behavior from renderer-metric bugs
  - if one font materially improves Yazi immediately, record it as a temporary recommended terminal font while the metric path is corrected
- Continue broader colorscheme parity testing across multiple live nvim themes
- Validate whether any remaining TUIs expose grid-fit issues outside the now-fixed box-drawing border path

### Migration Boundary Inventory

- `heca-core/src/backend/mod.rs`
  - legacy `BackendRenderData::Terminal { lines: Vec<TerminalLine>, ... }`
  - new `terminal_snapshot()` hook added for migration
  - terminal cells now carry grapheme text and cell width
- `heca-core/src/backend/fake.rs`
  - now exposes `TerminalSnapshot`
  - still adapts snapshots back into legacy `BackendRenderData`
- `heca-core/src/backend/terminal.rs`
  - now composes PTY + `wezterm-term` wrappers
  - still adapts snapshots back into legacy `BackendRenderData`
- `heca-core/src/backend/terminal/pty.rs`
  - owns `portable-pty` process setup, reader thread, writer sharing, resize, and exit polling
- `heca-core/src/backend/terminal/engine.rs`
  - owns `wezterm-term` initialization, viewport resize, title access, palette resolution, and snapshot conversion
- `heca/src/app/render.rs`
  - now prefers `terminal_snapshot()` in the live pane render loop
  - still has a legacy `BackendRenderData` fallback path
  - now paints terminal content in an inset content rect
  - now paints per-cell backgrounds and styled grapheme text in the transitional path
  - now flushes pane content through scissor-clipped primitive/text passes
  - no longer paints the pane name centered over content
- `heca/src/app/lifecycle.rs`
  - now removes panes whose backends report `should_close()`
- `heca/src/app/backend_factory.rs`
  - creates `TerminalBackend` by default and falls back to `FakeBackend` only when PTY startup fails
- `heca-renderer/src/text.rs`
  - still generic UI text pipeline
  - now supports clipped per-pane flushes
  - now caches shaped/rasterized labels across frames instead of rebuilding identical runs every redraw
  - now supports per-run font-family overrides so terminal text can use a dedicated font separate from UI/chrome
- `heca-renderer/src/font.rs`
  - embeds Maple Mono Normal NF regular/bold as terminal fallback assets
- `heca-renderer/src/primitive.rs`
  - now supports clipped per-pane flushes
- `heca/src/app/startup.rs`
  - startup pane now uses `create_terminal_backend(...)`
- `heca/src/handlers.rs`
  - split/new-pane paths now use `create_terminal_backend(...)`
- `heca/src/app/mutations.rs`
  - placeholder pane creation now uses `create_terminal_backend(...)`

### Next Recommended Task

- Validate font sensitivity first, then fix terminal metrics for file-manager style TUIs:
  - test multiple terminal Nerd Fonts in heca and compare Yazi behavior
  - determine whether the current misalignment is mostly Maple-specific or remains across fonts
  - then derive or tune row height/advance more accurately for the actual loaded font metrics
  - remove squeezed/narrow item layout in Yazi
  - verify that the fix does not regress the now-correct shell/nvim cursor spacing
- After cell-metric correction, decide whether Phase 3 still needs a more direct terminal-native font-metric path instead of the remaining shared `TextRenderer` internals
- Keep richer graphics/image protocol support explicitly out-of-scope for the immediate metric fix, but track Yazi image preview as the next protocol-facing TODO
  - cursor/selection colors if needed
  - clear separation between outer app chrome theme and terminal-internal color theme
- then continue terminal visual/cell-fidelity refinement where runtime gaps remain
- after terminal rendering/input completion, start Phase 8 pane-shell integration with `heca-grid-ui`

### In-Flight Work

- Snapshot contract groundwork landed in `heca-core`
- PTY + `wezterm-term` backend composition landed in `heca-core`
- Pane content-rect separation landed in `heca/src/app/render.rs`
- Review-driven fixes landed for grapheme preservation, terminal exit handling, resize consistency, and transitional cell styling
- App pane creation now defaults to real terminal backends and pane content is clipped per pane in the render loop
- Dedicated terminal renderer module now exists and owns terminal background/cursor drawing plus app-facing terminal render entrypoints
- Generic text rendering still supplies the shared rasterization/cache internals underneath the dedicated module
- The live app render loop now consumes only `terminal_snapshot()` for terminal panes and no longer falls back to `render_data()`
- `heca/src/app/terminal_host.rs` now provides a `Rectangle`-based terminal mount preparation step that future `heca-grid-ui` pane shells can reuse
- `heca/src/app/terminal_host.rs` now also owns terminal content-area mouse routing and focus notifications, making terminal interaction less dependent on the current pane implementation
- Terminal panes now use dedicated terminal font settings and fallback assets instead of inheriting only the UI theme font
- A future pane-shell phase is now planned to host this terminal inside `heca-grid-ui` while adding process/global-state awareness at the shell layer

### Runtime Notes

- Normal app runtime now uses `TerminalBackend`, not `FakeBackend`
- `FakeBackend` remains only as:
  - backend-factory fallback when PTY startup fails
  - testing/dev placeholder backend
- The original "typing a character takes seconds" lag is no longer the primary issue after the `heca-renderer/src/text.rs` cache change
- The current user-verified runtime state is:
  - terminal appears
  - typing is fast again
  - text rendering is still misaligned/off
- This means the current bottleneck has moved from gross per-frame shaping/upload cost to visual correctness of terminal cell placement

### Blockers

- None at planning level

### Verification State

- `cargo check -p heca-core` passes with `portable-pty` + `wezterm-term`
- `cargo test -p heca-core` passes with backend lifecycle tests
- `cargo check -p heca` passes with the composed terminal backend
- `cargo check -p heca-renderer` passes after renderer label-cache changes
- `cargo test -p heca-renderer` passes with text-label cache hit/miss/prune/invalidation tests
- `cargo clippy -p heca-renderer --all-targets` passes after renderer label-cache changes
- `cargo clippy --workspace --all-targets --all-features` passes after the renderer label-cache changes
- User smoke test after the renderer cache change:
  - terminal is interactive again
  - text is still visually misaligned/off
- After terminal-font/fallback work:
  - `cargo check -p heca` passes
  - `cargo check -p heca-renderer` passes
  - `cargo clippy -p heca --all-targets` passes
  - `cargo clippy -p heca-renderer --all-targets` passes
  - floating-pane terminal panic from row-width mismatch is fixed

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
