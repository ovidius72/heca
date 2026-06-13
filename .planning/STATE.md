---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: Phase 3 of 4 (The Content)
status: in_progress
last_updated: "2026-06-14T00:00:00.000Z"
progress:
  total_phases: 4
  completed_phases: 2
  total_plans: 1
  completed_plans: 0
  percent: 62
---

# State: heca

**Current Phase:** Phase 3 — The Content
**Status:** In progress
**Last Action:** Closed the renderer/core review blockers and follow-up cleanup, including u32 primitive indices, safe terminal grid fitting, deterministic spawn sizing, underline/undercurl safety, and shared terminal-grid helpers

## Product Phase Progress

| Phase | Status | Notes |
|-------|--------|-------|
| 1 — The Shell | ✅ Done | GPU windowing, theme/config, core renderer foundation landed |
| 2 — The Workspace | ✅ Done | NIRI-style layout, chrome, actions, and refactoring track landed |
| 3 — The Content | 🔄 In Progress | Real terminal backend live; typing lag improved; structured input path landed; final validation/review loop remains before merge |
| 4 — The Platform | ⬜ Pending | Session, RPC, plugins, damage tracking polish |

## Phase 3 Status

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

### Remaining For Phase 3

- Finish the final live validation pass across a few more terminal fonts and a few more `nvim` colorschemes
- Re-run the phase-end Rust review now that the terminal renderer/core hardening pass is landed
- Fix anything the final Rust review finds, then rerun until clean
- Keep the shared terminal symbol/decorations renderer app-agnostic:
  - box drawing stays deterministic geometry
  - powerline separators stay deterministic geometry
  - underline/undercurl stay GUI-native and font-independent
- Start the Neovim pane implementation after the terminal rendering/input path is fully settled

## Backend Status

### Real Runtime Path

- Normal runtime path uses `TerminalBackend`
- `FakeBackend` is no longer the normal app backend

### Why `FakeBackend` Still Exists

- startup fallback if PTY-backed terminal creation fails
- tests/dev placeholder behavior

## Known Risks

- Terminal drawing now enters through a dedicated renderer module, but glyph shaping still relies on generic `TextRenderer` internals
- Yazi image preview still spins forever because richer graphics/image protocol support is not implemented yet
- Terminal palette/theme fidelity still benefits from a few more live checks across additional themes and fonts before merge
- Terminal font family naming must match the embedded font metadata (`Maple Mono Normal NF`)
- Mouse-aware TUIs now have a structured forwarding path, but live behavior still needs one more verification pass after the latest renderer sync
- The full clipboard/selection/backscroll/post-merge backlog is intentionally deferred and should not block this PR

## Resume Point

If resuming from a fresh session, do this first:

1. Read `terminal-implementation.md`
2. Continue Phase 3, not Phase 4
3. Inspect the current live metrics/render path:
- `heca/src/app/terminal_metrics.rs`
- `heca/src/app/terminal_host.rs`
- `heca-renderer/src/text.rs`
- `heca-renderer/src/terminal.rs`
4. Re-run the last user-validated runtime checks before making new changes:
- `nvim` colorscheme switching
- Yazi layout and right-edge symbol rendering
- shell autosuggestion cursor placement
- font-sensitive file-manager layout using at least one alternative Nerd Font if needed
5. Then run the phase-end Rust review and fix anything it finds before merging

## Full Terminal Backlog (Post-Merge)

- terminal selection state/rendering
- copy selected terminal text to the system clipboard
- paste system clipboard text into the focused terminal backend
- bracketed paste
- OSC 52 clipboard integration
- hyperlink/open-link behavior
- bell handling
- alternate-screen and focus-reporting validation
- richer mouse protocol coverage and selection-vs-terminal-mouse policy
- scrollback search / terminal UX actions
- richer image/graphics protocol support

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
- Current phase is not complete yet, so no phase-end Rust-skill review has been run for this latest renderer work.
