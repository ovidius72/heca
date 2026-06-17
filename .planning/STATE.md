---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: Phase 4 of 4 (The Platform)
status: in_progress
last_updated: "2026-06-17T00:00:00.000Z"
progress:
  total_phases: 4
  completed_phases: 3
  total_plans: 1
  completed_plans: 0
  percent: 75
---

# State: heca

**Current Phase:** Phase 4 — The Platform
**Status:** In progress
**Last Action:** Phase 13 pane-shell visual blocker 1 (border/radius rendering) fixed in PR #121; blocker 2 (terminal_blur) remains. Stencil rounded content-clip + snug padding + chrome value clamps + theme-driven `pane_padding` landed; pre-existing clippy lints deferred to pre-PR cleanup.

## Product Phase Progress

| Phase | Status | Notes |
|-------|--------|-------|
| 1 — The Shell | ✅ Done | GPU windowing, theme/config, core renderer foundation landed |
| 2 — The Workspace | ✅ Done | NIRI-style layout, chrome, actions, and refactoring track landed |
| 3 — The Content | ✅ Done | Real terminal backend and renderer landed and merged |
| 4 — The Platform | 🔄 In Progress | Post-merge terminal backlog and future pane/platform integration work |

## Phase 4 Status

### Landed

- `portable-pty + wezterm-term + cosmic-text` selected as the terminal stack
- `PaneBackend` now exposes a renderer-agnostic `terminal_snapshot()` path
- `TerminalBackend` now composes:
  - PTY wrapper in `heca-core/src/backend/terminal/pty.rs`
  - terminal engine wrapper in `heca-core/src/backend/terminal/engine.rs`
- App pane creation now uses `TerminalBackend` by default through `heca/src/app/backend_factory.rs`
- App lifecycle closes panes whose terminal process exits
- Pane rendering now:
  - renders terminal content inside an inset content rect
  - clips per pane via renderer scissor rects
  - prefers `terminal_snapshot()` over legacy `render_data()`
- Terminal drawing now has a dedicated renderer entrypoint in `heca-renderer/src/terminal.rs`
- Terminal cell backgrounds and cursor rendering now live in the renderer crate instead of `heca/src/app/render.rs`
- Terminal glyph runs now use a terminal-specific line-box placement mode with exact cell-span widths
- Generic text renderer now caches shaped/rasterized labels across frames, which removed the worst terminal typing stall
- Generic text cache behavior now has unit coverage for hit/miss, pruning, and invalidation cases
- Live pane rendering no longer falls back to `render_data()` for terminal backends; it consumes `terminal_snapshot()` only
- App-side terminal mounting now goes through `heca/src/app/terminal_host.rs`, which prepares terminal mounts from `Rectangle` content geometry instead of inlining backend sizing/snapshot logic in pane loops
- Terminal font settings are now separate from UI theme font settings
- `config.toml` can now override terminal font family and terminal font size
- both underscore and kebab-case terminal font keys are accepted from `config.toml`
- Embedded Maple Mono Normal NF regular/bold is now available as the terminal fallback font
- Terminal backends are now resized from live pane content geometry before snapshot rendering
- PTY reader threads now wake the winit event loop through a user-event proxy so terminal output can trigger redraws without waiting for keyboard or mouse input
- Render-time active-pane highlighting now derives focus from the session active pane, not only from the mirrored `AppState.focused_pane`
- Sidebar/chrome rendering now happens after floating panes so sidebars remain visible in float/zoom states
- Pane backends now expose structured keyboard and mouse event hooks in addition to raw byte input
- Terminal keyboard input now prefers structured key-event encoding through `wezterm-term`
- Terminal mouse input now routes through `heca/src/app/terminal_host.rs`, which maps content-area pointer events into terminal cell coordinates
- Pane focus changes and window focus changes now notify the focused terminal backend for terminal focus tracking
- Terminal cells now carry italic/underline style flags in addition to foreground/background/bold
- Terminal snapshots now carry resolved terminal default foreground/background colors from `wezterm-term`
- Terminal rendering now respects reverse-video colors, italic text, underline decoration, and uses the resolved terminal default background instead of a hardcoded black backdrop
- Terminal theme config now supports a separate italic terminal font family in addition to terminal font family and size
- Bold ANSI foreground colors now follow wezterm-style brightening semantics for palette indices `0..7`
- When no separate italic face is configured, terminal italics now keep the terminal family and use a faux-italic slant instead of falling back to a generic italic face
- Terminal engine now supports explicit terminal default foreground/background overrides without forcing terminal defaults to follow the outer app theme
- Terminal snapshot generation now preserves styled blank cells across the full visible row, which is required for `nvim` and other full-screen TUIs to render their background colors correctly
- Terminal palette configuration now supports default colors plus ANSI/brights/cursor/selection colors through config/theme plumbing
- Phase 13 pane-shell visual blocker 1 (PR #121): pane borders/radius now render (root cause = `GridRenderer` persistent-buffer `begin_frame`-once-per-frame contract violated by `render_chrome`); terminal content clipped to the rounded border via a stencil-write pass (new `fs_stencil` WGSL + `STENCIL_FORMAT` + stencil-view plumbing across backdrop/primitive/text); snug `pane_padding` (8→4); chrome value clamps (radius/border_width/pane_padding) in resolvers; `Theme.pane_padding` field (theme-driven, no hardcode); `keybindings.toml` `[appearance]` reference documented; `heca-renderer/src/clip.rs` extracted (`intersect`/`combine_clip`). heca 196 / heca-renderer 10 / heca-config 31 tests green; 0 new clippy warnings. Blocker 2 (`terminal_blur`) remains open.

### Current User-Verified Runtime State

- Terminal pane appears correctly as a live PTY-backed pane
- Typing is fast again and no longer stalls for seconds
- Terminal text rendering is broadly correct again after the main sync, including shell autosuggestion cursor placement and cursor shape behavior
- Floating a pane no longer panics on row/column mismatch during terminal background rendering
- Terminal text now uses per-cell placement plus measured font-derived cell sizing, and Yazi/nvim are looking materially better after the powerline/symbol follow-up
- `nvim` redraw behavior is now healthy because PTY output wakes the app directly
- Sidebar visibility during zoom/float is now correct because chrome renders after floats
- Focus-border issues on newly created panes are now resolved by render highlighting following session focus
- Structured terminal keyboard/mouse forwarding is landed and has been live-tested in `nvim`
- Terminal style fidelity is improved; live validation now reports color themes as much closer to the source terminals
- Yazi now renders correctly, including the previously broken powerline/status separator cases
- `nvim` now renders correctly, including the previously broken powerline/status separator cases
- Underline and undercurl decoration now render explicitly and are live-tested
- Review-driven renderer/core hardening is landed and the affected checks/tests are green again

### Sequencing (revised 2026-06-17)

1. **Now — Phase 13 blocker 2: `terminal_blur`** visual response (= PLAN.md priority #2 in-app blur app-wiring). `terminal_blur` 0→100 must be visibly monotonic. Diagnose `heca-renderer/src/backdrop.rs` + `backdrop.wgsl` (0.15-alpha stamp + 24px cap hypothesis); wire real modulation through the `[appearance]` plumbing.
2. **Post-merge terminal backlog** (we have worked on this; additions marked **NEW**): Phase 9 shared selection → Phase 10 clipboard/paste → Phase 11 UX/attention (**NEW: terminal contextual menu**, mouse-triggered) → Phase 12 graphics/**image rendering** (Yazi + tools; wezterm supports Sixel/iTerm2/Kitty — see Phase 12 entry).
3. **At the end — Phase 13 wrap-up**: rust-skill phase-end review + pre-existing clippy-lint cleanup (8 Phase-9 `selection_model` dead_code → annotate with reason; `too_many_arguments` pre-existing on main).
4. **External (separate branch, not this effort)**: SharedChromeState (P0) consumer migration, F4.4 marker/rail + targeting, pane numbering, appearance/zoom/font controls. F4.5 (DnD/grab-cursor) landed on main via PR #120. Do not re-plan these here.

> **UI rule:** all new UI elements/components/widgets introduced by this backlog (e.g. the Phase 11 context menu, any image-preview chrome) MUST be proper `heca-grid-ui` widgets per PLAN.md locked rules (embed `Base`, read ALL styling from `Theme`, domain-neutral, no hardcoded sizes/colors/alphas, behavior via `ActionRegistry`/`KeymapRegistry`). Low-level image *texture* rendering (Phase 12) is `heca-renderer` (wgpu) — a different altitude, not a grid-ui widget.

### Active Post-Merge Terminal Backlog

- Phase 9 — Shared host selection capability
  - selection must be reusable across terminal, future Neovim GUI, browser, and host-native panes
  - selection must be reachable through mouse, keyboard/actions, and RPC where meaningful
  - terminal-specific gestures may exist as entry paths, but selection itself must not remain terminal-only
- Phase 10 — Clipboard and paste semantics
  - copy selected content to system clipboard
  - paste into focused pane
  - bracketed paste
  - `OSC 52`
- Phase 11 — Terminal UX and attention features
  - bell handling
  - scrollback search
  - hyperlink/open-link behavior
  - richer mouse protocol coverage and final selection-vs-terminal-mouse policy
  - **NEW — terminal contextual menu (mouse-triggered)**: right-click / mouse-triggered menu on the terminal pane offering actions — copy current selection (if any), close pane, and others. Build as a generic `heca-grid-ui` context-menu widget (domain-neutral); route every action through `ActionRegistry` (mouse/UI + keybinding + RPC parity per AGENTS). "Copy" depends on Phase 10 clipboard.
- Phase 12 — Richer graphics / image protocols
  - **NEW — image rendering** so Yazi and other tools can show images. wezterm **does** support image protocols — Sixel, iTerm2 inline images (imgcat), and the Kitty graphics protocol; `wezterm-term` parses these and tracks images via its image-attachment system (`ImageData`/`ImageCell`/placement). heca's `TerminalSnapshot` currently carries only cells/colors/cursor (no image data), so this needs: (a) verify the pinned wezterm-term rev's image API surface (may differ from `main`), (b) extract image data + cell→image placement from the terminal model, (c) carry image info in or alongside the snapshot, (d) render images as wgpu textures in `heca-renderer` (new image render path; respect the stencil rounded-clip + scissor). Yazi image preview currently spins forever because these protocols aren't implemented. **Layering:** parse via `wezterm-term` (images are part of the terminal byte stream + grid placement — not a separate parser); draw via heca-renderer/wgpu (wezterm-term is headless, gives RGBA + placement); a Rust `image` crate is only an optional helper for format edge cases or a future non-terminal image pane — not a replacement for the protocol parser.
  - broader image/graphics protocol coverage (Kitty animations, etc.)
- Phase 13 — Pane-shell integration with `heca-grid-ui`
  - terminal host mounted as content inside the future pane shell
  - selection/copy/paste actions preserved across shell migration

## Backend Status

### Real Runtime Path

- Normal runtime path uses `TerminalBackend`
- `FakeBackend` is no longer the normal app backend

### Why `FakeBackend` Still Exists

- startup fallback if PTY-backed terminal creation fails
- tests/dev placeholder behavior

## Known Risks

- Terminal drawing now enters through a dedicated renderer module, but glyph shaping still relies on generic `TextRenderer` internals
- Phase 13 pane-shell integration — visual correctness:
  - ✅ terminal transparency amount responds to config
  - ✅ terminal pane border/radius now render correctly through the `heca-grid-ui` `Pane` container path (PR #121 — root cause: `render_chrome` violated the `GridRenderer` persistent-buffer `begin_frame`-once-per-frame contract, resetting the vertex offset; fix = `begin_frame()` once at frame top)
  - ✅ terminal content follows the pane's rounded border (stencil rounded content-clip, no corner overflow) + snug padding (`pane_padding` 8→4) + chrome value clamps (radius [0,20], border_width [0,10], pane_padding [0,20]) + theme-driven `pane_padding`
  - ⛔ terminal blur amount still does not produce a strong visible difference across values — **next**
- Yazi image preview still spins forever because richer graphics/image protocol support is not implemented yet
- Terminal palette/theme fidelity still benefits from a few more live checks across additional themes and fonts before merge
- Terminal font family naming must match the embedded font metadata (`Maple Mono Normal NF`)
- Mouse-aware TUIs now have a structured forwarding path, but live behavior still needs one more verification pass after the latest renderer sync
- Selection/copy/paste must now follow the shared host-selection contract rather than a terminal-only design
- Selection mode must become keyboard-first:
  - entering selection mode places a caret at the terminal cursor
  - entering selection mode does not automatically start selection
  - `v` / `Space` begin selection from the caret
  - `o` flips the active endpoint
  - `y` copies and stays in selection mode
  - `x` is reserved for paste once paste is implemented

## Resume Point

If resuming from a fresh session, do this first:

1. Read `terminal-implementation.md`
2. Continue the post-merge Phase 4 terminal backlog per **Sequencing (revised 2026-06-17)**; immediate next = Phase 13 blocker 2 (`terminal_blur`), then Phase 9 → 10 → 11 (+context menu) → 12 (+image rendering)
3. Read the shared-selection contract in `terminal-implementation.md` before extending selection/copy/paste
4. Keep selection reusable across terminal, browser, future Neovim GUI, and host-native panes
5. If touching bell/clipboard/graphics, avoid `heca-grid-ui` chrome files unless the task is explicitly pane-shell integration
6. For the current pane-shell blocker:
   - stay on the agreed `Pane` container path
   - do not invent a new shell abstraction without approval
   - ✅ missing visible pane border/radius — fixed (PR #121)
   - ⛔ weak / non-distinct `terminal_blur` response — **remaining focus**

## Structured Post-Merge Terminal Backlog

- Phase 9 — Shared host selection capability
- Phase 10 — Clipboard and paste semantics
- Phase 11 — Terminal UX and attention features (+ **NEW: terminal contextual menu** — mouse-triggered: copy selection / close pane / others; via `ActionRegistry`)
- Phase 12 — Richer graphics / image protocols (+ **NEW: image rendering** — wezterm supports Sixel/iTerm2/Kitty; heca must extract + render images)
- Phase 13 — Pane-shell integration with `heca-grid-ui` (blocker 1 done — PR #121; blocker 2 `terminal_blur` = now)

## Verification

- `cargo check -p heca-core`
- `cargo test -p heca-core`
- `cargo check -p heca-renderer`
- `cargo test -p heca-renderer`
- `cargo check -p heca`
- `cargo test -p heca`
- `cargo clippy -p heca --all-targets`
- `cargo clippy -p heca-renderer --all-targets`
- `cargo clippy --workspace --all-targets --all-features`

All passing at the current pause point.

## Notes

- Do not run the Rust-skill review on each task.
- Run the Rust-skill review only at the end of a full implementation phase, then fix/review until clean.
- Phase 3 is complete and merged; new work should follow the explicit post-merge backlog phases above.
