use serde::{Deserialize, Serialize};
use heca_grid_ui::widgets::ToastSeverity;

/// **Who sees a notification** when several windows share one session.
///
/// A notification raised in response to a window's own action (a config reload, a rename) is
/// feedback for *that* window; an ambient event (a background process finished) belongs to the
/// whole session. The producer declares which; the default is the whole session, so nothing a
/// caller forgets to mark goes missing.
///
/// The client cannot name a window — it does not know its own connection id — so it raises with
/// [`Originator`](Self::Originator) and the server resolves it to [`Window`](Self::Window) for the
/// connection the request arrived on. Delivery filtering is the server's
/// (`visible_views_for`): the window never learns there was a choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    /// Every window on the session — the default, for an ambient event.
    #[default]
    Session,
    /// Only the window that raised it. The client sets this; the server resolves it to
    /// [`Window`](Self::Window) for the originating connection.
    Originator,
    /// Only the window on this server-assigned connection id — the resolved form of
    /// [`Originator`](Self::Originator). A client never builds it.
    Window(u64),
}

impl Audience {
    /// Whether the window on connection `client` sees a notification with this audience.
    pub fn allows(self, client: u64) -> bool {
        match self {
            // An unresolved `Originator` means no connection was named yet; show it rather than
            // hide it, so a missed resolution can never silently swallow a notification.
            Self::Session | Self::Originator => true,
            Self::Window(id) => id == client,
        }
    }
}

/// The closed vocabulary of notification origins.
///
/// Extensible in the sense that a new variant is an additive, non-breaking
/// change for matching code that uses a non-exhaustive `_` arm; builders and
/// serialization treat it as a stable enum. Each kind pairs with an opaque
/// [`id`](NotificationSource::id) string that disambiguates within the kind
/// (a plugin id, a command name, a pane handle rendered as a string, …).
///
/// Notably **does not** include an `Agent` variant — agent-originated
/// notifications belong to a separate feature, not the core notification
/// domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSourceKind {
    /// Produced by heca itself (e.g. config reload result, internal status).
    App,
    /// Produced by a config-driven rule or default (e.g. a startup hint).
    Config,
    /// Produced on behalf of a tiled or floating pane (e.g. a bell, exit
    /// status). The pane is identified by an opaque handle string, not an
    /// index, because indices become stale after archival.
    Pane,
    /// Produced by a registered chrome provider (docker, git, …).
    Provider,
    /// Produced by a loaded plugin (future WASM host). The plugin id is the
    /// opaque `id`.
    Plugin,
    /// Produced by a user-invoked command (e.g. `:notify`, a bound action).
    Command,
}

impl Default for NotificationSourceKind {
    /// The default source is the app itself; explicit sources override it.
    fn default() -> Self {
        Self::App
    }
}

impl NotificationSourceKind {
    /// Human-readable label for log/history lines and dedup keys. Lowercase,
    /// stable, no spaces — suitable as the first segment of a dedup key.
    pub fn as_label(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Config => "config",
            Self::Pane => "pane",
            Self::Provider => "provider",
            Self::Plugin => "plugin",
            Self::Command => "command",
        }
    }
}

/// A descriptive, extensible origin for a notification.
///
/// Used for log lines, notification history, and dedup keys. It carries **no**
/// routing or [`ActionPolicy`](crate::app::interaction) semantics — a
/// `Pane` source does not by itself make a notification pane-scoped or
/// floating-only; policy is decided by the action the notification carries,
/// not by where it came from.
///
/// The `id` is an opaque, caller-supplied string. For `Pane` it should be a
/// stable pane handle (not an index); for `Provider`/`Plugin` the registered
/// id; for `Command` the command name; for `App`/`Config` it may be a subsystem
/// tag or be left empty. It is never parsed by the notification domain.
///
/// # Examples
///
/// Not run as a test: the `notification` module is private, so this cannot compile from outside.
///
/// ```ignore
/// use heca::notification::{NotificationSource, NotificationSourceKind};
///
/// let plugin = NotificationSource::new(NotificationSourceKind::Plugin, "git-watch");
/// let pane = NotificationSource::new(NotificationSourceKind::Pane, "pane-4f2a");
/// let app = NotificationSource::app("config");
///
/// assert_eq!(plugin.kind(), NotificationSourceKind::Plugin);
/// assert_eq!(plugin.id(), Some("git-watch"));
/// assert_eq!(app.kind(), NotificationSourceKind::App);
/// // dedup key first segment is the kind label:
/// assert_eq!(pane.kind().as_label(), "pane");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationSource {
    kind: NotificationSourceKind,
    /// Opaque, caller-supplied identifier within `kind`. Empty means
    /// "unspecified"; never parsed by the notification domain.
    id: Option<String>,
}

impl NotificationSource {
    /// A source of `kind` with an opaque identifier.
    pub fn new(kind: NotificationSourceKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: Some(id.into()),
        }
    }

    /// A source of `kind` with no specific identifier.
    pub fn of(kind: NotificationSourceKind) -> Self {
        Self { kind, id: None }
    }

    /// An `App` source tagged with an optional subsystem name.
    pub fn app(subsystem: impl Into<String>) -> Self {
        Self::new(NotificationSourceKind::App, subsystem)
    }

    /// The origin kind.
    pub fn kind(&self) -> NotificationSourceKind {
        self.kind
    }

    /// The opaque identifier within the kind, if any.
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// A stable, non-empty dedup key segment: `<kind>[:<id>]`. Suitable as the
    /// first segment of a full dedup key; later tasks may append a sub-key.
    pub fn dedup_segment(&self) -> String {
        match &self.id {
            Some(id) if !id.is_empty() => format!("{}:{}", self.kind.as_label(), id),
            _ => self.kind.as_label().to_string(),
        }
    }
}

impl Default for NotificationSource {
    /// Defaults to an unspecified `App` source, matching
    /// [`NotificationSourceKind::default`].
    fn default() -> Self {
        Self::of(NotificationSourceKind::default())
    }
}

/// How long a notification stays visible once the store promotes it to the
/// active queue.
///
/// This is the *configured* lifetime only — it carries no `Instant` and no
/// `expires_at`. The host store computes `expires_at` lazily, at the moment it
/// promotes a notification to visible: the timer starts when the store promotes
/// the notification to visible, not at push time.
/// Keeping the instant out of this type lets the domain builder stay pure and
/// deterministic in tests.
///
/// `Sticky` notifications never expire on their own but can still be closed by
/// the user when [`dismissible`](NotificationLifecycle::dismissible) is true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationLifetime {
    /// Auto-dismiss after the given number of seconds once visible.
    Auto(std::time::Duration),
    /// Stay until explicitly dismissed (or replaced). Does not expire.
    Sticky,
}

impl Default for NotificationLifetime {
    /// The configuration fallback. The runtime applies the user's current setting
    /// when a draft has no explicit lifetime.
    fn default() -> Self {
        Self::Auto(std::time::Duration::from_millis(
            heca_config::settings::NotificationSystemConfig::default().auto_dismiss_ms,
        ))
    }
}

/// Lifecycle policy for a notification: when it leaves the active queue and
/// whether the user can close it.
///
/// Split from [`NotificationLifetime`] because dismissibility is orthogonal
/// to lifetime: a `Sticky` notification can be dismissible or not, and an
/// `Auto` notification can be made non-dismissible (it then expires on its own
/// but cannot be closed early).
///
/// Carries no `Instant`. The host store computes expiry lazily on promotion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationLifecycle {
    lifetime: NotificationLifetime,
    /// Whether the × dismiss affordance is shown and the notification can be
    /// closed by the user. Sticky + non-dismissible = stays until replaced.
    dismissible: bool,
}

impl NotificationLifecycle {
    /// A lifecycle with the given lifetime that is dismissible.
    pub fn new(lifetime: NotificationLifetime) -> Self {
        Self {
            lifetime,
            dismissible: true,
        }
    }

    /// A dismissible, auto-dismiss lifecycle.
    pub fn auto(duration: std::time::Duration) -> Self {
        Self::new(NotificationLifetime::Auto(duration))
    }

    /// A dismissible sticky lifecycle (stays until closed).
    pub fn sticky() -> Self {
        Self::new(NotificationLifetime::Sticky)
    }

    /// A non-dismissible sticky lifecycle (stays until replaced).
    pub fn sticky_persistent() -> Self {
        Self {
            lifetime: NotificationLifetime::Sticky,
            dismissible: false,
        }
    }

    /// Set whether the user can dismiss this notification.
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }

    /// The configured lifetime (no `expires_at`; computed lazily on promotion).
    pub fn lifetime(&self) -> NotificationLifetime {
        self.lifetime
    }

    /// Whether the user can close this notification.
    pub fn is_dismissible(&self) -> bool {
        self.dismissible
    }

    /// Whether this notification expires on its own (only `Auto`).
    pub fn expires(&self) -> bool {
        matches!(self.lifetime, NotificationLifetime::Auto(_))
    }

    /// Replace the auto-dismiss duration **only if** this lifecycle auto-dismisses.
    /// A `Sticky` lifecycle is returned unchanged. This is how the host applies the
    /// configured default after the draft has checked for an explicit lifetime.
    pub(crate) fn with_auto_duration(self, duration: std::time::Duration) -> Self {
        match self.lifetime {
            NotificationLifetime::Auto(_) => Self {
                lifetime: NotificationLifetime::Auto(duration),
                ..self
            },
            NotificationLifetime::Sticky => self,
        }
    }
}

impl Default for NotificationLifecycle {
    /// Defaults to a dismissible auto-dismiss lifecycle using the config fallback.
    fn default() -> Self {
        Self::new(NotificationLifetime::default())
    }
}

// Notifications do NOT vary their lifetime by severity. Severity is tone,
// not importance. An explicit lifetime wins; otherwise the configured default applies.

/// Canonical app-owned notification severity.
///
/// This enum is deliberately independent of the grid-ui presentation enum;
/// severity defines tone while the adapter
/// [`NotificationSeverity::as_toast_severity`] maps it to paint-time tokens.
/// It does not reorder notifications already visible in the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSeverity {
    Error,
    Warning,
    Success,
    #[default]
    Info,
}

impl NotificationSeverity {
    /// Convert to the presentation severity used by `ToastSpec`/`Toast`.
    ///
    /// This is total and exhaustive: adding a new domain severity requires an
    /// explicit presentation decision instead of silently falling back.
    pub fn as_toast_severity(self) -> ToastSeverity {
        match self {
            Self::Info => ToastSeverity::Info,
            Self::Success => ToastSeverity::Success,
            Self::Warning => ToastSeverity::Warning,
            Self::Error => ToastSeverity::Danger,
        }
    }
}

impl crate::args::EnumArg for NotificationSeverity {
    const VALUES: &'static [&'static str] = &["info", "success", "warning", "danger", "error"];
}

impl std::str::FromStr for NotificationSeverity {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "info" => Ok(Self::Info),
            "success" => Ok(Self::Success),
            "warning" => Ok(Self::Warning),
            // "danger" is the wire/presentation spelling (matches `ToastSeverity::Danger`);
            // "error" is accepted too since that's this variant's own Rust name.
            "danger" | "error" => Ok(Self::Error),
            _ => Err(format!("unknown notification severity: {s}")),
        }
    }
}

impl From<NotificationSeverity> for ToastSeverity {
    fn from(severity: NotificationSeverity) -> Self {
        severity.as_toast_severity()
    }
}
