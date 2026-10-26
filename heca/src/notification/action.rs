use super::{NotificationDraft, NotificationSeverity, raise};

/// Register the `notify` action — the raise capability.
///
/// Dismissing a notification was always a real action (`notification_dismiss_one`/`_all`/`_last`,
/// [`crate::input::WmAction`] variants); raising one was not — [`NotificationRuntime::push`](super::NotificationRuntime::push) is a
/// plain Rust method, unreachable from RPC, a keybinding, or a future WASM plugin. This closes that
/// gap the same way any provider's own action would: name-keyed, via
/// [`register_dynamic`](crate::actions::register_dynamic), **not** a `WmAction` variant — a WASM
/// plugin can only ever dispatch by name, never add an enum variant to a closed enum, and "any
/// plugin can raise a notification" is the point.
///
/// No action-button argument in v1 — deliberate scope, not a widget limitation: `ToastSpec`
/// already supports several typed actions per card, but there is no argument kind for a
/// nested `Intent`, so a remote caller has no typed way to say "and run this other action with
/// these args" as one argument value. A Rust-internal caller that wants a button uses
/// [`NotificationDraft::action`] directly with a real `Intent` it already holds — unaffected by
/// this restriction.
///
/// Call once, at startup, from wherever the host's own built-ins would register alongside mounted
/// components (`HecaApp::resumed`, after `providers::register_provider_actions` — this is a host
/// action, but it takes the identical name-keyed door because `WmAction` cannot express it).
pub fn register_notify_action(
    registry: &mut crate::actions::ActionRegistry,
    catalog: &mut crate::actions::ActionCatalog,
) -> Result<crate::actions::ActionHandle, crate::actions::DuplicateAction> {
    use crate::actions::ActionMeta;
    use crate::args::EnumArg;
    use crate::args::{ArgKind, ArgSpec};

    crate::actions::register_dynamic(
        registry,
        catalog,
        ActionMeta {
            name: "notify".into(),
            label: "Raise Notification".into(),
            description: "Raise an in-app toast notification. The only door RPC, a keybinding, \
                or a plugin has to the notification system — a Rust caller with a real Intent in hand uses \
                NotificationDraft::action directly instead."
                .into(),
            category: crate::actions::ActionCategory::Chrome,
            owner: None,
            icon: None,
            policy: crate::app::interaction::ActionPolicy::Global,
            args: vec![
                ArgSpec {
                    name: "title".into(),
                    kind: ArgKind::Text,
                    required: true,
                    description: "The notification's title.".into(),
                    values: Vec::new(),
                },
                ArgSpec {
                    name: "body".into(),
                    kind: ArgKind::Text,
                    required: false,
                    description: "Optional body text.".into(),
                    values: Vec::new(),
                },
                ArgSpec {
                    name: "severity".into(),
                    kind: ArgKind::Enum,
                    required: false,
                    description: "info (default) | success | warning | danger.".into(),
                    values: NotificationSeverity::VALUES
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                },
                ArgSpec {
                    name: "dedup_key".into(),
                    kind: ArgKind::Text,
                    required: false,
                    description: "Optional key: a second `notify` with the same key updates the \
                        existing notification in place instead of stacking a new one."
                        .into(),
                    values: Vec::new(),
                },
            ],
            confirm: None,
        },
        Some(std::rc::Rc::new(|state, intent| {
            let Some(heca_view::PropValue::Text(title)) = intent.args.get("title") else {
                return;
            };
            let mut draft = NotificationDraft::new(title.clone());
            if let Some(heca_view::PropValue::Text(body)) = intent.args.get("body") {
                draft = draft.body(body.clone());
            }
            if let Some(heca_view::PropValue::Text(severity)) = intent.args.get("severity")
                && let Ok(severity) = severity.parse::<NotificationSeverity>()
            {
                draft = draft.severity(severity);
            }
            if let Some(heca_view::PropValue::Text(key)) = intent.args.get("dedup_key") {
                draft = draft.dedup_key(key.clone());
            }
            raise(state, draft);
        })),
    )
}
