# Rule

This file is the source of truth for the terminal implementation. The `HANDOFF` section at the end of this file **must** be updated every time a phase is completed, whenever scope or architecture changes, whenever a blocker or regression is found, and on demand when another agent or session needs current context.

At the end of each completed implementation phase, the Rust code for that phase must be reviewed against the Rust skill rules before the phase is considered done. Do not run the Rust-skill subagent review after every individual task. If issues are found at phase review time, they must be fixed and reviewed again until no issues remain. Do not use `#[allow(dead_code)]`, `#[expect(dead_code)]`, or similar suppression just to force code through review. When the phase passes review, create a commit and ask the user to review it. If the user accepts, pull `origin/main`, sync, open a PR, and then start the next task or phase.

---

# Terminal Implementation

> **RESOLVED (2026-06-22, compositor Phase 3 / PR #169):** `terminal_blur`,
> `terminal_frost_color`, `terminal_frost_opacity()`, and
> `effective_terminal_frost_color()` are **removed**. Tiled terminal frost is
> now owned by the heca z=0 blurred-gradient background layer (`background_blur` +
> `background_transparency`); floating panes keep `terminal_floating_blur` +
> `terminal_floating_transparency`. All `terminal_blur` references in this file
> (the Phase 8/9/13 checklists, the “no visible blur difference” findings, etc.)
> are **historical** — they describe the pre-z=0 tiled-tint approach that the
> compositor-blur refactor superseded. See `compositor-blur-refactor-plan.md`
> and the “Appearance & Frost (z=0 background layer)” section in `README.md`.
> The canonical list of terminal theme tokens actually consumed by the backend
> + renderer lives in `theming-documentation.md` §8 (“How terminal theming works
> today”) — `terminal_foreground`, `terminal_background`, `terminal_cursor_*`,
> `terminal_selection_*`, `terminal_ansi`, `terminal_brights`. Font family/size
> have moved out of `Theme` into the dedicated `[font]` block (see
> `heca-config/src/font.rs` and `theming-documentation.md` §1 “Fonts are NOT
> part of the theme”).

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
  - `[font.family.terminal]` (`normal` + optional `bold`/`italic`/`bold_italic`) + `[font.size].terminal` (see `heca-config/src/font.rs`)
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

Initial config targets (now landed in the unified `[font]` block — see
`heca-config/src/font.rs`):

```toml
[font.family.terminal]
normal = "JetBrains Mono"
# bold / italic / bold_italic optional → fall back to normal

[font.size]
terminal = 14.0
```

Later expansion (not yet implemented):

```toml
[font.family.terminal]
normal = "JetBrains Mono"
# per-style slots: bold / italic / bold_italic

[font.size]
terminal = 14.0

# future knobs (not yet implemented):
# line_height = 1.15
# ligatures = false
# font_features = ["calt=0", "liga=0"]
# fallback_families = ["Symbols Nerd Font", "Noto Color Emoji"]
# cursor_style = "block"
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
- this depends on a retained terminal-content path; if the app still clears the
  full scene each frame, skipping unchanged rows would erase them instead of
  optimizing redraw cost
- keep the broader app/compositor damage-preservation work explicit rather than
  burying it inside the terminal renderer task

Files:

- `heca-renderer/src/terminal.rs`
- terminal snapshot types

Done when:

- redraw cost scales with changed content rather than full pane size

## Phase 3A — Terminal Damage-Preservation Foundation

Goal:

- make dirty-row rendering possible without breaking visible output
- preserve terminal damage through the backend/app/renderer boundary instead of
  draining and discarding it
- add the minimum retained-content foundation so unchanged terminal rows can
  remain visible across frames

Why this phase exists:

- the live app currently clears the scene each frame before re-rendering panes
- terminal backends expose a damage API, but the app currently acknowledges that
  damage before rendering and drops it on the floor
- as long as unchanged terminal content is not preserved somewhere, "render only
  dirty rows" is not an optimization; it is a correctness bug because skipped
  rows disappear on the next frame

### Task 3A.1 — Preserve terminal damage through the app path

Details:

- stop acknowledging terminal damage inside mount preparation before render uses it
- thread damage metadata alongside `TerminalSnapshot` through the terminal mount
  and render path
- keep the contract renderer-agnostic: backend owns damage production, app owns
  routing, renderer consumes already-decided visible row ranges

Files:

- `heca-core/src/backend/mod.rs`
- `heca-core/src/backend/snapshot.rs`
- `heca/src/app/terminal_host.rs`
- `heca/src/app/terminal_render.rs`

Done when:

- a terminal render pass receives both the snapshot and the pending
  `TerminalDamage` for that pane

### Task 3A.2 — Produce real visible-row damage from the terminal backend

Details:

- replace the current `dirty: bool -> Full|None` behavior with visible row-range
  damage where possible
- derive dirty visible rows from `wezterm-term` / `termwiz` line sequence
  numbers or equivalent viewport-aware invalidation data instead of diffing full
  snapshots blindly
- coalesce adjacent visible rows into `TerminalRowRange`
- fall back to `Full` when damage cannot be described safely: resize,
  viewport-shape changes, alternate-screen transitions, scroll-region changes,
  or any uncertain state transition

Files:

- `heca-core/src/backend/terminal.rs`
- `heca-core/src/backend/terminal/engine.rs`
- `heca-core/src/backend/snapshot.rs`

Done when:

- terminal damage is usually `Rows(...)` for ordinary output/cursor changes and
  conservatively `Full` for structural viewport changes

### Task 3A.3 — Add retained terminal-content foundation

Details:

- introduce the minimum retained-content mechanism required so unchanged rows
  remain visible when only dirty rows are redrawn
- keep this scoped to terminal panes; do not silently expand it into general
  compositor-wide damage optimization in the same task
- acceptable implementations include:
  - a retained terminal layer/texture per pane
  - app-side preserved scene content scoped to pane damage
- whichever implementation is chosen must make the dependency explicit: terminal
  row damage is invalid without retained content

Files:

- `heca/src/app/render.rs`
- `heca/src/app/terminal_render.rs`
- `heca-renderer` modules only if a terminal-specific retained surface is needed

Done when:

- unchanged terminal rows remain visible across frames while only dirty rows are
  redrawn

### Task 3A.4 — Verify the dependency itself

Details:

- add focused tests for:
  - backend row-range damage production
  - app-path damage propagation
  - retained-content correctness when only dirty rows are redrawn
- keep renderer-side tests small and structural; broader visual validation stays
  in the later manual matrix

Files:

- `heca-core/src/backend/terminal.rs`
- `heca-renderer/src/terminal.rs`
- app-side test modules where feasible

Done when:

- the retained-content prerequisite is covered well enough that Phase 3.5 can
  optimize row redraw without relying on undocumented assumptions

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

## Phase 3A

- [x] 3A.1 Preserve terminal damage through the app path
- [x] 3A.2 Produce real visible-row damage from the terminal backend
- [~] 3A.3 Add retained terminal-content foundation
- [~] 3A.4 Verify the dependency itself

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

- [~] 8.1 Define pane-shell hosting contract — implicit via `heca/src/app/terminal_host.rs` Rectangle-based mount; not yet formalized as a documented content-slot contract
- [~] 8.2 Adapt terminal host to `heca-grid-ui` pane shell — terminals render inside `heca_grid_ui::widgets::Pane` via `paint_terminal_pane_shell` (border/radius fixed #121/#122); outer chrome/layout still owned by `heca/src/app/render.rs`, so full shell-owned migration is Phase 13
- [x] 8.3 Add process-aware pane state model — `ProcessStatus { Running, Idle, Success, Error }` + `PaneRuntime` in `heca-core/src/runtime`; `PaneBackend::runtime()`; `TerminalBackend`/`FakeBackend` report it (pane-runtime-state initiative, archived/shipped)
- [x] 8.4 Define future global process metadata channel — `heca/src/host.rs` exposes `pane_runtime()`/`pane_status()` selectors + `App::on(event)` event bus; chrome store mirrors `PaneRuntime` via signals
- [~] 8.5 Keep terminal host pane-agnostic — `terminal_host.rs` depends only on content/clip rects + focus/state/style inputs; not yet formally verified against a real pane-shell swap

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

> Reconciled 2026-06-21 against the current codebase. Items marked with a status
> note reflect verified reality, not the doc's prior self-report.

- [x] Real shell renders in panes
- [x] No pane-border text overlap
- [x] No center title overlay inside terminal content
- [ ] No per-frame full-grid reconstruction as the default path — **OPEN**: `heca-renderer/src/terminal.rs` still iterates `lines.iter().take(max_rows)` and renders every visible cell each frame (Phase 3.5 dirty-region)
- [x] Redraws are prompt and bounded
- [x] Terminal font loads from `config.toml`
- [x] Truecolor output works
- [ ] Unicode fallback works — **UNVERIFIED** (awaiting Phase 7.3 manual validation matrix)
- [x] Mouse-aware TUIs work
- [ ] Architecture is documented and handoff is current — **in progress**: this reconcile pass updates the handoff; architecture reference prose is still valid
- [x] Terminal host can mount cleanly inside future `heca-grid-ui` pane shell — terminals render via `heca_grid_ui::widgets::Pane` (`paint_terminal_pane_shell`); full shell-owned chrome migration = Phase 13
- [x] Pane shell can reflect process/global-state metadata without coupling to terminal rendering internals — `heca/src/host.rs` `pane_runtime()`/`pane_status()` selectors + chrome store `PaneRuntime` mirror

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

> **RECONCILE (2026-06-21):** the Progress Checklist, Overall Acceptance, and Phase 8/9/13
> checklists were re-ticked against the current codebase this pass. Findings: Phase 8.3/8.4 and
> Phase 9.1/9.2 (+ most of the Phase 9 task checklist) are **done** and were wrongly unchecked;
> the terminal `Pane` shell border/radius blocker is **fixed** (#121/#122). The only carry-over
> from the old blocker is `terminal_blur`, now being resolved by `compositor-blur-refactor-plan.md`
> (z=0 background model) — that plan supersedes the tiled-tint approach. Note also PR #160
> (`frappe`→`latte` theme-unification) set `latte.toml` `terminal_background = "#e6e9ef00"`
> (alpha 0), which bypasses the `terminal_transparency`→`surface_alpha` channel and is the
> prime suspect for the deferred “no blur/transparency” regression; the compositor-blur plan's
> Phase 3 critical review owns reconciling which channel owns pane translucency.
>
> **RECONCILE (2026-06-23):** `terminal-00` was advanced in the `feature/terminal-followups`
> worktree. `3A.1` and `3A.2` are now done: terminal damage is preserved through the app path
> and the backend now emits visible row-range damage where safe. `3A.3` is only **partial**:
> a retained terminal-content foundation exists, but the first live damaged-frame presentation
> caused a real runtime regression (oversized glyphs while resizing, freshly typed text becoming
> temporarily invisible). The current safety mitigation is to present the retained layer only on
> clean frames while still updating the cache in the background on damaged frames. `3A.4` is also
> partial: focused validation exists, but retained-content correctness under damaged-frame
> presentation still needs direct coverage before `3.5` / `terminal-01` can be considered safe.
>
> **RECONCILE (2026-06-24):** the retained damaged-frame presentation path was re-enabled after
> fixing the offscreen scratch sizing bug in `terminal_render`/`app_state`: the shared scratch
> texture now matches each pane's exact physical size instead of reusing an oversized target that
> cropped the copied update bands. That bug was the concrete explanation for the earlier resize
> regression (oversized glyphs while resizing, typed text disappearing until a later frame). `3A.3`
> remains **partial** until runtime review confirms the regression is actually gone. `3A.4`
> remains **partial** because compile + focused test coverage are green, but direct app-path
> retained-presentation verification is still thinner than the backend/renderer helper coverage.
>
> **RECONCILE (2026-06-24, later runtime validation):** resize now looks good on the restored
> retained path, so the concrete glyph-scale regression appears fixed. The next runtime gap is
> broader terminal scrollback/navigation: wheel scrolling at a shell prompt does not work, PageUp
> / PageDown do not scroll host history, and selection mode cannot scroll beyond the currently
> visible rows. The reason is architectural, not just a missing keybinding: the host still has no
> terminal viewport/scrollback model. Today the app forwards wheel as PTY mouse input and forwards
> PageUp/PageDown as PTY key input, while `move_focused_terminal_selection(...)` clamps movement to
> `terminal_snapshot().rows`. A dedicated host scrollback phase is now tracked in `BACKLOG.md` as
> `terminal-01a`.
>
> **RECONCILE (2026-06-24, `/grill-me` design lock for `terminal-01a` + config single-source sync):**
> the full host-scrollback-viewport design was locked via `/grill-me` and is captured in
> `handoff-terminal-01a-scrollback.md` (READ THAT FILE — it is the authoritative contract for the
> phase, superseding the prose here for scrollback specifics). Summary of locked decisions:
> - **Q1 hybrid ownership:** `TerminalEngine` owns `viewport_offset` + projects via `TerminalSnapshot`
>   (adds `viewport_offset`/`at_bottom`/`scrollback_rows`); AppState mirrors into the chrome store +
>   `ChromeEvent::TerminalViewportChanged` + `host.terminal_viewport(pane_id)` (one writer, many readers).
> - **Q2 wheel:** scrollback unless `is_mouse_grabbed()`; Shift+wheel always scrollback; default 3 rows/notch.
> - **Q3-revised (tmux-style, user override):** plain PageUp/PageDown forward to PTY (do NOT intercept);
>   `prefix+PageUp`/`prefix+PageDown` enter `InputMode::Selection` + scroll one page; wheel-up at a
>   non-grabbed prompt also enters Selection mode + scrolls (tmux `mouse on`).
> - **Q4 selection:** edge movement auto-scrolls the viewport; `SelectionRegion::HostGrid` moves from
>   visible-row to **stable-row + col** coords (real refactor).
> - **Q5 snap:** forwarded key input snaps to bottom; new output snaps only if already at bottom;
>   explicit `ScrollToBottom`/indicator-click/reaching-bottom also snap. Config knobs deferred.
> - **Q6 damage:** viewport motion → `TerminalDamage::Full` (incremental viewport damage is the
>   SEPARATE `terminal-01` phase — do NOT fold it in).
> - **Q7 GUI/UX (user directive "exploit the GUI"):** animated viewport offset (easing via `tick(dt)`),
>   theme-driven `heca-grid-ui` scrollbar widget (clickable/draggable to jump), and scrolled-up
>   indicator badge with click-to-snap-to-bottom — all generic catalog widgets.
> - **Q8 actions:** five new `WmAction` variants (`ScrollbackPage{direction}`, `ScrollbackLine{direction,amount}`,
>   `ScrollbackToTop`, `ScrollbackToBottom`, `ExitScrollback`), all `FocusedPaneLocal`, full 11-step
>   registry treatment + RPC. `prefix+s` stays the selection-mode entry (do NOT use `prefix+[` = `prev_pane`).
> - **`terminal_mouse`:** new terminal-only setting (default true) gates wheel-enters-scrollback;
>   the global `settings.mouse` is NOT reused (chrome depends on it).
>
> **Config/keybinding single-source refactoring (PR #185 / commit `3dfc458`, merged into this branch
> 2026-06-24 as `1f54d91`):** default keybindings + default settings moved OUT of Rust into versioned
> TOML files. `keybindings.default.toml` (embedded via `include_str!`, parsed by `parse_default_keys()`,
> `KeysConfig::default()` calls it) is now the single source for ALL default keybindings —
> `heca-config/src/keys.rs` dropped from ~700 → 204 lines (types only). `config.default.toml` (renamed
> from `example.config.toml`) holds `[settings]`/`[appearance]`/`[font]`/`[program]` defaults. `Config::default()`
> = `embedded_base().try_into()`. `load_config_file()` deep-merges user `config.toml` + `keybindings.toml`
> over the embedded base (tables merge per-key; arrays replace). **Flat bindings (`name = "combo"`)
> CANNOT carry args** (`KeybindingMap` has no args field); **mode bindings (`[[keys.mode.bindings]]`) DO
> support `args`**. So scrollback flat bindings use unit action names (`scrollback_page_up` etc.) mapped
> by `action_from_name` to the parameterized `WmAction` variants. The plan file
> `HANDOFF-config-single-source.md` is the *plan*; the *actual code* is the source of truth — read the
> actual files. The `terminal-00` handoff does NOT mention this; `handoff-terminal-01a-scrollback.md` does.

> **RECONCILE (2026-06-24, slice 1 of `terminal-01a` DONE):** the backend viewport model
> landed with no UI/actions yet. `TerminalEngine` owns `viewport_offset` (0 = live bottom) +
> a `viewport_changed` flag, with `scroll_viewport(delta)` / `scroll_to_top()` /
> `scroll_to_bottom()` / `take_viewport_changed()`; `visible_lines()` now projects
> bottom-minus-offset clamped to `[0, scrollback_rows - visible_rows]`, and `resize()` re-clamps
> the offset after the wezterm reflow. `TerminalSnapshot` carries `viewport_offset` / `at_bottom` /
> `scrollback_rows` (validated in `debug_assert_valid`). `PaneBackend` gained
> `scroll_viewport` / `scroll_to_top` / `scroll_to_bottom` (default no-op); `TerminalBackend`
> delegates and its `take_terminal_damage` forces `TerminalDamage::Full` on viewport motion (Q6).
> `HecaTerminalConfig::scrollback_size()` is now overridden from the new
> `SettingsConfig::terminal_scrollback_lines` (default 3500, alias `terminal-scrollback-lines`),
> wired end-to-end: `config.default.toml` → `SettingsConfig` → `AppState.terminal_scrollback_lines`
> → `backend_factory::terminal_backend_options` → `TerminalBackendOptions.scrollback_size` →
> `TerminalEngine::new` → `HecaTerminalConfig`. Tests added: viewport clamping, at_bottom, snap
> jumps, history projection, resize re-clamp, scrollback_size override. Gates green:
> `cargo test -p heca-core` 66/66, `-p heca-config` 70/70, `-p heca` 247/247,
> `cargo clippy --workspace --all-targets --all-features` 0 warnings.
>
> **RECONCILE (2026-06-24, slice 2 of `terminal-01a` DONE):** the selection model stable-row
> refactor landed with cursor rendering fixes. `SelectionRegion::HostGrid anchor_row/focus_row`
> renamed to `anchor_stable_row/focus_stable_row` (`usize` → `isize`); `Caret::row` →
> `Caret::stable_row`. `visible_row_to_stable_row()` helper added in `terminal_host.rs`;
> `build_selection_overlay` takes `&TerminalSnapshot` instead of `cols`, using stable→visible
> row conversion via `viewport_top_stable_row`. `lines_in_stable_range` added to `PaneBackend`
> trait + `TerminalBackend` for fetching scrollback rows by stable range;
> `handle_copy_selection` fetches via this method. Caret rendering unified to LEFT edge of the
> cell (both caret-only and selection-endpoint), eliminating the visual bar-position jump.
> Block cursor reverted to thin 2px bar (user preference). Gates:
> `cargo test -p heca` 259/259, `-p heca-core` 67/67, `cargo clippy` 0 warnings.
> Committed `bc66d31`, pushed to `feature/terminal-followups`, rebased onto `origin/main`.

- Stack decision: `portable-pty + wezterm-term + cosmic-text`
- Execution state: real PTY-backed terminal panes are live by default; dedicated terminal rendering, structured input, redraw wakeups, atlas-renderer sync, measured terminal-cell sizing, and GUI-native terminal symbol/decorations are all landed
- Active implementation phase: terminal core (Phases 0–5) is complete and merged; Phase 3A partial (`3A.1`/`3A.2` done, `3A.3`/`3A.4` still open), Phase 6 partial (6.1/6.2 done, 6.3 ligatures + 6.4 richer-protocol hooks open); Phase 7 partial (only 7.4 done — 7.1/7.2 tests + 7.3 manual validation matrix open); Phase 8 partial (8.3/8.4 done, 8.1/8.2/8.5 partial); Phase 9 partial (state model + actions + caret done, backend-capability contract + full terminal migration open); Phases 10–13 unbuilt (Phase 13 overlaps the partial Phase 8 pane-shell work)
- Last completed phase: Phase 5 (input fidelity). Phase 6/7/8/9 are in-progress/partial; Phases 10–13 are backlog.
- Last materially advanced areas:
  - renderer sync onto `main`'s atlas-based text path
  - cursor/text regressions after the sync
  - measured terminal metrics replacing theme-ratio bootstrapping for PTY grid sizing
  - pane-runtime-state initiative (Phases 8.3/8.4) shipped + archived
  - selection model + action wiring (Phase 9.1/9.2) landed
  - `frappe`→`latte` theme-unification (PR #160) and its terminal-bg-alpha interaction
  - `terminal-00` prerequisite work: app-path damage preservation, backend visible-row damage, exact-size retained scratch updates, and live damaged-frame retained presentation pending runtime verification

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
- current live behavior is "fast and visually close": typing latency is acceptable again and the most obvious text-placement/cursor issues have been resolved
- future pane migration should treat the current terminal host as inner content inside a `heca-grid-ui` pane shell, not as the permanent outer pane implementation
- future pane shells must be able to surface process/global metadata such as idle/running/error state, git status/branch/changes, and AI-agent activity
- terminal font settings are now separated from the UI theme font, with an embedded Maple Mono Normal NF fallback for terminal text
- `config.toml` can now override terminal font family and terminal font size on top of the selected theme (later superseded: these moved out of `[settings]`/`Theme` into the dedicated `[font]` block — see `heca-config/src/font.rs`)
- both `terminal_font_family` / `terminal_font_size` and `terminal-font-family` / `terminal-font-size` were accepted from `config.toml` (superseded by `[font.family.terminal]` + `[font.size].terminal`)
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
- terminal style config now includes a separate italic family path so italic rendering can use a different family than regular terminal text when needed (now `[font.family.terminal].italic` in the `[font]` block)
- bold ANSI foreground colors now follow wezterm-style brightening semantics for palette indices `0..7`
- when italic text does not have a separate terminal italic family, terminal rendering now keeps the same family and applies a faux-italic slant instead of falling back to an unrelated generic italic face
- terminal engine now supports explicit terminal default foreground/background overrides without tying terminal defaults to the outer app chrome theme
- terminal snapshot generation now seeds per-column blank cells from the full wezterm line state before overlaying visible grapheme anchors, so TUIs like `nvim` can preserve background-colored blank space instead of collapsing back to the terminal default background
- terminal palette configuration now supports explicit terminal defaults plus ANSI/brights/cursor/selection colors through config/theme plumbing
- after syncing `origin/main`, terminal-specific text behavior now rides on top of the atlas/retained `TextRenderer` instead of the old per-label texture path
- terminal text visibility after the sync now depends on explicit `TextRenderer::set_target_size(...)` wiring during startup and resize
- terminal cursor now renders in a separate topmost overlay pass so shell autosuggestion text does not occlude it
- terminal text now renders per visible cell instead of batched shaped runs, which keeps cursor position and glyph placement aligned for shell autosuggestions and similar inline-terminal UX
- terminal cell sizing for PTY creation, pane mount fitting, and mouse hit-testing is now derived from the actual loaded terminal font through `TextRenderer::measure_monospace_cell(...)` and stored in app state instead of bootstrapping from `Theme::terminal_cell_size()` heuristics alone
- terminal symbol correctness must be handled by a general renderer policy, not by app-specific hacks:
  - known terminal-UI symbol families should render as deterministic GUI geometry
  - ordinary text and private-use icons without stable geometry semantics should remain on the font pipeline
  - this policy must stay app-agnostic so Yazi, `nvim`, Telescope, lazygit, tmux-like prompts, and future TUIs all benefit automatically
- the shared terminal symbol renderer has now started with:
  - box-drawing geometry
  - powerline-family geometry for ``, ``, ``, ``, ``, ``
- terminal underline rendering is now GUI-native and style-aware:
  - `Single`
  - `Double`
  - `Curly` (undercurl)
  - `Dotted`
  - `Dashed`
  - terminal snapshots now preserve the concrete underline style instead of collapsing it to a boolean
- `PtyError` now preserves operation context and typed/source error chains instead of flattening PTY failures into strings
- config loader now has explicit precedence coverage proving `config.toml` terminal font/color overrides beat bundled theme terminal defaults without clobbering unspecified theme values
- terminal exit-state handling now has focused regression coverage for:
  - reader disconnect before child reap
  - close-on-reap after disconnect
  - conservative close behavior on `try_wait()` error
- shader color handling now converts UI/theme colors consistently into linear space, which materially improved live nvim colorscheme fidelity
- right-edge border artifacts in Telescope/FzfLua/lazygit were reduced by grid fitting and then fixed by rendering common box-drawing characters as deterministic GUI geometry instead of relying on font glyph joins
- the current renderer/core review blockers are now addressed:
  - primitive renderer indices now use `u32`
  - terminal grid fitting now guards non-finite/invalid cell metrics before integer conversion
  - half-ellipse powerline rendering no longer overlaps bands
  - terminal spawn backends now use estimated live workspace grid size instead of hardcoded `80x24` on the active app path
  - PTY/backend tests now use a deterministic test shell so the test suite does not depend on local interactive shell startup

### Current Known Risks

- terminal rendering now has a dedicated renderer module, but glyph shaping still routes through shared `TextRenderer` internals rather than a fully independent terminal atlas/path
- background rendering and text shaping are still transitional and may still lag under dense terminal workloads even with renderer-side label caching
- terminal default-family naming is tied to the embedded font metadata (`Maple Mono Normal NF`), not the shorter marketing name
- measured terminal-cell sizing plus the shared terminal symbol/decorations renderer materially improved Yazi and `nvim`, but broader live validation is still needed across more TUIs and fonts before Phase 3 can close
- structured keyboard forwarding is landed, but live verification is still needed for modifier-heavy terminal apps and function-key behavior
- terminal mouse forwarding is landed in the app/backend path, but live verification is still needed for `nvim` mouse mode, wheel behavior, and drag/move interaction boundaries
- host-side terminal scrollback/navigation is not implemented yet:
  - wheel is forwarded as PTY mouse input only
  - PageUp/PageDown are forwarded as PTY key input only
  - selection-mode movement clamps to the current visible snapshot and cannot scroll the viewport
- terminal style fidelity is much improved, but live verification is still needed for broad colorscheme parity across more themes and TUIs
- retained terminal-content groundwork exists, but presenting the retained layer during damaged frames currently regresses resize-time glyph scale and can temporarily hide fresh typing; the live path is guarded to clean frames until that is fixed
- italic styling is supported, but the embedded terminal fallback currently includes only Maple Mono Normal NF regular/bold assets; without an installed italic face or a configured `[font.family.terminal].italic`, italic runs synthesize an oblique from the normal family
- `FakeBackend` still exists as an error fallback and testing backend, not as the normal pane path
- future work must avoid coupling terminal backend/renderer to a specific pane widget implementation while pane shells evolve
- current app integration still lives in `heca/src/app/render.rs`, but terminal sizing/snapshot acquisition now sits behind a dedicated terminal-host adapter rather than being inlined into pane drawing loops
- Yazi image preview is not supported yet; selecting an image currently triggers an infinite loading spinner because richer graphics/image protocol handling is still unimplemented

### User-Verified TODOs

- Yazi:
  - treat image preview support as a separate richer-protocol task; current behavior is an infinite spinner
- Merge-readiness hardening:
  - keep the renderer on `u32` primitive indices to avoid large-scene overflow
  - keep terminal grid fitting guarded against invalid cell metrics
  - keep style-preservation fixes for underline variants and wide-cell filler behavior
- Font sensitivity follow-up:
  - test 2-3 terminal Nerd Fonts inside heca, not just Maple Mono NF
  - compare Yazi and other terminal-UI alignment across fonts after the measured-metric and symbol-renderer changes to separate remaining font-specific behavior from renderer bugs
  - if one font materially improves terminal-UI fidelity further, record it as a temporary recommended terminal font while the metric path is refined
- Continue broader colorscheme parity testing across multiple live nvim themes
- Validate whether any remaining TUIs expose grid-fit or symbol-family issues outside the now-fixed box-drawing and powerline paths

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

- Finish the remaining `3A.3` / `3A.4` runtime verification notes:
  - resize looks good again on the restored retained path
  - keep typed-prompt / multi-pane validation in mind while touching terminal input or viewport code
- Start the newly tracked host scrollback phase (`terminal-01a`) before claiming terminal runtime UX is healthy:
  - add a host-managed terminal viewport/scrollback model instead of always projecting the live bottom viewport
  - route wheel / PageUp / PageDown / selection-mode edge movement through that model
  - preserve TUI mouse forwarding where appropriate instead of replacing it blindly
- After host scrollback exists:
  - re-run runtime validation for shell prompt history, long output scrollback, selection mode, and mouse-enabled TUIs
  - then continue toward dirty-row rendering (`3.5` / `terminal-01`) with viewport-aware damage semantics

### Planned Post-Merge Terminal Backlog

These items are intentionally post-merge and must not be folded back into the
completed Phase 3 terminal-core work. They are split into explicit phases so
multiple agents can work on them without inventing incompatible interaction
models.

## Phase 9 — Shared Host Selection Capability

Goal:

- build selection as a reusable **host capability**, not a terminal-only feature
- make the same selection model usable by:
  - terminal panes
  - future custom Neovim GUI panes
  - future embedded browser panes
  - future heca-native content surfaces

### Why this phase exists

Selection must not be implemented as:

- terminal-only state
- mouse-only interaction
- `Shift+drag` as the whole feature
- a renderer-only overlay with no action model

The correct boundary is:

- host owns the selection model and actions
- each pane/backend advertises how it participates
- rendering and text extraction are backend/surface specific

### Required behavior contract

Selection must be reachable through:

1. mouse / UI
2. keyboard via actions / keybindings
3. RPC where meaningful

Selection must support:

- begin
- update / expand
- end / confirm
- clear / cancel
- copy selected content
- paste later through the same capability family

### Shared terminology

- `selection owner`
  - the pane/surface that currently owns the active selection
- `selection source`
  - mouse drag
  - keyboard mode
  - RPC/programmatic request
- `selection state`
  - inactive
  - selecting
  - selected
- `selection rendering mode`
  - host-rendered
  - backend-native

### Rendering model

Two rendering strategies are supported.

#### A. Host-rendered selection

Used for surfaces where heca owns the text/grid model directly.

Examples:

- terminal pane
- future custom Neovim GUI pane
- future host-drawn text surfaces

Host responsibilities:

- hold selection anchor/focus state
- map pointer or keyboard movement into logical cell/text positions
- draw the selection overlay
- extract selected text through the backend/surface adapter

#### B. Backend-native selection

Used for surfaces whose own engine already has strong native selection semantics.

Examples:

- future embedded browser

Host responsibilities:

- route actions into the backend
- know whether the backend currently owns a selection
- ask for copied text when needed

The host must not force every pane type into host-rendered cell selection.

### Mouse policy

The default mouse policy must avoid breaking terminal/TUI mouse input.

Rules:

- unmodified mouse input continues to go to the focused terminal/TUI when mouse mode is active
- host-side pointer selection must use an explicit entry path unless the active pane type declares safe plain-drag selection
- the temporary default for terminal panes may remain `Shift + left-drag`, but this is only an entry gesture into the shared host selection subsystem, not the final definition of selection

This means:

- the gesture may vary by pane type
- the underlying selection model must remain shared

### Keyboard/action policy

Selection must become a real action-driven mode.

Entering selection mode must **not** automatically begin a selection.
Instead:

- entering selection mode places a keyboard caret at the current terminal cursor
- movement keys move that caret before any selection exists
- a separate action begins selection from the current caret position
- once selection exists, movement updates the active end
- another action can flip which end is active

This should feel closer to tmux copy-mode / Vim visual mode than to a
mouse-driven webview selection model.

Required actions to plan for:

- `EnterSelectionMode`
- `BeginSelection`
- `ClearSelection`
- `ToggleSelectionEndpoint`
- `CopySelection`
- `PasteClipboard`
- `SelectAll` later where meaningful

Required movement actions:

- `SelectionLeft`
- `SelectionRight`
- `SelectionUp`
- `SelectionDown`

Mode semantics:

- before selection starts:
  - `h/j/k/l` and arrows move the caret
- begin selection:
  - direct mode-local `v`
  - direct mode-local `Space`
- after selection starts:
  - movement updates the active endpoint
- `o` flips the active endpoint so expansion can continue from the other side
- `y` copies and **stays in selection mode**
- `x` pastes in selection mode later, once paste is implemented
- `Esc` clears selection and exits selection mode

No prefix should be required for the mode-local bindings above. They must still
be routed through `WmAction` + `ActionRegistry` + `KeymapRegistry` + config
bindings, but while already in selection mode the active keymap should make:

- `v`
- `Space`
- `o`
- `y`
- `x` later
- movement keys

direct mode-local bindings.

### Backend/surface capability contract

Add a reusable capability boundary rather than terminal-specific ad hoc fields.

Target shape:

- host asks whether the focused pane supports selection
- host can begin/update/end/clear selection through a shared interface
- host can request selected text for copy
- host can ask the pane to paste clipboard content later

The exact Rust type names can evolve, but the contract must support:

- `supports_selection`
- `selection_model_kind`:
  - `host_grid`
  - `backend_native`
- `begin_selection`
- `update_selection`
- `end_selection`
- `clear_selection`
- `selection_text`

### Phase 9 checklist

- [x] Define the shared selection state in app/core terms — `SelectionState` (Inactive/Caret/Selecting/Selected) + `SelectionRegion::{HostGrid,BackendNative}` in `heca/src/app/selection_model.rs`
- [x] Define action names and input-mode integration — all selection `WmAction`s in `heca/src/input.rs`, wired via `ActionRegistry`, `interaction.rs` (`FocusedPaneLocal`), `rpc.rs` parsers, `app/registry.rs` bindings
- [x] Add a keyboard caret model separate from “selection already exists” — `SelectionState::Caret` variant + begin/toggle/end transitions
- [ ] Define backend/surface capability contract — `selection_model_kind` / `supports_selection` / `begin_selection` / `selection_text` boundary not yet formalized as a trait
- [ ] Refactor current terminal-only selection groundwork onto the shared model — terminal adapter not yet fully migrated onto the shared owner model
- [x] Implement host-rendered selection for terminal panes — `selection_overlay_for_pane` + `build_selection_overlay` in `heca/src/app/terminal_render.rs` (HostGrid rendering)
- [x] Add keyboard-driven selection mode — `InputMode::Selection` + caret movement
- [x] Add explicit begin-selection actions (`v`, `Space`) — `WmAction::BeginSelection`
- [x] Add active-end inversion action (`o`) — `WmAction::ToggleSelectionEndpoint`
- [~] Add copy action on top of shared selection — `WmAction::CopySelection` exists; system-clipboard wiring is Phase 10 (not yet implemented)
- [x] Add RPC-facing hooks where meaningful — `heca/src/rpc.rs` selection commands
- [ ] Document pane-type-specific mouse entry rules — **OPEN** (deferred to Phase 11.3)

### Phase 9 tasks

#### Task 9.1 — Shared selection state model

Details:

- introduce one shared selection owner model in app state
- selection must be keyed by pane/surface owner
- do not bury it inside terminal-only structs

Done when:

- terminal, browser, and Neovim GUI can all target the same host concept

#### Task 9.2 — Action and mode integration

Details:

- add selection actions through `WmAction`
- wire them through `ActionRegistry`
- add a selection input mode rather than relying on mouse-only behavior
- entering selection mode must position a caret at the terminal cursor, not start a selection immediately
- mode-local bindings must be direct once inside selection mode (no prefix), but still action/keymap/config-driven

Done when:

- selection can be entered and manipulated from keyboard/action dispatch

#### Task 9.3 — Terminal adapter on shared model

Details:

- terminal becomes the first concrete backend using the shared selection system
- preserve TUI mouse behavior by keeping pointer selection behind an explicit gesture/policy
- terminal selection overlay/rendering must use the shared selection state
- keyboard-only flow must support:
  - enter selection mode
  - move caret to a start point
  - begin selection with `v` or `Space`
  - expand via movement keys
  - flip active end via `o`

Done when:

- terminal selection no longer behaves like a terminal-only special case

#### Task 9.4 — Copy extraction

Details:

- terminal/backend must expose selected text extraction
- copy action must operate on the active selection owner
- do not hardwire copy to terminal only

Done when:

- host can copy selected content regardless of owning pane type

#### Task 9.5 — Backend-native selection path design

Details:

- define how future browser panes opt into backend-native selection
- define how the host knows whether to render selection or defer to the backend

Done when:

- the browser/Neovim GUI path is specified clearly enough that another agent can implement it without redesign

## Phase 10 — Clipboard and Paste Semantics

Goal:

- layer system clipboard and paste behavior on top of the shared selection model

### Phase 10 checklist

- [ ] Copy selected content to the system clipboard
- [ ] Paste clipboard text into the focused pane through actions
- [ ] Support bracketed paste for terminal panes
- [ ] Support `OSC 52`
- [ ] Define copy-on-select policy, if desired
- [ ] When notifications/toasts are implemented, show a short shared host toast on successful copy instead of terminal-specific visual feedback

### Phase 10 tasks

#### Task 10.1 — Copy action

- copy must use the shared selection owner
- copy must not assume terminal-only text sources
- reminder: copy-success feedback should later use the shared notification/toast system, not a terminal-only flash or overlay

#### Task 10.2 — Paste action

- paste must route through the focused pane/backend
- terminal paste must respect bracketed-paste policy when enabled

#### Task 10.3 — Protocol-level clipboard support

- add `OSC 52`
- keep host clipboard and terminal protocol clipboard rules separate but compatible

## Phase 11 — Terminal UX and Attention Features

Goal:

- finish user-facing runtime behaviors that are broader than core rendering/input

### Phase 11 checklist

- [ ] bell handling policy
- [ ] scrollback search
- [ ] hyperlink/open-link behavior
- [ ] alternate-screen and focus-reporting validation
- [ ] richer mouse protocol coverage and selection-vs-terminal-mouse policy

### Phase 11 tasks

#### Task 11.1 — Bell policy

- backend alert capture
- host window attention
- audible bell policy
- visual bell or pane-attention indicator policy

#### Task 11.2 — Hyperlinks and scrollback UX

- `OSC 8` links
- open-link action
- scrollback search entry points and actions

#### Task 11.3 — Selection-vs-mouse final policy

- define exactly how selection and TUI mouse mode coexist
- document terminal defaults vs future pane-type-specific defaults

## Phase 12 — Richer Graphics / Image Protocols

Goal:

- support modern terminal graphics without contaminating the core text/grid model

### Phase 12 checklist

- [ ] image/graphics protocol surface design
- [ ] Yazi image preview support
- [ ] renderer placement model for graphics
- [ ] future hover/activation affordances where relevant

## Phase 13 — Pane-Shell Integration with `heca-grid-ui`

Goal:

- integrate the completed terminal capability set into the future pane shell / chrome host architecture

### Phase 13 checklist

- [~] terminal host mounted as content inside pane shell — partly: terminals render via `heca_grid_ui::widgets::Pane` (`paint_terminal_pane_shell`), but outer chrome/layout is still owned by `heca/src/app/render.rs`, not a shell host
- [x] process-aware shell state surfaced cleanly — `PaneRuntime` → chrome store → `heca/src/host.rs` selectors
- [ ] selection/copy/paste actions still reachable through mouse, keyboard, and RPC after shell migration — not yet verified post shell-migration (shell migration itself pending)
- [ ] float/transparency/blur planning remains shell-owned, not terminal-owned — **OPEN**: blur/frost is being resolved by `compositor-blur-refactor-plan.md` (z=0 background model); ownership boundary not yet settled

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
- terminal mounts now preserve `TerminalDamage` through the app path instead of draining it before render can use it
- terminal backend damage now derives visible changed-row ranges from `wezterm-term` sequence data where safe, with conservative `Full` fallback on uncertain structural transitions
- a retained terminal-content foundation now exists in app/renderer state, but its live damaged-frame presentation path is still guarded after a real resize/typing regression
- a follow-up review-fix pass cleaned up redundant clones, stale duplicated docs, TODO tracking, and targeted lint suppressions without broadening task scope

### Current Blocker — Terminal Pane Shell / Frosted Terminal Surface

> **RECONCILE (2026-06-19) — this blocker is largely RESOLVED; everything below is historical.**
> - **Pane shell border / radius: FIXED** (shipped PRs #121/#122). Verified in code:
>   `heca/src/app/terminal_render.rs::paint_terminal_pane_shell` renders the `heca-grid-ui` `Pane`
>   (`.border(color, width).radius(radius)`) with terminal content stencil-clipped to the inset content
>   rect; pane **border width / radius / gap / padding** are config-driven in
>   `heca-config/src/appearance.rs` (`pane_border_width`, `pane_border_radius`, `pane_gap`, `pane_padding`).
>   Tracked as `shared-tasks.md` **Task 07 → Accepted**.
> - **`terminal_blur`: partly addressed** by PR #125 (terminal-blur), but `PLAN.md` notes the blur rework is
>   still WIP — verify before considering it closed. This is the only item from this section still open.
>   **(2026-06-21 update)** — the blur/frost regression is now owned by `compositor-blur-refactor-plan.md`
>   (z=0 background model), which supersedes the tiled-tint approach. PR #160's `latte.toml`
>   `terminal_background = "#e6e9ef00"` (alpha 0) bypasses `terminal_transparency`→`surface_alpha` and is
>   the prime suspect for the “no blur/transparency” regression; the compositor-blur plan's Phase 3
>   critical review owns the translucency-channel reconciliation.
> - Selection (Phase 9) is **partly built** — `EnterSelectionMode` / `SelectionLeft..Down` / `CopySelection`
>   / `ToggleSelectionEndpoint` already exist as `WmAction`s in `heca/src/app/interaction.rs`.
>   **(2026-06-21 update)** — verified further: `SelectionState` (Inactive/Caret/Selecting/Selected) +
>   `SelectionRenderMode::{HostGrid,BackendNative}` exist in `heca/src/app/selection_model.rs`; the Phase 9
>   checklist is now re-ticked (9.1/9.2 + most task items done; backend-capability contract + full terminal
>   migration remain open).
> - The phase checklists / Overall Acceptance below were **not** re-ticked box-by-box this pass (each needs
>   its own code check). Do a fresh-session pass to mark Phase 8/13 + Overall Acceptance accurately.
>   **(2026-06-21 update)** — DONE this pass: Phase 8/9/13 checklists + Overall Acceptance re-ticked against
>   the codebase (see the reconcile note in `### Current Status` above).

Current user-verified state:

- terminal transparency amount now clearly responds to `terminal_transparency`
- terminal blur amount does **not** clearly respond to `terminal_blur`
- terminal panes still do **not** show the expected `Pane` shell border / radius
- pane gap now reads mainly because the terminal content rect is inset correctly, not because the shell border is visibly rendering

Agreed contract:

- `Pane` is the container of each terminal
- `Pane` owns:
  - border width
  - border color
  - border radius
  - active/inactive shell treatment
- the scrolling/layout container owns pane gap
- the terminal renders **inside** the pane content rect only
- terminal blur/transparency remain terminal-specific config knobs

What was implemented and should be kept:

- terminal-specific config:
  - `appearance.terminal_transparency`
  - `appearance.terminal_blur`
- runtime wiring:
  - `AppState::terminal_surface_opacity()`
  - terminal blur gate/radius in `heca/src/app/render.rs`
- real renderer fix:
  - `heca-renderer/src/backdrop.rs`
  - `heca-renderer/src/backdrop.wgsl`
  - fixed the `wgpu` uniform-layout mismatch
- structural cleanup:
  - terminal pane composition was extracted out of `heca/src/app/render.rs`
  - new owner: `heca/src/app/terminal_render.rs`
- content-rect alignment:
  - `heca/src/app/terminal_host.rs` now uses the same pane content inset rule as the render path

What was tried and did **not** solve it:

1. Letting the pane shell own the terminal surface fill
- result:
  - terminal panes became visually wrong / over-transparent
  - border/radius still did not read
- reverted

2. Falling back tiled terminal shell color to `theme.float_background`
- result:
  - semantically wrong
  - not approved
  - did not solve border visibility
- reverted

3. Rendering terminal pane shell under terminal content
- result:
  - shell was visually swallowed by the terminal surface/content
- changed

4. Rendering terminal pane shell over terminal content
- result:
  - still no visible border/radius in the real runtime
- still unresolved

5. Debugging through `Pane::bordered()` shell fill
- added obvious diagnostic shell fills through the `Pane` widget path
- result:
  - shell did not read the way expected
  - not sufficient to prove the border path was correct
- removed

6. Debugging through a direct low-level `DrawCommand::Rect`
- result:
  - proved the outer pane geometry/path can reach the screen
  - but did not resolve the actual `Pane` border/radius rendering problem
- removed

What this means technically:

- the terminal content rect is no longer the primary bug
- config reload for terminal transparency is no longer the primary bug
- the unresolved bug is the **pane shell visual path** for terminal panes
- there is also a second unresolved issue: `terminal_blur` does not produce a clearly distinct visual response across values in the current tiled-pane composition

Likely remaining causes:

1. the `Pane` widget path is not producing a visible border-only shell for terminals in the current render ordering/composition
2. tiled terminal blur is still too visually weak / too similar across values because the current blurred source and terminal surface composition do not produce enough perceptible change

What should be done next:

1. Do **not** keep guessing style values in `render.rs`
2. Keep terminal pane composition isolated in `heca/src/app/terminal_render.rs`
3. Debug the `Pane` shell path itself:
   - verify exactly which `DrawCommand`s are emitted for terminal panes
   - compare them to a sidebar shell / showcase pane that visibly renders borders
   - identify whether the issue is:
     - missing border command emission
     - wrong border alpha/color after theme bridging
     - wrong clip/order in the terminal shell pass
4. Separately debug tiled blur strength:
   - compare `terminal_blur = 0`, `20`, `70`, `100`
   - if visually flat, revisit the tiled-pane blur composition rather than terminal config parsing
5. Do not invent a new shell abstraction:
   - stay on the agreed `Pane` container path unless the user explicitly approves a change

Uncommitted local files at pause point for this blocker:

- `heca-config/src/appearance.rs`
- `heca-renderer/src/backdrop.rs`
- `heca-renderer/src/backdrop.wgsl`
- `heca-renderer/src/terminal.rs`
- `heca/src/app/mod.rs`
- `heca/src/app/render.rs`
- `heca/src/app/terminal_host.rs`
- `heca/src/app/terminal_render.rs`
- `heca/src/app_state.rs`

### Runtime Notes

- Normal app runtime now uses `TerminalBackend`, not `FakeBackend`
- `FakeBackend` remains only as:
  - backend-factory fallback when PTY startup fails
  - testing/dev placeholder backend
- The original "typing a character takes seconds" lag is no longer the primary issue after the `heca-renderer/src/text.rs` cache change
- The current user-verified runtime state is:
  - terminal appears
  - typing is fast again
  - text rendering is now much closer to correct, with the most obvious placement and cursor issues resolved
- This means the current bottleneck has moved from gross per-frame shaping/upload cost to final validation and small residual terminal-fidelity checks

### Blockers

- **(2026-06-19) Reconciled:** the terminal `Pane` shell **border/radius blocker is FIXED** (#121/#122 —
  see the RECONCILE note under "Current Blocker"; `shared-tasks.md` Task 07 → Accepted). Phase 13 pane-shell
  hosting is **no longer blocked** on border/radius.
- **Still open:** `terminal_blur` — partly addressed by #125 but the blur rework is WIP per `PLAN.md`; verify
  whether values produce a clearly distinct visual response before closing. **(2026-06-21)** The active
  resolution path is now `compositor-blur-refactor-plan.md` (z=0 background model, which supersedes the
  tiled-tint approach). That plan has been reconciled with PR #160; its Phase 3 critical review owns the
  `latte.toml` transparent-bg / `surface_alpha` reconciliation. Do **not** fix the blur regression ad hoc in
  `heca/src/app/render.rs` — route all frost/blur work through the compositor-blur plan.

### Verification State

- `cargo check -p heca-core` passes with `portable-pty` + `wezterm-term`
- `cargo test -p heca-core` passes with backend lifecycle tests
- `cargo check -p heca` passes with the composed terminal backend
- `cargo check -p heca-renderer` passes after renderer label-cache changes and the renderer/core hardening pass
- `cargo test -p heca-renderer` passes with text-label cache hit/miss/prune/invalidation tests
- `cargo clippy -p heca-renderer --all-targets` passes after renderer label-cache changes
- `cargo clippy --workspace --all-targets --all-features` passes after the renderer label-cache changes
- User smoke test after the renderer cache change:
  - terminal is interactive again
  - text placement issues are now much improved after the latest sizing/cursor fixes
- After terminal-font/fallback work:
  - `cargo check -p heca` passes
  - `cargo check -p heca-renderer` passes
  - `cargo clippy -p heca --all-targets` passes
  - `cargo clippy -p heca-renderer --all-targets` passes
  - floating-pane terminal panic from row-width mismatch is fixed
- During the current pane-shell blocker investigation:
  - `cargo check -p heca` passes after each structural/render-order change
  - `cargo test -p heca-renderer terminal::tests -- --nocapture` passes for the terminal alpha helpers
- During the `terminal-00` damage-preservation pass:
  - `cargo check -p heca` passes
  - targeted `heca` terminal-render tests passed during retained-layer implementation
  - targeted backend damage tests passed while landing visible-row damage production
- During the 2026-06-23 review-fix pass:
  - `cargo check -p heca` passes
  - `cargo test -p heca app::interaction::tests -- --nocapture` passes
  - `cargo test -p heca app::selection_model::tests -- --nocapture` passes

### Fresh Session Restart Steps

If starting a brand-new session for this exact blocker:

1. Read `AGENTS.md`
2. Read `terminal-implementation.md`
3. Read `.planning/STATE.md`
4. Read the current local diffs in:
   - `heca/src/app/terminal_render.rs`
   - `heca/src/app/render.rs`
   - `heca-renderer/src/terminal.rs`
   - `heca-config/src/appearance.rs`
   - `heca/src/app_state.rs`
   - `heca/src/app/terminal_host.rs`
5. Preserve these decisions:
   - do not invent a new shell path without approval
   - `Pane` remains the terminal container abstraction
   - terminal blur/transparency remain terminal-owned, not shell-owned
   - ask before changing unplanned architecture/styling/layer decisions
6. Start from the two concrete unresolved questions only:
   - why the `Pane` shell border/radius is still not visibly rendering
   - why `terminal_blur` values still do not produce a strong visible difference

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
