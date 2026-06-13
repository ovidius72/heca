# Phase 3: The Content — Plan

**Phase:** 3
**Name:** The Content
**Waves:** 4
**Requirements:** PANE-WIRE, 8.3, PANE-ELP, PANE-03

---

## Must-Haves

1. A real shell runs inside panes instead of a static fake pattern.
2. Terminal output renders smoothly — PTY reader wakes the event loop.
3. `render_data()` doesn't copy the full grid every frame — only changed rows.
4. Mouse events inside a terminal pane are forwarded as SGR sequences.
5. PTY creation failures are handled gracefully.

---

## Wave 1: Wire TerminalBackend Into App (PANE-WIRE)

### Plan 1.1 — Export TerminalBackend

Add `pub use terminal::TerminalBackend;` to `heca-core/src/backend/mod.rs` so app code can import it.

**Files:** `heca-core/src/backend/mod.rs`

### Plan 1.2 — Wire startup.rs

Replace `FakeBackend::new(80, 24)` with `TerminalBackend::new(80, 24).expect(...)` in `heca/src/app/startup.rs`. Panic on failure — can't start without a terminal.

**Files:** `heca/src/app/startup.rs`

### Plan 1.3 — Wire handlers.rs (5 creation sites)

Replace all `FakeBackend::new(80, 24)` calls with `TerminalBackend::new(80, 24)`. On error, fall back to `FakeBackend` + log the error so the pane still shows something.

**Files:** `heca/src/handlers.rs`

### Plan 1.4 — Wire mutations.rs

Same replacement in `heca/src/app/mutations.rs`.

**Files:** `heca/src/app/mutations.rs`

---

## Wave 2: Fix Render-Data Cloning (8.3 — Damage Tracking)

### Plan 2.1 — Add dirty-rows tracking to Grid

Add `dirty_rows: Vec<bool>` to `Grid`. Every mutation (`put_char`, `scroll_up`, `clear_*`, `resize`) marks affected rows dirty. Initialize all `true`.

**Files:** `heca-core/src/backend/terminal.rs`

### Plan 2.2 — Optimize render_data()

Only rebuild rows where `dirty_rows[row] == true`. Clear dirty flags after rebuilding. Clean rows are skipped — no allocation for unchanged content.

**Files:** `heca-core/src/backend/terminal.rs`

### Plan 2.3 — Audit all mutation paths

Ensure every Grid mutation correctly sets `dirty_rows`. Add a `mark_dirty(row)` helper. Check all ESC sequence handlers in the vte `Perform` impl.

**Files:** `heca-core/src/backend/terminal.rs`

---

## Wave 3: EventLoopProxy (PANE-ELP)

### Plan 3.1 — Add EventLoopProxy to AppState

Add `event_loop_proxy: Option<winit::event_loop::EventLoopProxy<()>>` to `AppState`. Initialize to `None`. Set from `main.rs` during startup.

**Files:** `heca/src/app_state.rs`, `heca/src/main.rs`

### Plan 3.2 — Wake on PTY data

In `handle_about_to_wait()`, after `backend.update()` returns true, send a wake event via the proxy. Handle the custom event in `main.rs` to set `needs_redraw = true`.

**Files:** `heca/src/app/lifecycle.rs`, `heca/src/main.rs`

---

## Wave 4: SGR Mouse Forwarding (PANE-03)

### Plan 4.1 — Parse mouse enable/disable in vte

Add `mouse_tracking` and `button_event_tracking` bools to `Grid`. Handle `ESC[?1006h/l` (SGR) and `ESC[?1002h/l` (button-event) in the vte `csi_dispatch`.

**Files:** `heca-core/src/backend/terminal.rs`

### Plan 4.2 — Expose on PaneBackend trait

Add `fn mouse_tracking(&self) -> bool` (default `false`) to the `PaneBackend` trait. `TerminalBackend` returns `mouse_tracking && button_event_tracking`. `FakeBackend` uses the default.

**Files:** `heca-core/src/backend/mod.rs`, `heca-core/src/backend/terminal.rs`

### Plan 4.3 — Translate winit events to SGR sequences

Helper function: pixel coords → cell coords using pane geometry and `cell_size()`. Builds SGR escape for press (`M`), release (`m`), and scroll (button 64/65).

**Files:** `heca/src/mouse.rs` (or `heca/src/mouse/sgr.rs`)

### Plan 4.4 — Wire forwarding into main handlers

After WM-level mouse handling, if the target pane has mouse tracking, send the SGR sequence via `backend.process_input()`. Additive — doesn't affect focus/drag/sidebar behavior.

**Files:** `heca/src/main.rs`, `heca/src/mouse.rs`

---

## Threat Model

| Threat | Mitigation |
|--------|-----------|
| PTY failure at startup | Panic with clear message |
| PTY failure at dynamic creation | Fall back to FakeBackend, log error |
| render_data() performance | Dirty-row tracking — only changed rows rebuilt |
| EventLoopProxy wake floods | Best-effort send, one wake per update() cycle |
| Mouse tracking state desync | Backend exit → should_close() → pane cleaned up |

---

## Checklist

### Wave 1 — PANE-WIRE
- [ ] 1.1 Export TerminalBackend from mod.rs
- [ ] 1.2 Wire TerminalBackend in startup.rs
- [ ] 1.3 Wire TerminalBackend in handlers.rs (5 sites)
- [ ] 1.4 Wire TerminalBackend in mutations.rs

### Wave 2 — 8.3 Damage Tracking
- [ ] 2.1 Add dirty_rows to Grid, mark on all mutations
- [ ] 2.2 Optimize render_data() to skip clean rows
- [ ] 2.3 Audit all mutation paths for dirty marking

### Wave 3 — PANE-ELP
- [ ] 3.1 Add EventLoopProxy to AppState
- [ ] 3.2 Request redraw when backend has new data

### Wave 4 — PANE-03
- [ ] 4.1 Parse mouse enable/disable in vte
- [ ] 4.2 Expose mouse_tracking on PaneBackend trait
- [ ] 4.3 Translate winit events to SGR sequences
- [ ] 4.4 Wire forwarding into main event handlers

### Verification
- [ ] cargo check — clean
- [ ] cargo clippy — clean
- [ ] cargo test — all pass
- [ ] Manual: real shell in panes
- [ ] Manual: keyboard input works
- [ ] Manual: mouse forwarding (if PANE-03 done)
