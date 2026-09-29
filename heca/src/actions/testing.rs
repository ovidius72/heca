//! Shared test fixtures for the actions module.

use super::{ActionCategory, ActionMeta};
use heca_grid_ui::Glyph;

/// A name-keyed action's metadata, as a provider would declare it.
pub(super) fn dyn_meta(name: &str, policy: crate::app::interaction::ActionPolicy) -> ActionMeta {
    ActionMeta {
        name: name.to_string(),
        label: "Restart Container".to_string(),
        description: "Restart the selected Docker container.".to_string(),
        category: ActionCategory::System,
        owner: None,
        icon: Some(Glyph::Trash),
        policy,
        args: Vec::new(),
        confirm: None,
    }
}
