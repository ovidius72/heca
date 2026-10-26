use heca_view::Intent;
use serde::{Deserialize, Serialize};

/// A notification action: a user-facing label, a serializable view intent, and how the button
/// reads. Field order — label, intent, variant — is the order the author thinks in: what they
/// read, what it does, how it looks.
///
/// The intent stores a name-keyed action id and optional `PropMap` arguments. It
/// is resolved only when activated through the app's `dispatch_view_intent`
/// path; this type never stores a closure and is not limited to `WmAction`.
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
        }
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
