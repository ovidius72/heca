//! `[confirm]` — per-action confirmation toggles.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The `[confirm]` config table: per-action-name confirmation toggles.
///
/// Generic by design — any action (built-in or plugin) is user-configurable **by name**
/// (`[confirm].<action> = true | false`) without a dedicated settings field. An action declares a
/// default via its `ConfirmSpec`; this table overrides it. Replaces the old
/// `[settings] confirm_close_pane` / `confirm_delete_column` / `confirm_delete_workspace` flags
/// (now keyed `close` / `delete_column` / `delete_workspace`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ConfirmConfig {
    /// Action config name → whether its confirmation prompt is enabled.
    #[serde(flatten)]
    pub actions: HashMap<String, bool>,
}

impl ConfirmConfig {
    /// Whether the confirmation prompt for `action_name` is enabled: the user's `[confirm]` value
    /// when set, else the action's declared `default`.
    pub fn enabled(&self, action_name: &str, default: bool) -> bool {
        self.actions.get(action_name).copied().unwrap_or(default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_falls_back_to_the_declared_default() {
        let c = ConfirmConfig::default();
        assert!(c.enabled("close", true));
        assert!(!c.enabled("close", false));
    }

    #[test]
    fn table_overrides_the_default() {
        let c: ConfirmConfig =
            toml::from_str("close = false\ndelete_column = true\n").expect("parse [confirm]");
        assert!(!c.enabled("close", true), "user false wins over default true");
        assert!(c.enabled("delete_column", false), "user true wins over default false");
        assert!(c.enabled("delete_workspace", true), "unset → default");
    }
}
