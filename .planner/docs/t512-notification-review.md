# T512 Notification Review

## Current State

Implementation was verified in `/Users/antonio/projects/heca-t512`, branch
`refactor/t512-notification`, based on main `6e3e6d36`. Antonio authorized the
commit and PR. Code commit `6c96195d` was merged through
[PR #298](https://github.com/ovidius72/heca/pull/298) into main at
`9fb81aee8eaf46cedb95afd27a8c399a3811b419`. The merge was verified on GitHub and
local main was fast-forwarded while preserving the shared planner/graphify edits.
T512's code and review are complete. Its clean worktree and merged local branch
were removed after Antonio's approval.

The original 3,431-line `heca/src/notification.rs` is split into `notification/`:
`model/`, `store/`, `runtime/`, `presentation/`, `sink/`, `action.rs` and `mod.rs`.
The largest file is 373 lines. All 73 existing notification tests passed after
the mechanical move. The approved corrections add five tests, for 78 total.

## Preflight And Review

- Reuse `Notification`, `NotificationDraft::auto_dismiss`, `NotificationRuntime::push`
  and `NotificationSystemConfig`; do not invent a second raise API or widget.
- Existing task: P082(F003)/T512. No new feature, task or visual component is needed.
- Approved lifetime behavior is F009 decision `861eeaaf-9fbc-4168-a647-2afc1a14912a`.
- Worktree/ownership coordination is P099 decision `906824be-33d4-49c3-9320-6da25abfce15`.

Corrected review findings:

1. Native extensions could not name the fluent builder. `heca/src/lib.rs` now
   reexports `Notification` and `NotificationAction`; an external-crate test proves it.
2. The public builder lacked `auto_dismiss(Duration)`. It now forwards to the
   existing draft method, without exposing host types.
3. Runtime config unconditionally replaced an explicit producer duration.
   The draft now records whether its lifetime was explicit and applies a default
   only when omitted. Changing dismissibility does not mark a lifetime explicit.
4. The model hardcoded five seconds while config used four seconds. The fallback
   now comes from `NotificationSystemConfig::default()` / `config.default.toml`.
5. Store/runtime capacity duplicated the config's default. Both now use the
   config-owned default, retaining explicit constructor capacity.
6. Runtime docs incorrectly described owning AppState and a reactive signal.
   Docs now describe the actual plain state and retained presentation data.
7. Runnable builder rustdoc and README now document optional duration precedence.
   Minimal existing broken doc links in terminal/component/mod.rs and server/change.rs
   were corrected; neither change modifies behavior.

## Duration Contract

```rust
use heca::Notification;
use std::time::Duration;

Notification::success("Build finished").send();
Notification::success("Build finished")
    .auto_dismiss(Duration::from_secs(10))
    .send();
Notification::warning("Needs attention").sticky().send();
```

An explicit duration wins. If omitted, the host uses
`settings.notification_system.auto_dismiss_ms`. If that setting is omitted, the
bundled configuration supplies its fallback (currently 4000 ms). Sticky has no
automatic expiry. Lifetime setters follow builder order: the last setter wins.
An explicit zero duration is preserved. Timers start on promotion to visible.
Config reload affects later accepted drafts, not deadlines already assigned.

## Verification

- Before the fix, four notification assertions failed: explicit draft duration,
  explicit lifecycle equal to the fallback, reload precedence, and config-derived fallback.
- The external API test failed to compile before the public method was added.
- `cargo test -p heca --all-targets`: 1034 library tests passed, 3 existing ignored;
  all integration suites passed, including 3 external notification API tests.
- `cargo test -p heca-config --all-targets --quiet`: 121 passed.
- `./scripts/lint-changed.sh`: heca and heca-config clippy passed with `-D warnings`.
- `cargo test -p heca -p heca-config --doc --quiet`: passed, including the public
  Notification example; pre-existing ignored examples remain ignored.
- `cargo doc -p heca --no-deps --document-private-items --quiet`: passed after the
  server/change.rs documentation-link correction.
- `git diff --check HEAD`: passed.

No new GUI acceptance is claimed for this task. The showcase image-flicker report
is separate renderer work; no change was made to `heca-rendering`.

## T544 Boundary And Delivery

T544 belongs to heca-dev2 in `/Users/antonio/projects/heca-p099-current`, branch
`feat/p099-headless`. Its native detach/reattach acceptance was reported passed by
Antonio and heca-dev2, not rerun as part of T512.

Model/store/runtime have no operational AppState or window references. However,
store and runtime still project/cache `ToastSpec`; model actions/severity also
reference presentation vocabulary. This split preserves that existing API.
Removing presentation dependencies and introducing the plain server/client
projection, server-owned timers and notification transport remain T544's work.
The host adapter in `sink::raise` still accepts AppState in this base branch.

PR #298 is merged. Have heca-dev2 preserve all staged, unstaged and untracked
T544 changes, then integrate origin/main into feat/p099-headless before C8 wiring.
Do not reset or discard that dirty worktree; any conflicts must preserve T544's
server/client changes and the public notification exports delivered here.
Take the notification module tree wholesale, not a mixture with notification.rs.
T512's delivery coordination is complete because its result is landed on main.
Applying main to T544 is successor work, not an unfinished part of T512.
