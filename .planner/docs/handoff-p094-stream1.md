# Stream 1 handoff (T449, P094/F011) — written when context hit ~65%

Orchestrator: session `master` (uds:/tmp/cc-socks/80269.sock). Rules: AGENTS.md in full; plugin-author builder APIs first; no literals; fix at the owner; break-check every test; no `cargo fmt` (hand-apply, use `./scripts/lint-changed.sh fmt|clippy|test origin/main`); no commits unless master says; no .planner edits (planner tools still point at the OLD folder until /mcp reconnect).

## Branch stack in ~/projects/heca (each on top of the previous; none pushed)
1. refactor/terminal-host-audit  — (A) 3f7d82dd refactor (one grid rule, wheel.rs, link hints ask the terminal, TerminalSpec::in_workspace, scene_clipped_to). Approved.
2. feat/t449-outline-band        — 0e512104 Scene outline band + Pane frame over children. Approved.
3. feat/t449-scene-order-flush   — 31c13226 HECA_FRAME_TIMES; 4028a795 terminals drawn in scene order (Scene::base_runs, scene_flush.rs); 9045d9e1 render_frame split (app/frame/{mod,panes,draw}.rs). Approved.
4. feat/t449-terminal-named      — (cherry-picked A as 7f15b5fd) + 50a0d991 5a: terminals no pane owns (`demo.terminal("shell")`), store owners (TerminalOwner::Pane/Named, id before process), client maps keyed by TerminalId, docked terminals drawn by the same flush, resize/wheel/scroll by id. Approved + committed.
   + 520425f3 5b step 1 COMMITTED: mouse reporting by id — HeardPresses keyed by TerminalId; on_press/on_release/on_move take (terminal, pane: Option<PaneId>) in heca/src/app/terminal_host/input.rs. fmt/clippy clean, 1638 tests pass; break-checked (the release-only-where-pressed test goes red when the terminal key is ignored). Working tree is clean apart from .planning/STATE.md (leave it) and untracked .planner docs (master moves them).
Other worktree: ~/projects/heca-fix, branch fix/unfloat-and-header-gap, head 473b27e8 (un-float FloatOrigin fix + ButtonGroup gap Xs + fmt commit), approved by master, not pushed.
Binaries for Antonio's timing/eye checks (do not delete): /private/tmp/claude-501/-Users-antonio-projects-heca/8f66b02a-e225-4b32-8cb4-51b87e1d5a3b/scratchpad/heca-before (0e512104 + timer) and heca-after (9045d9e1). Rebuild `heca-after` only if master asks.
PR order master wants: (A) -> 1+2 -> 3+4 (+ fix branch) -> 5a -> 5b -> 5c; stream2 (P104 step 2b) waits for (A) and 5a to merge. Tell master when 5a is merged. Before each PR: fetch, rebase on origin/main, re-run tests+clippy.

## 5b plan (approved by master, with changes)
- DONE and committed (520425f3): mouse by id.
- NEXT: keyboard for a terminal in a dock, the browser's way. NO `.focus_on_press()` builder. Rule in heca-grid-ui's own dispatch: a press gives keyboard focus to the DEEPEST FOCUSABLE widget under it by default; a press elsewhere moves focus; a press on nothing focusable leaves focus where it is (browser rule — CHECK that). First find how Input and other focusables get focus on a click today (library FocusManager::focus_at exists in heca-grid-ui/src/focus.rs; the app has its own paths: chrome focus is a container id in chrome_state.focused_container; layers use keymap.deliver_press). If some widgets do it by hand, fold them into the one rule and delete the copies. Terminal just declares `Component::focusable` (base.focusable = true). Tests + docs in heca-grid-ui: press on a focusable child focuses it; press elsewhere moves focus; press on nothing focusable keeps it.
- Host: the dock branch of key handling (heca/src/app/input.rs, "Focus is the mode": dock swallows unclaimed keys; heca/src/app/events.rs layer branch uses `keymap.deliver_press` into window_root) must deliver unclaimed keys into the tree like the layer branch, swallowing only what the tree did not take. Prefix and prefix sequences stay the host's while a terminal has focus; Escape goes to the program.
- Terminal widget (heca/src/chrome/terminal/component.rs): on_key_down / text handlers -> new TerminalInput::Key / Text variants (heca/src/chrome/terminal/input.rs) -> host feeds the backend BY ID (backend.process_key_event with the BackendKeyEvent encoder + process_input; scroll_to_bottom on typing; see app/input.rs lines ~170-190 for the pane version using winit_key_to_backend_event / winit_key_to_terminal_input — need a GridKey->BackendKeyEvent encoder).
- Title rename: SAME action and dialog as renaming a pane (grep RenameTarget / RenameTarget in heca/src/handlers/rename.rs and the rename dialog). SHOW master the design BEFORE building. Title lives on Terminal::shown_title() (name = identity, fixed).
- Docs: at `Terminal::kill()` (rustdoc + docs/widgets.md Terminal entry) say once: an ended named terminal stays ended; its dock keeps showing the ended terminal; the declared table (heca/src/chrome/terminal/declared.rs) keeps the entry.
## 5c (after 5b, inside T449): SelectionOwner for any terminal (today SelectionOwner::Pane), search, link hints for terminals no pane owns; remove the pane-only gates in terminal_host/input.rs (on_press shift-select) and hyperlinks.rs (collect_link_hints walks laid_out_pane_ids).
## Known loose ends / notes for master
- AGENTS.md § 0d example now uses demo.terminal("shell") (done in 50a0d991).
- stream2 may have used BackendStore::terminal_of/terminal_named (removed; use identity_of/get/get_by_id).
- Window capture of the GPU window failed from here; visual checks are Antonio's.
- 5a eye check steps were sent to master (extend_heca: right sidebar, Notes dock with demo.shell).

## Orchestrator note (2026-10-02)
- Reviewed 520425f3: OK. One follow-up for the focus step: a terminal no pane owns currently hears pointer moves whenever the pointer is over it; once the library focus rule lands, make it "while it holds focus", the same rule as a pane.
- Every brief and review: plugin authors get DOM-like, builder-style APIs; no ids/registries/host-private types; AGENTS.md in full; review also against the rust-best-practices checklist.
