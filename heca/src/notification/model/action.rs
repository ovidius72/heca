use heca_view::Intent;
use serde::{Deserialize, Serialize};

/// A notification action: a user-facing label, a serializable view intent, and how the button
/// reads. Field order — label, intent, variant — is the order the author thinks in: what they
/// read, what it does, how it looks.
///
/// The intent stores a name-keyed action id and optional `PropMap` arguments. It
/// is resolved only when activated through the app's `dispatch_view_intent`
/// path, and is not limited to `WmAction`.
///
/// **Or a closure**, for a one-off button that needs no public action ([`run`](Self::run)). The
/// closure stays in the process that raised the notification and lives in the notification store
/// for as long as the notification does: a notification that is dropped (a duplicate, `mode =
/// "none"`) drops it with it, so nothing is registered and nothing can leak. Pressing the button in
/// any window asks the server to run it.
/// `dismiss_after` controls the post-dispatch policy: keep it `false` for
/// retry-like actions, while dismissive actions close only according to the
/// dispatch result and never bypass `ActionRegistry`.
///
/// `variant` mirrors [`ToastAction`](heca_grid_ui::widgets::ToastAction)'s exact spelling — one
/// convention across the app and widget layers, not two. Crosses the wire as its **name**
/// (`button_variant_as_name`), never the widget enum itself: a variant is semantic intent, like
/// severity already is, and the host resolves the name — the same rule `docs/widgets.md` states
/// for every description. An unknown name on deserialize degrades to the default rather than
/// failing, matching how the rest of this model treats untrusted input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationAction {
    /// Label rendered on the action affordance.
    pub label: String,
    /// Name-keyed action intent with optional arguments.
    pub intent: Intent,
    /// How the button reads at rest. Default [`ButtonVariant::Primary`](heca_grid_ui::widgets::ButtonVariant::Primary).
    #[serde(with = "button_variant_as_name")]
    pub variant: heca_grid_ui::widgets::ButtonVariant,
    /// Whether a successful activation may dismiss the notification.
    pub dismiss_after: bool,
    /// `true` for a button made with [`run`](Self::run): pressing it runs the closure where the
    /// notification was raised, instead of dispatching `intent`. Crosses the wire so every window
    /// knows to ask for that.
    #[serde(default)]
    pub runs: bool,
    /// The closure itself. Never serialized: it stays in the process that raised the notification.
    #[serde(skip)]
    run: Option<RunAction>,
}

/// A button's closure, shared by the copies of its notification. Two are equal only when they are
/// the same closure.
#[derive(Clone)]
struct RunAction(std::sync::Arc<dyn Fn() + Send + Sync>);

impl std::fmt::Debug for RunAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RunAction")
    }
}

impl PartialEq for RunAction {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Serde shim: a [`ButtonVariant`](heca_grid_ui::widgets::ButtonVariant) crosses the wire as its
/// snake_case name (`"primary"`, `"ghost"`, …), never the widget enum — a widget type has no
/// business in serialized app data. Uses the enum's own [`PropName`](heca_grid_ui::PropName)
/// vocabulary (`from_prop_name`) for decoding, so the accepted spellings are the same ones
/// `docs/widgets.md`'s description model already uses; there is no second name for the same
/// variant. An unknown name degrades to the default, matching every other untrusted-input path
/// in this model.
mod button_variant_as_name {
    use heca_grid_ui::PropName;
    use heca_grid_ui::widgets::ButtonVariant;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &ButtonVariant, s: S) -> Result<S::Ok, S::Error> {
        let name = match v {
            ButtonVariant::Primary => "primary",
            ButtonVariant::Secondary => "secondary",
            ButtonVariant::Destructive => "destructive",
            ButtonVariant::Outline => "outline",
            ButtonVariant::Ghost => "ghost",
            ButtonVariant::Link => "link",
        };
        s.serialize_str(name)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<ButtonVariant, D::Error> {
        let name = String::deserialize(d)?;
        Ok(ButtonVariant::from_prop_name(&name).unwrap_or_default())
    }
}

impl NotificationAction {
    /// Create a non-dismissing action (appropriate for Retry), default variant.
    pub fn new(label: impl Into<String>, intent: Intent) -> Self {
        Self {
            label: label.into(),
            intent,
            variant: heca_grid_ui::widgets::ButtonVariant::default(),
            dismiss_after: false,
            runs: false,
            run: None,
        }
    }

    /// **A one-off button that runs a closure** — no public action to declare:
    ///
    /// ```ignore
    /// Notification::info("Agent finished")
    ///     .action(NotificationAction::run("Open pane", move || open(pane)))
    ///     .send();
    /// ```
    ///
    /// Raise it from server code (an extension service): the closure stays on the server and the
    /// button works in every window of the session. It lives as long as the notification does — a
    /// dropped or duplicate notification drops it. Each button gets its own name
    /// (`notification_run.<n>`), so several on one notification never collide.
    pub fn run(label: impl Into<String>, closure: impl Fn() + Send + Sync + 'static) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self {
            runs: true,
            run: Some(RunAction(std::sync::Arc::new(closure))),
            ..Self::new(label, Intent::new(format!("notification_run.{n}")))
        }
    }

    /// Let go of this button's closure — the notification left the screen and its button can no
    /// longer be pressed.
    pub(crate) fn release_closure(&mut self) {
        self.run = None;
    }

    /// Run this button's closure, if it has one here. `false` when there is none — a button made
    /// with [`run`](Self::run) in a window, whose closure did not travel to the server.
    pub(crate) fn run_closure(&self) -> bool {
        let Some(RunAction(closure)) = &self.run else { return false };
        closure();
        true
    }

    /// Set how the button reads. Mirrors `ToastAction::variant`.
    pub fn variant(mut self, variant: heca_grid_ui::widgets::ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set whether successful activation may dismiss the notification.
    pub fn dismiss_after(mut self, dismiss: bool) -> Self {
        self.dismiss_after = dismiss;
        self
    }

    /// Convenience constructor for an action that dismisses on successful dispatch.
    pub fn dismissing(label: impl Into<String>, intent: Intent) -> Self {
        Self::new(label, intent).dismiss_after(true)
    }
}
