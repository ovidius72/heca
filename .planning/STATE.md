---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: Phase 3 of 4 (The Content)
status: in_progress
last_updated: "2026-06-12T16:45:00.000Z"
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
**Last Action:** Fixed terminal default-color rendering and added separate italic terminal font-family routing

## Product Phase Progress

| Phase | Status | Notes |
|-------|--------|-------|
| 1 — The Shell | ✅ Done | GPU windowing, theme/config, core renderer foundation landed |
| 2 — The Workspace | ✅ Done | NIRI-style layout, chrome, actions, and refactoring track landed |
| 3 — The Content | 🔄 In Progress | Real terminal backend live; typing lag improved; structured input path landed; visual refinement and Neovim still pending |
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
- Terminal text rendering is still visually misaligned/off inside the pane
- Floating a pane no longer panics on row/column mismatch during terminal background rendering
- Terminal text now uses box-based placement for stable vertical centering, but live alignment still needs verification
- `nvim` redraw behavior should improve because PTY output can now wake the app directly, but live verification is still pending
- Sidebar visibility during zoom/float should improve because chrome now renders after floats, but live verification is still pending
- User-reported focus-border issues on newly created panes should improve because render highlighting now follows session focus, but live verification is still pending
- Structured terminal keyboard/mouse forwarding is now implemented, but live verification is still pending for `nvim` mouse mode, wheel behavior, and modifier-heavy key combinations
- Terminal style fidelity is improved, but live verification still shows colorscheme mismatch with other terminals
- The plain shell background is still wrong in practice; it is currently falling back to stock terminal palette behavior because full terminal palette/theme support is not implemented yet
- Italic styling works, but italic runs may still resolve to the wrong family unless an italic face is installed or `terminal_italic_font_family` is configured
- Straight underline works; undercurl is not implemented yet

### Remaining For Phase 3

- Implement proper terminal palette/theme support so terminal default colors and ANSI colors can be configured independently from outer app chrome
- Decide whether undercurl belongs in Phase 3 or is explicitly deferred
- Finish the dedicated terminal renderer by replacing the remaining transitional generic text glyph path
- Validate terminal mouse, focus tracking, and remaining style fidelity in live TUIs
- Start Neovim pane implementation after terminal rendering/input path is solid

## Backend Status

### Real Runtime Path

- Normal runtime path uses `TerminalBackend`
- `FakeBackend` is no longer the normal app backend

### Why `FakeBackend` Still Exists

- startup fallback if PTY-backed terminal creation fails
- tests/dev placeholder behavior

## Known Risks

- Terminal drawing now enters through a dedicated renderer module, but glyph shaping still relies on generic `TextRenderer` internals
- Terminal text placement is visually incorrect even though interactivity is much better
- Terminal font family naming must match the embedded font metadata (`Maple Mono Normal NF`)
- Mouse-aware TUIs now have a structured forwarding path, but live behavior still needs verification
- Review backlog still open for richer `PtyError` typing and config-loader precedence coverage

## Resume Point

If resuming from a fresh session, do this first:

1. Read `terminal-implementation.md`
2. Continue Phase 3, not Phase 4
3. First validate the structured terminal input path live:
- inspect `heca/src/app/terminal_host.rs`
- inspect `heca/src/app/events.rs`
- inspect `heca-core/src/backend/terminal/engine.rs`
- verify `nvim` mouse mode, wheel behavior, focus tracking, and modifier-heavy key combinations
4. Then continue terminal visual correctness:
- inspect `heca-renderer/src/terminal.rs`
- inspect `heca-renderer/src/text.rs`
- validate and refine the new line-box glyph placement before touching Neovim work

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
