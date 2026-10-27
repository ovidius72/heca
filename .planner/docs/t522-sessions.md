# T522: Sessions

Antonio resumed P100(F012)/T522 on 2026-10-27 after T544 landed in
main through PR #300. T522 is in progress. No implementation is complete yet.

## Location And Scope

- Code: `/Users/antonio/projects/heca-p100`, branch `feat/p100-sessions`.
- Base: current main, `1c74bdf3`.
- Canonical planner: `/Users/antonio/projects/heca/.planner`.
- Approved order: P100/T522 first, then P105/T527. Do not duplicate P099's
  multiwindow/follow task T545 or event stream task T546.
- No commit or PR without Antonio's request. Keep main's unrelated planner,
  graphify and Codex icon changes, and other agents' worktrees, untouched.

### Worktree Rule And Current Ownership

Antonio's 2026-10-27 rule is recorded in canonical Project Guidelines: each new
coding task gets a new worktree from current synchronized main, in a folder
named `heca-feat-[num]`; finish, open a PR, verify merge, then the owning agent
removes its worktree/folder and merged branch. The current worktrees remain as
they are. T522 stays in `heca-p100`. Opus owns T545 in `heca-t545`;
`heca-ui-authoring` belongs to another Codex instance. Do not change their work.

The separate Codex icon correction was merged as PR #302; its worktree and
branch were removed. It is no longer an outstanding local code change.

## Preflight

### Existing Pieces

- Reuse `server/run.rs::ServerLoop`, `ClientMsg`/`ServerMsg` and the existing
  socket and in-process links. `ServerState` owns backends, notification runtime
  and process settings. There must not be a second transport or process owner.
- Reuse `Terminals::start` and `TerminalSpec.cwd` for the session's project
  folder. Do not add another shell-launch path.
- Reuse `CommandPalette`, `Command`, `modes::MODES`, `populate`, `SearchModel`
  and the existing search store. The widget owns filtering, history and input.
- Reuse `ActionCatalog`, `ActionRegistry`, `WmAction` and the existing policy
  and confirmation path. Metadata supplies action text, icons and shortcuts.
- Read `docs/widgets.md`'s CommandPalette entry, `heca/src/components/mod.rs`
  and `chrome/surfaces/palette/{mod,model,modes,targets}.rs`. No new generic
  widget or custom row drawing is needed.

### Existing Ownership

P100/T526 is done and owns the single palette mode table. T522 owns sessions;
P105/T527 owns JSON persistence. No new planner task is required for this scope.

## Current Code

- `server/run.rs` is 650 lines including tests. It owns exactly one server state
  and layout session. All attached clients receive that session's data.
- `server/bootstrap.rs` creates that initial session and shell.
  `server/runtime` and `client/local` drive the same owner loop.
- `client/mod.rs` always sends an untargeted `Attach`. `app_state/client_updates.rs`
  rebuilds content with the old session ID. Session switching needs explicit
  identity and removal of the old client replicas and pending requests.
- `app/cli/{mod,catalog}.rs` is one declaration table for validation, help and
  launch. Extend that table, not a separate flag parser.
- Palette modes currently are commands (:), panes (@) and workspaces ($).
  Declare # in `MODES` and preserve the existing search scopes.

Existing violation reported to Antonio: `server/bootstrap.rs` hardcodes the
notification history capacity to 50. It is not added to T522's scope.

## Execution Order

1. Extend server ownership to several named sessions with folder and optional
   metadata. Keep terminal, notification and image data isolated even when
   different sessions have equal pane IDs.
2. Extend the existing protocol with target-aware attach, switch and list.
   Reset old client data on a switch; leave other windows on their own sessions.
3. Register create, rename, terminate, restart, delete, detach and switch through
   the existing action catalog and policy path, usable by palette, keys, plugins
   and RPC. Capture original launch data and native harness session identity.
4. Add # rows using the existing palette and search memory. Add --sessions
   [--json], --attach, --new, --kill and last-used selection through the existing
   CLI table. Standalone still runs the same server in-process.
5. Verify duplicate names, unknown attach, cross-session isolation, active/last
   session closure, terminal continuity, real socket CLI and native same-window
   palette switching.
6. Update docs and check only acceptance items backed by evidence. Disk restore
   belongs to P105; T522 C5 was clarified so P100 does not depend back on T527.

## Accepted Server-Loss Behavior

F012 decision `c205c5f5-cd0a-4212-9fda-8a8c63c98d3d` records Antonio's
approval: keep disconnected windows open with a blocking Dialog offering Close
or Restart and reconnect. The GUI connects to an existing server or starts one
in the background. No terminal command is required. Already-open windows must
not each undo an intentional server stop automatically. Normal OS launch still
starts the server when needed.

T522 owns this client flow; P105/T527 owns restoring saved layouts and applying
the configurable program/harness recovery policy below. Neither means freezing
or resuming a killed process's memory. Attach from the GUI uses the agreed #
palette list. A retained terminated session is not the same as a live detached
session or a permanently deleted one.

## Accepted Individual-Session Termination Behavior

Antonio confirmed that terminating one session leaves its windows open in the
same blocked condition as server loss. Show a message explaining that the
session is unavailable, with [Restart] [Exit]. Other sessions and their windows
remain unaffected. Do not automatically close the affected windows, switch them
to another session or restart the terminated session. The same F012 decision
above owns this policy.

## Accepted Termination And Deletion Semantics

Terminate stops processes but retains the session definition and stopped state.
It remains stopped across server restarts until explicit Restart. Delete stops
it if active, removes its definition and all Heca-owned session references, and
closes the clients that were using it. Project files are never deleted.
--kill <name> is the planned terminate operation, not permanent deletion; the
permanent-delete CLI spelling is still unspecified. These are planner contracts,
not implemented behavior.

## Accepted Recovery Configuration

F012 decision `2fc26220-2583-4392-bc2f-4724d406a555` records the approved policy:

```toml
resume_programs = true
resume_harness = true
```

The independent flags govern ordinary program relaunch and registered harness
resume. Both defaults were explicitly chosen by Antonio. Save original launch
executable, complete structured arguments and working directory in session JSON.
For a harness also save its registered driver identity and native session ID,
which is not Heca's session ID or a PID. Its driver constructs the proper CLI
resume command for that harness; there is no universal --sessionId switch.
Do not replay arbitrary shell history or silently start a new harness
conversation when native resume fails.

T522 captures live launch/lifecycle data and client effects. T527 persists them,
applies the settings and restores eligible sessions. Terminated sessions require
explicit Restart regardless of flag values; deleted sessions cannot return.

## Accepted Fresh Defaults When Resume Is Disabled

Keep the pane and layout, then initialize its content with that content's normal
fresh defaults. A terminal-hosted program or harness becomes the user's normal
shell. A browser opens its configured default home page. An editor opens with
no files. Future content types use the same contract through their own default
constructor/factory. There is no inactive placeholder, fixed content-kind list,
universal Terminal substitute or new recovery-specific default.

T527 consumes the managed-content creation/lifecycle/restore ownership in
P094(F011)/T452, based on P067(F003)/T398's open factory contract. It does not
create a second registry or implement Browser/Editor here. A registered custom
nonterminal fixture must prove that a future content type needs no recovery
call-site special case. A false resume flag deliberately creates fresh content;
an enabled resume that fails is still an observable failure, not a successful
resume or an excuse to silently create a fresh harness conversation.

## Remaining CLI Question

Permanent-delete CLI spelling remains unspecified.

## Baseline Verification

Unchanged source at `1c74bdf3`:

- `cargo test -p heca --lib server::run::tests`: 8 passed.
- `cargo test -p heca --lib server::runtime::tests`: 4 passed.
- `cargo test -p heca --test headless_server`: 1 passed, with a real binary,
  socket and PTY retaining its process and history after detach/reattach.

These are baseline checks, not proof of T522 implementation or native UI
acceptance. The implementation worktree is still source-clean.
