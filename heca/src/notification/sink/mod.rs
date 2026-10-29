use super::{NotificationAction, NotificationDraft};

/// The callback [`Notification::send`] posts a draft through, once the host installs it via
/// [`install_notification_sink`].
type NotificationSink = Box<dyn Fn(NotificationDraft)>;

std::thread_local! {
    /// Host-installed hook `Notification::send` posts a draft through — the notification
    /// equivalent of `heca_grid_ui::install_frame_request`/`request_frame`. Thread-local because
    /// the app is single-threaded; installed once at startup (`app::startup`), wired to post
    /// `AppEvent::RaiseNotification` so the host reads `Instant::now()` and calls the
    /// crate-private `NotificationRuntime::push` on the event loop's own turn — never the
    /// caller's. A no-op until installed (e.g. in a headless test), so `Notification::send` is
    /// always safe to call.
    static NOTIFICATION_SINK: std::cell::RefCell<Option<NotificationSink>> =
        const { std::cell::RefCell::new(None) };
}

/// Install the callback [`Notification::send`] posts a draft through. Call once, at startup.
pub fn install_notification_sink(f: impl Fn(NotificationDraft) + 'static) {
    NOTIFICATION_SINK.with(|c| *c.borrow_mut() = Some(Box::new(f)));
}

/// Bind native notifications raised during one server request to that request's session.
/// The guard also restores the previous sink during unwinding and nested collection.
pub(crate) fn collect_during<T>(work: impl FnOnce() -> T) -> (T, Vec<NotificationDraft>) {
    use std::{cell::RefCell, rc::Rc};
    struct Restore(Option<NotificationSink>);
    impl Drop for Restore {
        fn drop(&mut self) { NOTIFICATION_SINK.with(|c| *c.borrow_mut() = self.0.take()); }
    }
    let drafts = Rc::new(RefCell::new(Vec::new()));
    let pending = drafts.clone();
    let previous = NOTIFICATION_SINK.with(|c| c.replace(Some(Box::new(move |draft| pending.borrow_mut().push(draft)))));
    let restore = Restore(previous);
    let result = work();
    drop(restore);
    let collected = std::mem::take(&mut *drafts.borrow_mut());
    (result, collected)
}

#[cfg(test)]
mod collection_tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn nested_collection_and_unwinding_restore_the_previous_session_sink() {
        let received = Rc::new(RefCell::new(Vec::new()));
        let sink = received.clone();
        install_notification_sink(move |draft| sink.borrow_mut().push(draft));
        let (_, outer) = collect_during(|| {
            Notification::info("outer").send();
            let (_, inner) = collect_during(|| Notification::info("inner").send());
            assert_eq!(inner.len(), 1);
            assert!(std::panic::catch_unwind(|| collect_during(|| panic!("request failed"))).is_err());
            Notification::info("outer again").send();
        });
        assert_eq!(outer.len(), 2);
        Notification::info("original").send();
        assert_eq!(received.borrow().len(), 1);
        NOTIFICATION_SINK.with(|sink| { sink.borrow_mut().take(); });
    }
}

/// The host accepting a draft into the store.
///
/// **The one place `Instant::now()` is read for a raised notification, and the only non-test
/// caller of the crate-private [`NotificationRuntime::push`](super::NotificationRuntime::push).** Both raise doors converge here:
///
/// - [`Notification::send`] → the installed sink → `AppEvent::RaiseNotification` → this, on the
///   event loop's own turn;
/// - the name-keyed `notify` action (RPC / keybinding / WASM plugin) → this, from its handler.
///
/// A producer never reaches `push` directly: this is what keeps the caller's clock and the
/// store's internal verb off the public surface.
pub(crate) fn raise(state: &mut crate::app_state::AppState, draft: NotificationDraft) {
    // Over the wire to the server, which owns the queue, timers and dedup. It drops the draft under
    // `mode = "none"` and falls back to the in-app stack under `system` — both are its rules, not
    // the window's — then sends the new projection back for the window to draw.
    state.client.notify(crate::server::NotificationCommand::Raise(draft));
}

/// **The raise capability — the one public door.** A native producer and an extension author type
/// the identical line; there is no second, lower-level path a producer can reach (the store, and
/// `NotificationRuntime::push`'s clock, are both host-private on purpose — see `push`'s own doc).
///
/// ```
/// use heca::Notification;
/// use std::time::Duration;
///
/// Notification::info("Build finished")
///     .body("3 warnings in heca-grid-ui")
///     .auto_dismiss(Duration::from_secs(10))
///     .send();
/// ```
///
/// The duration is optional: omit `auto_dismiss` to use the configured default.
/// There is no delivery delay or caller timestamp; the host owns acceptance time.
///
/// A closure-based one-off action (`.action(label, variant, || { .. })`, registering an
/// anonymous name-keyed action behind the scenes) is deliberately **not yet built** — it needs
/// its own careful design around when the anonymous action registers and unregisters (it must
/// register only once the draft is actually accepted into the store, so a
/// deduped or rejected notification never leaks a handler). Only the named `NotificationAction`
/// form exists today.
#[derive(Debug, Clone, PartialEq)]
pub struct Notification {
    draft: NotificationDraft,
}

impl Notification {
    pub fn info(title: impl Into<String>) -> Self {
        Self {
            draft: NotificationDraft::info(title),
        }
    }
    pub fn success(title: impl Into<String>) -> Self {
        Self {
            draft: NotificationDraft::success(title),
        }
    }
    pub fn warning(title: impl Into<String>) -> Self {
        Self {
            draft: NotificationDraft::warning(title),
        }
    }
    pub fn danger(title: impl Into<String>) -> Self {
        Self {
            draft: NotificationDraft::danger(title),
        }
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.draft = self.draft.body(body);
        self
    }

    pub fn dedup_key(mut self, key: impl Into<String>) -> Self {
        self.draft = self.draft.dedup_key(key);
        self
    }

    /// Override the configured auto-dismiss duration for this notification.
    /// Omit this method to use `[settings.notification_system] auto_dismiss_ms`.
    /// The timer starts when the toast becomes visible, not when it is sent.
    pub fn auto_dismiss(mut self, duration: std::time::Duration) -> Self {
        self.draft = self.draft.auto_dismiss(duration);
        self
    }

    /// Stay until dismissed — no auto-dismiss timer. The opt-in for a notification that must
    /// persist (an error to acknowledge, a long-running status); everything else auto-dismisses
    /// after its explicit duration or `[settings.notification_system] auto_dismiss_ms`.
    pub fn sticky(mut self) -> Self {
        self.draft = self.draft.sticky();
        self
    }

    /// Append an inline action. Callable more than once, in order — matches `actions: Vec<..>`.
    pub fn action(mut self, action: NotificationAction) -> Self {
        self.draft = self.draft.action(action);
        self
    }

    /// Raise it. Posts through the host-installed sink; a no-op if nothing installed it (a
    /// headless test, or a call before startup finishes installing it).
    pub fn send(self) {
        NOTIFICATION_SINK.with(|c| {
            if let Some(f) = c.borrow().as_ref() {
                f(self.draft);
            }
        });
    }
}

#[cfg(test)]
mod tests;
