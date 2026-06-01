# Phase 3: The Content — Context & Notes

## Requirements (from ROADMAP.md)

| ID | Requirement | Status | Blocker |
|----|-------------|--------|---------|
| PANE-01 | Terminal pane — spawns PTY, renders cell grid via `vte` parser | Code exists, not wired | — |
| PANE-02 | Terminal pane forwards keyboard input to PTY | Code exists, not wired | — |
| PANE-03 | Terminal pane forwards mouse input as SGR mouse sequences | Not started | **PANE-01** |
| PANE-04 | Neovim pane — spawns `nvim --embed`, speaks msgpack-RPC | Not started | — |
| PANE-05 | Neovim pane renders grid updates from `redraw` events | Not started | **PANE-04** |
| PANE-06 | Neovim pane handles multiple grids (floating windows) | Not started | **PANE-05** |
| PANE-07 | Neovim pane forwards keyboard input as nvim input notation | Not started | **PANE-04** |
| PLUG-01 | `PaneBackend` trait defines pane lifecycle | ✅ Done | — |
| PLUG-02 | Terminal and Neovim are built-in implementations of `PaneBackend` | Partial | — |

## Critical dependency chain

```
TerminalBackend switch (PANE-01 + PANE-02)
        ↓
EventLoopProxy custom events (PTY reader thread → wake event loop)
        ↓
SGR mouse forwarding (PANE-03)
        ↓
Neovim msgpack-RPC backend (PANE-04)
        ↓
Neovim grid rendering (PANE-05)
        ↓
Neovim floating windows (PANE-06)
```

## Why this ordering

1. **TerminalBackend before SGR mouse**: FakeBackend never enables mouse mode — there's no application inside it to request `ESC[?1006h`. SGR forwarding only makes sense when a real shell/vim/tmux is running inside the PTY.

2. **EventLoopProxy before anything live**: The PTY reader thread runs in a background thread. Without waking the winit event loop, terminal output only renders on the next keyboard/mouse event. The app feels laggy.

3. **Neovim after Terminal**: Neovim backend is another `PaneBackend` implementation. The infrastructure for PTY spawning, reader threads, and event-loop wakeups is shared. TerminalBackend proves the pipeline works before adding msgpack-RPC complexity.

## Current state

- `heca-core/src/backend/terminal.rs` — PTY + `vte` parser + background reader thread exist
- `heca-core/src/backend/fake.rs` — Currently used by `heca/src/main.rs` for all panes
- `heca/src/main.rs` — Keyboard forwarding to backend exists; needs to switch which backend type is created
- No `EventLoopProxy` custom events wired yet
- No mouse forwarding to backend yet
