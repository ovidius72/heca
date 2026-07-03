# Notification System Plan

> **Planner-compatible structure:** Feature → Phases → Tasks.  
> **Status:** draft plan / analysis only.  
> **Branch:** `feature/notifications-analysis`.  
> **Primary implementation target:** in-app notifications first.  
> **Deferred:** OS-level notifications, global KeyHint integration for toast actions.  
> **Out of scope:** confirmation modals / confirmation-before-action flow.

---

## Plan Metadata

### Name

`notification-system`

### Description

Implement a configurable notification system for heca that can route notifications to:

```toml
[settings]
notification_system = "app" # "app" | "system" | "none"
```

The first implementation target is the **in-app** notification backend using existing
`heca-grid-ui` widgets:

- `Toast`
- `ToastStack`
- `ToastSpec`
- `ToastSeverity`

The notification system must be app-owned: `heca-grid-ui` only renders notification widgets;
heca owns queueing, lifetimes, timers, deduplication, action dispatch, and routing.

### Tools / Components Used

#### Existing heca-grid-ui widgets

- `Toast` — display-only notification card.
- `ToastStack` — display-only overlay stack of `ToastSpec` entries.
- `ToastSpec` — presentation data consumed by `ToastStack`.
- `ToastSeverity` — visual severity mapping.
- `KeyHint` / `paint_keycap` — deferred integration for global keyboard hints.

#### Existing heca systems

- `AppState` — owns runtime notification store.
- `ChromeSignals` / retained chrome tree — owns signal passed into `ToastStack`.
- `ActionRegistry` — executes notification actions.
- `KeymapRegistry` — notification actions must remain reachable through registered actions/keybindings where appropriate.
- `ChromeEventBus` — future producers may emit notifications from chrome/plugin/provider events.
- `handle_about_to_wait` lifecycle loop — runs notification expiry timers.
- `HecaApp::reload_config` — first real notification producer.

#### External / future tools

- `notify-rust` — candidate for OS/system notifications, deferred.

### Key References

- `docs/widgets.md`
  - `Toast`
  - `ToastStack`
  - `KeyHint`
- `heca-grid-ui/src/widgets/toast.rs`
- `heca-grid-ui/src/widgets/toast_stack.rs`
- `heca-grid-ui/src/widgets/key_hint.rs`
- `heca-renderer/examples/showcase.rs`
  - Host-owned `Signal<Vec<ToastSpec>>` example.
  - `t` key pushes sample toast.
  - Dismiss removes item from host-owned list.
- `heca/src/app/lifecycle.rs`
  - `handle_about_to_wait` timing loop.
- `heca/src/main.rs`
  - `HecaApp::reload_config` currently owns real reload result.
- `heca/src/app/events.rs`
  - `AppEvent` event routing.
- `heca/src/app/interaction.rs`
  - `dispatch_action` and `ActionPolicy`.
- `heca/src/actions.rs`
  - action descriptors.
- `heca/src/input.rs`
  - `WmAction`, `action_from_name`, `action_priority`.
- `heca-config/src/settings.rs`
  - target for `[settings].notification_system`.
- `config.default.toml`
- `README.md`

### Non-Goals

- Do **not** implement confirmation modals.
- Do **not** implement confirmation-before-action.
- Do **not** create new ad-hoc UI widgets when `Toast` / `ToastStack` already exist.
- Do **not** put queue/timer/dedup lifecycle in `heca-grid-ui`.
- Do **not** run toast actions as direct closures mutating `AppState`.
- Do **not** implement OS-level notifications in v1.
- Do **not** implement global `KeyHint` support for toast actions until the WIP `prefix+/` branch lands.

---

# Feature F1 — Notification Domain Model

## Feature Description

Define heca-owned runtime notification types. These types represent what heca wants to communicate,
not how grid-ui renders it.

`ToastSpec` remains a presentation DTO owned by `heca-grid-ui`; `AppNotification` is the app-level
source of truth.

## Feature Dependencies

- Existing `WmAction` from `heca/src/input.rs` for notification actions.
- Existing `ToastSpec` and `ToastSeverity` from `heca-grid-ui` for conversion.

## Feature Blocking Tasks

- None. This feature can be designed first.

## Feature References

- `heca-grid-ui/src/widgets/toast_stack.rs`
- `docs/widgets.md` § `Toast` / `ToastStack`

---

## Phase F1.P1 — Define Core Notification Types

### Phase Description

Create the core app-side model needed by the queue/store and rendering adapter.

### Phase Dependencies

- None.

### Phase Avoid

- Do not store UI callbacks in the notification model.
- Do not store `Toast` widgets in app state.
- Do not store `ToastSpec` as the canonical notification type.

---

### Task F1.P1.T1 — Define `NotificationSystem`

#### Description

Add an enum for the user-selected notification target.

#### Implementation Notes

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSystem {
    #[default]
    App,
    System,
    None,
}
```

#### Target Files

- `heca-config/src/settings.rs`

#### Dependencies

- `serde::{Serialize, Deserialize}` already used by config.

#### Blocking Tasks

- Blocks F6 config implementation.

#### Avoid

- Do not use booleans like `notifications_enabled` + `system_notifications_enabled`; the requested config is a single mode.
- Do not use stringly typed runtime matching outside serde parsing.

#### Validation

- Unit test config parsing:

```toml
[settings]
notification_system = "app"
```

```toml
[settings]
notification_system = "system"
```

```toml
[settings]
notification_system = "none"
```

---

### Task F1.P1.T2 — Define `NotificationSeverity`

#### Description

Define app-level severity independent from grid-ui `ToastSeverity`.

#### Example

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationSeverity {
    Info,
    Success,
    Warning,
    Danger,
}
```

#### Target Files

- New: `heca/src/notifications/model.rs`

#### Dependencies

- None.

#### Blocking Tasks

- Blocks conversion to `ToastSpec`.

#### Avoid

- Do not add theme colors here. Theme mapping belongs to `Toast` / grid-ui.

#### Validation

- Unit test maps all variants to `ToastSeverity`.

---

### Task F1.P1.T3 — Define `NotificationLifecycle`

#### Description

Represent whether a notification auto-dismisses or remains until dismissed.

#### Example

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationLifecycle {
    AutoDismiss(Duration),
    Sticky,
}
```

#### Default Policy

| Severity | Lifecycle |
|---|---|
| `Info` | `AutoDismiss(4000ms)` |
| `Success` | `AutoDismiss(3000ms)` |
| `Warning` | `AutoDismiss(6000ms)` |
| `Danger` | `Sticky` |

#### Target Files

- `heca/src/notifications/model.rs`

#### Dependencies

- `std::time::Duration`

#### Blocking Tasks

- Blocks store expiry implementation.

#### Avoid

- Do not use floating-point seconds in state; store `Duration`.

#### Validation

- Unit test lifecycle default by severity.

---

### Task F1.P1.T4 — Define `NotificationSource`

#### Description

Track where a notification came from. This supports dedup, future history, debugging, and filtering.

#### Example

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotificationSource {
    System,
    Config,
    Action { name: String },
    Pane { pane_id: PaneId },
    Workspace { ws_idx: usize },
    ChromeProvider { provider_id: String },
    Agent,
}
```

#### Target Files

- `heca/src/notifications/model.rs`

#### Dependencies

- `PaneId` from `heca_core::layout` if pane source is included.

#### Blocking Tasks

- Optional for v1, but useful for producers.

#### Avoid

- Do not make source responsible for routing. Routing is handled by `NotificationSystem`.

---

### Task F1.P1.T5 — Define `NotificationAction`

#### Description

Represent the primary action shown on a toast.

The action must be a `WmAction`, not a closure.

#### Example

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct NotificationAction {
    pub label: String,
    pub action: WmAction,
    pub dismiss_after: bool,
}
```

#### Examples

```rust
NotificationAction {
    label: "Retry".to_string(),
    action: WmAction::ReloadConfig,
    dismiss_after: false,
}
```

```rust
NotificationAction {
    label: "Focus Pane".to_string(),
    action: WmAction::FocusPane { pane_id },
    dismiss_after: true,
}
```

#### Target Files

- `heca/src/notifications/model.rs`

#### Dependencies

- `WmAction` from `heca/src/input.rs`.

#### Blocking Tasks

- Blocks toast action dispatch.

#### Avoid

- Do not store `Box<dyn Fn()>`.
- Do not mutate `AppState` from toast callbacks.
- Do not add multiple actions in v1; `ToastSpec` supports one inline action.

#### Validation

- Unit test clone/equality if feasible.
- Ensure action can be looked up after notification id click.

---

### Task F1.P1.T6 — Define `AppNotification`

#### Description

Define the canonical app-owned notification object.

#### Example

```rust
#[derive(Clone, Debug)]
pub struct AppNotification {
    pub id: NotificationId,
    pub severity: NotificationSeverity,
    pub title: String,
    pub body: Option<String>,
    pub source: NotificationSource,
    pub lifecycle: NotificationLifecycle,
    pub action: Option<NotificationAction>,
    pub dedup_key: Option<String>,
    pub created_at: Instant,
    pub expires_at: Option<Instant>,
}
```

#### Target Files

- `heca/src/notifications/model.rs`

#### Dependencies

- F1.P1.T2-T5.

#### Blocking Tasks

- Blocks notification store.

#### Avoid

- Do not persist `Instant`; this is runtime-only.
- Do not embed `ToastSpec`.

---

## Phase F1.P2 — Presentation Mapping

### Phase Description

Provide conversion from app-owned notifications into grid-ui presentation data.

### Dependencies

- F1.P1 core model.
- `heca-grid-ui::widgets::{ToastSpec, ToastSeverity}`.

---

### Task F1.P2.T1 — Map `NotificationSeverity` to `ToastSeverity`

#### Description

Convert all app severity variants to existing grid-ui toast severity variants.

#### Example

```rust
impl From<NotificationSeverity> for ToastSeverity {
    fn from(value: NotificationSeverity) -> Self {
        match value {
            NotificationSeverity::Info => ToastSeverity::Info,
            NotificationSeverity::Success => ToastSeverity::Success,
            NotificationSeverity::Warning => ToastSeverity::Warning,
            NotificationSeverity::Danger => ToastSeverity::Danger,
        }
    }
}
```

#### Target Files

- `heca/src/notifications/toast_adapter.rs`

#### Avoid

- Do not duplicate theme colors.

---

### Task F1.P2.T2 — Convert `AppNotification` to `ToastSpec`

#### Description

Build `ToastSpec` from the canonical app notification.

#### Example

```rust
pub fn to_toast_spec(n: &AppNotification) -> ToastSpec {
    let mut spec = ToastSpec::new(n.id.get(), n.title.clone())
        .severity(n.severity.into())
        .dismissible(true);

    if let Some(body) = &n.body {
        spec = spec.body(body.clone());
    }

    if let Some(action) = &n.action {
        spec = spec.action(action.label.clone());
    }

    spec
}
```

#### Target Files

- `heca/src/notifications/toast_adapter.rs`

#### Dependencies

- F1.P2.T1.

#### Avoid

- Do not encode action behavior in `ToastSpec`; only label.
- Do not expose more than one action in v1.

#### Validation

- Unit test conversion with and without body/action.

---

# Feature F2 — Notification Store and Queue

## Feature Description

Implement app-owned storage and lifecycle rules for active notifications.

`ToastStack` does not manage queue/timers; this feature fills that gap.

## Dependencies

- F1 model.
- `std::time::{Instant, Duration}`.

## Blocking Tasks

- Blocks UI integration.
- Blocks action dispatch because action lookup happens through store by notification id.

## References

- `heca-grid-ui/src/widgets/toast_stack.rs` docs explicitly state lifecycle is host-owned.

---

## Phase F2.P1 — Store Implementation

### Task F2.P1.T1 — Define `NotificationStore`

#### Description

Store active notifications, hidden/queued notifications, and optional history.

#### Example

```rust
pub struct NotificationStore {
    next_id: u64,
    active: Vec<AppNotification>,
    history: VecDeque<AppNotification>,
    max_visible: usize,
}
```

#### Target Files

- `heca/src/notifications/store.rs`

#### Dependencies

- F1.P1.T6.

#### Avoid

- Do not use `ToastSpec` as stored state.
- Do not call rendering code from the store.

---

### Task F2.P1.T2 — Implement `push`

#### Description

Add a notification to the store, assigning id and expiry.

#### Expected Behavior

- Assign monotonically increasing `NotificationId`.
- Compute `expires_at` from lifecycle.
- Apply dedup before adding a new item.
- Mark store as changed so UI sync can update signal.

#### Example

```rust
pub fn push(&mut self, draft: NotificationDraft, now: Instant) -> NotificationId
```

#### Avoid

- Do not let callers choose arbitrary ids.
- Do not panic on empty title; either reject or normalize.

---

### Task F2.P1.T3 — Implement `dismiss`

#### Description

Dismiss notification by id.

#### Behavior

- Remove from active list.
- Optionally append to history.
- Return whether anything changed.

#### Example

```rust
pub fn dismiss(&mut self, id: NotificationId) -> Option<AppNotification>
```

#### Used By

- Toast `×` click.
- Notification action with `dismiss_after = true`.

---

### Task F2.P1.T4 — Implement `action_for`

#### Description

Lookup notification action by id.

#### Example

```rust
pub fn action_for(&self, id: NotificationId) -> Option<NotificationAction>
```

#### Dependencies

- F1.P1.T5.

#### Avoid

- Do not execute action here.
- Store returns data; app event handler dispatches it.

---

### Task F2.P1.T5 — Implement `expire`

#### Description

Remove expired notifications.

#### Example

```rust
pub fn expire(&mut self, now: Instant) -> bool
```

#### Behavior

- Remove items where `expires_at <= now`.
- Keep sticky notifications.
- Return true if store changed.

---

### Task F2.P1.T6 — Implement dedup policy

#### Description

Use `dedup_key` to avoid spam.

#### Behavior

If a notification with the same key exists:

- update title/body/severity/action;
- reset `created_at`;
- recompute `expires_at`;
- do not allocate new id.

#### Examples

- `config.reload.failed`
- `sidecar.disconnected`
- `provider.error:<provider_id>`

#### Avoid

- Do not dedup notifications without explicit key.

---

### Task F2.P1.T7 — Implement visible list policy

#### Description

Convert active store to visible toasts.

#### Example

```rust
pub fn visible_toast_specs(&self) -> Vec<ToastSpec>
```

#### v1 Policy

- `max_visible = 4` hardcoded initially.
- Sticky/danger notifications have priority.
- Newer notifications before older ones, unless sticky priority requires otherwise.

#### Avoid

- Do not make max-visible config in first slice unless needed.

---

## Phase F2.P2 — Lifecycle Integration

### Task F2.P2.T1 — Add expiry to `handle_about_to_wait`

#### Description

Call store expiry from the event loop lifecycle.

#### Reference

- `heca/src/app/lifecycle.rs`
- Existing animation/timer scheduling uses `ControlFlow::WaitUntil`.

#### Pseudocode

```rust
let now = Instant::now();
if state.notifications.expire(now) {
    state.mark_full_redraw();
}
```

#### Avoid

- Do not busy-loop.
- Do not request continuous redraw only for future expiry.

---

### Task F2.P2.T2 — Schedule next expiry

#### Description

Expose next expiry time so lifecycle can wake up when the next notification should disappear.

#### Example

```rust
pub fn next_expiry(&self) -> Option<Instant>
```

#### Integration Rule

If no animation is ongoing but a notification expires in 3 seconds, use:

```rust
ControlFlow::WaitUntil(next_expiry)
```

If animation is ongoing, existing frame interval wins.

#### Avoid

- Do not lower animation frame timing because of notifications.

---

# Feature F3 — In-App Toast Rendering

## Feature Description

Render active notifications using existing `ToastStack` in the retained chrome tree.

## Dependencies

- F1 model.
- F2 store.
- Existing retained chrome tree.
- Existing `ToastStack` widget.

## References

- `heca-renderer/examples/showcase.rs`
  - Host-owned `Signal<Vec<ToastSpec>>`.
- `heca-grid-ui/src/widgets/toast_stack.rs`
- `heca/src/chrome/mod.rs`
  - retained chrome root / `ChromeSignals` / `sync_chrome_signals`.

---

## Phase F3.P1 — AppState and Chrome Signals

### Task F3.P1.T1 — Add store to `AppState`

#### Description

Add notification store to app state.

#### Example

```rust
pub notifications: NotificationStore,
```

#### Target Files

- `heca/src/app_state.rs`
- `heca/src/app/startup.rs` if initialization lives there.

#### Dependencies

- F2.P1.T1.

---

### Task F3.P1.T2 — Add toast signal to `ChromeSignals`

#### Description

`ToastStack` consumes `Signal<Vec<ToastSpec>>`. Retained chrome needs to keep this signal.

#### Example

```rust
pub(crate) struct ChromeSignals {
    // existing fields...
    notification_toasts: Option<Signal<Vec<ToastSpec>>>,
}
```

#### Target Files

- `heca/src/chrome/mod.rs`

#### Avoid

- Do not rebuild the chrome tree on every notification change.
- Use signal update like existing pane/status signals.

---

### Task F3.P1.T3 — Sync notification specs into signal

#### Description

Update `notification_toasts` in `sync_chrome_signals`.

#### Pseudocode

```rust
if let Some(sig) = retained.signals.notification_toasts {
    let next = state.notifications.visible_toast_specs();
    if sig.get_untracked() != next {
        sig.set(next);
        changed = true;
    }
}
```

#### Dependencies

- F2.P1.T7.

#### Avoid

- Do not allocate/rebuild toast widgets manually; `ToastStack` reconciles by id.

---

## Phase F3.P2 — ToastStack Mounting

### Task F3.P2.T1 — Create `ToastStack` in `chrome_root`

#### Description

Add `ToastStack` to retained chrome root as overlay-capable component.

#### Example

```rust
let notification_signal = signal(Vec::<ToastSpec>::new());
signals.notification_toasts = Some(notification_signal);

let toast_stack = ToastStack::new(notification_signal)
    .corner(ToastCorner::TopRight)
    .on_dismiss(/* send AppEvent */)
    .on_action(/* send AppEvent */);
```

#### Target Files

- `heca/src/chrome/mod.rs`

#### References

- Showcase around `let toasts = signal(Vec::<ToastSpec>::new())`.

#### Avoid

- Do not hardcode styles/colors.
- Do not write custom toast layout.

---

### Task F3.P2.T2 — Route dismiss/action callbacks to app events

#### Description

Callbacks should enqueue events, not directly mutate app state.

#### Candidate Events

```rust
AppEvent::NotificationDismiss { id: NotificationId }
AppEvent::NotificationAction { id: NotificationId }
```

#### Target Files

- `heca/src/app/events.rs`
- `heca/src/chrome/mod.rs`

#### Avoid

- Do not capture mutable `AppState` in callbacks.

---

### Task F3.P2.T3 — Ensure overlay interaction remains pass-through

#### Description

Verify `ToastStack` consumes only pointer events on toast cards.

#### Existing Behavior

`ToastStack::event` returns `Handled::No` for pointer misses and keyboard events.

#### Validation

- Click outside toast still focuses/interacts with underlying UI.
- Click dismiss removes toast.
- Click action triggers `NotificationAction` event.

---

# Feature F4 — Toast Action Dispatch

## Feature Description

Execute notification actions through the existing action system.

Toast action buttons are not direct callbacks; they map to `WmAction` and dispatch through heca's
registry/policy path.

## Dependencies

- F1 `NotificationAction`.
- F2 store action lookup.
- F3 app events.
- Existing `ActionRegistry` / `dispatch_action`.

## References

- `heca/src/app/interaction.rs`
- `heca/src/actions.rs`
- `heca/src/input.rs`

---

## Phase F4.P1 — Notification App Events

### Task F4.P1.T1 — Add notification event variants

#### Description

Extend `AppEvent`.

#### Example

```rust
pub enum AppEvent {
    BackendWake,
    RequestRedraw,
    ChromeIntent { ... },
    NotificationDismiss { id: NotificationId },
    NotificationAction { id: NotificationId },
}
```

#### Target Files

- `heca/src/app/events.rs`

#### Dependencies

- F1 notification id type.

---

### Task F4.P1.T2 — Handle dismiss event

#### Description

When dismiss event arrives, remove notification from store.

#### Pseudocode

```rust
AppEvent::NotificationDismiss { id } => {
    state.notifications.dismiss(id);
    state.mark_full_redraw();
}
```

#### Target Files

- `heca/src/main.rs` user event handler or event dispatch module.

---

### Task F4.P1.T3 — Handle action event

#### Description

Lookup action, route through `dispatch_action`, then apply dismiss policy.

#### Pseudocode

```rust
let Some(action) = state.notifications.action_for(id) else { return; };
let wm = action.action.clone();
if action.dismiss_after {
    state.notifications.dismiss(id);
}
dispatch_action(state, registry, InteractionSource::MouseContent, &wm);
```

#### Important

Exact placement must respect current borrow rules: clone action before mutating store/dispatching.

#### Avoid

- Do not call handlers directly.
- Do not bypass `ActionRegistry`.

---

## Phase F4.P2 — Action Semantics

### Task F4.P2.T1 — Define notification action source

#### Description

Decide `InteractionSource` for toast actions.

#### Recommendation

Add future source:

```rust
InteractionSource::ChromeOverlay
```

or reuse existing `MouseContent`/chrome source if adding source is too large.

#### Avoid

- Do not let floating-domain policy accidentally block true global notification actions like `ReloadConfig`.

---

### Task F4.P2.T2 — Validate action policies

#### Description

Any `WmAction` used by notifications must already be correctly classified.

Examples:

- `ReloadConfig` must remain `ActionPolicy::Global`.
- `FocusPane` source-dependent policy must still make sense from toast action.

#### Target Files

- `heca/src/app/interaction.rs`

#### Validation

- Add tests only when implementation touches action policy.

---

# Feature F5 — First Producer: Reload Config Notifications

## Feature Description

Use config reload as the first real producer of notifications.

This validates success, failure, sticky errors, and action button (`Retry`).

## Dependencies

- F1-F4.
- Existing `ReloadConfig` action.

## References

- `heca/src/handlers.rs` — `handle_reload_config` sets `pending_reload`.
- `heca/src/main.rs` — `HecaApp::reload_config` does the actual work and currently only prints errors.

---

## Phase F5.P1 — Result-Aware Reload

### Task F5.P1.T1 — Refactor `reload_config` to return result

#### Description

Make reload config report success/failure to caller.

#### Current Behavior

```rust
fn reload_config(&mut self) {
    let new_config = match AppConfig::try_load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!(...);
            return;
        }
    };
    // apply config...
}
```

#### Target Behavior

```rust
fn reload_config(&mut self) -> Result<(), ReloadConfigError> {
    let new_config = AppConfig::try_load()?;
    // apply config...
    Ok(())
}
```

#### Target Files

- `heca/src/main.rs`

#### Avoid

- Do not remove existing safe behavior: failed reload must keep current config.
- Do not hide errors.

---

### Task F5.P1.T2 — Success notification

#### Description

Push success toast after reload applies successfully.

#### Notification

```text
Severity: Success
Title: Config reloaded
Body: Theme, keymaps and settings updated.
Lifecycle: AutoDismiss(3000ms)
Dedup: config.reload.success
```

#### Dependencies

- F2 store.

---

### Task F5.P1.T3 — Failure notification

#### Description

Push sticky danger toast when reload fails.

#### Notification

```text
Severity: Danger
Title: Config reload failed
Body: <error text>
Lifecycle: Sticky
Action: Retry -> WmAction::ReloadConfig
Dedup: config.reload.failed
```

#### Avoid

- Do not spam repeated failures; dedup key should update the existing toast.

---

### Task F5.P1.T4 — Preserve stderr logs

#### Description

Keep `eprintln!` logs for terminal/debug visibility.

#### Rationale

In-app notification is user-facing feedback; stderr remains useful for logs/tests.

---

# Feature F6 — Config Integration

## Feature Description

Add the requested configuration field under `[settings]`.

## Dependencies

- F1 `NotificationSystem`.

---

## Phase F6.P1 — Config Schema

### Task F6.P1.T1 — Add `notification_system` to `SettingsConfig`

#### Description

Add field with serde default.

#### Example

```rust
#[serde(default)]
pub notification_system: NotificationSystem,
```

#### Target Files

- `heca-config/src/settings.rs`

---

### Task F6.P1.T2 — Add default config value

#### Description

Add default to embedded config.

#### Target File

- `config.default.toml`

#### Value

```toml
[settings]
notification_system = "app"
```

---

### Task F6.P1.T3 — Add tests for parsing

#### Description

Ensure all three values parse.

#### Target Files

- `heca-config/src/settings.rs` tests

#### Cases

- ``notification_system = "app"`
- `notification_system = "system"`
- `notification_system = "none"`
- missing field defaults to `App`
- unknown value fails config parsing with a clear serde error

#### Example Test Shape

```rust
#[test]
fn parses_notification_system_modes() {
    let app: NotificationSystem = toml::from_str(r#""app""#).unwrap();
    assert_eq!(app, NotificationSystem::App);

    let system: NotificationSystem = toml::from_str(r#""system""#).unwrap();
    assert_eq!(system, NotificationSystem::System);

    let none: NotificationSystem = toml::from_str(r#""none""#).unwrap();
    assert_eq!(none, NotificationSystem::None);
}
```

#### Avoid

- Do not silently accept unknown strings.
- Do not make this a free-form string field.

---

### Task F6.P1.T4 — Runtime reload applies notification setting

#### Description

`prefix+Shift+r` reloads config at runtime. The new `notification_system` setting must update without restart.

#### Implementation Notes

When `HecaApp::reload_config` applies new settings, ensure the setting propagates to runtime state/config used by the notification router.

#### Target Files

- `heca/src/main.rs`
- any current runtime config copy site in `AppState`

#### Dependencies

- F5 result-aware reload is the first producer, but this setting must work independently.

#### Avoid

- Do not require app restart.
- Do not cache notification mode in multiple divergent places.

---

## Phase F6.P2 — Documentation

### Task F6.P2.T1 — Document `notification_system` in README

#### Description

Add user-facing documentation for the setting.

#### Target Files

- `README.md`

#### Example Documentation

```toml
[settings]
# app    = show in-app toast notifications
# system = use OS notifications when supported; initially reserved/deferred
# none   = suppress non-critical notifications
notification_system = "app"
```

#### Avoid

- Do not claim OS notifications are implemented in v1.
- Do not document confirmation/Modal behavior in this plan.

---

### Task F6.P2.T2 — Document default in `config.default.toml`

#### Description

Add a concise comment near other settings.

#### Target File

- `config.default.toml`

#### Example

```toml
# Notification backend: "app", "system", or "none".
# "system" is reserved until OS notification support lands.
notification_system = "app"
```

---

# Feature F7 — Notification Routing

## Feature Description

Route app notification requests according to `[settings].notification_system`.

The routing layer decides whether to push to in-app store, drop, or use future OS backend.

## Dependencies

- F1 model.
- F2 store.
- F6 config field.

## Blocking Tasks

- Blocks producers from respecting user preference.

## References

- `heca-config/src/settings.rs`
- `heca/src/app_state.rs`

---

## Phase F7.P1 — Router API

### Task F7.P1.T1 — Define `NotificationRouter` or store-facing emit helper

#### Description

Create one choke point for notification emission.

#### Option A — Router Type

```rust
pub struct NotificationRouter;

impl NotificationRouter {
    pub fn emit(
        store: &mut NotificationStore,
        mode: NotificationSystem,
        notification: NotificationDraft,
        now: Instant,
    ) -> Option<NotificationId> {
        match mode {
            NotificationSystem::App => Some(store.push(notification, now)),
            NotificationSystem::None => None,
            NotificationSystem::System => {
                // v1 behavior decided in F7.P1.T2
                Some(store.push(notification, now))
            }
        }
    }
}
```

#### Option B — AppState helper

```rust
impl AppState {
    pub fn notify(&mut self, notification: NotificationDraft) -> Option<NotificationId> {
        self.notifications.emit(self.settings.notification_system, notification)
    }
}
```

#### Recommendation

Use an `AppState` helper for ergonomics, backed by a small router function if needed.

#### Target Files

- `heca/src/notifications/router.rs`
- `heca/src/app_state.rs`

#### Avoid

- Do not let each producer match on `NotificationSystem` manually.
- Do not let providers access the store internals directly.

---

### Task F7.P1.T2 — Decide v1 behavior for `system`

#### Description

`notification_system = "system"` is planned before OS backend exists. v1 must have deterministic behavior.

#### Recommended Decision

Use in-app fallback plus one-time warning:

```text
System notifications are not implemented yet; using in-app notifications for this session.
```

#### Alternative

No-op for all notifications when `system` is selected.

#### Recommendation Rationale

Fallback is safer during development because errors like config reload failure remain visible.

#### Blocking Decision

This must be decided before implementation of F7.

#### Avoid

- Do not silently fall back forever without documenting it.
- Do not show the fallback warning repeatedly.

---

### Task F7.P1.T3 — Implement `none` behavior

#### Description

When `notification_system = "none"`, suppress non-critical notifications.

#### v1 Rule

All notification producers in this plan are non-critical and should be dropped under `none`.

#### Future Rule

Truly critical errors may still log to stderr or status areas, but not to notification toasts.

#### Avoid

- Do not show toast notifications when mode is `none`.
- Do not suppress debug/stderr logging solely because notifications are disabled.

---

# Feature F8 — Producer Helpers and Initial Producers

## Feature Description

Standardize how app subsystems create notifications. Avoid every subsystem hand-building full notification objects.

## Dependencies

- F1-F7.

## References

- `heca/src/main.rs` for reload config.
- `heca/src/backend` / pane exit paths for future producer.
- `heca/src/providers/mod.rs` / `heca/src/chrome/events.rs` for provider/plugin errors.

---

## Phase F8.P1 — Builder / Draft API

### Task F8.P1.T1 — Define `NotificationDraft`

#### Description

A caller-friendly draft object before id/time/expires are assigned.

#### Example

```rust
#[derive(Clone, Debug)]
pub struct NotificationDraft {
    pub severity: NotificationSeverity,
    pub title: String,
    pub body: Option<String>,
    pub source: NotificationSource,
    pub lifecycle: Option<NotificationLifecycle>,
    pub action: Option<NotificationAction>,
    pub dedup_key: Option<String>,
}
```

#### Target Files

- `heca/src/notifications/model.rs`

#### Avoid

- Do not require every producer to manually compute `created_at` or `expires_at`.

---

### Task F8.P1.T2 — Add convenience constructors

#### Description

Make producers concise and consistent.

#### Example

```rust
impl NotificationDraft {
    pub fn success(title: impl Into<String>) -> Self { ... }
    pub fn info(title: impl Into<String>) -> Self { ... }
    pub fn warning(title: impl Into<String>) -> Self { ... }
    pub fn danger(title: impl Into<String>) -> Self { ... }

    pub fn body(mut self, body: impl Into<String>) -> Self { ... }
    pub fn source(mut self, source: NotificationSource) -> Self { ... }
    pub fn action(mut self, label: impl Into<String>, action: WmAction) -> Self { ... }
    pub fn dedup_key(mut self, key: impl Into<String>) -> Self { ... }
    pub fn sticky(mut self) -> Self { ... }
    pub fn auto_dismiss(mut self, duration: Duration) -> Self { ... }
}
```

#### Usage Example

```rust
state.notify(
    NotificationDraft::danger("Config reload failed")
        .body(error.to_string())
        .source(NotificationSource::Config)
        .dedup_key("config.reload.failed")
        .action("Retry", WmAction::ReloadConfig)
        .sticky(),
);
```

#### Avoid

- Do not add a macro unless simple builder methods become too verbose.

---

## Phase F8.P2 — Reload Config Producer

### Task F8.P2.T1 — Emit success notification on reload success

#### Description

After config reload completes and state is updated, emit success.

#### Example

```rust
state.notify(
    NotificationDraft::success("Config reloaded")
        .body("Theme, keymaps and settings updated.")
        .source(NotificationSource::Config)
        .dedup_key("config.reload.success"),
);
```

#### Dependencies

- F5 result-aware reload.

#### Avoid

- Do not emit success before all settings/keymaps/theme are actually applied.

---

### Task F8.P2.T2 — Emit failure notification on reload failure

#### Description

On config load/apply error, emit sticky danger notification with retry action.

#### Example

```rust
state.notify(
    NotificationDraft::danger("Config reload failed")
        .body(error.to_string())
        .source(NotificationSource::Config)
        .dedup_key("config.reload.failed")
        .action("Retry", WmAction::ReloadConfig)
        .sticky(),
);
```

#### Avoid

- Do not replace the current working config after failed reload.
- Do not create a new toast every frame/retry with same error; use dedup.

---

## Phase F8.P3 — Additional Producer Backlog

### Task F8.P3.T1 — Pane exited notification

#### Description

When a pane backend exits, notify user.

#### Example

```text
Severity: Info or Warning depending exit status
Title: Pane exited
Body: <program/status>
Action: View / Focus Pane -> WmAction::FocusPane { pane_id }
Dedup: pane.exited:<pane_id>
```

#### Dependencies

- Need exact pane-exit event path identified before implementation.

#### Avoid

- Do not notify repeatedly for the same pane exit event.

---

### Task F8.P3.T2 — Spawn command failed notification

#### Description

When `SpawnCommand` or future spawn-pane action fails, show failure.

#### Example

```text
Severity: Danger
Title: Command failed to spawn
Body: <command/error>
Lifecycle: Sticky
```

#### Dependencies

- Existing spawn handler must expose error result.

#### Avoid

- Do not swallow spawn failures silently.

---

### Task F8.P3.T3 — Provider/plugin error notification

#### Description

Provider errors should be visible without crashing chrome.

#### Example

```text
Severity: Warning or Danger
Title: Provider error
Body: <provider_id>: <message>
Dedup: provider.error:<provider_id>
```

#### Dependencies

- Provider/render wiring maturity.
- `ChromeEventBus` event shape.

#### Avoid

- Do not let a failing provider spam each frame.

---

### Task F8.P3.T4 — Agent event notifications

#### Description

Future agent integration may notify:

- agent waiting for input;
- agent completed task;
- sidecar disconnected;
- room orchestration failed.

#### Dependencies

- `agents-communication-plan.md` implementation.
- agent-comms sidecar/control plane.

#### Avoid

- Do not implement agent-specific producer before local agent model exists.

---

# Feature F9 — OS/System Notification Backend

## Feature Description

Future backend for desktop notifications outside the heca window.

## Status

Deferred. Do not implement in v1.

## Dependencies

- F7 router.
- Platform validation.

## Candidate Tool

- `notify-rust`

## References

- `notify-rust` crate documentation.
- Platform docs:
  - Linux freedesktop/D-Bus notifications.
  - macOS Notification Center permission/app identity.
  - Windows toast notification app identity.

---

## Phase F9.P1 — Backend Trait

### Task F9.P1.T1 — Define `SystemNotificationBackend`

#### Description

Abstract OS notification sending behind a trait.

#### Example

```rust
pub trait SystemNotificationBackend {
    fn notify(&self, notification: &AppNotification) -> anyhow::Result<()>;
}
```

#### Avoid

- Do not make `NotificationStore` depend on system backend.
- Do not add platform-specific code to producers.

---

### Task F9.P1.T2 — Decide fallback behavior

#### Description

If system backend fails, decide whether to fall back to in-app notification.

#### Recommendation

Fallback to app notification for errors emitted by heca itself, but avoid recursive notification loops.

#### Avoid

- Do not emit infinite “system notification failed” notifications.

---

## Phase F9.P2 — notify-rust Evaluation

### Task F9.P2.T1 — Spike Linux behavior

#### Description

Validate freedesktop notifications under common Linux environments.

#### Validation

- Notification appears.
- App name/icon are acceptable.
- Actions support is understood but not required.

---

### Task F9.P2.T2 — Spike macOS behavior

#### Description

Validate Notification Center requirements.

#### Validation

- Permissions/app identity behavior known.
- No crash when permission missing.

---

### Task F9.P2.T3 — Spike Windows behavior

#### Description

Validate toast notification requirements.

#### Validation

- App identity requirements documented.
- Fallback behavior defined.

---

# Feature F10 — Keyboard Hint Integration for Toast Actions

## Feature Description

Make toast action/dismiss affordances compatible with the future global `prefix+/` hint system.

## Status

Deferred until the WIP branch that adds global `HintKey` support lands.

## Dependencies

- Future global `prefix+/` hint collection/activation branch.
- Existing `KeyHint` widget and `paint_keycap` helper.

## References

- `heca-grid-ui/src/widgets/key_hint.rs`
- existing heca chrome usage of `KeyHint` for pane/column/workspace pick flows.
- `docs/widgets.md` § `KeyHint`.

---

## Phase F10.P1 — Toast Hint Presentation

### Task F10.P1.T1 — Extend `ToastSpec` with hint fields

#### Description

Add optional hint labels for action/dismiss.

#### Example

```rust
pub struct ToastSpec {
    // existing fields...
    pub action_hint: Option<String>,
    pub dismiss_hint: Option<String>,
}
```

#### Target Files

- `heca-grid-ui/src/widgets/toast.rs`
- `heca-grid-ui/src/widgets/toast_stack.rs` if spec construction helpers live there.

#### Avoid

- Do not implement before global hint mode requirements are known.
- Do not hardcode hint letters in `Toast`.

---

### Task F10.P1.T2 — Paint keycaps over toast action/dismiss rects

#### Description

`Toast` currently draws action/dismiss internally. Use `paint_keycap` on the known rects.

#### Example

```rust
if let Some(hint) = &self.spec.action_hint {
    paint_keycap(cx, action_rect, hint, font, None);
}
```

#### Avoid

- Do not wrap internal action with `KeyHint` unless Toast is refactored into child widgets.
- Do not change toast layout in a way that breaks existing click hit testing.

---

## Phase F10.P2 — Hint Activation

### Task F10.P2.T1 — Connect hint activation to notification events

#### Description

When global hint mode assigns a key to a toast affordance, pressing the key should emit the same event as click.

#### Mapping

- action hint → `AppEvent::NotificationAction { id }`
- dismiss hint → `AppEvent::NotificationDismiss { id }`

#### Avoid

- Do not bypass event bridge.
- Do not execute notification action directly from hint mode.

---

### Task F10.P2.T2 — Preserve keyboard-first contract

#### Description

Every toast action must be reachable by keyboard once hint mode exists.

#### Rule

If a toast has an action button, it must expose either:

- a global hint activation; or
- a registered action/keybinding path elsewhere.

#### Avoid

- Do not ship mouse-only toast actions after hint branch lands.

---

# Feature F11 — Tests and Verification

## Feature Description

Add unit and integration tests for notification model, store behavior, config parsing, and action routing.

## Dependencies

- F1-F8 for v1 tests.
- F9/F10 tests deferred with those features.

---

## Phase F11.P1 — Unit Tests

### Task F11.P1.T1 — Model conversion tests

#### Description

Verify `AppNotification` converts to `ToastSpec` correctly.

#### Cases

- title only
- title + body
- success/warning/danger severity mapping
- action label mapping

---

### Task F11.P1.T2 — Store lifecycle tests

#### Description

Test push/dismiss/expire behavior.

#### Cases

- auto-dismiss expires after deadline
- sticky does not expire
- dismiss removes exact id
- next expiry returns earliest expiry

---

### Task F11.P1.T3 — Dedup tests

#### Description

Ensure dedup updates existing notification.

#### Cases

- same dedup key keeps same id
- body/title update
- timeout resets
- notification without key does not dedup

---

### Task F11.P1.T4 — Config parsing tests

#### Description

Ensure new config mode parses and defaults correctly.

#### Cases

- `app`
- `system`
- `none`
- missing field defaults to `app`
- invalid value errors

---

## Phase F11.P2 — Integration / Behavior Tests

### Task F11.P2.T1 — Reload config success/failure smoke tests

#### Description

Verify reload config producer creates expected notification.

#### Notes

May require extracting reload application into a testable function if current `HecaApp` setup is too heavy.

#### Avoid

- Do not build brittle GPU/window tests for notification store logic.

---

### Task F11.P2.T2 — Toast action dispatch test

#### Description

Verify clicking/triggering notification action dispatches expected `WmAction` through registry path.

#### Example

- create notification with `WmAction::ReloadConfig`;
- emit `NotificationAction` event;
- verify pending reload or registry handler effect.

#### Avoid

- Do not call handler directly in the test; test the event/registry path.

---

# Feature F12 — Documentation and Planner Handoff

## Feature Description

Keep the implementation discoverable and aligned with project rules.

## Dependencies

- F1-F8 for v1 docs.

---

## Phase F12.P1 — User Documentation

### Task F12.P1.T1 — README notification section

#### Description

Document:

- what notifications are;
- how to configure backend;
- that v1 uses in-app toasts;
- that OS/system backend is planned;
- that notification actions execute registered app actions.

#### Avoid

- Do not mention Modal/confirmation system.

---

### Task F12.P1.T2 — Widget docs update only if grid-ui changes

#### Description

If v1 only uses existing `ToastStack`, `docs/widgets.md` may not need changes. If `ToastSpec` changes later for hint fields, update docs.

#### Target File

- `docs/widgets.md`

#### Avoid

- Do not document future hint fields before they exist.

---

## Phase F12.P2 — Plan/Backlog Updates

### Task F12.P2.T1 — Add implementation slice to project backlog when approved

#### Description

After user approval, copy this plan into the active backlog/planner source of truth.

#### Candidate Files

- `PLAN.md`
- `BACKLOG.md`
- `.planner/` extension planner project, if used

#### Avoid

- Do not mark plan tasks done before implementation.

---

# Cross-Feature Dependencies

```text
F1 Notification Domain Model
  -> F2 Notification Store
  -> F3 ToastStack Rendering
  -> F4 Toast Action Dispatch
  -> F5 Reload Config Producer

F1 NotificationSystem
  -> F6 Config Integration
  -> F7 Routing
  -> F8 Producers respect routing

F7 Routing
  -> F9 OS Backend later

Future prefix+/ hint system
  -> F10 Toast KeyHint Integration
```

# Recommended Implementation Slices

## Slice 1 — Model + Config

Includes:

- F1.P1 core types
- F1.P2 toast adapter
- F6.P1 config schema
- F6.P2 minimal docs

Validation:

```bash
cargo test -p heca-config
cargo test -p heca notifications
```

## Slice 2 — Store + Lifecycle

Includes:

- F2.P1 store
- F2.P2 expiry scheduling
- F11.P1 store tests

Validation:

```bash
cargo test -p heca notifications
```

## Slice 3 — ToastStack Rendering

Includes:

- F3 app state + signals
- F3 ToastStack mount
- dismiss event bridge

Validation:

```bash
cargo check
cargo test -p heca
```

Manual:

- create dev/test notification;
- verify toast appears top-right;
- click outside toast passes through;
- click dismiss removes toast.

## Slice 4 — Toast Actions

Includes:

- F4 event handling
- action lookup
- dispatch through registry

Validation:

- notification action triggers `WmAction` via registry;
- no direct handler calls.

## Slice 5 — Reload Config Producer

Includes:

- F5 result-aware reload
- F8.P2 success/failure notifications

Manual:

- valid config reload shows success toast;
- invalid config reload shows sticky danger toast;
- Retry button triggers reload action;
- current config remains active after failure.

## Slice 6 — Additional Producers

Includes:

- pane exited
- spawn command failed
- provider/plugin errors

Only start after reload producer validates the end-to-end path.

# Global Avoid List

- Do not implement Modal/confirmation features in this plan.
- Do not create ad-hoc UI outside `heca-grid-ui` widgets.
- Do not put lifecycle/timer logic inside `heca-grid-ui`.
- Do not hardcode colors, sizes, opacity, fonts, or theme tokens in app code.
- Do not bypass `ActionRegistry` for toast actions.
- Do not make toast actions mouse-only once global hint mode exists.
- Do not implement OS notification backend before in-app backend is stable.
- Do not make rooms/agents notification producers before agent integration model lands.

# Acceptance Criteria

## v1 Acceptance

- `[settings].notification_system` supports `app`, `system`, and `none` parse modes.
- Default mode is `app`.
- In-app notification store supports push, dismiss, expiry, sticky notifications, dedup, and max visible.
- `ToastStack` renders active notifications using existing grid-ui widgets.
- Dismiss click removes notification through app event path.
- Action click dispatches a `WmAction` through the registered action path.
- Reload config success shows auto-dismiss success toast.
- Reload config failure shows sticky danger toast with Retry action.
- `notification_system = "none"` suppresses toasts.
- `notification_system = "system"` behavior is documented and deterministic until OS backend lands.
- Tests cover config parsing, store lifecycle, dedup, and toast adapter conversion.

## Deferred Acceptance

- OS backend sends real desktop notifications on supported platforms.
- Toast action/dismiss hints appear under global `prefix+/` hint mode.
- Toast actions are fully keyboard reachable via hint activation.

