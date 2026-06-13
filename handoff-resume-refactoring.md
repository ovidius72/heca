# Handoff: heca terminal refactor closeout

## Goal
Resume the current terminal refactor branch and get it merge-ready against `main` without re-discovering the work that already landed.

## Current Branch / PR
- Branch: `refactor/paneid-newtype`
- PR: https://github.com/ovidius72/heca/pull/99
- PR state: ready for review
- Latest committed checkpoint: `d12bb45` - `Harden terminal renderer and sizing path`

## Current Status
- Real PTY-backed terminal panes are live by default.
- The renderer sync onto `main`'s atlas/retained `TextRenderer` is already reconciled.
- Terminal text is visible again after the post-sync `set_target_size(...)` wiring.
- Shell autosuggestion cursor placement is corrected.
- Yazi now renders correctly, including the previously broken right-edge symbol cases.
- `nvim` now renders correctly, including the previously broken powerline/status separator cases.
- Underline and undercurl rendering are GUI-native and live-tested.
- The renderer/core review blockers are closed.

## What Has Landed
- `portable-pty + wezterm-term + cosmic-text` is the chosen terminal stack.
- `TerminalBackend` owns the PTY + engine wrappers.
- `terminal_snapshot()` is the live app path for terminal panes.
- Terminal mounting is routed through `heca/src/app/terminal_host.rs`.
- Terminal mouse forwarding and focus notifications are wired through the host adapter.
- Terminal font settings are separate from the UI font/theme settings.
- Measured terminal cell sizing now comes from the loaded terminal font path instead of theme heuristics alone.
- Shared terminal symbol handling exists for:
  - box drawing
  - powerline separators
  - underline / undercurl / dashed / dotted / double underline
- The review-driven hardening pass has been applied:
  - `u32` primitive indices
  - safe grid fitting
  - deterministic spawn sizing
  - deterministic PTY test shell
  - underline style preservation
  - wide-cell filler fixes
  - cursor overlay ordering

## What Still Needs To Happen Before Merge
1. Re-run the final live validation pass across a couple more terminal fonts and a couple more `nvim` colorschemes.
2. Run the phase-end Rust review for the current phase.
3. Fix anything that review finds.
4. Re-run review until clean.
5. Refresh PR notes/checklist if anything substantive changes.

## Explicitly Post-Merge Backlog
These are planned, but they should **not** block merging the current terminal-core PR:
- terminal selection and clipboard
- bracketed paste and OSC 52
- scrollback search
- hyperlink / open-link behavior
- bell handling
- richer image / graphics protocols
- Phase 8 pane-shell integration with `heca-grid-ui`

## Known Risks / Open Follow-ups
- Glyph shaping still relies on shared `TextRenderer` internals underneath the dedicated terminal renderer.
- Yazi image preview still spins because richer graphics/image protocol support is not implemented yet.
- Terminal palette/theme fidelity still benefits from a few more live checks across additional themes and fonts.
- Mouse-aware TUIs have a structured forwarding path, but the final verification pass should still include `nvim` mouse mode and wheel behavior.
- Terminal font family naming must match the embedded font metadata (`Maple Mono Normal NF`).

## Resume Order
1. Read `terminal-implementation.md`.
2. Read `.planning/STATE.md`.
3. Check `git status` and confirm the branch is still clean apart from any intentional new work.
4. If no code changes are needed, run the final live validation pass.
5. Run the phase-end Rust review only after the full phase is complete.

## Key Files
- `terminal-implementation.md`
- `.planning/STATE.md`
- `heca/src/app/terminal_host.rs`
- `heca/src/app/backend_factory.rs`
- `heca/src/app/render.rs`
- `heca/src/app/events.rs`
- `heca-renderer/src/terminal.rs`
- `heca-renderer/src/text.rs`
- `heca-core/src/backend/terminal/engine.rs`
- `heca-core/src/backend/mod.rs`

## Suggested Skills
- `gsd-resume-work` - restore the current phase context cleanly
- `gsd-progress` - verify the next concrete checkpoint
- `gsd-code-review` - run the phase-end review loop when the phase is done
- `rust-skills` - apply Rust review guidance to the remaining code path

## Notes for the Next Agent
- Do not restart the renderer/terminal architecture discussion from scratch; the important architectural boundary is already decided.
- Do not move clipboard/selection/paste/backscroll/image protocols into the merge path for this PR.
- Keep terminal behavior app-agnostic; the current terminal host is meant to become content inside the future `heca-grid-ui` pane shell.
- The important question now is merge readiness, not more architectural rewrites.
