use super::*;
use std::time::Instant;
use heca_grid_ui::widgets::ToastSpec;
use heca_view::Intent;

/// The host's notification state and timers, with a retained toast projection.
///
/// Owns the [`NotificationStore`] and plain presentation data. It does not own a
/// window or a reactive signal. [`sync_visible_toasts`](Self::sync_visible_toasts)
/// refreshes the projection after each mutation that can change the visible set.
pub struct NotificationRuntime {
    store: NotificationStore,
    /// The cards on show, as plain data — what [`visible_toasts`](Self::visible_toasts) hands a
    /// window, kept to tell when the set changed. The store owns no signal: a window turns this list
    /// into whatever it draws.
    shown: Vec<ToastSpec>,
    /// The configured default auto-dismiss delay (`[settings.notification_system]
    /// auto_dismiss_ms`). Applied only when the producer omitted a lifetime.
    /// Explicit durations and sticky lifetimes are unaffected. Re-applied on config reload
    /// ([`set_auto_dismiss`](Self::set_auto_dismiss)).
    auto_dismiss: std::time::Duration,
    /// Delivery mode (`[settings.notification_system] mode`). `None` suppresses every
    /// raise; `App` and `System` both deliver to the in-app toast stack today (the OS
    /// backend that would make `System` distinct is not built). Re-applied on reload.
    mode: heca_config::settings::NotificationSystem,
    /// Whether the one-time "`system` mode has no OS backend yet" notice has already been
    /// shown for the current mode. Reset whenever [`set_mode`](Self::set_mode) changes the mode.
    system_fallback_announced: bool,
    /// **When the pointer came to rest on a card**, or `None` when it is not on one.
    ///
    /// The stack reports the *fact* through a signal (it has no clock and no idea what a card's
    /// lifetime is); this is where that fact becomes time. See [`set_hovered`](Self::set_hovered).
    hovered_since: Option<Instant>,
}

impl NotificationRuntime {
    pub fn new(
        history_limit: usize,
        auto_dismiss: std::time::Duration,
        mode: heca_config::settings::NotificationSystem,
    ) -> Self {
        Self::with_capacity(
            history_limit,
            auto_dismiss,
            mode,
            heca_config::settings::NotificationSystemConfig::default().max_visible,
        )
    }

    /// The same, with the configured number of simultaneous cards
    /// (`[settings.notification_system] max_visible`).
    pub fn with_capacity(
        history_limit: usize,
        auto_dismiss: std::time::Duration,
        mode: heca_config::settings::NotificationSystem,
        max_visible: usize,
    ) -> Self {
        Self {
            store: NotificationStore::with_capacity(history_limit, max_visible),
            shown: Vec::new(),
            auto_dismiss,
            mode,
            system_fallback_announced: false,
            hovered_since: None,
        }
    }

    /// Apply a new delivery mode (`prefix+Shift+r` reload path).
    pub(crate) fn set_mode(&mut self, mode: heca_config::settings::NotificationSystem) {
        if self.mode != mode {
            self.system_fallback_announced = false;
        }
        self.mode = mode;
    }

    /// Whether notifications are suppressed entirely (`mode = "none"`).
    pub(crate) fn suppressed(&self) -> bool {
        matches!(self.mode, heca_config::settings::NotificationSystem::None)
    }

    /// The one-time notice that `mode = "system"` has no OS backend yet and is falling back to
    /// the in-app stack. `Some` the first time it is asked while the mode is
    /// `System`, `None` after (until the mode changes). `App`/`None` never produce it.
    pub(crate) fn system_fallback_notice(&mut self) -> Option<NotificationDraft> {
        if self.mode != heca_config::settings::NotificationSystem::System
            || self.system_fallback_announced
        {
            return None;
        }
        self.system_fallback_announced = true;
        Some(
            NotificationDraft::new("OS notifications aren't available yet")
                .body("Showing notifications in-app until the system backend lands.")
                .dedup_key("notification.system.unavailable"),
        )
    }

    /// Apply a new configured auto-dismiss delay — the `prefix+Shift+r` reload path.
    /// Only affects notifications raised afterwards; anything already visible keeps
    /// the deadline it was promoted with.
    pub(crate) fn set_auto_dismiss(&mut self, auto_dismiss: std::time::Duration) {
        self.auto_dismiss = auto_dismiss;
    }

    /// Accept a draft, stamping `now` itself. **Not public.** The only public raise door is
    /// [`Notification::send`] — this is the deterministic primitive it (and tests) call into.
    /// A producer or a plugin author must never reach this directly: they cannot anyway (the
    /// field is host-private), and this is what keeps it that way on purpose.
    ///
    /// The configured [`auto_dismiss`](Self::auto_dismiss) delay is applied here, at the
    /// runtime boundary, so the store stays a pure `(draft, now)` primitive.
    pub(crate) fn push(
        &mut self,
        draft: NotificationDraft,
        now: Instant,
    ) -> Result<NotificationStorePush, NotificationStoreError> {
        let draft = draft.with_default_auto_dismiss(self.auto_dismiss);
        let result = self.store.push(draft, now);
        self.sync_visible_toasts();
        result
    }

    /// `NotificationDismissOne { id }`.
    pub fn dismiss_one(&mut self, id: NotificationId, now: Instant) -> NotificationDismissResult {
        let result = self.store.dismiss(id, now);
        self.sync_visible_toasts();
        result
    }

    /// `NotificationDismissLast` — dismiss the first eligible visible notification in stable
    /// toast order. A no-op (not an error) when nothing is eligible.
    pub fn dismiss_last(&mut self, now: Instant) -> NotificationDismissResult {
        let Some(id) = self.store.first_auto_dismissible_visible_id() else {
            return NotificationDismissResult::Unknown;
        };
        self.dismiss_one(id, now)
    }

    /// `NotificationDismissAll` — dismiss every currently visible notification. Returns whether
    /// anything actually changed (so the caller can skip an unnecessary redraw).
    pub fn dismiss_all(&mut self, now: Instant) -> bool {
        let ids: Vec<NotificationId> = self.store.visible().map(|n| n.id).collect();
        let mut changed = false;
        for id in ids {
            if matches!(
                self.store.dismiss(id, now),
                NotificationDismissResult::Dismissed(_)
            ) {
                changed = true;
            }
        }
        self.sync_visible_toasts();
        changed
    }

    /// Resolve the `Intent` behind `on_action(id, key)` — `key` is the action's own
    /// `intent.action` name (`AppNotification::action_for_visible`).
    pub fn action_intent_for_visible(&self, id: NotificationId, key: &str) -> Option<Intent> {
        self.store.action_intent_for_visible(id, key)
    }

    /// The action's `Intent` and `dismiss_after` flag together — what the relay handler needs
    /// to both fire the intent and decide whether to dismiss afterward, in one lookup.
    pub fn action_and_dismiss_after_for_visible(
        &self,
        id: NotificationId,
        key: &str,
    ) -> Option<(Intent, bool)> {
        self.store
            .action_for_visible(id, key)
            .map(|action| (action.intent.clone(), action.dismiss_after))
    }

    /// The next auto-dismiss deadline, for the lifecycle loop's wake scheduling.
    pub fn next_expiry(&self) -> Option<Instant> {
        self.store.next_expiry()
    }

    /// **Hold the stack still while the pointer rests on it, and give back the time it cost.**
    ///
    /// Driven by the signal [`ToastStack::hovered_signal`](heca_grid_ui::widgets::ToastStack::hovered_signal)
    /// writes: the widget reports whether the pointer is on a card, and the meaning of that — that
    /// a card being read must not retire under the cursor — is decided here, where the lifetime
    /// lives. Idempotent, so the caller may pass the same answer every frame.
    ///
    /// On release every visible deadline moves forward by however long the hover lasted, so each
    /// card resumes with the time it had left rather than the time it started with. Returns whether
    /// the deadlines moved, so the caller can re-arm its wake.
    ///
    /// **The hold is not a sticky-ness.** A card the user never hovers is unaffected, and one they
    /// hover and leave goes back to expiring — this only removes the case where the countdown ran
    /// while they were reaching for the button it was counting down to.
    pub fn set_hovered(&mut self, hovered: bool, now: Instant) -> bool {
        match (hovered, self.hovered_since) {
            // Resting on a card, and already known to be: the deadlines are frozen by not being
            // swept, so there is nothing to do until the pointer leaves.
            (true, Some(_)) => false,
            (true, None) => {
                self.hovered_since = Some(now);
                false
            }
            (false, Some(since)) => {
                self.hovered_since = None;
                self.store
                    .extend_visible_deadlines(now.saturating_duration_since(since))
            }
            (false, None) => false,
        }
    }

    /// Whether the stack is currently held still by the pointer resting on it.
    pub fn is_hovered(&self) -> bool {
        self.hovered_since.is_some()
    }

    /// Apply a new `max_visible` (`prefix+Shift+r`). Returns whether the projection changed.
    pub fn set_max_visible(&mut self, max_visible: usize, now: Instant) -> bool {
        let changed = self.store.set_max_visible(max_visible, now);
        if changed {
            self.sync_visible_toasts();
        }
        changed
    }

    /// Auto-dismiss everything past its deadline. Call from the lifecycle tick.
    ///
    /// **A no-op while the pointer rests on the stack** — that is the whole of the hold, and doing
    /// it here rather than at the call site means every caller gets it, including a future one.
    pub fn expire_due(&mut self, now: Instant) -> bool {
        if self.is_hovered() {
            return false;
        }
        let update = self.store.expire_due(now);
        let changed = update.visible_projection_changed();
        if changed {
            self.sync_visible_toasts();
        }
        changed
    }

    /// Push the store's current visible projection into the retained signal, if it changed.
    /// Returns whether it did (callers use this to decide whether to request a redraw).
    fn sync_visible_toasts(&mut self) -> bool {
        let next = self.store.visible_toasts();
        if self.shown == next {
            return false;
        }
        self.shown = next;
        true
    }

    /// **The cards on show**, in slot order — plain data a window draws from. Read it when the server
    /// reports [`NotificationsChanged`](crate::server::Change::NotificationsChanged).
    pub fn visible_toasts(&self) -> Vec<ToastSpec> {
        self.shown.clone()
    }
}

#[cfg(test)]
mod tests;
