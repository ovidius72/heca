//! [`StatusDot`] — a tiny glowing indicator dot in a semantic [`DotStatus`]
//! color (online/warning/error/offline). A display widget: no input, self-sized
//! to a small fixed circle. Pairs well next to a [`Label`](super::Label).

use crate::builders::LayoutExt;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::Glow;
use crate::style::Length;

/// Dot diameter (logical px).
const DOT_SIZE: f32 = 9.0;
/// Glow spread radius (px).
const GLOW_RADIUS: f32 = 8.0;
/// Glow intensity for active states.
const GLOW_INTENSITY: f32 = 0.18;

/// Semantic status, mapped to theme tokens at paint time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
    /// building one dot per state and toggling four of them (F003/P096/T483).
    status: Signal<DotStatus>,
}

#[heca_grid_ui_macros::props]
impl StatusDot {
    /// A new dot with the given `status`.
    pub fn new(status: DotStatus) -> Self {
        let mut base = Base::new();
        base.style.layout.width = Length::Px(DOT_SIZE);
        base.style.layout.height = Length::Px(DOT_SIZE);
        // **It never gives way.** Everything shrinks by default, which is right for text and for a
        // card carrying a design width — and wrong for a pip: squeezing one axis of a circle does
        // not make it smaller, it flattens it, and a tight sidebar row left this drawn as a 3px
        // sliver instead of a dot. It is already the smallest thing on any row it appears in, so
        // there is nothing to win by taking space from it (F003/P096/T483).
        base.style.layout.flex_shrink = Some(0.0);
        Self { base, status: signal(status) }
    }

    /// **Show a different status.** The pip changes in place — nothing is rebuilt, and a caller
    /// that holds the dot does not have to hold its signal to say so.
    #[heca_grid_ui_macros::host_only("an imperative state change, not a property a description sets")]
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

    /// Drive the dot from a status the host already keeps.
    #[heca_grid_ui_macros::host_only("binds a host-owned signal, which static data cannot drive")]
    pub fn status(mut self, status: Signal<DotStatus>) -> Self {
        self.status = status;
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
