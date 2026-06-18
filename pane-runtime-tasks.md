# Pane Runtime State — Orchestration Board

> Coordination board for the **pane-runtime-state** initiative. **Full context lives in
> `pane-runtime-state-plan.md`** — read the referenced phase section before starting; this board is only
> assignment + status + responses. (Separate from `shared-tasks.md`, which tracks unrelated work — do not
> mix them.)

## Roles
- **Orchestrator** (me): assigns phases, reviews completions, accepts/rejects, advances the order.
- **Agent**: claims one *Open* phase, implements per the plan's task checklist, verifies, reports under
  *Agent Completion*. Updates **only** the phase it owns.

## Rules
1. Only active / not-yet-accepted phases stay here; accepted ones are struck or removed.
2. One agent per phase at a time. Set status to `In Progress` and put your id in *Assigned*.
3. Honor **Depends-on** — do not start a phase whose dependency isn't `Accepted`.
4. Obey the plan's **§2 cross-cutting rules** (grid-ui widgets; registry/keymap/RPC; event-on-mutation;
   `heca-core` UI-free; showcase+docs; tests; no `cargo fmt`; clippy clean).
5. Report with **code + verification + architecture notes** — completion notes alone are not acceptance.
6. Branch per phase off latest `origin/main`; open a PR; never merge without explicit OK.

## Status values
`Open` · `In Progress` · `Completed by Agent` · `Needs Edit` · `Accepted` · `Rejected`

---

## Phase 0 — Chrome event bus + finish SharedChromeState migration  ⟶ FOUNDATION
**Status:** Completed by Agent · **Assigned:** agent (dispatched 2026-06-18) · **Depends-on:** none · **Plan:** §4 Phase 0
**One-liner:** typed event bus + emit-on-mutation; consume the dead store fields; retire the `Rc<Cell>`
click mailbox; feed signal changes into the damage/repaint path.
**Agent Completion:** Completed on current branch.
Built:
- typed chrome event bus + event-emitting store setters
- expanded-sidebar interaction routing via `ChromeIntent` app events instead of the `Rc<Cell>` mailbox
- `chrome_state` mirroring for pick candidates + hovered pane, with a real retained-tree hover consumer
- retained chrome repaint invalidation narrowed from root-wide dirtying to per-widget repaint requests
- actual chrome damage forwarding into `GridRenderer`/`TextRenderer`
Verification:
- `cargo test -p heca --quiet`
- `cargo clippy -p heca --all-targets --quiet`
Notes:
- I intentionally extended Phase 0 slightly beyond the minimum wording to finish repaint granularity in the same slice; leaving damage collection wired but unused would have made the new invalidation path misleading and much less valuable.
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 1 — Pane runtime state model
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 0 · **Plan:** §3 + §4 Phase 1
**One-liner:** `ProcessStatus`/`GitInfo`/`ContentKind`/`PaneRuntime` in core + reactive per-pane mirror in
the store with event-emitting setters.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 2 — Process detection (OS-native foreground + exit; event-first)
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 1 · **Plan:** §4 Phase 2
**One-liner:** exit+code (event), foreground program + running/idle (OS shim: macOS libproc / Linux /proc),
cwd OS-fallback, process-monitor service; poll foreground only as fallback.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 3 — Shell integration (OSC 133 / OSC 7)
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 2 · **Plan:** §4 Phase 3
**One-liner:** parse OSC 133 (success/error+code) + OSC 7 (cwd); re-sample foreground on markers; ship a
bash/zsh/fish hook auto-enabled via PTY env.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 4 — Git integration (git2 behind a trait)
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 3 (or Phase 2 cwd) · **Plan:** §4 Phase 4
**One-liner:** `GitProvider` trait + `git2` impl (branch, ahead/behind, +a/-d/Δ), debounced on cwd-change +
optional `.git` fs-watch; cache per repo.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 5 — Process catalog (extensible raw→{name, icon})
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 1 · **Plan:** §4 Phase 5
**One-liner:** built-in defaults + config-extensible map; new `Glyph`s (+ showcase/docs); resolver with raw
always available.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 6 — Command spawn (run real programs + kind + float + close-policy)
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 2 · **Plan:** §0.4 + §4 Phase 6
**One-liner:** real `CommandBuilder` spawn; extend `[[keys.command]]` (`kind`/`float`/`close_pane`/
`keep_on_error`/`keep_on_success`); close-policy on `pane.exited`; RPC parity; full action checklist.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 7 — Display: fixed default pane-info widgets
**Status:** Open · **Assigned:** — · **Depends-on:** Phases 1–6 (degrades gracefully) · **Plan:** §0.3 + §4 Phase 7
**One-liner:** sidebar card + optional pane-corner badge (icon · name · `(raw)` · status badge · git badges)
as theme-driven grid-ui widgets, reactive; showcase + docs.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 8 — Plugin event exposure + plan/docs updates
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 0 · **Plan:** §4 Phase 8
**One-liner:** first-party `app.on` + `app.state` selectors over the bus/store; document state access + event
model + the deferred token customization in `pluggable-chrome-plugin-plan.md`; point `PLAN.md` here.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

---

## Suggested order
`0 → 1 → 2 → 3 → 6 → 4 → 5 → 7 → 8` (single-threaded). Parallel once Phase 1 is Accepted: **2 & 5** together,
then 3→4 and 6 alongside.

## Activity log
- 2026-06-18 — board created; plan locked (`pane-runtime-state-plan.md`); all phases `Open`.
- 2026-06-18 — docs landed (PR #127); **Phase 0 dispatched** to an agent (foundation).
