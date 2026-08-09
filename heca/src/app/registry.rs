//! Registry and keymap construction helpers.
//!
//! This module owns config-driven action/keymap wiring so `main.rs` can stay
//! focused on application lifecycle and event dispatch.

use crate::actions::ActionRegistry;
use crate::handlers::*;
use crate::input::{self, SpawnKind, WmAction, action_from_name, build_action};
use crate::app::conflicts::{BindingConflict, Conflicts};
use crate::keymap::{index_binding, ActionRef, BindingIndex, KeyCombo, KeymapRegistry};
use heca_core::layout::PaneId;
use heca_core::runtime::PaneClosePolicy;
use std::collections::{BTreeMap, HashMap};

/// Resolve a config action **name** (+ its args) to what the key should be bound to.
///
/// Built-in ⇒ [`ActionRef::Builtin`], resolved right here at load, exactly as before: a unit variant
/// through `action_from_name`, a parameterized one through `build_action` (so a malformed `args`
/// still fails at load, not at press).
///
/// Anything else ⇒ [`ActionRef::Dynamic`] — **not an error** (plugin-04 G3). Config is loaded before
/// any provider or plugin has registered its actions, so a binding to `plugin.docker.restart` cannot
/// possibly resolve yet; it resolves at press time through the one dispatch door. A binding to an
/// action that never turns up no-ops with a debug warning when pressed, which is the price of not
/// rejecting bindings to actions that legitimately do not exist yet.
///
/// A **misspelled argument** is a different matter and is now reported, loudly, right here — see
/// [`binding_arg_problems`]. It used to land in the same silence as an unknown action name: a
/// required argument spelled wrong made `build_action` return `None`, so `focus_pane` with a
/// `pane_i` fell all the way through to `Dynamic` and did nothing for the rest of the session,
/// without a word in a release build.
fn action_ref_from_config(name: &str, args: &HashMap<String, String>) -> ActionRef {
    log_arg_problems(name, args);
    if let Some(built) = build_action(name, args) {
        return ActionRef::Builtin(built);
    }
    if let Some(unit) = action_from_name(name) {
        return ActionRef::Builtin(unit);
    }
    let mut intent = crate::chrome::Intent::new(name);
    for (k, v) in args {
        intent
            .args
            .insert(k.clone(), crate::chrome::PropValue::Text(v.clone()));
    }
    ActionRef::Dynamic(intent)
}

/// The action id a `[[keys.component]]` entry means, and the reference to bind (F003/P086/T362).
///
/// **Binding names are short; the action id is `<component>.<name>`.** A user writes `cursor_up`
/// under `name = "workspaces"` and means `workspaces.cursor_up` — they should never have to repeat
/// the component in every line of its own block.
///
/// The one exception is an id heca already knows: a component **binds** existing actions rather than
/// redeclaring them (`next_pane`, `zoom_column`), and those keep their catalog id. That is decided
/// by whether [`action_ref_from_config`] can resolve the name at load — a built-in resolves, a
/// component's or plugin's own name does not and becomes a `Dynamic` ref answered at press time.
///
/// An already-qualified name is left alone, so writing the id out in full is never wrong.
fn component_action(
    component: &str,
    written: &str,
    args: &HashMap<String, String>,
) -> (String, ActionRef) {
    let direct = action_ref_from_config(written, args);
    if matches!(direct, ActionRef::Builtin(_)) || written.starts_with(&format!("{component}.")) {
        return (written.to_string(), direct);
    }
    let id = format!("{component}.{written}");
    let action = action_ref_from_config(&id, args);
    (id, action)
}

/// What a binding was written as: which action, in which layer, under which key.
///
/// Carried together because the same three facts answer two questions — where to send the user when
/// two bindings collide, and what key an action answers to ([`BindingIndex`], F003/P086/T366).
struct Written<'a> {
    /// The action id the binding resolves to, after any component qualification.
    action: &'a str,
    /// The layer, named the way the config file names it.
    layer: &'a str,
    /// The combo exactly as the user types it — `prefix+` and all.
    key: &'a str,
}

fn bind_with_conflict_tracking(
    keymap: &mut KeymapRegistry,
    mode: &str,
    combo: KeyCombo,
    action: ActionRef,
    written: Written<'_>,
    conflicts: &mut Conflicts,
    index: &mut BindingIndex,
) {
    if let Some(previous_action) = keymap.resolve(mode, &combo).cloned()
        && previous_action != action
    {
        conflicts.key(BindingConflict {
            mode: mode.to_string(),
            combo: combo.clone(),
            previous_action,
            previous_source: "existing binding".to_string(),
            new_action: action.clone(),
            new_source: format!("{} {}", written.layer, written.action),
        });
    }
    keymap.bind(mode, combo, action);
    index_binding(index, written.action, written.layer, written.key);
}

/// Retire a combo from a layer, and from the index with it (F003/P086/T366).
///
/// An `unbind` that left the index alone would have `--keys-show` and every tooltip reporting a key
/// that no longer does anything — the exact drift the index exists to end. Matched on the parsed
/// combo rather than the literal string, so `"Prefix+W"` retires `"prefix+w"`.
fn unbind_and_deindex(
    keymap: &mut KeymapRegistry,
    mode: &str,
    layer: &str,
    key: &str,
    index: &mut BindingIndex,
) {
    let split = |s: &str| {
        let s = s.trim();
        match s.strip_prefix("prefix+") {
            Some(rest) => (true, KeyCombo::parse(rest.trim())),
            None => (false, KeyCombo::parse(s)),
        }
    };
    let target = split(key);
    keymap.unbind(mode, &target.1);
    for bound in index.values_mut() {
        bound.retain(|b| b.layer != layer || split(&b.key) != target);
    }
    index.retain(|_, bound| !bound.is_empty());
}

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


/// What is wrong with a binding's `args` table, judged against what the action declares it takes.
///
/// Empty for an action heca does not know (a provider's, a plugin's, or a typo in the action name)
/// — that name is resolved at press time, not here, so there is nothing yet to compare against.
pub(crate) fn binding_arg_problems(
    name: &str,
    args: &HashMap<String, String>,
) -> Vec<crate::actions::ArgProblem> {
    match crate::actions::builtin_args(name) {
        Some(specs) => crate::actions::check_args(&specs, args),
        None => Vec::new(),
    }
}

/// Report a binding's argument mistakes at config load, through the same channel as a keybinding
/// conflict — unconditionally, not only in a debug build. A wrong argument in `config.toml` is the
/// user's to fix, so the user has to hear about it.
fn log_arg_problems(name: &str, args: &HashMap<String, String>) {
    if cfg!(test) {
        return;
    }
    let problems = binding_arg_problems(name, args);
    for problem in &problems {
        eprintln!("[heca] binding '{name}': {problem}");
    }
    if problems
        .iter()
        .any(|p| matches!(p, crate::actions::ArgProblem::Missing { .. }))
    {
        eprintln!("[heca] binding '{name}' cannot be built and will do nothing when pressed");
    }
}


/// Convert a parsed [`KeyCombo`] into a renderer-agnostic `heca_grid_ui` chord
/// (`GridKey` + `Modifiers`), or `None` for a key name grid-ui does not model. Handles both
/// key-name sources: config strings parsed by `KeyCombo::parse` (lowercase, e.g. `"arrowdown"`)
/// **and** live events from `build_event_combo`/`normalize_key_text`, which name a `NamedKey`
/// via `{:?}` (capitalised, e.g. `"ArrowDown"`, `"Tab"`, `"Enter"`). The name is matched
/// **case-insensitively** so both resolve; modifiers map straight across (`super_` → `meta`).
pub(crate) fn combo_to_grid(
    combo: &KeyCombo,
) -> Option<(heca_grid_ui::GridKey, heca_grid_ui::Modifiers)> {
    use heca_grid_ui::GridKey;
    let lowered = combo.key.to_lowercase();
    let key = match lowered.as_str() {
        "enter" | "return" => GridKey::Enter,
        "space" => GridKey::Space,
        "tab" => GridKey::Tab,
        "escape" | "esc" => GridKey::Escape,
        "backspace" => GridKey::Backspace,
        "delete" | "del" => GridKey::Delete,
        "arrowleft" | "left" => GridKey::ArrowLeft,
        "arrowright" | "right" => GridKey::ArrowRight,
        "arrowup" | "up" => GridKey::ArrowUp,
        "arrowdown" | "down" => GridKey::ArrowDown,
        "home" => GridKey::Home,
        "end" => GridKey::End,
        s => {
            let mut chars = s.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => GridKey::Char(c),
                _ => return None,
            }
        }
    };
    let mods = heca_grid_ui::Modifiers {
        ctrl: combo.ctrl,
        alt: combo.alt,
        shift: combo.shift,
        meta: combo.super_,
    };
    Some((key, mods))
}

/// Build the **widget keymap** (`widget-keys-config`): the `[keys.widgets]` bindings resolved
/// into a `heca_grid_ui::Keymap` (`key chord → WidgetIntent`), the single host-owned map every
/// interactive widget/overlay consults. User bindings win when set (arrays replace wholesale),
/// else the bundled default. These names are **not** `WmAction`s, so `build_keymap` skips them —
/// they live only here and never hijack normal-mode input. `edit_*` entries are bound **first**
/// so a focused `Input` wins the `Ctrl+h` overload (field-first). See `docs/widgets.md` + README.
pub fn build_widget_keymap(config: &heca_config::theme::Config) -> heca_grid_ui::Keymap {
    use heca_grid_ui::WidgetIntent;
    let defaults = heca_config::theme::KeysConfig::default();
    // Edit shortcuts bound before the nav overload (so `Ctrl+h` resolves EditDeleteBack first).
    let entries = [
        ("edit_delete_back", WidgetIntent::EditDeleteBack),
        ("edit_delete_to_line_start", WidgetIntent::EditDeleteToLineStart),
        ("edit_select_all", WidgetIntent::EditSelectAll),
        ("item_previous", WidgetIntent::ItemPrevious),
        ("item_next", WidgetIntent::ItemNext),
        ("menu_up", WidgetIntent::MenuUp),
        ("menu_down", WidgetIntent::MenuDown),
        ("menu_history_up", WidgetIntent::MenuHistoryUp),
        ("menu_history_down", WidgetIntent::MenuHistoryDown),
        ("activate", WidgetIntent::Activate),
        ("dismiss", WidgetIntent::Dismiss),
    ];
    let mut km = heca_grid_ui::Keymap::new();
    for (name, intent) in entries {
        let value = config
            .keys
            .widgets
            .get(name)
            .or_else(|| defaults.widgets.get(name));
        if let Some(value) = value {
            for key_str in value.keys() {
                if let Some((key, mods)) = combo_to_grid(&KeyCombo::parse(key_str.trim())) {
                    km.bind(key, mods, intent);
                }
            }
        }
    }
    km
}

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

    for combo_str in config.keys.unbind.keys() {
        let trimmed = combo_str.trim();
        let mode = if trimmed.starts_with("prefix+") { "normal" } else { "global" };
        unbind_and_deindex(&mut keymap, mode, "[keys]", trimmed, index);
    }

    for cmd_cfg in &config.keys.command {
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

/// The built-in mode keymaps that are **entered by focus rather than by a key**, so they never take
/// a trigger — see the note at the trigger site in [`build_modes`].
const UNTRIGGERED_MODES: &[&str] = &["sidebar", crate::app::input::FOCUS_LAYER];

/// Build **every** keymap layer, and the reverse index over all of them, from one config.
///
/// The single entry point, so startup and `prefix+Shift+r` cannot build a different set — and so
/// [`Keymaps::by_action`] is filled by the same pass that binds, and can never describe a layer that
/// was not built (F003/P086/T366).
pub fn build_keymaps(
    config: &heca_config::theme::Config,
    conflicts: &mut Conflicts,
) -> crate::keymap::Keymaps {
    let mut by_action = BindingIndex::new();
    let (modes, triggers) = build_modes(config, conflicts, &mut by_action);
    let mut flat = build_keymap(config, conflicts, &mut by_action);
    let (components, global_focus) = build_component_keymaps(config, conflicts, &mut by_action);
    bind_global_focus(&mut flat, &global_focus, conflicts, &mut by_action);
    crate::keymap::Keymaps {
        flat,
        modes,
        components,
        triggers,
        by_action,
    }
}

/// Put each container's `global_focus` into the **global** map, aimed at that container.
///
/// It belongs here rather than in the container's own layer because it has to work while the
/// container does *not* have focus — which is the only time it is useful. Bound like any flat
/// binding, so `prefix+e` lands in the leader map and `e` in the direct one, and so two components
/// claiming one combo come out of the conflict report rather than silently overwriting each other
/// (F003/P086/T363).
fn bind_global_focus(
    flat: &mut KeymapRegistry,
    entries: &[GlobalFocus],
    conflicts: &mut Conflicts,
    index: &mut BindingIndex,
) {
    for entry in entries {
        let action = ActionRef::Builtin(WmAction::FocusDock {
            dock: Some(entry.target.clone()),
        });
        let written = Written {
            action: "focus_dock",
            layer: &entry.source,
            key: &entry.key,
        };
        let (mode, combo) = match entry.key.strip_prefix("prefix+") {
            Some(rest) => ("normal", KeyCombo::parse(rest.trim())),
            None => ("global", KeyCombo::parse(&entry.key)),
        };
        bind_with_conflict_tracking(flat, mode, combo, action, written, conflicts, index);
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
fn layer_component_keys(
    base: &mut heca_config::theme::ComponentKeysConfig,
    over: heca_config::theme::ComponentKeysConfig,
) {
    // Identity travels with the entries so the merged layer can still say which `[[keys.component]]`
    // a conflict came from. A base only ever takes id-less entries, so it keeps `id: None`; a
    // placement is seeded from that base and then stamped with its own id here.
    base.name = over.name;
    if over.id.is_some() {
        base.id = over.id;
    }
    base.bindings.extend(over.bindings);
    for binding in over.bind {
        match base.bind.iter_mut().find(|b| b.keys == binding.keys) {
            Some(existing) => *existing = binding,
            None => base.bind.push(binding),
        }
    }
    base.unbind.extend(over.unbind);
}

/// Build the component binding layers from `[[keys.component]]` (F003/P086/T362).
///
/// **Keyed by placement, falling back to the kind.** An entry with no `id` speaks for the component
/// *type*, so writing it once covers every seating; an entry with an `id` is layered on top of that
/// base for one mount alone. The returned map therefore holds an entry under each component name and
/// an entry under each placement id that config actually mentions — [`focus_layer_action`] asks for
/// the focused mount first and falls back to its kind, so a placement nobody narrowed costs nothing.
///
/// [`focus_layer_action`]: crate::app::input
pub fn build_component_keymaps(
    config: &heca_config::theme::Config,
    conflicts: &mut Conflicts,
    index: &mut BindingIndex,
) -> (HashMap<String, KeymapRegistry>, Vec<GlobalFocus>) {
    let defaults = heca_config::theme::KeysConfig::default();
    let entries = || defaults.component.iter().chain(config.keys.component.iter());

    // Pass 1 — the per-component base: every entry with no `id`, defaults first so the user's file
    // layers over them. BTreeMap so the build order (and any conflict report) is deterministic.
    let mut bases: BTreeMap<String, heca_config::theme::ComponentKeysConfig> = BTreeMap::new();
    for entry in entries().filter(|e| e.id.is_none()) {
        layer_component_keys(bases.entry(entry.name.clone()).or_default(), entry.clone());
    }

    // Pass 2 — the placements, each seeded from its component's finished base. Run as a second pass
    // for exactly that reason: a narrowing entry must see the whole base, wherever it was written.
    let mut placements: BTreeMap<String, heca_config::theme::ComponentKeysConfig> = BTreeMap::new();
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
        // How this layer is named wherever it is reported: which `[[keys.component]]` entry the user
        // has to go and edit.
        let label = match layer.id.as_deref() {
            Some(id) => format!("[[keys.component]] {}:{id}", layer.name),
            None => format!("[[keys.component]] {}", layer.name),
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
                    Written { action: &id, layer: &label, key: key_str.trim() },
                    conflicts,
                    index,
                );
            }
        }
        for binding in &layer.bind {
            let (id, action) = component_action(&layer.name, &binding.action, &binding.args);
            bind_with_conflict_tracking(
                &mut keymap,
                &layer_name,
                KeyCombo::parse(&binding.keys),
                action,
                Written { action: &id, layer: &bind_label, key: binding.keys.trim() },
                conflicts,
                index,
            );
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
const GLOBAL_FOCUS: &str = "global_focus";

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

/// Build mode keymaps and triggers from config.
pub fn build_modes(
    config: &heca_config::theme::Config,
    conflicts: &mut Conflicts,
    index: &mut BindingIndex,
) -> (
    HashMap<String, KeymapRegistry>,
    HashMap<String, (KeyCombo, bool)>,
) {
    let mut mode_keymaps = HashMap::new();
    let mut mode_triggers: HashMap<String, (KeyCombo, bool)> = HashMap::new();

    let default_keys = heca_config::theme::KeysConfig::default();
    let mut merged_modes: BTreeMap<String, heca_config::theme::KeyModeConfig> = default_keys
        .mode
        .iter()
        .map(|mode| (mode.name.clone(), mode.clone()))
        .collect();

    for user_mode in &config.keys.mode {
        if let Some(existing) = merged_modes.get_mut(&user_mode.name) {
            existing.trigger = user_mode.trigger.clone();
            existing.sticky = user_mode.sticky;
            existing.bindings.extend(user_mode.bindings.clone());
        } else {
            merged_modes.insert(user_mode.name.clone(), user_mode.clone());
        }
    }

    for mode_cfg in merged_modes.values() {
        let mut mode_map = KeymapRegistry::new();
        let label = format!("[[keys.mode]] {}", mode_cfg.name);
        for binding in &mode_cfg.bindings {
            // Same rule as the flat bindings: an unresolvable name is a Dynamic ref, not a dropped
            // binding. A mode binding's `args` become the Intent's args.
            let action = action_ref_from_config(&binding.action, &binding.args);
            bind_with_conflict_tracking(
                &mut mode_map,
                &mode_cfg.name,
                KeyCombo::parse(&binding.keys),
                action,
                Written {
                    action: &binding.action,
                    layer: &label,
                    key: binding.keys.trim(),
                },
                conflicts,
                index,
            );
        }
        // **`Esc` is a guarantee, not a default** (F003/P086/T363). Every focused container must
        // have a way back to the main region, including one that declares nothing at all — so it is
        // re-asserted here after the merge instead of being left to the file. `[[keys.mode]]` arrays
        // are replaced wholesale by a user's config, so a `focus` block that simply forgot this line
        // would otherwise strand the keyboard in a dock with only the mouse to get out.
        //
        // Bound through the same door as everything else: putting something *else* on `Escape` in
        // this layer is a real collision and comes out in the report rather than silently losing.
        // Binding `unfocus_dock` to further keys is untouched — this adds a floor, not a ceiling.
        if mode_cfg.name == crate::app::input::FOCUS_LAYER {
            bind_with_conflict_tracking(
                &mut mode_map,
                &mode_cfg.name,
                KeyCombo::parse("Escape"),
                ActionRef::Builtin(WmAction::UnfocusDock),
                Written {
                    action: "unfocus_dock",
                    layer: "built-in (every container has a way out)",
                    key: "Escape",
                },
                conflicts,
                index,
            );
        }
        mode_keymaps.insert(mode_cfg.name.clone(), mode_map);
        // These two are **entered by focus, not by a key**: `sidebar` by `sidebar_focus` or a
        // click, `focus` by focusing a dock. A trigger would make them reachable as a plain
        // `InputMode::Mode`, where their bindings resolve against a focus nothing is holding — a
        // mode that looks entered and does nothing. So a trigger is refused even when a user
        // config supplies one.
        if !UNTRIGGERED_MODES.contains(&mode_cfg.name.as_str()) {
            let trigger_trimmed = mode_cfg.trigger.trim();
            let trigger_combo = if trigger_trimmed.starts_with("prefix+") {
                let rest = trigger_trimmed.strip_prefix("prefix+").unwrap().trim();
                KeyCombo::parse(rest)
            } else {
                KeyCombo::parse(trigger_trimmed)
            };
            mode_triggers.insert(mode_cfg.name.clone(), (trigger_combo, mode_cfg.sticky));
        }
    }

    (mode_keymaps, mode_triggers)
}

/// Build the action registry and register individual handlers for all actions.
pub fn build_registry() -> ActionRegistry {
    let mut registry = ActionRegistry::new();

    // ── Navigation ──
    registry.register(&WmAction::FocusLeft, handle_focus_left);
    registry.register(&WmAction::FocusRight, handle_focus_right);
    registry.register(&WmAction::FocusUp, handle_focus_up);
    registry.register(&WmAction::FocusDown, handle_focus_down);
    registry.register(&WmAction::NextPane, handle_next_pane);
    registry.register(&WmAction::PrevPane, handle_prev_pane);
    registry.register(&WmAction::WorkspaceNext, handle_workspace_next);
    registry.register(&WmAction::WorkspacePrev, handle_workspace_prev);
    registry.register(&WmAction::FocusToggleLocal, handle_focus_toggle_local);
    registry.register(&WmAction::FocusToggleGlobal, handle_focus_toggle_global);
    registry.register(
        &WmAction::FocusPane { pane_id: PaneId(0) },
        handle_focus_pane,
    );
    registry.register(
        &WmAction::FocusWorkspace { ws_idx: 0 },
        handle_focus_workspace,
    );

    // ── Layout ──
    registry.register(&WmAction::SplitHorizontal, handle_split_horizontal);
    registry.register(&WmAction::SplitVertical, handle_split_vertical);
    registry.register(&WmAction::ZoomColumn, handle_zoom_column);
    registry.register(
        &WmAction::ZoomColumnAtIndex { ws_idx: 0, col_idx: 0 },
        handle_zoom_column_at_index,
    );
    registry.register(&WmAction::OpenContextMenu, handle_open_context_menu);
    registry.register(&WmAction::ScrollViewLeft, handle_scroll_view_left);
    registry.register(&WmAction::ScrollViewRight, handle_scroll_view_right);
    registry.register(&WmAction::ResizeIncrease, handle_resize_increase);
    registry.register(&WmAction::ResizeDecrease, handle_resize_decrease);
    registry.register(&WmAction::PaneHeightIncrease, handle_pane_height_increase);
    registry.register(&WmAction::PaneHeightDecrease, handle_pane_height_decrease);
    registry.register(&WmAction::SwapLeft, handle_swap_left);
    registry.register(&WmAction::SwapRight, handle_swap_right);
    registry.register(&WmAction::SwapUp, handle_swap_up);
    registry.register(&WmAction::SwapDown, handle_swap_down);
    registry.register(
        &WmAction::MovePaneLeft { pane_id: None },
        handle_move_pane_left,
    );
    registry.register(
        &WmAction::MovePaneRight { pane_id: None },
        handle_move_pane_right,
    );
    registry.register(&WmAction::MoveColumnUp, handle_move_column_up);
    registry.register(&WmAction::MoveColumnDown, handle_move_column_down);
    registry.register(
        &WmAction::Swap {
            a_id: PaneId(0),
            b_id: PaneId(0),
        },
        handle_swap_param,
    );
    registry.register(
        &WmAction::Move {
            pane_id: PaneId(0),
            target_col: 0,
        },
        handle_move_param,
    );
    registry.register(
        &WmAction::MovePaneToWorkspace {
            pane_id: PaneId(0),
            ws_idx: 0,
        },
        handle_move_pane_to_workspace,
    );
    registry.register(
        &WmAction::MovePaneToColumn {
            pane_id: PaneId(0),
            ws_idx: 0,
            col_idx: 0,
        },
        handle_move_pane_to_column,
    );
    registry.register(
        &WmAction::MoveColumnToWorkspace {
            col_idx: 0,
            ws_idx: 0,
            focus: true,
        },
        handle_move_column_to_workspace,
    );
    registry.register(
        &WmAction::MoveColumn {
            src_ws: 0,
            src_col: 0,
            dst_ws: 0,
            dst_idx: 0,
            focus: true,
        },
        handle_move_column,
    );
    registry.register(
        &WmAction::SwapColumns {
            a_ws: 0,
            a_col: 0,
            b_ws: 0,
            b_col: 0,
        },
        handle_swap_columns,
    );
    registry.register(
        &WmAction::Resize {
            target: input::ResizeTarget::Column,
            axis: input::ResizeAxis::X,
            amount: 0.0,
        },
        handle_resize,
    );
    registry.register(
        &WmAction::ResizeColumnBy {
            col_idx: 0,
            delta: 0.0,
        },
        handle_resize_column_by,
    );
    registry.register(
        &WmAction::ResizePaneHeightBy {
            col_idx: 0,
            pane_idx: 0,
            delta: 0.0,
        },
        handle_resize_pane_height_by,
    );
    registry.register(
        &WmAction::ResizeTo {
            target: input::ResizeTarget::Column,
            width: 0.0,
            height: 0.0,
        },
        handle_resize_to,
    );

    // ── Pane ──
    registry.register(&WmAction::Float, handle_float);
    registry.register(&WmAction::ClosePane, handle_close_pane);
    registry.register(&WmAction::PaneSelect, handle_pane_select);
    registry.register(&WmAction::FollowLink, handle_follow_link);
    registry.register(&WmAction::HintPick, handle_hint_pick);
    registry.register(&WmAction::SwapPane, handle_swap_pane);
    registry.register(&WmAction::SwapAndFocusPane, handle_swap_and_focus_pane);
    registry.register(
        &WmAction::MoveColumnToWorkspacePick,
        handle_move_column_to_workspace_pick,
    );
    registry.register(
        &WmAction::MovePaneToWorkspacePick,
        handle_move_pane_to_workspace_pick,
    );
    registry.register(
        &WmAction::MovePaneToColumnPick,
        handle_move_pane_to_column_pick,
    );
    registry.register(&WmAction::PaneTake, handle_pane_take);
    registry.register(&WmAction::PaneTakeAndFocus, handle_pane_take_and_focus);
    registry.register(
        &WmAction::TakePane {
            pane_id: PaneId(0),
            focus_after: false,
        },
        handle_take_pane,
    );
    // Chrome container placement (plugin-02, §2.9).
    registry.register(
        &WmAction::MoveContainerToRegion {
            container_id: String::new(),
            region: crate::chrome::RegionId::LeftSidebar,
        },
        handle_move_container_to_region,
    );
    registry.register(
        &WmAction::ReorderContainerBefore {
            container_id: String::new(),
            before_id: None,
        },
        handle_reorder_container_before,
    );
    registry.register(
        &WmAction::ReorderContainerAfter {
            container_id: String::new(),
            after_id: String::new(),
        },
        handle_reorder_container_after,
    );
    registry.register(
        &WmAction::SetRegionVisible {
            region: crate::chrome::RegionId::LeftSidebar,
            visible: false,
        },
        handle_set_region_visible,
    );
    // Chrome region show/hide mounted-gate (sidebar-fu-6) — 12 unit actions, one handler.
    for action in [
        &WmAction::ShowLeftSidebar,
        &WmAction::HideLeftSidebar,
        &WmAction::ToggleLeftSidebar,
        &WmAction::ShowRightSidebar,
        &WmAction::HideRightSidebar,
        &WmAction::ToggleRightSidebar,
        &WmAction::ShowTopBar,
        &WmAction::HideTopBar,
        &WmAction::ToggleTopBar,
        &WmAction::ShowBottomBar,
        &WmAction::HideBottomBar,
        &WmAction::ToggleBottomBar,
    ] {
        registry.register(action, handle_set_chrome_region_shown);
    }
    registry.register(&WmAction::RenamePane, handle_rename_pane);
    registry.register(&WmAction::RenameColumn, handle_rename_column);
    registry.register(
        &WmAction::RenameColumnByIdx {
            ws_idx: 0,
            col_idx: 0,
        },
        handle_rename_column_by_idx,
    );
    registry.register(
        &WmAction::FloatAt {
            pane_id: PaneId(0),
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
        handle_float_at,
    );
    registry.register(
        &WmAction::ClosePaneById { pane_id: PaneId(0) },
        handle_close_pane_by_id,
    );
    registry.register(
        &WmAction::RenameTarget {
            pane_id: PaneId(0),
            name: String::new(),
        },
        handle_rename_target,
    );
    registry.register(
        &WmAction::RenamePaneById { pane_id: PaneId(0) },
        handle_rename_pane_by_id,
    );
    registry.register(&WmAction::ResetPaneName, handle_reset_pane_name);
    registry.register(
        &WmAction::ResetPaneNameById { pane_id: PaneId(0) },
        handle_reset_pane_name_by_id,
    );

    // ── Workspace ──
    registry.register(&WmAction::CreateWorkspace, handle_create_workspace);
    registry.register(&WmAction::RenameWorkspace, handle_rename_workspace);
    registry.register(
        &WmAction::RenameWorkspaceByIdx { ws_idx: 0 },
        handle_rename_workspace_by_idx,
    );
    registry.register(&WmAction::ResetWorkspaceName, handle_reset_workspace_name);
    registry.register(
        &WmAction::ResetWorkspaceNameByIdx { ws_idx: 0 },
        handle_reset_workspace_name_by_idx,
    );

    // ── Sidebar / Chrome ──
    registry.register(&WmAction::SidebarLeft, handle_sidebar_left);
    registry.register(&WmAction::SidebarRight, handle_sidebar_right);
    registry.register(&WmAction::FocusDock { dock: None }, handle_focus_dock);
    registry.register(
        &WmAction::ClearSearchHistory { scope: None },
        crate::handlers::handle_clear_search_history,
    );
    registry.register(
        &WmAction::ClearSearchRanking { scope: None },
        crate::handlers::handle_clear_search_ranking,
    );
    registry.register(&WmAction::UnfocusDock, handle_unfocus_dock);
    registry.register(
        &WmAction::CollapseCurrentWorkspace,
        handle_collapse_current_workspace,
    );
    registry.register(
        &WmAction::ExpandCurrentWorkspace,
        handle_expand_current_workspace,
    );
    registry.register(
        &WmAction::ToggleCurrentWorkspaceCollapsed,
        handle_toggle_current_workspace_collapsed,
    );
    registry.register(
        &WmAction::CollapseCurrentColumn,
        handle_collapse_current_column,
    );
    registry.register(&WmAction::ExpandCurrentColumn, handle_expand_current_column);
    registry.register(
        &WmAction::ToggleCurrentColumnCollapsed,
        handle_toggle_current_column_collapsed,
    );

    // ── System ──
    registry.register(&WmAction::CommandPalette, handle_command_palette);
    registry.register(
        &WmAction::SpawnCommand {
            command: String::new(),
            kind: SpawnKind::Terminal,
            float: false,
            close_policy: PaneClosePolicy::default(),
        },
        handle_spawn_command,
    );
    registry.register(
        &WmAction::DismissNotification { notification_id: 0 },
        handle_dismiss_notification,
    );
    registry.register(
        &WmAction::DismissLastNotification,
        handle_dismiss_last_notification,
    );
    registry.register(&WmAction::ReloadConfig, handle_reload_config);
    registry.register(
        &WmAction::OpenLink {
            url: String::new(),
        },
        handle_open_link,
    );

    // ── Font zoom (terminal) ──
    registry.register(
        &WmAction::AppFontZoom {
            step: input::FontZoomStep::In,
        },
        handle_app_font_zoom,
    );
    registry.register(
        &WmAction::PaneTerminalFontZoom {
            pane_id: None,
            step: input::FontZoomStep::In,
        },
        handle_pane_terminal_font_zoom,
    );

    // ── Sidebar-specific (parameterized) ──
    registry.register(
        &WmAction::AddPaneToColumn {
            ws_idx: 0,
            col_idx: 0,
        },
        handle_add_pane_to_column,
    );
    registry.register(
        &WmAction::AddColumnToWorkspace { ws_idx: 0 },
        handle_add_column_to_workspace,
    );

    // ── Destructive ──
    registry.register(
        &WmAction::DeleteColumn {
            ws_idx: 0,
            col_idx: 0,
        },
        handle_delete_column,
    );
    registry.register(
        &WmAction::DeleteWorkspace { ws_idx: 0 },
        handle_delete_workspace,
    );
    registry.register(
        &WmAction::DeleteCurrentColumn,
        handle_delete_current_column,
    );

    // ── Take ──
    // (registered above with PaneTake/PaneTakeAndFocus)

    // ── Mode ──
    registry.register(
        &WmAction::EnterMode {
            name: String::new(),
        },
        handle_enter_mode,
    );

    // ── Scrollback (host terminal viewport) ──
    registry.register(&WmAction::ScrollbackPageUp, handle_scrollback_page_up);
    registry.register(&WmAction::ScrollbackPageDown, handle_scrollback_page_down);
    registry.register(
        &WmAction::ScrollbackLineUp { amount: 3 },
        handle_scrollback_line_up,
    );
    registry.register(
        &WmAction::ScrollbackLineDown { amount: 3 },
        handle_scrollback_line_down,
    );
    registry.register(&WmAction::ScrollbackToTop, handle_scrollback_to_top);
    registry.register(&WmAction::ScrollbackToBottom, handle_scrollback_to_bottom);
    registry.register(&WmAction::ExitScrollback, handle_exit_scrollback);

    // ── Direct scroll (no selection mode / caret) ──
    registry.register(&WmAction::ScrollLineUp, handle_scroll_line_up);
    registry.register(&WmAction::ScrollLineDown, handle_scroll_line_down);
    registry.register(&WmAction::ScrollPageUp, handle_scroll_page_up);
    registry.register(&WmAction::ScrollPageDown, handle_scroll_page_down);
    registry.register(&WmAction::ScrollToTop, handle_scroll_to_top);
    registry.register(&WmAction::ScrollToBottom, handle_scroll_to_bottom);
    registry.register(&WmAction::ScrollPageLeft, handle_scroll_page_left);
    registry.register(&WmAction::ScrollPageRight, handle_scroll_page_right);
    registry.register(&WmAction::ScrollToLeftEdge, handle_scroll_to_left_edge);
    registry.register(&WmAction::ScrollToRightEdge, handle_scroll_to_right_edge);
    registry.register(
        &WmAction::ScrollToOffset { rows: 0 },
        handle_scroll_to_offset,
    );

    // ── Selection (host capability) ──
    registry.register(&WmAction::EnterSelectionMode, handle_enter_selection_mode);
    registry.register(&WmAction::SelectionLeft, handle_selection_left);
    registry.register(&WmAction::SelectionRight, handle_selection_right);
    registry.register(&WmAction::SelectionUp, handle_selection_up);
    registry.register(&WmAction::SelectionDown, handle_selection_down);
    registry.register(&WmAction::ClearSelection, handle_clear_selection);
    registry.register(&WmAction::CopySelection, handle_copy_selection);
    registry.register(&WmAction::PasteClipboard, handle_paste_clipboard);
    registry.register(&WmAction::BeginSelection, handle_begin_selection);
    registry.register(
        &WmAction::ToggleSelectionEndpoint,
        handle_toggle_selection_endpoint,
    );
    registry.register(&WmAction::OpenLinkAtCaret, handle_open_link_at_caret);
    registry.register(&WmAction::SearchScrollback, handle_search_scrollback);
    registry.register(&WmAction::SearchNextMatch, handle_search_next_match);
    registry.register(&WmAction::SearchPrevMatch, handle_search_prev_match);

    registry
}

#[cfg(test)]
mod tests {
    use super::{
        action_ref_from_config, bind_global_focus, binding_arg_problems,
        build_component_keymaps, build_keymap, build_modes, build_registry, build_widget_keymap,
    };
    use crate::app::conflicts::{Conflicts, format_combo};
    use crate::input::WmAction;
    use crate::keymap::{ActionRef, BindingIndex, KeyCombo, KeymapRegistry};
    use heca_config::theme::{KeyModeConfig, ModeBindingConfig};
    use std::collections::HashMap;

    #[test]
    fn widget_keymap_maps_default_bindings_by_axis() {
        use heca_grid_ui::{GridKey, Modifiers, WidgetIntent};
        let km = build_widget_keymap(&heca_config::theme::Config::default());
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        let meta = Modifiers { meta: true, ..Default::default() };
        let none = Modifiers::default();
        // Horizontal item_* (←/→, Ctrl+h/l).
        assert_eq!(km.resolve(GridKey::ArrowRight, none), &[WidgetIntent::ItemNext]);
        assert_eq!(km.resolve(GridKey::Char('l'), ctrl), &[WidgetIntent::ItemNext]);
        assert_eq!(km.resolve(GridKey::ArrowLeft, none), &[WidgetIntent::ItemPrevious]);
        // Vertical menu_* (↑/↓, Ctrl+k/j) — what an open context menu / palette / Select
        // navigates by (see `ContextMenu::event`, which acts on these intents).
        assert_eq!(km.resolve(GridKey::ArrowDown, none), &[WidgetIntent::MenuDown]);
        assert_eq!(km.resolve(GridKey::ArrowUp, none), &[WidgetIntent::MenuUp]);
        assert_eq!(km.resolve(GridKey::Char('j'), ctrl), &[WidgetIntent::MenuDown]);
        assert_eq!(km.resolve(GridKey::Char('k'), ctrl), &[WidgetIntent::MenuUp]);
        // Shared + edits.
        assert_eq!(km.resolve(GridKey::Enter, none), &[WidgetIntent::Activate]);
        assert_eq!(km.resolve(GridKey::Escape, none), &[WidgetIntent::Dismiss]);
        assert_eq!(km.resolve(GridKey::Char('u'), ctrl), &[WidgetIntent::EditDeleteToLineStart]);
        assert_eq!(km.resolve(GridKey::Char('a'), ctrl), &[WidgetIntent::EditSelectAll]);
        assert_eq!(km.resolve(GridKey::Char('a'), meta), &[WidgetIntent::EditSelectAll]);
        // The Ctrl+h overload: edit-first (field-first), then the horizontal nav.
        assert_eq!(
            km.resolve(GridKey::Char('h'), ctrl),
            &[WidgetIntent::EditDeleteBack, WidgetIntent::ItemPrevious],
        );
    }

    #[test]
    fn widget_key_names_are_not_wm_actions() {
        // The [keys.widgets] names live only in the widget keymap — NOT WmActions, so
        // `build_keymap` skips them and they never bind into normal/global.
        for name in [
            "item_previous",
            "item_next",
            "menu_up",
            "menu_down",
            "activate",
            "dismiss",
            "edit_delete_back",
            "edit_delete_to_line_start",
            "edit_select_all",
        ] {
            assert!(
                crate::input::action_from_name(name).is_none(),
                "{name} must not be a WmAction (widget-keys-config is a separate map)",
            );
        }
    }

    // ── plugin-04 / T2: config bindings can target dynamic action ids ──

    /// NON-REGRESSION: every binding in the DEFAULT keymap still resolves to a `Builtin` at load.
    /// `build_keymap` no longer skips unresolvable names (they become `Dynamic`), so a typo — or a
    /// rename — in `keybindings.default.toml` would silently degrade a real binding into a dynamic
    /// one that no-ops at press. This test is what stops that.
    #[test]
    fn every_default_binding_still_resolves_to_a_builtin_at_load() {
        let config = heca_config::theme::Config::default();
        let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        for mode in ["normal", "global"] {
            let Some(bindings) = keymap.bindings_in_mode(mode) else {
                continue;
            };
            for (combo, action) in bindings {
                assert!(
                    matches!(action, ActionRef::Builtin(_)),
                    "default binding {mode}:{} degraded to Dynamic — the action name in \
                     keybindings.default.toml does not resolve",
                    format_combo(combo),
                );
            }
        }
    }

    /// A binding to an action id that is UNKNOWN at load is **not** an error and is **not** dropped:
    /// it becomes a `Dynamic` ref that resolves at press. This is G3 — config is read before any
    /// provider or plugin has registered its actions.
    #[test]
    fn a_binding_to_an_unregistered_action_id_becomes_dynamic() {
        let action = action_ref_from_config("plugin.docker.restart", &HashMap::new());
        match action {
            ActionRef::Dynamic(intent) => {
                assert_eq!(intent.action, "plugin.docker.restart");
                assert!(intent.args.is_empty());
            }
            other => panic!("expected Dynamic, got {other:?}"),
        }
    }

    /// A dynamic binding carries its config `args` into the Intent, so the handler receives them at
    /// press exactly as a parameterized built-in would.
    #[test]
    fn a_dynamic_binding_carries_its_config_args_into_the_intent() {
        let mut args = HashMap::new();
        args.insert("container".to_string(), "web".to_string());
        match action_ref_from_config("plugin.docker.restart", &args) {
            ActionRef::Dynamic(intent) => {
                assert_eq!(
                    intent.args.get("container"),
                    Some(&crate::chrome::PropValue::Text("web".to_string()))
                );
            }
            other => panic!("expected Dynamic, got {other:?}"),
        }
    }

    /// A built-in name still resolves at LOAD, unit and parameterized alike — unchanged behaviour,
    /// including the arg parsing (a malformed arg still fails at load, not at press).
    #[test]
    fn builtin_names_still_resolve_at_load() {
        assert_eq!(
            action_ref_from_config("reload_config", &HashMap::new()),
            ActionRef::Builtin(WmAction::ReloadConfig)
        );

        let mut args = HashMap::new();
        args.insert("rows".to_string(), "5".to_string());
        assert_eq!(
            action_ref_from_config("scroll_to_offset", &args),
            ActionRef::Builtin(WmAction::ScrollToOffset { rows: 5 })
        );

        // A parameterized built-in with UNPARSEABLE args does not silently become a dynamic action
        // named `scroll_to_offset` — `action_from_name` catches it as the unit fallback, and when
        // there is no unit variant either it is a dynamic ref, which the press path warns about.
        let mut bad = HashMap::new();
        bad.insert("rows".to_string(), "not-a-number".to_string());
        assert!(matches!(
            action_ref_from_config("scroll_to_offset", &bad),
            ActionRef::Dynamic(_)
        ));
    }

    /// **Every catalogued action can actually run.** Metadata lives in `ActionRegistry::ALL` and
    /// handlers in `build_registry()`, two places that could drift; this closes the direction that
    /// matters — a descriptor whose action has no handler is an entry the whole UI advertises
    /// (icon, label, command palette, `list-actions`) and that panics in debug when pressed.
    ///
    /// The other direction — a handler with no descriptor — is already held by
    /// `every_wm_action_variant_is_reachable_by_name` in `input.rs`, which walks the variants
    /// rather than the names. Between them the two lists cannot fall out of step, which is the
    /// property F003/P010/T005 exists to guarantee.
    #[test]
    fn every_catalogued_action_has_a_handler() {
        use crate::actions::{ActionRegistry, ArgSpec, sample_args};
        use crate::input::{action_from_name, build_action};

        let registry = build_registry();
        let mut missing = Vec::new();
        for descriptor in ActionRegistry::ALL {
            let args: Vec<ArgSpec> = descriptor.args.iter().map(ArgSpec::from_descriptor).collect();
            let Some(action) = action_from_name(descriptor.name)
                .or_else(|| build_action(descriptor.name, &sample_args(&args)))
            else {
                // Not this test's business: `every_wm_action_variant_is_reachable_by_name` owns it.
                continue;
            };
            if !registry.has_handler(&action) {
                missing.push(descriptor.name);
            }
        }
        assert!(
            missing.is_empty(),
            "these actions are catalogued — they have a label, an icon and a place in the command \
             palette — but no handler is registered for them, so pressing one panics in debug and \
             does nothing in release: {missing:#?}",
        );
    }

    /// A binding whose `args` do not match what the action declares is **named** at load, instead
    /// of being left for the user to discover by pressing a key that does nothing.
    #[test]
    fn a_bindings_argument_mistakes_are_reported_at_load() {
        use crate::actions::ArgProblem;

        // A well-formed binding says nothing.
        let mut good = HashMap::new();
        good.insert("ws_idx".to_string(), "2".to_string());
        assert!(binding_arg_problems("delete_workspace", &good).is_empty());

        // A misspelled required argument: both halves of the mistake are named.
        let mut typo = HashMap::new();
        typo.insert("ws_idxx".to_string(), "2".to_string());
        let problems = binding_arg_problems("delete_workspace", &typo);
        assert!(problems.contains(&ArgProblem::Missing {
            name: "ws_idx".to_string()
        }));
        assert!(problems.contains(&ArgProblem::Unknown {
            name: "ws_idxx".to_string(),
            did_you_mean: Some("ws_idx".to_string()),
        }));
        // …and, because it cannot be built, it is not quietly bound to workspace 0 either.
        assert!(matches!(
            action_ref_from_config("delete_workspace", &typo),
            ActionRef::Dynamic(_)
        ));

        // An action heca does not know yet is not judged here — it is resolved at press time, when
        // its provider may have registered it.
        assert!(binding_arg_problems("plugin.docker.restart", &typo).is_empty());
    }

    /// `[keys.unbind]` is keyed by the COMBO, so it retires a dynamic binding exactly as it retires
    /// a built-in one.
    #[test]
    fn unbind_retires_a_dynamic_binding_too() {
        let mut keymap = KeymapRegistry::new();
        let combo = KeyCombo::parse("g");
        keymap.bind(
            "normal",
            combo.clone(),
            action_ref_from_config("plugin.docker.restart", &HashMap::new()),
        );
        assert!(keymap.resolve("normal", &combo).is_some());
        assert!(keymap.unbind("normal", &combo).is_some());
        assert!(keymap.resolve("normal", &combo).is_none());
    }

    // ── plugin-04 / T1: chrome placement actions carry stable dotted string ids ──

    /// Each placement id builds the `WmAction` it names, from its args — the same construction a
    /// plugin's Intent, an RPC call and a config binding all go through (`build_action`).
    #[test]
    fn every_chrome_placement_id_builds_its_action_from_args() {
        use crate::chrome::RegionId;
        let args = |pairs: &[(&str, &str)]| -> HashMap<String, String> {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };

        assert_eq!(
            action_ref_from_config(
                "chrome.container.move_to_region",
                &args(&[("container_id", "workspaces"), ("region", "right-sidebar")]),
            ),
            ActionRef::Builtin(WmAction::MoveContainerToRegion {
                container_id: "workspaces".to_string(),
                region: RegionId::RightSidebar,
            })
        );
        // The fixed-region conveniences.
        assert_eq!(
            action_ref_from_config(
                "chrome.container.move_left_sidebar",
                &args(&[("container_id", "workspaces")]),
            ),
            ActionRef::Builtin(WmAction::MoveContainerToRegion {
                container_id: "workspaces".to_string(),
                region: RegionId::LeftSidebar,
            })
        );
        assert_eq!(
            action_ref_from_config(
                "chrome.container.move_right_sidebar",
                &args(&[("container_id", "workspaces")]),
            ),
            ActionRef::Builtin(WmAction::MoveContainerToRegion {
                container_id: "workspaces".to_string(),
                region: RegionId::RightSidebar,
            })
        );
        // `before_id` is optional — omitted ⇒ move to the end of the region.
        assert_eq!(
            action_ref_from_config(
                "chrome.container.reorder_before",
                &args(&[("container_id", "workspaces")]),
            ),
            ActionRef::Builtin(WmAction::ReorderContainerBefore {
                container_id: "workspaces".to_string(),
                before_id: None,
            })
        );
        assert_eq!(
            action_ref_from_config(
                "chrome.container.reorder_after",
                &args(&[("container_id", "workspaces"), ("after_id", "agents")]),
            ),
            ActionRef::Builtin(WmAction::ReorderContainerAfter {
                container_id: "workspaces".to_string(),
                after_id: "agents".to_string(),
            })
        );
    }

    /// A placement id is a BUILT-IN (it has a `WmAction` and a native handler), not a name-keyed
    /// dynamic action — it must not fall through to `Dynamic` when its args are supplied.
    #[test]
    fn a_placement_id_is_a_builtin_not_a_dynamic_action() {
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let registry = build_registry();
        for name in [
            "chrome.container.move_to_region",
            "chrome.container.move_left_sidebar",
            "chrome.container.move_right_sidebar",
            "chrome.container.reorder_before",
            "chrome.container.reorder_after",
        ] {
            let meta = catalog
                .find(name)
                .unwrap_or_else(|| panic!("{name} is not in the catalog"));
            assert_eq!(meta.category, crate::actions::ActionCategory::Chrome);
            // Chrome placement acts on regions, not on the tiled/floating pane domain, so it stays
            // reachable in EITHER domain. `Global` is the only policy that survives a floating pane
            // owning the domain (`AlwaysAllowed` does NOT — it is a misnomer).
            assert_eq!(
                meta.policy,
                crate::app::interaction::ActionPolicy::Global,
                "{name} must stay reachable while a floating pane owns the domain",
            );
            assert_eq!(
                registry.dispatch_of(&catalog, name),
                Some(crate::actions::Dispatch::Native),
                "{name} has a WmAction + native handler",
            );
        }
    }

    /// Surface parity (rule P2): the same capability is reachable from a config binding / plugin
    /// intent (by dotted name) AND from RPC (by kebab command), and both land on the same action.
    #[test]
    fn placement_actions_have_rpc_parity_with_their_dotted_ids() {
        use crate::chrome::RegionId;
        let mut args = HashMap::new();
        args.insert("container_id".to_string(), "workspaces".to_string());
        args.insert("region".to_string(), "right-sidebar".to_string());

        let from_name = action_ref_from_config("chrome.container.move_to_region", &args);
        let from_rpc =
            crate::rpc::parse_rpc_command("move-container-to-region workspaces right-sidebar")
                .unwrap();
        assert_eq!(from_name, ActionRef::Builtin(from_rpc));

        let mut after = HashMap::new();
        after.insert("container_id".to_string(), "workspaces".to_string());
        after.insert("after_id".to_string(), "agents".to_string());
        assert_eq!(
            action_ref_from_config("chrome.container.reorder_after", &after),
            ActionRef::Builtin(
                crate::rpc::parse_rpc_command("reorder-container-after workspaces agents").unwrap()
            )
        );
        // The region spellings come from ONE parser (RegionId's FromStr), so config and RPC can
        // never drift apart: the short alias works on both surfaces.
        assert_eq!("right".parse::<RegionId>(), Ok(RegionId::RightSidebar));
        assert_eq!(
            "right-sidebar".parse::<RegionId>(),
            Ok(RegionId::RightSidebar)
        );
        assert!("nowhere".parse::<RegionId>().is_err());
    }

    /// Dotted built-ins are explicitly allowlisted — chrome placement plus the
    /// contextual notification dismissal action. No snake_case alias is quietly
    /// added, and existing snake_case built-ins remain unrenamed.
    #[test]
    fn dotted_builtin_names_are_explicitly_allowlisted() {
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let dotted: Vec<&str> = crate::actions::ActionRegistry::ALL
            .iter()
            .map(|d| d.name)
            .filter(|n| n.contains('.'))
            .collect();
        assert_eq!(
            dotted,
            [
                "chrome.container.move_to_region",
                "chrome.container.move_left_sidebar",
                "chrome.container.move_right_sidebar",
                "chrome.container.reorder_before",
                "chrome.container.reorder_after",
                "notification.dismiss",
            ]
        );
        // No snake_case alias for the same capability.
        assert!(catalog.find("move_container_to_region").is_none());
        assert!(catalog.find("reorder_container_before").is_none());
    }

    #[test]
    fn chrome_container_placement_actions_have_handlers() {
        // Every WmAction variant must have a registered handler (execute() panics
        // in debug otherwise) — assert the plugin-02 container actions are wired.
        let registry = super::build_registry();
        assert!(registry.has_handler(&WmAction::MoveContainerToRegion {
            container_id: String::new(),
            region: crate::chrome::RegionId::LeftSidebar,
        }));
        assert!(registry.has_handler(&WmAction::ReorderContainerBefore {
            container_id: String::new(),
            before_id: None,
        }));
        assert!(registry.has_handler(&WmAction::ReorderContainerAfter {
            container_id: String::new(),
            after_id: String::new(),
        }));
        assert!(registry.has_handler(&WmAction::SetRegionVisible {
            region: crate::chrome::RegionId::LeftSidebar,
            visible: true,
        }));
    }

    #[test]
    fn default_font_size_modes_build_with_triggers_and_keys() {
        use crate::input::FontZoomStep;
        let config = heca_config::theme::Config::default();
        let (mode_keymaps, mode_triggers) = build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());

        // Both modes exist, are sticky, and are entered by prefix+! / prefix+@.
        let (app_trigger, app_sticky) = mode_triggers
            .get("app_font_size")
            .expect("app_font_size mode trigger");
        assert!(app_sticky, "app_font_size must be sticky");
        assert_eq!(*app_trigger, KeyCombo::parse("!"));
        let (pane_trigger, pane_sticky) = mode_triggers
            .get("pane_font_size")
            .expect("pane_font_size mode trigger");
        assert!(pane_sticky, "pane_font_size must be sticky");
        assert_eq!(*pane_trigger, KeyCombo::parse("@"));

        // Inner keys resolve to the right actions (k/ArrowUp = bigger, j = smaller,
        // 0 = reset) — whole-app for app_font_size, focused pane for pane_font_size.
        let app = mode_keymaps.get("app_font_size").unwrap();
        assert_eq!(
            app.resolve_builtin("app_font_size", &KeyCombo::parse("k")),
            Some(&WmAction::AppFontZoom {
                step: FontZoomStep::In
            })
        );
        assert_eq!(
            app.resolve_builtin("app_font_size", &KeyCombo::parse("ArrowUp")),
            Some(&WmAction::AppFontZoom {
                step: FontZoomStep::In
            })
        );
        assert_eq!(
            app.resolve_builtin("app_font_size", &KeyCombo::parse("j")),
            Some(&WmAction::AppFontZoom {
                step: FontZoomStep::Out
            })
        );
        assert_eq!(
            app.resolve_builtin("app_font_size", &KeyCombo::parse("0")),
            Some(&WmAction::AppFontZoom {
                step: FontZoomStep::Reset
            })
        );

        let pane = mode_keymaps.get("pane_font_size").unwrap();
        assert_eq!(
            pane.resolve_builtin("pane_font_size", &KeyCombo::parse("k")),
            Some(&WmAction::PaneTerminalFontZoom {
                pane_id: None,
                step: FontZoomStep::In
            })
        );
        assert_eq!(
            pane.resolve_builtin("pane_font_size", &KeyCombo::parse("0")),
            Some(&WmAction::PaneTerminalFontZoom {
                pane_id: None,
                step: FontZoomStep::Reset
            })
        );
    }

    #[test]
    fn default_ctrl_k_binding_stays_swap_up() {
        let config = heca_config::theme::Config::default();
        let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());

        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Ctrl+k")),
            Some(&WmAction::SwapUp)
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Ctrl+j")),
            Some(&WmAction::SwapDown)
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Ctrl+Shift+k")),
            Some(&WmAction::MoveColumnUp)
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Ctrl+Shift+j")),
            Some(&WmAction::MoveColumnDown)
        );
    }

    #[test]
    fn default_font_zoom_bindings_resolve_without_collision() {
        use crate::input::FontZoomStep;
        let config = heca_config::theme::Config::default();
        let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());

        // Global (app-wide) branch: prefix+Ctrl+= / - / 0.
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Ctrl+=")),
            Some(&WmAction::AppFontZoom {
                step: FontZoomStep::In
            })
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Ctrl+-")),
            Some(&WmAction::AppFontZoom {
                step: FontZoomStep::Out
            })
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Ctrl+0")),
            Some(&WmAction::AppFontZoom {
                step: FontZoomStep::Reset
            })
        );

        // Focused-pane branch: prefix+Ctrl+Shift+= / - / 0 (pane_id resolved at
        // dispatch). Ctrl+Shift is used instead of Alt because macOS rewrites the
        // character under the Option key, so Alt+= would never match.
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Ctrl+Shift+=")),
            Some(&WmAction::PaneTerminalFontZoom {
                pane_id: None,
                step: FontZoomStep::In
            })
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Ctrl+Shift+-")),
            Some(&WmAction::PaneTerminalFontZoom {
                pane_id: None,
                step: FontZoomStep::Out
            })
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Ctrl+Shift+0")),
            Some(&WmAction::PaneTerminalFontZoom {
                pane_id: None,
                step: FontZoomStep::Reset
            })
        );

        // No collision: the bare `=`/`-` (resize) and Shift+`=` (pane height) keys
        // keep their original actions — the Ctrl / Ctrl+Shift variants are distinct.
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("=")),
            Some(&WmAction::ResizeIncrease)
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("-")),
            Some(&WmAction::ResizeDecrease)
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Shift+=")),
            Some(&WmAction::PaneHeightIncrease)
        );
        // The other Ctrl+Shift bindings (move column up/down) keep their actions.
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Ctrl+Shift+k")),
            Some(&WmAction::MoveColumnUp)
        );
    }

    #[test]
    fn default_workspace_aliases_include_ctrl_p_and_ctrl_n() {
        let config = heca_config::theme::Config::default();
        let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());

        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("u")),
            Some(&WmAction::WorkspacePrev)
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("d")),
            Some(&WmAction::WorkspaceNext)
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Ctrl+p")),
            Some(&WmAction::WorkspacePrev)
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Ctrl+n")),
            Some(&WmAction::WorkspaceNext)
        );
    }

    #[test]
    fn default_sidebar_global_collapse_bindings_exist() {
        let config = heca_config::theme::Config::default();
        let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());

        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("(")),
            Some(&WmAction::ToggleCurrentColumnCollapsed)
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("<")),
            Some(&WmAction::ToggleCurrentWorkspaceCollapsed)
        );
    }

    #[test]
    fn default_pane_navigation_and_palette_bindings_are_separate() {
        let config = heca_config::theme::Config::default();
        let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());

        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("[")),
            Some(&WmAction::PrevPane)
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("]")),
            Some(&WmAction::NextPane)
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("p")),
            Some(&WmAction::CommandPalette)
        );
    }

    #[test]
    fn default_selection_bindings_resolve() {
        let config = heca_config::theme::Config::default();
        let keymap = build_keymap(&config, &mut Conflicts::default(), &mut BindingIndex::new());

        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("s")),
            Some(&WmAction::EnterSelectionMode)
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("Shift+s")),
            Some(&WmAction::ClearSelection)
        );
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("y")),
            Some(&WmAction::CopySelection)
        );
        // paste_clipboard is intentionally not given a default flat binding
        // to avoid colliding with established keys; users bind it in config.
        assert_eq!(
            keymap.resolve_builtin("normal", &KeyCombo::parse("p")),
            Some(&WmAction::CommandPalette)
        );
    }

    #[test]
    fn default_selection_mode_bindings_resolve() {
        let config = heca_config::theme::Config::default();
        let (mode_keymaps, _) = build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let keymap = mode_keymaps
            .get("selection")
            .expect("selection mode exists");

        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("h")),
            Some(&WmAction::SelectionLeft)
        );
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("ArrowLeft")),
            Some(&WmAction::SelectionLeft)
        );
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("l")),
            Some(&WmAction::SelectionRight)
        );
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("ArrowUp")),
            Some(&WmAction::SelectionUp)
        );
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("ArrowDown")),
            Some(&WmAction::SelectionDown)
        );
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("y")),
            Some(&WmAction::CopySelection)
        );
        // BeginSelection: direct keys in selection mode.
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("v")),
            Some(&WmAction::BeginSelection)
        );
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("Space")),
            Some(&WmAction::BeginSelection)
        );
        // ToggleSelectionEndpoint.
        assert_eq!(
            keymap.resolve_builtin("selection", &KeyCombo::parse("o")),
            Some(&WmAction::ToggleSelectionEndpoint)
        );
    }

    // ── Component layers — `[[keys.component]]` (F003/P086/T362) ──

    /// The per-component layers alone. `global_focus` comes back separately (F003/P086/T363) and
    /// has its own tests; every other case here is about what a focused container answers to.
    fn layers_only(
        config: &heca_config::theme::Config,
        conflicts: &mut Conflicts,
        index: &mut BindingIndex,
    ) -> HashMap<String, KeymapRegistry> {
        build_component_keymaps(config, conflicts, index).0
    }


    /// Build a config carrying these `[[keys.component]]` entries, as TOML would produce.
    fn with_layers(
        layers: Vec<heca_config::theme::ComponentKeysConfig>,
    ) -> heca_config::theme::Config {
        let mut config = heca_config::theme::Config::default();
        config.keys.component = layers;
        config
    }

    fn with_layer(layer: heca_config::theme::ComponentKeysConfig) -> heca_config::theme::Config {
        with_layers(vec![layer])
    }

    /// One `[[keys.component]]` entry: which component, which placement (if narrowed), its bindings.
    fn layer_of(
        name: &str,
        id: Option<&str>,
        entries: &[(&str, &str)],
    ) -> heca_config::theme::ComponentKeysConfig {
        heca_config::theme::ComponentKeysConfig {
            name: name.to_string(),
            id: id.map(str::to_string),
            bindings: entries
                .iter()
                .map(|(a, k)| {
                    (
                        a.to_string(),
                        heca_config::theme::BindingValue::Single(k.to_string()),
                    )
                })
                .collect(),
            ..Default::default()
        }
    }

    /// A component's layer resolves under its **name**, and binds any action id — its own or an
    /// existing built-in it simply reuses.
    #[test]
    fn a_component_layer_binds_its_own_actions_and_existing_ones() {
        let config = with_layer(layer_of(
            "docker",
            None,
            &[("docker.restart_selected", "r"), ("next_pane", "n")],
        ));
        let maps = layers_only(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let docker = maps.get("docker").expect("an id-less entry is keyed by name");

        match docker.resolve("docker", &KeyCombo::parse("r")) {
            Some(ActionRef::Dynamic(intent)) => assert_eq!(intent.action, "docker.restart_selected"),
            other => panic!("its own action resolves at press time: {other:?}"),
        }
        assert_eq!(
            docker.resolve_builtin("docker", &KeyCombo::parse("n")),
            Some(&WmAction::NextPane),
            "an existing action is bound here, not redeclared",
        );
    }

    /// **The table merges per key**: overriding one binding keeps every other default. Two entries
    /// for the same component are now just two entries in the array — the builder layers them.
    #[test]
    fn overriding_one_binding_keeps_the_rest() {
        let config = with_layers(vec![
            layer_of(
                "docker",
                None,
                &[("docker.restart_selected", "r"), ("docker.stop_selected", "s")],
            ),
            layer_of("docker", None, &[("docker.stop_selected", "x")]),
        ]);
        let maps = layers_only(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let docker = &maps["docker"];

        assert!(
            docker.resolve("docker", &KeyCombo::parse("x")).is_some(),
            "the overridden action moved to its new key",
        );
        assert!(
            docker.resolve("docker", &KeyCombo::parse("r")).is_some(),
            "and every other default survived",
        );
    }

    /// **Binding names are short.** `cursor_up` under `name = "workspaces"` is
    /// `workspaces.cursor_up`; a built-in keeps its catalog id, because a component binds those
    /// rather than redeclaring them; and writing the id out in full is never wrong.
    #[test]
    fn a_short_name_means_the_components_own_action() {
        let config = with_layer(layer_of(
            "workspaces",
            None,
            &[
                ("cursor_up", "k"),
                ("next_pane", "n"),
                ("workspaces.peek_selected", "Space"),
            ],
        ));
        let maps = layers_only(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let layer = &maps["workspaces"];

        match layer.resolve("workspaces", &KeyCombo::parse("k")) {
            Some(ActionRef::Dynamic(intent)) => assert_eq!(intent.action, "workspaces.cursor_up"),
            other => panic!("a short name is qualified with the component: {other:?}"),
        }
        assert_eq!(
            layer.resolve_builtin("workspaces", &KeyCombo::parse("n")),
            Some(&WmAction::NextPane),
            "a built-in keeps its own id — it is bound here, not redeclared",
        );
        match layer.resolve("workspaces", &KeyCombo::parse("Space")) {
            Some(ActionRef::Dynamic(intent)) => {
                assert_eq!(intent.action, "workspaces.peek_selected", "never qualified twice")
            }
            other => panic!("an already-qualified name is left alone: {other:?}"),
        }
    }

    // ── The two keys a container has for free (F003/P086/T363) ──

    /// `global_focus` must **not** land in the container's own layer: that layer is only consulted
    /// while the container already holds focus, so a key to *take* focus placed there could never
    /// fire. It goes into the flat map, aimed at this container.
    #[test]
    fn global_focus_goes_to_the_global_map_not_the_containers_own_layer() {
        let config = with_layer(layer_of(
            "docker",
            None,
            &[("global_focus", "prefix+d"), ("restart_selected", "r")],
        ));
        let mut index = BindingIndex::new();
        let (layers, global) =
            build_component_keymaps(&config, &mut Conflicts::default(), &mut index);

        assert!(
            layers["docker"].resolve("docker", &KeyCombo::parse("d")).is_none(),
            "a key that takes focus is useless in the layer that needs focus first",
        );
        assert!(
            layers["docker"].resolve("docker", &KeyCombo::parse("r")).is_some(),
            "…while its ordinary bindings are untouched",
        );

        let mut flat = KeymapRegistry::new();
        bind_global_focus(&mut flat, &global, &mut Conflicts::default(), &mut index);
        assert_eq!(
            flat.resolve_builtin("normal", &KeyCombo::parse("d")),
            Some(&WmAction::FocusDock { dock: Some("docker".to_string()) }),
            "`prefix+d` lands in the leader map, aimed at this container",
        );
        assert!(
            index["focus_dock"]
                .iter()
                .any(|b| b.key == "prefix+d" && b.layer.contains("docker")),
            "and --keys-show can say where it came from: {index:?}",
        );
    }

    /// An `id` aims the key at **one seating**; without one it names the component, and
    /// `placement_for` resolves that at press time to the seating the user was last in.
    #[test]
    fn an_id_aims_global_focus_at_one_placement() {
        let config = with_layers(vec![
            layer_of("docker", None, &[("global_focus", "prefix+d")]),
            layer_of("docker", Some("docker.right"), &[("global_focus", "prefix+Shift+d")]),
        ]);
        let (_, global) =
            build_component_keymaps(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let mut flat = KeymapRegistry::new();
        bind_global_focus(&mut flat, &global, &mut Conflicts::default(), &mut BindingIndex::new());

        assert_eq!(
            flat.resolve_builtin("normal", &KeyCombo::parse("Shift+d")),
            Some(&WmAction::FocusDock { dock: Some("docker.right".to_string()) }),
            "the narrowed entry names its placement",
        );
        assert_eq!(
            flat.resolve_builtin("normal", &KeyCombo::parse("d")),
            Some(&WmAction::FocusDock { dock: Some("docker".to_string()) }),
            "the id-less entry names the component, resolved at press time",
        );
    }

    /// Two components asking for one combo is exactly what the report exists to say out loud.
    #[test]
    fn two_containers_claiming_one_global_focus_is_reported() {
        let config = with_layers(vec![
            layer_of("docker", None, &[("global_focus", "prefix+d")]),
            layer_of("notes", None, &[("global_focus", "prefix+d")]),
        ]);
        let (_, global) =
            build_component_keymaps(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let mut conflicts = Conflicts::default();
        bind_global_focus(
            &mut KeymapRegistry::new(),
            &global,
            &mut conflicts,
            &mut BindingIndex::new(),
        );
        assert_eq!(conflicts.keys.len(), 1, "one combo, two claimants");
    }

    /// **`Esc` is a floor, not a default.** `[[keys.mode]]` arrays are replaced wholesale by a
    /// user's config, so a `focus` block that forgot this line would strand the keyboard in a dock
    /// with only the mouse to get out. It is re-asserted after the merge.
    #[test]
    fn escape_always_releases_a_focused_container() {
        let mut config = heca_config::theme::Config::default();
        // A user's `focus` layer that keeps the paging keys and drops the way out.
        config.keys.mode = vec![heca_config::theme::KeyModeConfig {
            name: crate::app::input::FOCUS_LAYER.to_string(),
            trigger: String::new(),
            sticky: true,
            bindings: vec![heca_config::theme::ModeBindingConfig {
                action: "scroll_page_up".to_string(),
                keys: "PageUp".to_string(),
                args: HashMap::new(),
            }],
        }];
        let (modes, _) = build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let focus = &modes[crate::app::input::FOCUS_LAYER];

        assert_eq!(
            focus.resolve_builtin(crate::app::input::FOCUS_LAYER, &KeyCombo::parse("Escape")),
            Some(&WmAction::UnfocusDock),
            "a container can always be left, whatever the config says",
        );
    }

    /// Putting something *else* on `Escape` in the focus layer is a real collision — the guarantee
    /// wins, and the user is told rather than left wondering.
    #[test]
    fn taking_escape_in_the_focus_layer_is_reported() {
        let mut config = heca_config::theme::Config::default();
        config.keys.mode = vec![heca_config::theme::KeyModeConfig {
            name: crate::app::input::FOCUS_LAYER.to_string(),
            trigger: String::new(),
            sticky: true,
            bindings: vec![heca_config::theme::ModeBindingConfig {
                action: "scroll_to_top".to_string(),
                keys: "Escape".to_string(),
                args: HashMap::new(),
            }],
        }];
        let mut conflicts = Conflicts::default();
        let (modes, _) = build_modes(&config, &mut conflicts, &mut BindingIndex::new());

        assert_eq!(
            modes[crate::app::input::FOCUS_LAYER]
                .resolve_builtin(crate::app::input::FOCUS_LAYER, &KeyCombo::parse("Escape")),
            Some(&WmAction::UnfocusDock),
            "the guarantee wins",
        );
        // Two lines, both true: the user's `Escape` displaced the shipped default, and the
        // guarantee then displaced the user's. A key this consequential deserves the noise.
        assert!(
            conflicts
                .keys
                .iter()
                .any(|c| format_combo(&c.combo).eq_ignore_ascii_case("escape")),
            "and it is not silent: {:?}",
            conflicts.keys,
        );
    }

    /// The index is what `--keys-show` and every tooltip read, so it must carry the **qualified** id
    /// and the layer — and an `unbind` must take its entry out with it (F003/P086/T366).
    #[test]
    fn the_index_records_the_qualified_id_and_forgets_what_is_unbound() {
        let mut layer = layer_of("workspaces", None, &[("cursor_up", "k"), ("cursor_down", "j")]);
        layer.unbind.insert("j".to_string(), true);
        let mut index = BindingIndex::new();
        build_component_keymaps(&with_layer(layer), &mut Conflicts::default(), &mut index);

        let up = &index["workspaces.cursor_up"];
        assert_eq!(up.len(), 1);
        assert_eq!(up[0].key, "k");
        assert_eq!(up[0].layer, "[[keys.component]] workspaces");
        assert!(
            !index.contains_key("workspaces.cursor_down"),
            "a retired key must not be reported as still running the action: {index:?}",
        );
    }

    /// The reason the shape is an array: an `id` narrows an entry to one placement, and that
    /// placement's layer carries the id-less base underneath it — so the mount lookup alone is
    /// enough at press time.
    #[test]
    fn an_id_layers_over_the_base_for_one_placement_only() {
        let config = with_layers(vec![
            layer_of(
                "docker",
                None,
                &[("docker.restart_selected", "r"), ("docker.stop_selected", "s")],
            ),
            layer_of("docker", Some("docker.right"), &[("docker.stop_selected", "x")]),
        ]);
        let maps = layers_only(&config, &mut Conflicts::default(), &mut BindingIndex::new());

        let right = &maps["docker.right"];
        assert!(
            right.resolve("docker.right", &KeyCombo::parse("x")).is_some(),
            "the narrowed binding applies to this placement",
        );
        assert!(
            right.resolve("docker.right", &KeyCombo::parse("r")).is_some(),
            "and the base came with it — one lookup, no merging at press time",
        );

        let base = &maps["docker"];
        assert!(
            base.resolve("docker", &KeyCombo::parse("s")).is_some(),
            "every other placement still has the base binding",
        );
        assert!(
            base.resolve("docker", &KeyCombo::parse("x")).is_none(),
            "and never sees what was written for one seating",
        );
    }

    /// A narrowing entry sees the **whole** base, wherever it was written — which is why the build
    /// runs the id-less entries in a first pass rather than in file order.
    #[test]
    fn a_placement_is_seeded_from_the_finished_base() {
        let config = with_layers(vec![
            layer_of("docker", Some("docker.right"), &[("docker.stop_selected", "x")]),
            // Written *after* the placement entry, and still part of what it inherits.
            layer_of("docker", None, &[("docker.restart_selected", "r")]),
        ]);
        let maps = layers_only(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        assert!(
            maps["docker.right"]
                .resolve("docker.right", &KeyCombo::parse("r"))
                .is_some(),
            "file order does not decide what a placement inherits",
        );
    }

    /// An `id` naming a placement that will never mount is not an error: config is read before
    /// anything mounts, so the layer is simply built and never asked for.
    #[test]
    fn an_id_for_a_placement_that_never_mounts_is_harmless() {
        let mut conflicts = Conflicts::default();
        let config = with_layer(layer_of("docker", Some("nowhere"), &[("docker.stop", "s")]));
        let maps = layers_only(&config, &mut conflicts, &mut BindingIndex::new());
        assert!(maps.contains_key("nowhere"));
        assert!(conflicts.is_empty(), "nothing to report");
    }

    /// **The array merges by `keys`** — the rule that makes the arg-carrying form usable at all.
    /// Replacing the array wholesale (TOML's own rule for arrays) would mean adding one binding
    /// silently drops every shipped default.
    #[test]
    fn an_arg_carrying_override_replaces_only_its_own_combo() {
        use heca_config::theme::ModeBindingConfig;
        let bind = |action: &str, keys: &str, cmd: &str| ModeBindingConfig {
            action: action.to_string(),
            keys: keys.to_string(),
            args: HashMap::from([("command".to_string(), cmd.to_string())]),
        };
        let shipped = heca_config::theme::ComponentKeysConfig {
            name: "docker".to_string(),
            bind: vec![
                bind("spawn_command", "t", "lazydocker"),
                bind("spawn_command", "g", "lazygit"),
            ],
            ..Default::default()
        };
        // The user rebinds `t` only, in a second entry the builder layers over the first.
        let user = heca_config::theme::ComponentKeysConfig {
            name: "docker".to_string(),
            bind: vec![bind("spawn_command", "t", "ctop")],
            ..Default::default()
        };

        let maps = layers_only(
            &with_layers(vec![shipped, user]),
            &mut Conflicts::default(),
            &mut BindingIndex::new(),
        );
        let docker = &maps["docker"];
        match docker.resolve("docker", &KeyCombo::parse("t")) {
            Some(ActionRef::Builtin(WmAction::SpawnCommand { command, .. })) => {
                assert_eq!(command, "ctop", "exactly that combo was replaced")
            }
            other => panic!("unexpected: {other:?}"),
        }
        assert!(
            docker.resolve("docker", &KeyCombo::parse("g")).is_some(),
            "and the other arg-carrying default survived — the whole point of merging by key",
        );
    }

    /// `unbind` is keyed by the **combo**, so it retires a binding whatever it points at — never a
    /// null or empty-string convention.
    #[test]
    fn a_component_layer_unbinds_by_combo() {
        let mut layer = layer_of("docker", None, &[("docker.stop_selected", "s")]);
        layer.unbind.insert("s".to_string(), true);
        let maps = layers_only(&with_layer(layer), &mut Conflicts::default(), &mut BindingIndex::new());
        assert!(maps["docker"].resolve("docker", &KeyCombo::parse("s")).is_none());
    }

    /// The focus layer ships the keys the **widgets** answer, and is never enterable by a trigger
    /// — it is entered by focusing a dock (F003/P085/T352).
    #[test]
    fn the_focus_layer_ships_the_scroll_keys_and_the_way_out() {
        let config = heca_config::theme::Config::default();
        let (mode_keymaps, mode_triggers) = build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let focus = mode_keymaps
            .get(crate::app::input::FOCUS_LAYER)
            .expect("the focus layer is a built-in mode keymap");

        for (key, want) in [
            ("PageUp", WmAction::ScrollPageUp),
            ("PageDown", WmAction::ScrollPageDown),
            ("Home", WmAction::ScrollToTop),
            ("End", WmAction::ScrollToBottom),
            ("Alt+PageUp", WmAction::ScrollPageLeft),
            ("Alt+PageDown", WmAction::ScrollPageRight),
            ("Alt+Home", WmAction::ScrollToLeftEdge),
            ("Alt+End", WmAction::ScrollToRightEdge),
            ("Escape", WmAction::UnfocusDock),
        ] {
            assert_eq!(
                focus.resolve_builtin(crate::app::input::FOCUS_LAYER, &KeyCombo::parse(key)),
                Some(&want),
                "{key} must reach {want:?} while a dock holds the keyboard",
            );
        }
        assert!(
            !mode_triggers.contains_key(crate::app::input::FOCUS_LAYER),
            "focus is entered by focusing a dock, never by a key",
        );
    }

    /// …and a user trigger on it is refused, exactly as it is for `sidebar`: the mode's bindings
    /// only mean anything while a dock is focused, so an enterable copy would be a dead mode.
    #[test]
    fn a_user_trigger_on_the_focus_layer_is_refused() {
        let mut config = heca_config::theme::Config::default();
        config.keys.mode.push(KeyModeConfig {
            name: crate::app::input::FOCUS_LAYER.to_string(),
            trigger: "prefix+Shift+f".to_string(),
            sticky: true,
            bindings: Vec::new(),
        });
        let (_, mode_triggers) = build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        assert!(!mode_triggers.contains_key(crate::app::input::FOCUS_LAYER));
    }

    #[test]
    fn user_mode_with_same_name_merges_with_defaults() {
        let mut config = heca_config::theme::Config::default();
        config.keys.mode.push(KeyModeConfig {
            name: "resize".to_string(),
            trigger: "prefix+r".to_string(),
            sticky: true,
            bindings: vec![ModeBindingConfig {
                action: "resize_increase".to_string(),
                keys: "x".to_string(),
                args: HashMap::new(),
            }],
        });

        let (mode_keymaps, mode_triggers) = build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let resize = mode_keymaps.get("resize").expect("resize mode exists");

        assert!(resize.resolve_builtin("resize", &KeyCombo::parse("h")).is_some());
        assert_eq!(
            resize.resolve_builtin("resize", &KeyCombo::parse("x")),
            Some(&WmAction::ResizeIncrease)
        );
        assert_eq!(
            mode_triggers.get("resize"),
            Some(&(KeyCombo::parse("r"), true))
        );
    }
}
