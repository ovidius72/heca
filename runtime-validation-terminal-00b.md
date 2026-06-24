# Runtime validation — `terminal-task-00b` (retained terminal-content foundation)

> This is the **only** remaining gap for `terminal-task-00b`. The code is in place
> and audited; these scenarios must be confirmed by running the app, because they
> depend on real GPU/wgpu behavior and HiDPI scaling that unit tests can't cover.
> A previous attempt regressed here (oversized glyphs while resizing + hidden
> freshly-typed content); the scratch-sizing fix claims to have resolved it, but
> only runtime sign-off can close the task.

## What the retained path does (so you know what to look for)

Each terminal pane has a **per-pane retained GPU texture** that holds the last
frame's content. On each frame with damage:

1. The offscreen **scratch** texture is re-sized to the pane's *exact* physical
   size and cleared.
2. Only the **dirty rows** (backend `TerminalDamage::Rows`) are rendered into the
   scratch; a resize, scale-factor change, or font/alpha/style change forces a
   `Full` repaint.
3. Only the **dirty pixel bands** are copied from the scratch back into the
   retained layer — unchanged rows are untouched (this is what keeps history
   visible and avoids re-rendering the whole pane every frame).
4. The retained layer is blitted onto the scene; cursor + selection overlays are
   drawn on top.

A bug in step 1 (scratch larger than the pane → cropped copy) was the previous
regression. The scenarios below exercise the boundaries that regression touched.

## Scenarios to run

Start heca, open a shell pane, and walk through these. **Build first:**
`cargo run -p heca` (debug) or `cargo run -p heca --release`.

### A. Resize keeps glyphs crisp (the main previous regression)

1. Split into 2–3 panes (`prefix+Enter` / `prefix+v`).
2. Drag the column width (`prefix+=` / `prefix+-` or mouse resize) to shrink and
   grow a pane repeatedly — including making it very narrow then wide again.
3. **Pass:** text stays the same pixel density / crispness at every size. No
   "big letters," no smearing, no double-image ghosts. Content reflows to the new
   column count.
4. Repeat on a HiDPI display if available (or change `window_width/height` and
   scale factor) — the scratch must track the physical size, not logical.

### B. Typed text appears immediately (the second previous regression)

1. Focus a pane at a shell prompt.
2. Type a long line quickly (e.g. `echo hello world …` repeatedly, or hold a key).
3. **Pass:** every keystroke shows up instantly in the right cell. No "typed text
   hidden until next redraw," no lag, no missing glyphs mid-line.
4. Press Enter to run a command — output scrolls in and renders fully.

### C. Scrollback / output stays correct while idle

1. Run `for i in $(seq 1 200); do echo "line $i"; done` to fill scrollback.
2. Let it finish and leave the pane idle (no new output).
3. **Pass:** all visible rows are present and correct; nothing is blanked or
   flickering while the pane is idle (the retained layer holds content with no
   per-frame redraw).
4. Resize the pane once after it's idle — history reflows and stays visible.

### D. Multiple panes don't bleed

1. Have 2+ panes side by side with different content.
2. Resize one pane; type in one while the other is idle.
3. **Pass:** the idle pane's content is unchanged and not overwritten by the
   active pane's redraw. No pane draws into a neighbor's area after a resize.

### E. Style change repaints fully

1. Trigger a config reload (`prefix+Shift+r`) after changing the terminal font
   size or `terminal_transparency` in `config.toml`.
2. **Pass:** the whole pane repaints with the new font/alpha — no stale
   rows left at the old size/alpha (the render-key change forces `Full`).

## Sign-off

If A–E all pass, mark `terminal-task-00b` `[x]` in `BACKLOG.md` and delete this
file (or move the note into the handoff). If any fail, capture which scenario +
symptom so the retained-path geometry/copy logic can be revisited — do **not**
mark the task done on a partial pass.