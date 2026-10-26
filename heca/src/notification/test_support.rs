pub(super) use super::*;
pub(super) use heca_view::Intent;
pub(super) use heca_grid_ui::widgets::ToastSeverity;
pub(super) use std::time::Instant;

pub(super) fn test_runtime(auto_ms: u64) -> NotificationRuntime {
    NotificationRuntime::new(
        50,
        std::time::Duration::from_millis(auto_ms),
        heca_config::settings::NotificationSystem::App,
    )
}
