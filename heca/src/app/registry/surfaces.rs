//! **The surface keymaps** — `[[keys.surface]]` / `[[keys.component]]`, each container's
//! `global_focus`, and the keys a plugin registers for its own component at runtime.
//!
//! Owns nothing about the global map, modes or floors.

use super::binding::{Written, bind_flat, bind_with_conflict_tracking, unbind_and_deindex};
use super::global::merge_by;
use super::names::{action_ref_from_config, component_action};
use crate::app::conflicts::Conflicts;
use crate::input::WmAction;
use crate::keymap::{ActionRef, BindingIndex, KeyCombo, KeymapRegistry, index_binding};
use std::collections::{BTreeMap, HashMap};

/// Bind a component's **runtime-registered** key into its layer, unless the user already spoke
/// (F003/P086/T366).
///
/// The plugin path — see [`bind_provider_keybindings`](crate::providers::bind_provider_keybindings)
/// for why only a plugin uses it. **User config always wins, and a registration never silently
/// shadows one.** Two ways it can lose: the combo is already bound in this layer (the user put
/// something else there), or the action already answers to a key **in any layer** — the index is
/// built from config before this runs, so an entry there means the user has spoken about this
/// action, wherever they wrote it, and it must not also keep the key the plugin asked for.
pub fn register_component_keybinding(
    keymaps: &mut HashMap<String, KeymapRegistry>,
    index: &mut BindingIndex,
    component: &str,
    keys: &str,
    action: &str,
) {
    let trimmed = keys.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unbound") {
        return;
    }
    let layer = keymaps
        .entry(component.to_string())
        .or_insert_with(KeymapRegistry::new);
    if index.contains_key(action) {
        return;
    }
    for key_str in trimmed.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let combo = KeyCombo::parse(key_str);
        if layer.resolve(component, &combo).is_some() {
            continue;
        }
        layer.bind(
            component,
            combo,
            action_ref_from_config(action, &HashMap::new()),
        );
        index_binding(index, action, "plugin", key_str);
    }
}

/// Put each container's `global_focus` into the **global** map, aimed at that container.
///
/// It belongs here rather than in the container's own layer because it has to work while the
/// container does *not* have focus — which is the only time it is useful. Bound like any flat
/// binding, so `prefix+e` lands in the leader map and `e` in the direct one, and so two components
/// claiming one combo come out of the conflict report rather than silently overwriting each other
/// (F003/P086/T363).
pub(super) fn bind_global_focus(
    flat: &mut KeymapRegistry,
    entries: &[GlobalFocus],
    conflicts: &mut Conflicts,
    index: &mut BindingIndex,
) {
    for entry in entries {
        let action = ActionRef::Builtin(WmAction::ToggleDock {
            dock: Some(entry.target.clone()),
        });
        let written = Written {
            action: "toggle_dock",
            layer: &entry.source,
            key: &entry.key,
        };
        bind_flat(flat, action, written, conflicts, index);
    }
}

/// Layer one `[[keys.component]]` entry over another (F003/P086/T362).
///
/// **The merge rules, and why they differ between the two forms.** A TOML table merges per key
/// already, so the plain `action = "key"` entries need nothing special — overriding one keeps the
/// rest. An **array** is replaced wholesale, which for `[[keys.component.bind]]` would mean adding
/// one arg-carrying binding silently drops every shipped default. So those are merged **by their
/// `keys` field**: a keymap *is* a map from combo to action, and merging on the combo is the
/// ordinary table rule applied to the thing the array is really keyed by — not a special case.
///
/// `unbind` is applied last, at build time, and keyed by the **combo**, so it retires a binding
/// whatever it points at — the same rule as the global `[keys.unbind]`, and never a null or
/// empty-string convention.
///
/// Carried over verbatim from the `[keys.<kind>]` shape it replaces: only *where the entries come
/// from* changed, never how two of them combine.
pub(super) fn layer_component_keys(
    base: &mut heca_config::theme::SurfaceKeysConfig,
    over: heca_config::theme::SurfaceKeysConfig,
) {
    // Identity travels with the entries so the merged layer can still say which `[[keys.component]]`
    // a conflict came from. A base only ever takes id-less entries, so it keeps `id: None`; a
    // placement is seeded from that base and then stamped with its own id here.
    base.name = over.name;
    if over.id.is_some() {
        base.id = over.id;
    }
    base.bindings.extend(over.bindings);
    merge_by(&mut base.bind, &over.bind, |b| b.keys.clone());
    base.unbind.extend(over.unbind);
}

/// Build the surface binding layers from `[[keys.surface]]` / `[[keys.component]]` (F003/P086/T362,
/// generalised to any surface by F003/P082/T416).
///
/// **Keyed by placement, falling back to the kind.** An entry with no `id` speaks for the surface
/// *type*, so writing it once covers every seating; an entry with an `id` is layered on top of that
/// base for one mount alone. The returned map therefore holds an entry under each surface name and
/// an entry under each placement id that config actually mentions — [`focus_layer_action`] asks for
/// the focused mount first and falls back to its kind, so a placement nobody narrowed costs nothing.
///
/// A **layer** (the exposé, a plugin's panel) has no placement to narrow: its name is the layer's
/// own name, and it lands in the same map beside the docks, because a dock and an overlay are the
/// same thing to the keyboard.
///
/// [`focus_layer_action`]: crate::app::input
pub fn build_component_keymaps(
    config: &heca_config::theme::Config,
    conflicts: &mut Conflicts,
    index: &mut BindingIndex,
) -> (HashMap<String, KeymapRegistry>, Vec<GlobalFocus>) {
    let defaults = heca_config::theme::KeysConfig::default();
    // `surfaces()` is the one reader of both spellings — see `KeysConfig::surfaces`.
    let entries = || defaults.surfaces().chain(config.keys.surfaces());

    // Pass 1 — the per-component base: every entry with no `id`, defaults first so the user's file
    // layers over them. BTreeMap so the build order (and any conflict report) is deterministic.
    let mut bases: BTreeMap<String, heca_config::theme::SurfaceKeysConfig> = BTreeMap::new();
    for entry in entries().filter(|e| e.id.is_none()) {
        layer_component_keys(bases.entry(entry.name.clone()).or_default(), entry.clone());
    }

    // Pass 2 — the placements, each seeded from its component's finished base. Run as a second pass
    // for exactly that reason: a narrowing entry must see the whole base, wherever it was written.
    let mut placements: BTreeMap<String, heca_config::theme::SurfaceKeysConfig> = BTreeMap::new();
    for entry in entries() {
        let Some(id) = entry.id.clone() else { continue };
        let seeded = placements
            .entry(id)
            .or_insert_with(|| bases.get(&entry.name).cloned().unwrap_or_default());
        layer_component_keys(seeded, entry.clone());
    }

    // A placement id that is also a component name would otherwise be built twice; the placement is
    // the more specific of the two and already contains the base, so it wins.
    let bases: Vec<_> = bases
        .into_iter()
        .filter(|(name, _)| !placements.contains_key(name))
        .collect();
    let layers = bases.into_iter().chain(placements);

    let mut out = HashMap::new();
    let mut global_focus = Vec::new();
    for (layer_name, layer) in layers {
        let mut keymap = KeymapRegistry::new();
        let no_args = HashMap::new();
        // The label a conflict is reported under: which entry the user has to go and edit.
        // How this layer is named wherever it is reported: which `[[keys.surface]]` entry the user
        // has to go and edit.
        let label = match layer.id.as_deref() {
            Some(id) => format!("[[keys.surface]] {}:{id}", layer.name),
            None => format!("[[keys.surface]] {}", layer.name),
        };
        let bind_label = format!("{label}.bind");
        for (name, value) in &layer.bindings {
            // `global_focus` is the one binding that must NOT land in this layer: the layer is only
            // consulted while the container already has focus, so a key to *take* focus placed here
            // could never fire (F003/P086/T363). It goes to the global map instead, below.
            if name == GLOBAL_FOCUS {
                for key in value.keys() {
                    global_focus.push(GlobalFocus {
                        target: layer.id.clone().unwrap_or_else(|| layer.name.clone()),
                        key: key.trim().to_string(),
                        source: label.clone(),
                    });
                }
                continue;
            }
            let (id, action) = component_action(&layer.name, name, &no_args);
            for key_str in value.keys() {
                bind_with_conflict_tracking(
                    &mut keymap,
                    &layer_name,
                    KeyCombo::parse(key_str.trim()),
                    action.clone(),
                    Written {
                        action: &id,
                        layer: &label,
                        key: key_str.trim(),
                    },
                    conflicts,
                    index,
                );
            }
        }
        for binding in &layer.bind {
            let (id, action) = component_action(&layer.name, &binding.action, &binding.args);
            for key_str in binding.keys.keys() {
                bind_with_conflict_tracking(
                    &mut keymap,
                    &layer_name,
                    KeyCombo::parse(key_str),
                    action.clone(),
                    Written {
                        action: &id,
                        layer: &bind_label,
                        key: key_str,
                    },
                    conflicts,
                    index,
                );
            }
        }
        for combo in layer.unbind.keys() {
            unbind_and_deindex(&mut keymap, &layer_name, &label, combo, index);
        }
        out.insert(layer_name, keymap);
    }
    (out, global_focus)
}

/// The binding name every container answers to, reserved so it can never be a component's own
/// action (F003/P086/T363).
pub(super) const GLOBAL_FOCUS: &str = "global_focus";

/// One container's key to **take** the keyboard, pulled out of its `[[keys.component]]` entry.
pub struct GlobalFocus {
    /// The placement id when the entry named one, else the component — resolved at press time by
    /// `placement_for`, which falls back to the seating you were last in.
    target: String,
    /// The combo as written, `prefix+` and all.
    key: String,
    /// The entry it came from, for the conflict report and `--keys-show`.
    source: String,
}

#[cfg(test)]
mod tests;
