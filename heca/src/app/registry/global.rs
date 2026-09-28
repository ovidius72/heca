//! **The global keymap** — `[keys]`, `[[keys.bind]]`, `[[keys.command]]` and `[keys.unbind]`.
//!
//! Also owns [`merge_by`], the rule for how a user's keyed array layers over the defaults, which
//! the surface tables reuse. Owns nothing about surfaces, modes or floors.

use super::binding::{Written, bind_with_conflict_tracking, unbind_and_deindex};
use super::names::action_ref_from_config;
use crate::app::conflicts::Conflicts;
use crate::input::{SpawnKind, WmAction};
use crate::keymap::{ActionRef, BindingIndex, KeyCombo, KeymapRegistry};
use heca_core::runtime::PaneClosePolicy;
use std::collections::{BTreeMap, HashMap};

pub fn build_keymap(
    config: &heca_config::theme::Config,
    conflicts: &mut Conflicts,
    index: &mut BindingIndex,
) -> KeymapRegistry {
    let mut keymap = KeymapRegistry::new();

    let default_keys = heca_config::theme::KeysConfig::default();
    let mut merged_bindings: BTreeMap<String, heca_config::theme::BindingValue> = default_keys
        .bindings
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (k, v) in &config.keys.bindings {
        merged_bindings.insert(k.clone(), v.clone());
    }
    // `[[keys.bind]]` — the same normal-mode keymap, for the bindings that carry `args`.
    let mut merged_bind = default_keys.bind.clone();
    merge_by(&mut merged_bind, &config.keys.bind, |b| b.keys.clone());

    let no_args = HashMap::new();
    for (action_name, value) in &merged_bindings {
        // An unknown name is NOT skipped any more: it becomes a Dynamic ref resolved at press
        // (G3 — a plugin action does not exist yet when config is read).
        let action = action_ref_from_config(action_name, &no_args);
        for key_str in value.keys() {
            let trimmed = key_str.trim();
            let written = Written {
                action: action_name,
                layer: "[keys]",
                key: trimmed,
            };
            if let Some(rest) = trimmed.strip_prefix("prefix+") {
                bind_with_conflict_tracking(
                    &mut keymap,
                    "normal",
                    KeyCombo::parse(rest.trim()),
                    action.clone(),
                    written,
                    conflicts,
                    index,
                );
            } else {
                bind_with_conflict_tracking(
                    &mut keymap,
                    "global",
                    KeyCombo::parse(trimmed),
                    action.clone(),
                    written,
                    conflicts,
                    index,
                );
            }
        }
    }

    // The arg-carrying bindings, into the same keymap and through the same seam as the flat ones —
    // `action_ref_from_config` is what turns a name plus an `args` table into a built action, so a
    // parameterized global binding is not a second resolution path.
    for binding in &merged_bind {
        let action = action_ref_from_config(&binding.action, &binding.args);
        for key_str in binding.keys.keys() {
            let written = Written {
                action: &binding.action,
                layer: "[[keys.bind]]",
                key: key_str,
            };
            let (mode, combo) = match key_str.strip_prefix("prefix+") {
                Some(rest) => ("normal", KeyCombo::parse(rest.trim())),
                None => ("global", KeyCombo::parse(key_str)),
            };
            bind_with_conflict_tracking(
                &mut keymap,
                mode,
                combo,
                action.clone(),
                written,
                conflicts,
                index,
            );
        }
    }

    for combo_str in config.keys.unbind.keys() {
        let trimmed = combo_str.trim();
        let mode = if trimmed.starts_with("prefix+") {
            "normal"
        } else {
            "global"
        };
        unbind_and_deindex(&mut keymap, mode, "[keys]", trimmed, index);
    }

    // `[[keys.command]]` — a program on a key. It had no merge at all, so defining one replaced
    // every default outright; the defaults ship none uncommented today, which is the only reason
    // nobody lost a binding to it yet.
    let mut merged_commands = default_keys.command.clone();
    merge_by(&mut merged_commands, &config.keys.command, |c| {
        c.key.trim().to_string()
    });
    for cmd_cfg in &merged_commands {
        let action = ActionRef::Builtin(WmAction::SpawnCommand {
            command: cmd_cfg.command.clone(),
            kind: cmd_cfg.kind.parse().unwrap_or(SpawnKind::Terminal),
            float: cmd_cfg.float,
            close_policy: PaneClosePolicy {
                close_pane: cmd_cfg.close_pane,
                keep_on_error: cmd_cfg.keep_on_error,
                keep_on_success: cmd_cfg.keep_on_success,
            },
        });
        let trimmed = cmd_cfg.key.trim();
        let written = Written {
            action: &cmd_cfg.command,
            layer: "[[keys.command]]",
            key: trimmed,
        };
        if let Some(rest) = trimmed.strip_prefix("prefix+") {
            bind_with_conflict_tracking(
                &mut keymap,
                "normal",
                KeyCombo::parse(rest.trim()),
                action,
                written,
                conflicts,
                index,
            );
        } else {
            bind_with_conflict_tracking(
                &mut keymap,
                "global",
                KeyCombo::parse(trimmed),
                action,
                written,
                conflicts,
                index,
            );
        }
    }

    keymap
}

/// **A keyed array merges by its key, never wholesale.**
///
/// TOML merges a table per key, so overriding one `action = "combo"` line keeps the rest. An
/// **array** is replaced entire — which for a binding list means adding one entry silently deletes
/// every default, with nothing to see and nothing to fail. That is a trap the config sets and the
/// user springs.
///
/// A keymap *is* a map from combo to action, so merging on the combo is the ordinary table rule
/// applied to what the array is really keyed by — not a special case. Written once here because it
/// was written three times: `[[keys.bind]]`, `[[keys.surface.bind]]`, and not at all for
/// `[[keys.command]]`, which is how that one kept the trap. A fourth keyed array gets the rule by
/// calling this rather than by remembering it.
pub(super) fn merge_by<T: Clone, K: PartialEq>(
    base: &mut Vec<T>,
    over: &[T],
    key: impl Fn(&T) -> K,
) {
    for entry in over {
        match base.iter_mut().find(|b| key(b) == key(entry)) {
            Some(existing) => *existing = entry.clone(),
            None => base.push(entry.clone()),
        }
    }
}

#[cfg(test)]
mod tests;
