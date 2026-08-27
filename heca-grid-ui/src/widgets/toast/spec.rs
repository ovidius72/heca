//! [`ToastAction`] and [`ToastSpec`] — the plain data a host hands the
//! [`ToastStack`](super::ToastStack).
//!
//! No callbacks and no widgets: a spec is a value the app can keep in its own list, compare,
//! de-duplicate and time out. The stack turns each one into a [`Toast`](super::Toast) and reports
//! interactions back by **id** and by the pressed action's **key**, so the host never holds a
//! widget handle and never a closure.

use crate::widgets::{ButtonVariant, Glyph, ToastSeverity};

/// One action offered on a notification — **plain data, never a closure.**
///
/// The `key` is what comes back when it is pressed ([`ToastStack::on_action`]), so it is the
/// caller's own name for the act (an action id, a `WmAction` name); the widget never parses it.
/// That is what lets the same declaration be answered by a click, by a `prefix+/` pick, and by an
/// RPC call, none of which could carry a `Box<dyn Fn()>`.
///
/// **The variant lives here, per action**, because a card with two actions rarely wants them to
/// read the same: "Retry" is primary and "Dismiss" is a ghost. The stack builds what the spec
/// says and picks no face of its own.
///
/// ```ignore
/// ToastSpec::new(1, "Build failed")
///     .action("rebuild", "Retry")
///     .action_with(ToastAction::new("open_log", "View log").variant(ButtonVariant::Ghost));
/// ```
///
/// [`ToastStack::on_action`]: super::ToastStack::on_action
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastAction {
    /// The caller's name for this act — reported back when it is pressed.
    pub key: String,
    /// What the control reads.
    pub label: String,
    /// How it reads at rest. Default [`ButtonVariant::Primary`].
    pub variant: ButtonVariant,
}

impl ToastAction {
    /// An action named `key`, labelled `label`, in the default variant.
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            variant: ButtonVariant::default(),
        }
    }

    /// How this one reads at rest — primary, secondary, ghost, and the rest of the vocabulary.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }
}

/// A single notification the host wants shown — plain data (no callbacks). The
/// host owns these in a `Signal<Vec<ToastSpec>>`; the stack renders them.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastSpec {
    /// Stable identity — used to reconcile widgets across frames and reported
    /// back by `on_dismiss`/`on_action`.
    pub id: u64,
    pub severity: ToastSeverity,
    pub icon: Option<Glyph>,
    pub title: String,
    pub body: Option<String>,
    /// **Every action this notification offers**, in the order they are shown. Empty means the
    /// card has no action row at all — and it then costs no space, because an unfilled slot leaves
    /// the layout rather than sitting empty.
    ///
    /// It is a `Vec` because a notification that can be retried *and* inspected needs two, and
    /// "one action" was a widget limitation written into the data rather than a real rule.
    pub actions: Vec<ToastAction>,
    /// Whether the × dismiss affordance is shown.
    pub dismissible: bool,
}

impl ToastSpec {
    /// A new info spec with `id` + `title`. Chain the setters for the rest.
    pub fn new(id: u64, title: impl Into<String>) -> Self {
        Self {
            id,
            severity: ToastSeverity::Info,
            icon: None,
            title: title.into(),
            body: None,
            actions: Vec::new(),
            dismissible: true,
        }
    }
    #[heca_grid_ui_macros::prop]
    pub fn severity(mut self, s: ToastSeverity) -> Self {
        self.severity = s;
        self
    }
    #[heca_grid_ui_macros::prop]
    pub fn icon(mut self, g: Glyph) -> Self {
        self.icon = Some(g);
        self
    }
    #[heca_grid_ui_macros::prop]
    pub fn body(mut self, b: impl Into<String>) -> Self {
        self.body = Some(b.into());
        self
    }

    /// **Append an action**, named `key` and labelled `label`, in the default variant.
    ///
    /// Sugar over [`action_with`](ToastSpec::action_with): it builds the very [`ToastAction`] you
    /// would have built, so there is one path and not two. Call it again for a second action.
    #[heca_grid_ui_macros::host_only("a list of composed values — a description uses `children`")]
    pub fn action(self, key: impl Into<String>, label: impl Into<String>) -> Self {
        self.action_with(ToastAction::new(key, label))
    }

    /// [`action`](ToastSpec::action) for a fully specified one — the form that carries a variant.
    #[heca_grid_ui_macros::host_only("a list of composed values — a description uses `children`")]
    pub fn action_with(mut self, action: ToastAction) -> Self {
        self.actions.push(action);
        self
    }

    #[heca_grid_ui_macros::prop]
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }
}
