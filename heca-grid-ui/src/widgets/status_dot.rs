//! [`StatusDot`] — a tiny glowing indicator dot in a semantic [`DotStatus`]
//! color (online/warning/error/offline). A display widget: no input, self-sized
//! to a small fixed circle. Pairs well next to a [`Label`](super::Label).

use crate::builders::LayoutExt;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::Glow;
use crate::style::Length;

/// **Dot diameter, as a share of the text it sits with** — 9px at the default 15px font. A share of
/// the font rather than a pixel count, so the dot follows a font change and a size variant: a
/// caption-sized row (or an [`Avatar`](super::Avatar) badge) gets a smaller dot instead of one that
/// covers what it marks.
const DOT_EM: f32 = 0.6;
// Per-widget glow, intended: this widget's own measure, still scaled by `glow_size`.
// See docs/widgets.md → "Each widget's own glow and border".
/// Glow spread radius (px).
const GLOW_RADIUS: f32 = 8.0;
/// Glow intensity for active states.
const GLOW_INTENSITY: f32 = 0.18;

/// Semantic status, mapped to theme tokens at paint time.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    heca_grid_ui_macros::PropName,
)]
#[serde(rename_all = "snake_case")]
pub enum DotStatus {
    /// Healthy / connected (success color).
    #[default]
    Online,
    /// Degraded (warning color).
    Warning,
    /// Failed (danger color).
    Error,
    /// Inactive (muted, no glow).
    Offline,
}

/// A small glowing status dot.
pub struct StatusDot {
    base: Base,
    /// What it is showing. A **signal**, so a host that keeps its tree between frames changes the
    /// state in place — the sidebar shows one dot per pane and rewrites its status, rather than
    /// building one dot per state and toggling four of them.
    status: Signal<DotStatus>,
}

#[heca_grid_ui_macros::props]
impl StatusDot {
    /// A new dot with the given `status`.
    pub fn new(status: DotStatus) -> Self {
        let mut base = Base::new();
        base.style.layout.width = Length::Em(DOT_EM);
        base.style.layout.height = Length::Em(DOT_EM);
        // **It never gives way.** Everything shrinks by default, which is right for text and for a
        // card carrying a design width — and wrong for a pip: squeezing one axis of a circle does
        // not make it smaller, it flattens it, and a tight sidebar row left this drawn as a 3px
        // sliver instead of a dot. It is already the smallest thing on any row it appears in, so
        // there is nothing to win by taking space from it.
        base.style.layout.flex_shrink = Some(0.0);
        Self {
            base,
            status: signal(status),
        }
    }

    /// **Show a different status.** The pip changes in place — nothing is rebuilt, and a caller
    /// that holds the dot does not have to hold its signal to say so.
    #[heca_grid_ui_macros::host_only(
        "an imperative state change, not a property a description sets"
    )]
    pub fn set(&self, status: DotStatus) {
        if self.status.get_untracked() != status {
            self.status.set(status);
            self.base.mark_needs_paint();
        }
    }

    /// The status signal, for a host that would rather bind than call — the same value `set`
    /// writes.
    pub fn status_signal(&self) -> Signal<DotStatus> {
        self.status
    }

    /// **What the dot shows** — a status, or one that changes (a `Signal`, a `Memo`, a function
    /// reading signals), followed in place:
    ///
    /// ```ignore
    /// StatusDot::online().status(move || pane.get().status)
    /// ```
    #[heca_grid_ui_macros::host_only("a value that changes, which static data cannot drive")]
    pub fn status(self, status: impl crate::builders::IntoStatusValue) -> Self {
        status.into_status_value().show_in(self.status);
        self
    }

    /// Convenience constructors, one per status.
    pub fn online() -> Self {
        Self::new(DotStatus::Online)
    }
    pub fn warning() -> Self {
        Self::new(DotStatus::Warning)
    }
    pub fn error() -> Self {
        Self::new(DotStatus::Error)
    }
    pub fn offline() -> Self {
        Self::new(DotStatus::Offline)
    }
}

impl Component for StatusDot {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let t = cx.theme();
        let status = self.status.get_untracked();
        let color = match status {
            DotStatus::Online => t.colors.success,
            DotStatus::Warning => t.colors.warning,
            DotStatus::Error => t.colors.danger,
            DotStatus::Offline => t.colors.muted,
        };
        let glow = (status != DotStatus::Offline).then_some(Glow {
            color,
            radius: GLOW_RADIUS,
            intensity: GLOW_INTENSITY,
        });
        let dot = self.base.bounds;
        cx.rect(dot, color, None, (dot.size.h / 2.0) as f32, glow);
    }
}

impl LayoutExt for StatusDot {}
