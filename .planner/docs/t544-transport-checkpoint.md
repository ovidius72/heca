# T544 Transport Checkpoint

## Toast countdown bar — library prop (2026-10-26)

Antonio asked for a progress bar on timed toasts (not sticky), at library level, enabled by a prop
`.progress(true)` (default false). Built in `heca-grid-ui`:

- `Toast::progress(bool)` (default off) + `Toast::countdown(Duration)` (the span it depletes over) +
  `Toast::countdown_paused(Signal<bool>)` (freezes the bar while the stack is held still under the
  pointer — the same hover the server freezes the deadline on). The bar is painted as card chrome
  (like the bracket frame), toned by severity, inset by the corner radius; it animates locally in
  `tick`. A sticky toast has no span, so no bar.
- `ToastSpec::countdown(Duration)` carries the span; `ToastStack` turns `progress(true)` on for any
  card that has one and wires the hover signal, so the app never sets it per card.
- App side: `NotificationView.auto_dismiss: Option<Duration>` (from the lifecycle's `Auto` lifetime,
  `None` for sticky); `toast_spec` sets the spec's countdown. So every timed toast shows a draining
  bar, sticky ones don't — exactly the ask.
- Note: the bar runs over the full lifetime from when the card appears; on a reattach mid-life it
  restarts full (the server still expires it at the right time). Exact-remaining-on-reattach would
  need the view to carry remaining; deferred as cosmetic.
- Tests: `toast::countdown_tests` (full→deplete→empty; sticky shows none; hold-still freezes and
  resumes). Documented in `docs/widgets.md` (Toast). grid-ui 482 lib tests + all targets green.

## Reload-after-fix report (2026-10-26)

Antonio: with a broken config the app loads; after fixing it, reload "doesn't work". Verified: the
notification feedback is sound — `update_duplicate` fully replaces a dedup-matched card, so the
success toast replaces the sticky failure on Retry or `prefix+Shift+r`. The window-side reload
(theme, keymaps, fonts, layout) is unchanged by C8. The C8 change only replaced a no-op (config to
a dead in-process server) with the real server reloading from disk. Prime suspect for a value not
applying: terminal colors (known wezterm palette-fork reload bug, tracked as `terminal-theming-1`).
Asked Antonio which setting didn't apply to localize window vs server vs the known terminal bug.

## C8 — notification server/client ownership + projection (code done, 2026-10-26)

Done in `heca-p099-current` on `feat/p099-headless`, after fast-forwarding the branch onto
origin/main (PR #298, the T512 split). Scoped build/clippy/tests all green (heca 1074, heca-core
255, heca-config 124, incl. the headless detach/reattach integration test). No commit/PR authorized.

What changed: the window was talking to a **dead in-process `AppState.server`** for notifications
(raise/dismiss/hover/config) while everything else went to the real server over the wire — so
notifications were per-window-private and never shared, never survived detach. C8 moves them onto
the wire and removes `AppState.server` entirely.

- **Serializable projection, not `ToastSpec`.** New `NotificationView` (notification/presentation):
  `AppNotification → NotificationView` server-side (`notification_view`), `NotificationView →
  ToastSpec` window-side (`toast_spec`). One projection split at the serialization boundary. The
  store/runtime now hold/return `NotificationView`; `ServerState::visible_notifications()` replaces
  `visible_toasts()`. The domain was already serde-ready (`NotificationAction` carries its `Intent`
  + a ButtonVariant name shim; `NotificationSeverity` serde). Only `NotificationDraft` needed the
  derive added.
- **Wire.** `ClientMsg::Notify(NotificationCommand)` (Raise/DismissOne/DismissLast/DismissAll/Hover),
  `ClientMsg::Reload`, `ServerMsg::Notifications(Vec<NotificationView>)` (coalesced in the queue).
  The server sends the projection on attach and whenever `NotificationsChanged` is produced, via a
  dedicated `publish_notifications` path — NOT the generic `broadcast`, so a notification no longer
  drags the whole `SessionContent` across on every raise/tick.
- **Config reload (Antonio's decision 2026-10-26: server reloads from disk).** `prefix+Shift+r` on
  the window reloads only its own view-side config and sends `ClientMsg::Reload`; the server
  re-reads `AppConfig::load()` and re-applies its own terminal + notification settings
  (`ServerLoop::reload_config`). The window never hands a `Theme` over the wire. This also fixes
  terminal-config propagation to the real server (previously applied to the dead in-process one).
  Matches tmux (parsing/application on the server); Heca is simpler — the server reads the file
  itself behind the owner-only socket, no file-contents transfer.
- **`AppState.server` removed.** raise→`client.notify(Raise)`; dismiss handlers→`client.notify`;
  the action relay resolves the pressed key→`Intent` from the received `notifications` views (the
  `ToastSpec` keeps no intent); hover sent once per change in `lifecycle`; the server ticks itself
  (no window-side tick); `next_wake` is the client's.
- **Precedence preserved** (explicit duration → config → fallback, `.sticky()` no expiry): untouched
  in the model; a new serde round-trip test proves a view keeps its `Intent` + `dismiss_after`.

Remaining for C8/C9: **native eye-test** — raise a toast, watch it auto-dismiss, and confirm a
toast present at detach is still there after reattach. Antonio drives the GUI.

## Native Acceptance — PASSED (2026-10-26)

Antonio ran the real GUI in `heca-p099-current` and the detach/reattach acceptance passed:

- Plain `heca` auto-spawned the background server and attached a window.
- `Meta+Q` detached the window; the server kept the shell and a running command alive.
- Running `heca` again reattached to the same pane — still running, scrollback intact.
- `heca --standalone` opened a fresh, isolated session (no attach), as designed.
- The server stopped cleanly with `pkill -f "heca --server"`.

This closes the task's "done when" (close the window mid-command, reattach, still running).
Documented in `README.md` (Sessions — detach and reattach) and
`docs/server-client-architecture.md` (The connection). Remaining T544 code: C8 notification
server/client, still blocked on Codex's reviewed T512. No commit or PR authorized.

## Current Work

Canonical planner: `/Users/antonio/projects/heca/.planner`.
Code: `/Users/antonio/projects/heca-p099-current`, branch `feat/p099-headless`.
Task: P099(F012)/T544.

## Approved Notification Order

Antonio approved P099 decision `6a40db9e-0dc6-41bc-a984-0981ebfc154c` on
2026-10-25. T544 checklist C7-C9 and T512 C1 track this order:

1. Export the existing fluent `Notification` API for external native extensions
   and prove access with an integration test. Do not replace it with caller-written
   `notify` intents, `PropValue`, a clock, or a transport handle.
2. Verify the old deferral reasons and concurrent ownership, then use existing
   P082(F003)/T512 to split the notification module without changing behavior.
   Keep T544 as the lifecycle return target. No duplicate task.
3. Return to T544 for notification server ownership/transport, then finish
   config reload, terminal damage/resource checks and native acceptance.

The phase handoff was written, read back and verified resume-ready. Read it with
`planner-handoff-show P099(F012)` through the official planner rooted in heca.
Nothing needed for recovery lives in `/tmp`. The MCP tools attached to the
heca-pro conversation target heca-pro; do not mutate that planner for this work.

The current worktree was created directly from origin/main at `6e3e6d36`, after
P102 was merged and pushed. P099 source and transport changes were transferred
without planner files or generated graphs. CLI integration uses P102's new
`app/cli/mod.rs`, not the removed `app/cli.rs`.
The old `/Users/antonio/projects/heca-p099` worktree remains a recovery copy;
do not resume or merge its unfinished pre-P102 merge. Its transfer snapshot tree
is `779a78235f47eb291afaccdab9db44698718e066` and the backup patch is in
`target/t544-transfer-4k9ieN/code.patch` there. No T544 commit or PR exists.

## Current Verification (2026-10-25)

### Notification API And T512 Move

The official planner switch made P082(F003)/T512 the temporary active task,
with T544 preserved as its return target. T544 C7 is checked: `lib.rs` exports
`Notification` and `NotificationAction`; both external API integration tests
pass after first failing to compile before the export.

The 3431-line notification file is now `heca/src/notification/`, separated into
model, store, runtime, presentation, action and sink. The largest file has 382
lines. All 73 original notification test names are retained and all pass.
The full Heca all-target suite and scoped clippy with `-D warnings` pass.
Logs in `/Users/antonio/projects/heca-p099/target/`:
`t512-move-tests.log`, `t512-all-tests.log`, `t512-clippy.log`, `t512-doc.log`.

T512 is not complete: its phase requires rule review after the move. Antonio
was asked to confirm using config-owned defaults and respecting explicitly
supplied lifetimes before behavioral corrections. Existing code hardcodes a
five-second model default against a four-second config default, repeats the
capacity default, and overwrites explicit auto lifetimes at runtime. These
behaviors were preserved by the move, not silently fixed. Server ownership of
plain notification data remains T544. T512 C2 is checked; C1/C3 remain open.

Rustdoc failures introduced by moved notification links were corrected. The
remaining doc failures are existing private `Shared::size` / `Shared::bounds`
links at `heca/src/chrome/panes/terminal/component/mod.rs:268` and `:270`.
The P099 handoff has the current T512 focus, pending answer and T544 return.

### Latest Resume

The normal window now uses `ClientState`; standalone uses the same `ServerLoop`
through an in-process link. `--server` starts without winit/GPU. A real binary
integration test verifies PTY identity, command completion and retained history
across detach/reattach. These are process/socket checks, not native UI acceptance.

Latest fixes: ordinary output no longer floods the ordered-effects queue; an
action targeting a removed terminal receives an explicit refusal carrying its
request ID; a spawned daemon is reaped independently of the client connection.
The targeted server suite passed 101 tests and the daemon watcher test passed.
Scoped all-target clippy passed with `-D warnings`; log:
`/Users/antonio/projects/heca-p099/target/t544-resume-clippy.log`.
The full scoped all-target suite passed: 1068 Heca library tests (three existing
ignored), 124 configuration tests, 255 core tests and all integration targets,
including the real headless detach/reattach test. No test filters were added.
The latest full suite log is `t544-resume-all-tests.log` in that target folder.

Still open: notification ownership/transport (AppState retains legacy
ServerState for notifications), server config reload, terminal damage delivery,
resource lifetime review and native UI acceptance. Module split approval is now
explicit; follow the Approved Notification Order above.

The native test app `local.heca.t544.window` ran, but CUA reported
`cgWindowNotFound`; no native screenshot or close/reopen acceptance exists.
Its isolated artifacts live under `heca-p099-current/target/t544-native`.
The owned native client/server test processes were stopped after the check.
C3-C6 remain open. No T544 commit or PR exists.

### Earlier Evidence (Superseded Where Noted Above)

- `cargo test -p heca -p heca-core -p heca-config --all-targets` passed, with no
  added test filters. The Heca library has 1063 passing tests and three existing
  ignored tests; heca-config has 124 passing tests. The focus-door scanner test
  now passes with the delivered main fix.
- Full log: `/Users/antonio/projects/heca-p099/target/t544-current-all-tests.log`.
- Clippy log: `/Users/antonio/projects/heca-p099/target/t544-current-clippy.log`.
- Cargo uses the old worktree's target directory for build caching only.
- Shared connection limits now come from `[settings.connection]` in the embedded
  config. Nonzero limits, partial overrides and default loading are tested.
- `server/runtime/mod.rs` adds a bounded owner inbox and a window-free loop
  sleeping until incoming events or ServerState's nearest deadline. Only wake
  signals coalesce; connection/stop overflow returns an error. Tests construct
  Rc-owning ServerState inside its thread, attach again after detach, and verify
  a notification deadline without further client traffic.
- In-process links now wake their reader after sends and disconnects. Sender
  teardown disconnects before waking, so an idle reader cannot miss EOF.

At that earlier checkpoint the owner loop was not wired to CLI startup or the
real PTY/window path. The Latest Resume section supersedes that implementation
state. Native GUI acceptance is still missing; C3-C6 remain open.

## Implemented

- `server/wire/mod.rs` replaces the old `wire.rs`. ClientMsg, ServerMsg and Asker
  serialize through the existing serde vocabulary. ClientLink and ServerLink
  report empty queues separately from disconnected peers.
- `server/wire/frame.rs` carries JSON behind a big-endian u32 length prefix.
  Nonzero limits are supplied by the host, not fixed by individual callers.
  Oversized/zero lengths are rejected before allocation. Partial frames and
  invalid JSON return errors; clean EOF is distinct from an incomplete header.
- `server/wire/socket.rs` supplies both typed socket ends. Blocking I/O lives on
  owned reader/writer threads. Queues are bounded, sends never block the server,
  incoming messages and connection failures wake the owner, and dropping a link
  shuts down I/O and joins both workers. Queue overflow reports failure rather
  than silently dropping protocol messages. C5 still owns state coalescing and
  terminal snapshot/damage delivery.
- `server/run.rs` accepts either link through the same attach/action loop.
  Detached/dropped peers are removed; actions before attach, after detach, or
  belonging to the window cannot enter the server action handler.
- `server/socket/mod.rs` replaces the old `socket.rs`. Its listener is now
  guarded by `socket/listener.rs`: a live socket is never blindly unlinked,
  stale refused sockets can be reclaimed, ordinary files are preserved,
  permissions are checked, and cleanup removes only the same socket inode.
  A file lock serializes bind/reclaim. The Unix-only `fs2` dependency avoids
  requiring std file locking newer than the workspace's declared Rust 1.85.

## Earlier Transport Evidence (Before Main Integration)

Pre-edit and post-edit `cargo check -p heca` passed.
`cargo clippy -p heca --all-targets -- -D warnings` passed before the listener
change; the final run is recorded in `target/t544-clippy.log`.

Latest test command:

```sh
cargo test -p heca --all-targets -- --skip only_the_focus_door_writes_the_focused_flag
```

Result: 1,043 passed, zero failed, one explicitly filtered test, across 14 test
executables. Log: `target/t544-tests.log` in the code worktree. Tests cover real
Unix socket exchange and ServerLoop attach/detach, wakeups, malformed/truncated
frames, bounded queues, disconnects, worker teardown, live/stale socket safety
and preservation of replacement sockets/files.

Regression check: deliberately removed the outgoing size check; the dedicated
oversized-frame test failed. Restored the check and reran the suites above.

The unfiltered all-target run passed all 1,009 library tests at that point and
then failed `focus_door` on the unchanged private `view.focused` assignments in
`heca-grid-ui/src/widgets/input/component.rs`. The already-approved scanner fix
exists in the main checkout but is absent from this branch. Neither file was
changed as part of T544. Do not describe the unfiltered suite as passing.

No GUI or process-survival verification is claimed. These links are not yet
the running application's transport.

## Checklist Correction

C1 remains checked. C2 is now checked with the transport evidence above.
C3 was reopened: SessionContent projection/rebuild exists, but the window still
reads its local AppState.session and AppState.server directly. The old tick
did not prove the full wording. C4, C5 and C6 remain open.

## Resume

P102 is delivered on main and origin/main; that blocker is resolved. Reuse its
installer CLI. Current source contains the transferred RPC hook in the new CLI
module. The action registry still requires AppState, including dynamic action
handlers: centralize execution ownership before enabling headless RPC. Do not
create a second registry or hard-code a subset of extension names/actions.

After that integration choice, continue the central client/server wiring:

1. Continue in heca-p099-current only; use the planner in heca/main.
2. Connect the existing socket entry to the attach protocol and runtime inbox,
   using settings.connection limits; do not create a second command door.
3. Move process ownership/polling/startup into the headless runtime. Replace the
   remaining direct backend reads through one client-facing API, not fallback
   branches at each call site. Keep each window's view and scrolling local.
4. Deliver terminal snapshots/damage and images, bounded state coalescing, and
   equivalent standalone operation. Add CLI startup/attach without duplicating
   P102 integration commands. Preserve T545/T546/T547/T548 ownership.
5. Verify in the real app: close a window mid-command and reattach with the same
   running process and scrollback. Only then consider task closure. At phase
   completion prepare the PR to main with Antonio's commit/publish approval.
