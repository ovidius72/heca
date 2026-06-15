# Handoff: heca terminal refactor closeout

## Goal
Resume from the merged Phase 3 terminal-core baseline without re-discovering the work that already landed.

## Current Branch / PR
- Branch: `refactor/paneid-newtype`
- PR: https://github.com/ovidius72/heca/pull/100
- PR state: merged
- Merge timestamp: `2026-06-14T11:29:39Z`
- Latest branch checkpoint before merge closeout updates: `dbb9888` - `Polish terminal review fixes`

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

## Phase 3 Outcome
1. PR `#100` is merged.
2. Phase 3 terminal-core integration is complete.
3. Remaining terminal work is deferred to the explicit post-merge backlog.

## Explicitly Post-Merge Backlog
These are planned, but they should **not** be treated as part of the old Phase 3 merge work:
- Phase 9 — shared host selection capability
- Phase 10 — clipboard and paste semantics
- Phase 11 — bell / scrollback / hyperlinks / mouse-policy UX
- Phase 12 — richer image / graphics protocols
- Phase 13 — pane-shell integration with `heca-grid-ui`

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
4. Start from the post-merge backlog or Phase 4 work, not from terminal-core merge readiness.

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
- Do not reintroduce clipboard/selection/paste/backscroll/image protocols into the old Phase 3 merge path.
- Shared selection is now a host capability requirement, not a terminal-only feature.
- Keep terminal behavior app-agnostic; the current terminal host is meant to become content inside the future `heca-grid-ui` pane shell.
- The important question now is which post-merge terminal/platform task to start next, not terminal-core merge readiness.
