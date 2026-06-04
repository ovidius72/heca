//! [`StatusDot`] — a tiny glowing indicator dot in a semantic [`DotStatus`]
//! color (online/warning/error/offline). A display widget: no input, self-sized
//! to a small fixed circle. Pairs well next to a [`Label`](super::Label).

use crate::builders::LayoutExt;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::SignalGet;
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
    status: DotStatus,
}

impl StatusDot {
    /// A new dot with the given `status`.
    pub fn new(status: DotStatus) -> Self {
        let mut base = Base::new();
        base.style.width = Length::Px(DOT_SIZE);
        base.style.height = Length::Px(DOT_SIZE);
        Self { base, status }
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
        let color = match self.status {
            DotStatus::Online => t.success,
            DotStatus::Warning => t.warning,
            DotStatus::Error => t.danger,
            DotStatus::Offline => t.muted,
        };
        let glow = (self.status != DotStatus::Offline).then_some(Glow {
            color,
            radius: GLOW_RADIUS,
            intensity: GLOW_INTENSITY,
        });
        let dot = self.base.bounds;
        cx.rect(dot, color, None, (dot.size.h / 2.0) as f32, glow);
    }
}

impl LayoutExt for StatusDot {}
