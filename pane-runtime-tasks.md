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
**Status:** Completed by Agent · **Assigned:** agent · **Depends-on:** Phase 0 · **Plan:** §3 + §4 Phase 1
**One-liner:** `ProcessStatus`/`GitInfo`/`ContentKind`/`PaneRuntime` in core + reactive per-pane mirror in
the store with event-emitting setters.
**Agent Completion:** Completed on current branch.
Built:
- canonical pane runtime types in `heca-core` and attached `PaneRuntime` to core `Pane`
- per-pane reactive runtime mirror in `SharedChromeState.workspaces` with guarded setters for program/status/cwd/git/kind
- runtime event coverage for `pane.process.changed`, `pane.status.changed`, `pane.cwd.changed`, and `pane.git.changed`
- session→store runtime projection in `sync_chrome_state`, including stale-pane pruning when panes disappear
Verification:
- `cargo test -p heca-core --quiet`
- `cargo test -p heca --quiet`
- `cargo clippy -p heca --all-targets --quiet` (existing warnings only in `selection_model.rs` and `terminal_render.rs`)
Notes:
- I extracted the runtime projection into a dedicated helper so Phase 1 has a direct acceptance test for canonical session state mirroring into the reactive store, rather than only store-local setter tests.
**Reviewer Decision:** Accepted · **Reviewer Notes:** Rust-skill review on the merged diff (PR #130 / `1bc51ee`). All findings F1–F7 fixed in the review-fix PR:
- F1 `#[allow(dead_code)]` on `with_pane_runtime` now has an explanatory comment (AGENTS rule 7).
- F2 reactive hazard fixed: setters now decide inside the `panes` borrow and `.set()`+emit **outside** it (signal handles are `Copy`, captured into outer locals); `PaneRuntimeSignals` now `Copy`.
- F3 `sync_pane_runtime_state` per-frame push documented as tech debt (TODO) for the Phase 0 reactive damage-path reconciliation.
- F4 `set_pane_*`/`with_pane_runtime`/`retain_panes` tightened to `pub(crate)`.
- F5 `set_pane_runtime` collapsed to a single `panes.update` (was six per-field borrows).
- F6 `PaneRuntimeSignals` fields + struct documented; `Copy` rationale explained.
- F7 `HashSet` imported instead of fully-qualified.
Verification: `cargo test -p heca-core -p heca` 204+40 green; `cargo clippy -p heca-core -p heca --all-targets --all-features` — 0 new lints (same 8 pre-existing).
Note: the 6 per-field setters carry a documented `#[allow(dead_code)]` — they are the Phase 2 process-monitor write API, exercised by tests today; not removable dead code.

## Phase 2 — Process detection (OS-native foreground + exit; event-first, NO polling timer)
**Status:** Completed by Agent · **Accepted** (merged) · **Assigned:** agent · **Depends-on:** Phase 1 · **Plan:** §4 Phase 2 (see also §0.2, §0.6)
**One-liner:** exit+code captured from `try_wait` (event, reader EOF wake) + `pane.exited{code}`; foreground
program + running/idle via `tcgetpgrp` vs `process_group_leader()` (macOS libproc / Linux /proc, basename);
cwd OS-fallback (**Linux `/proc/<pid>/cwd` ✓; macOS OS-cwd deferred → Phase 3 OSC 7**, fragile FFI); **event-driven on output/EOF wakes + 250 ms debounce — NO periodic poll timer** (deferred);
auto-close stays (only fires on shell death — §0.6); **remove `ProcessStatus::Exit`** (dead pane closes);
`PaneBackend::runtime()` + per-wake monitor → `Pane.runtime` (Phase 1) → store. FakeBackend tests.
**Agent Completion:** PR #133 (commit `e8ac856`) — merged 2026-06-18. Code: `feat/phase2-process-detection`; design/deferral docs: PR #132.
**Reviewer Decision:** Accepted by merge · **Reviewer Notes:** FakeBackend + TerminalBackend unit tests (idle/running, exit-captures-code, debounce, monitor); clippy 0 new warnings. macOS cwd OS-fallback deferred → Phase 3 OSC 7 (tracked in plan §4 + one-liner).

## Phase 3 — Shell integration (OSC 133 / OSC 7)
**Status:** Accepted · **Assigned:** agent · **Depends-on:** Phase 2 · **Plan:** §4 Phase 3
**One-liner:** passive OSC snooper in heca-core before `advance_bytes` (§0.8) — parse OSC 133 (success/error+code) + OSC 7 (cwd, macOS preferred); re-sample foreground on markers; **hybrid shell wrap** (bash `--init-file` / zsh `ZDOTDIR` / fish `-C source`) auto-enabled by `settings.shell_integration` (bool, default true). **Implementation-shape decisions locked in §0.8** after an agent flagged them as pre-coding blockers.
**Agent Completion:** Completed on current branch.
Built:
- passive OSC snooper in `heca-core` before `engine.advance_bytes()` with fragmentation handling and `BEL`/`ST` terminators
- OSC 133 routing for `PromptStart` / `CommandStart` / `PreExec` / `CommandFinished(code)` into pane runtime status/exit-code updates
- OSC 7 cwd parsing with percent-decoding and runtime cwd updates
- semantic-status preservation so `Success`/`Error` is not immediately clobbered by a shell-foreground idle refresh
- shell-integration asset materialization under `~/.config/heca/runtime/shell-integration/` and shell-specific PTY wrapping for bash/zsh/fish
- new `[settings].shell_integration` bool plumbed through config reload/startup and centralized terminal-backend spawn helpers
- review cleanup: replaced the long shell-integration constructor with `TerminalBackendOptions`, preallocated the hot-path OSC event vec, added public API field docs, and broadened shell wrapper detection for suffixed shell names
Verification:
- `cargo test -p heca-config --quiet`
- `cargo test -p heca-core --quiet`
- `cargo test -p heca --quiet`
- `cargo clippy -p heca-core --all-targets --quiet`
- `cargo clippy -p heca --all-targets --quiet` (existing warnings only in `selection_model.rs` and `terminal_render.rs`)
Notes:
- Added a deterministic end-to-end bash PTY test that verifies `false` → `Error`, `true` → `Success`, and `cd /tmp` → OSC 7 cwd update without depending on a user shell rc file.
**Reviewer Decision:** Accepted by merge (PR #136, commit `0879b3c`) · **Reviewer Notes:** Review follow-ups addressed on branch: constructor naming/API cleanup, config-threading reduction at spawn call sites, OSC vec preallocation, public-field docs, and broader shell-kind matching. **Lead §0.8 compliance verified post-merge:** passive snooper forwards all bytes unchanged to wezterm-term (`observe` immutable → `advance_bytes` full); BEL + ST (ESC \\) + C1-ST (0x9c) terminators + cross-read fragmentation; routes 133 D→Success/Error+code, 7→cwd (percent-decoded, closes the Phase 2 macOS cwd deferral), A/B/C→force foreground re-sample; `semantic_status_active` correctly defers to OS detection between commands. Shell wrap = bash `--init-file` / zsh `ZDOTDIR`+`OLD_ZDOTDIR` / fish `-C source`, snippets source user RC first. `settings.shell_integration` (bool, default true) gates `Some`/`None` + refreshed on reload. Event-first + 250 ms debounce, no periodic timer. 51 heca-core tests, clippy clean (0 new). **Process nit:** PR #136 head branch was misnamed `feat/phase1-pane-runtime-state` (carried Phase 3 code) — harmless but rename future branches to match their phase.

## Phase 4 — Git integration (git2 behind a trait)
**Status:** Completed by Agent · **Assigned:** agent · **Depends-on:** Phase 3 (or Phase 2 cwd) · **Plan:** §4 Phase 4
**One-liner:** `GitProvider` trait + `git2` impl (branch, ahead/behind, +a/-d/Δ), debounced on cwd-change +
optional `.git` fs-watch; cache per repo.
**Agent Completion:** Completed on current branch.
Built:
- added `app/git_monitor.rs`: a provider-trait seam plus `git2` implementation that resolves repo root, branch, ahead/behind, and dirty counts (`added` / `modified` / `deleted` / `dirty`)
- added a host-owned `GitRuntimeCache` on `AppState`, keyed by pane cwd + shared repo root, so panes in the same repo reuse one cached snapshot
- wired a debounced git sync into `sync_chrome_state`: canonical `Pane.runtime.cwd` now drives canonical `Pane.runtime.git` before the chrome-store mirror runs, so existing `set_pane_runtime` change-guards emit `pane.git.changed` on real change only
- implemented non-repo behavior as `git = None`, with no per-frame churn for unchanged non-repo cwd values
- added tests for runtime projection, shared repo-root cache reuse across panes, timed refresh after the debounce window, and a temp-repo `git2` integration check for branch + dirty counts
**Reviewer Decision:** — · **Reviewer Notes:** Pending review / merge.

## Phase 5 — Process catalog (`[programs.<raw>]` → {name, icon, description, color})
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 1 · **Plan:** §0.7 + §4 Phase 5
**One-liner:** new `heca-config/src/programs.rs` (`ProgramMeta`/`ProgramsConfig`/`ProgramView`); seeded shell
defaults + user `[programs.<raw>]` overrides; free-form glyph-string icons (no new `Glyph` enum); optional
`color` pane tint; resolver with raw always available.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 6 — Command spawn (run real programs + kind + float + close-policy)
**Status:** Completed by Agent · **Assigned:** agent · **Depends-on:** Phase 2 · **Plan:** §0.4 + §4 Phase 6
**One-liner:** real `CommandBuilder` spawn; extend `[[keys.command]]` (`kind`/`float`/`close_pane`/
`keep_on_error`/`keep_on_success`); close-policy on `pane.exited`; RPC parity; full action checklist.
**Agent Completion:** Completed on current branch.
Built:
- extended `[[keys.command]]` parsing with `kind`, `float`, `close_pane`, `keep_on_error`, and `keep_on_success` (default `kind = "terminal"`, plus `keys` alias support)
- extended `WmAction::SpawnCommand` payloads and config/RPC mapping to carry spawn kind, float mode, and pane close-policy
- added a real PTY command-spawn path in `TerminalBackend`/`PtyHandle`, using the user's shell as the command trampoline (`-ic` on Unix, `/C` on Windows) while keeping shell integration disabled for direct command spawns
- implemented tiled and floating command-pane creation in `handle_spawn_command`, including pane-owned close-policy storage
- applied close-policy exactly once from the drained exit-event path, while preserving shell-pane auto-close behavior
- documented the current `[[keys.command]]` contract in `README.md` and `keybindings.toml`
- review cleanup: documented `PaneClosePolicy` public fields, added config-load validation for invalid `[[keys.command]].kind`, clarified RPC `spawn-command` separator/empty-command errors, documented why shell integration stays off for direct command panes, and fixed the small process-monitor signature formatting artifact
Verification:
- `cargo test -p heca-core -p heca-config -p heca --quiet`
- `cargo clippy -p heca-core -p heca-config -p heca --all-targets --quiet` (existing warnings only in `selection_model.rs` and `terminal_render.rs`)
Notes:
- `kind = "terminal"` is implemented today; `app` and `plugin` currently report a clear "not yet implemented" message without forking a second spawn path.
**Reviewer Decision:** — · **Reviewer Notes:** Review follow-ups addressed on branch: interactive command shell mode (`-ic`), clearer RPC separator errors, documented public close-policy fields, config validation for invalid command kinds, explicit direct-command shell-integration rationale, and minor formatting cleanup.

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
- 2026-06-18 — **Phase 2 implemented + merged**: code PR #133 (`e8ac856`), design/deferral docs PR #132. Accepted by merge. macOS cwd OS-fallback deferred → Phase 3 OSC 7 (tracked).
- 2026-06-18 — **Phase 3 implementation-shape locked (§0.8)**: shell-hook mechanism (hybrid bash/zsh/fish wrap), config switch (`settings.shell_integration` bool), OSC parsing ownership (passive pre-parse snooper before `advance_bytes`). **Lesson:** an agent raised these as pre-coding blockers — they were implementation-shape decisions the lead left open after locking only the high-level Phase 3 design. The lead should lock implementation-shape up-front, not just design intent. Gaps now recorded so no agent has to guess.
- 2026-06-18 — **Phase 3 implemented + merged**: PR #136 (`0879b3c` — OSC snooper `osc.rs`, shell assets, `settings.shell_integration`, routing). Lead verified §0.8 compliance post-merge (no drift); Reviewer Decision = Accepted. Plan §4 Phase 3 tasks ticked.
