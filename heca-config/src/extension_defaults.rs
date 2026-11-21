//! **What an extension ships as its defaults** — its own `config.default.toml` and
//! `keybindings.default.toml`, layered between heca's defaults and the user's files:
//!
//! ```text
//! heca's defaults  →  each extension's defaults  →  the user's config.toml / keybindings.toml  →  the project's
//! ```
//!
//! An extension declares them once, before heca starts (`heca::extension("rooms").defaults(..)`), and
//! from then on every reader of the defaults sees them: the loaded configuration, `KeysConfig::default`
//! (so its keys bind like heca's own, `[[keys.surface]]` included), `--show-config` and `--show-keys`.
//! The user overrides them in the same files and the same way as heca's.
//!
//! **An extension's defaults speak only for that extension.** Its config file may hold only its own
//! `[extensions.<name>]` table; its keys file only bindings of its own actions (`<name>.…`) and its
//! own surfaces (`[[keys.surface]]` named `<name>` or `<name>.…`). Anything else is refused with the
//! reason, so one extension can never change heca's keys or another extension's.

use std::sync::Mutex;

/// One extension's shipped defaults, already checked.
#[derive(Clone, Debug)]
struct Layer {
    name: String,
    /// Its config and keys files, merged into one value.
    value: toml::Value,
}

/// The extensions' defaults, in the order they were declared. A process-wide list: the defaults are
/// read from more than one thread (the window, the server).
static LAYERS: Mutex<Vec<Layer>> = Mutex::new(Vec::new());

/// **Add extension `name`'s shipped defaults** — the text of its `config.default.toml` and
/// `keybindings.default.toml` (either may be empty). Declaring the same extension again replaces its
/// defaults. Refused, with the reason, when a file is not TOML or speaks for something that is not
/// the extension's own.
pub fn add_extension_defaults(name: &str, config: &str, keys: &str) -> Result<(), String> {
    let value = checked(name, config, keys)?;
    let mut layers = LAYERS.lock().unwrap_or_else(|poison| poison.into_inner());
    layers.retain(|layer| layer.name != name);
    layers.push(Layer { name: name.to_owned(), value });
    Ok(())
}

/// **Every extension's defaults layered onto `base`** (heca's own defaults). Tables merge key by key;
/// the keyed lists of `[keys]` — `surface` (and its old spelling `component`) and `bind` — are
/// *added to* heca's, never replace them: an extension brings its own surface, it does not take
/// heca's away.
pub(crate) fn layered_onto(base: toml::Value) -> toml::Value {
    layers().into_iter().fold(base, |base, (_, value)| merge_layer(base, value))
}

/// The extensions' defaults as `(name, value)`, in declaration order — what `--show-config` lists.
pub(crate) fn layers() -> Vec<(String, toml::Value)> {
    LAYERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .map(|layer| (layer.name.clone(), layer.value.clone()))
        .collect()
}

/// The keyed lists of `[keys]` an extension adds to instead of replacing.
const ADDED_LISTS: [&str; 3] = ["surface", "component", "bind"];

fn merge_layer(base: toml::Value, layer: toml::Value) -> toml::Value {
    let toml::Value::Table(mut layer) = layer else { return base };
    let mut base = base;
    if let Some(toml::Value::Table(mut keys)) = layer.remove("keys") {
        if let Some(toml::Value::Table(base_keys)) = base.as_table_mut().and_then(|t| t.get_mut("keys")) {
            for list in ADDED_LISTS {
                if let Some(toml::Value::Array(added)) = keys.remove(list) {
                    match base_keys.get_mut(list) {
                        Some(toml::Value::Array(existing)) => existing.extend(added),
                        _ => {
                            base_keys.insert(list.to_owned(), toml::Value::Array(added));
                        }
                    }
                }
            }
        }
        layer.insert("keys".to_owned(), toml::Value::Table(keys));
    }
    super::loader::deep_merge(base, toml::Value::Table(layer))
}

/// Extension `name`'s two files, parsed, checked and merged into one value — or the reason not.
fn checked(name: &str, config: &str, keys: &str) -> Result<toml::Value, String> {
    let config = parse(name, "config.default.toml", config)?;
    let keys = parse(name, "keybindings.default.toml", keys)?;
    check_config(name, &config)?;
    check_keys(name, &keys)?;
    Ok(super::loader::deep_merge(config, keys))
}

fn parse(name: &str, file: &str, text: &str) -> Result<toml::Value, String> {
    toml::from_str(text).map_err(|error| format!("extension '{name}': its {file} is not valid TOML: {error}"))
}

/// The config file may hold only `[extensions.<name>]`.
fn check_config(name: &str, config: &toml::Value) -> Result<(), String> {
    let table = config.as_table().map_or_else(Default::default, Clone::clone);
    for (key, value) in &table {
        let own = key == "extensions"
            && value.as_table().is_some_and(|extensions| extensions.keys().all(|k| k == name));
        if !own {
            return Err(format!(
                "extension '{name}': its config.default.toml may set only [extensions.{name}], not `{key}`"
            ));
        }
    }
    Ok(())
}

/// The keys file may bind only the extension's own actions and surfaces.
fn check_keys(name: &str, keys: &toml::Value) -> Result<(), String> {
    let own = |id: &str| id == name || id.starts_with(&format!("{name}."));
    let refuse = |what: String| Err(format!("extension '{name}': its keybindings.default.toml {what}"));
    let table = keys.as_table().map_or_else(Default::default, Clone::clone);
    for (key, value) in &table {
        if key != "keys" {
            return refuse(format!("may hold only [keys], not `{key}`"));
        }
        for (entry, value) in value.as_table().into_iter().flatten() {
            match entry.as_str() {
                "surface" | "component" => {
                    for surface in value.as_array().into_iter().flatten() {
                        let surface_name = surface.get("name").and_then(toml::Value::as_str).unwrap_or_default();
                        if !own(surface_name) {
                            return refuse(format!(
                                "may describe only its own surfaces ({name} or {name}.…), not `{surface_name}`"
                            ));
                        }
                    }
                }
                "bind" => {
                    for bind in value.as_array().into_iter().flatten() {
                        let action = bind.get("action").and_then(toml::Value::as_str).unwrap_or_default();
                        if !own(action) {
                            return refuse(format!("may bind only its own actions ({name}.…), not `{action}`"));
                        }
                    }
                }
                action if own(action) && action != name => {}
                other => {
                    return refuse(format!("may bind only its own actions ({name}.…), not `{other}`"));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// heca's own defaults with extension `name`'s layered on — the pure half of what the registry
    /// does, so no test leaves a layer behind for the others (the list is process-wide).
    fn with_layer(name: &str, config: &str, keys: &str) -> toml::Value {
        let heca = crate::loader::config_from_layers("").map(|c| toml::Value::try_from(c).unwrap()).unwrap();
        merge_layer(heca, checked(name, config, keys).unwrap())
    }

    #[test]
    fn an_extensions_defaults_join_heca_s_and_its_surface_adds_to_theirs() {
        let merged = with_layer(
            "rooms",
            "[extensions.rooms]\nhelper = \"auto\"\n",
            "[keys]\n\"rooms.focus\" = \"prefix+Shift+a\"\n[[keys.surface]]\nname = \"rooms\"\ncursor_up = \"k\"\n",
        );
        let config: crate::loader::Config = merged.try_into().unwrap();
        let table: toml::Table = config.extension("rooms").unwrap();
        assert_eq!(table.get("helper").and_then(toml::Value::as_str), Some("auto"), "its setting is a default");
        assert!(config.keys.bindings.contains_key("rooms.focus"), "its key is a default key");
        assert!(config.keys.surfaces().any(|s| s.name == "rooms"), "its surface joins heca's");
        assert!(config.keys.surfaces().any(|s| s.name != "rooms"), "heca's own surfaces are still there");
    }

    #[test]
    fn the_users_file_wins_over_an_extensions_defaults() {
        let merged = with_layer("rooms", "[extensions.rooms]\nhelper = \"auto\"\n", "");
        let user: toml::Value = toml::from_str("[extensions.rooms]\nhelper = \"off\"\n").unwrap();
        let config: crate::loader::Config = crate::loader::deep_merge(merged, user).try_into().unwrap();
        let table: toml::Table = config.extension("rooms").unwrap();
        assert_eq!(table.get("helper").and_then(toml::Value::as_str), Some("off"));
    }

    #[test]
    fn an_extension_cannot_speak_for_heca_or_another_extension() {
        let refused = |config: &str, keys: &str| checked("rooms", config, keys).unwrap_err();
        assert!(refused("[settings]\ntheme = \"x\"\n", "").contains("only [extensions.rooms]"));
        assert!(refused("[extensions.other]\na = 1\n", "").contains("only [extensions.rooms]"));
        assert!(refused("", "[keys]\nfocus_left = \"h\"\n").contains("its own actions"));
        assert!(refused("", "[keys]\nprefix = \"ctrl+a\"\n").contains("its own actions"));
        assert!(refused("", "[[keys.surface]]\nname = \"heca.workspaces\"\nx = \"j\"\n").contains("its own surfaces"));
        assert!(refused("", "[[keys.bind]]\naction = \"spawn_command\"\nkeys = \"t\"\n").contains("its own actions"));
        assert!(refused("", "[font]\n").contains("only [keys]"));
        assert!(refused("not toml", "").contains("not valid TOML"));
        assert!(checked("rooms", "", "").is_ok(), "both files may be empty");
    }
}
