//! **An extension ships its own defaults** — what a program built on heca writes, the way heca-pro
//! does: its settings and keys become defaults like heca's own, and the user's files still win.
//!
//! Its own test binary, because an extension's defaults are process-wide: declaring one here must
//! not reach heca's other tests.

const CONFIG: &str = "[extensions.rooms]\nhelper = \"auto\"\n";
const KEYS: &str = "[keys]\n\"rooms.focus\" = \"prefix+Shift+a\"\n\n[[keys.surface]]\nname = \"rooms\"\ncursor_up = \"k\"\nactivate_selected = \"Enter\"\n";

#[test]
fn an_extensions_defaults_are_defaults_like_hecas_own() {
    let rooms = heca::extension("rooms").defaults(CONFIG, KEYS);
    assert_eq!(rooms.name(), Some("rooms"));

    // Its setting is a default, and the user's file still wins.
    let config = heca_config::loader::config_from_layers("").unwrap();
    let table: toml::Table = config.extension("rooms").unwrap();
    assert_eq!(table.get("helper").and_then(toml::Value::as_str), Some("auto"));
    let user = heca_config::loader::config_from_layers("[extensions.rooms]\nhelper = \"off\"\n").unwrap();
    let table: toml::Table = user.extension("rooms").unwrap();
    assert_eq!(table.get("helper").and_then(toml::Value::as_str), Some("off"));

    // Its keys are default keys: an action's own (not a dock's) and its surface, beside heca's.
    let keys = heca_config::keys::KeysConfig::default();
    assert!(keys.bindings.contains_key("rooms.focus"), "a default key for an action that is not a dock's");
    let surface = keys.surfaces().find(|s| s.name == "rooms").expect("its surface is a default surface");
    assert!(surface.bindings.contains_key("cursor_up"));
    assert!(keys.surfaces().any(|s| s.name != "rooms"), "heca's own surfaces are still there");

    // `--show-config` names where they came from.
    let sources = heca_config::loader::config_sources().unwrap();
    assert!(sources.sources.iter().any(|s| s.key == "extensions.rooms.helper"
        && s.file.to_string_lossy().contains("extension rooms")));
}

#[test]
fn defaults_that_speak_for_heca_are_dropped() {
    // Another extension, so the first test's stay untouched.
    heca::extension("intruder").defaults("", "[keys]\nfocus_left = \"z\"\n");
    let keys = heca_config::keys::KeysConfig::default();
    let focus_left = keys.bindings.get("focus_left").map(|v| v.keys()).unwrap_or_default();
    assert!(!focus_left.contains(&"z"), "an extension cannot rebind heca's own action: {focus_left:?}");
}
