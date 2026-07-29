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

/// Toggle keys that were renamed, as `(retired, current)`.
///
/// A rename here silently turns a prompt the user had switched **off** back on: they wrote `false`
/// and the app quietly stops reading that line. So a retired key keeps working, and loses only to
/// the current one being set as well.
const RENAMED: &[(&str, &str)] = &[
    // A pane is *closed*, not deleted — the word its action id, its binding name and its label
    // already used. The dialog and this toggle were the two that disagreed (F003/P086/T370).
    ("delete_pane", "close"),
];

impl ConfirmConfig {
    /// Whether the confirmation prompt for `action_name` is enabled: the user's `[confirm]` value
    /// when set, else a retired name for the same toggle, else the action's declared `default`.
    pub fn enabled(&self, action_name: &str, default: bool) -> bool {
        if let Some(set) = self.actions.get(action_name) {
            return *set;
        }
        RENAMED
            .iter()
            .find(|(_, current)| *current == action_name)
            .and_then(|(retired, _)| self.actions.get(*retired))
            .copied()
            .unwrap_or(default)
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

    /// A rename must not switch a prompt the user turned off back on (F003/P086/T370).
    #[test]
    fn a_retired_toggle_name_still_disables_its_prompt() {
        let c: ConfirmConfig = toml::from_str("delete_pane = false\n").expect("parse [confirm]");
        assert!(
            !c.enabled("close", true),
            "the old name still speaks for the pane prompt",
        );

        let both: ConfirmConfig =
            toml::from_str("delete_pane = false\nclose = true\n").expect("parse [confirm]");
        assert!(both.enabled("close", false), "the current name wins when both are set");
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
