//! Shared test setup for the keymap tests: build a config holding just the surface layers a test
//! names, and read one layer back out.

use super::build_component_keymaps;
use crate::app::conflicts::Conflicts;
use crate::keymap::{BindingIndex, KeymapRegistry};
use std::collections::HashMap;

/// The per-component layers alone. `global_focus` comes back separately (F003/P086/T363) and
/// has its own tests; every other case here is about what a focused container answers to.
pub(super) fn layers_only(
    config: &heca_config::theme::Config,
    conflicts: &mut Conflicts,
    index: &mut BindingIndex,
) -> HashMap<String, KeymapRegistry> {
    build_component_keymaps(config, conflicts, index).0
}

/// Build a config carrying these `[[keys.component]]` entries, as TOML would produce.
pub(super) fn with_layers(
    layers: Vec<heca_config::theme::SurfaceKeysConfig>,
) -> heca_config::theme::Config {
    let mut config = heca_config::theme::Config::default();
    config.keys.component = layers;
    config
}

pub(super) fn with_layer(
    layer: heca_config::theme::SurfaceKeysConfig,
) -> heca_config::theme::Config {
    with_layers(vec![layer])
}

/// One `[[keys.component]]` entry: which component, which placement (if narrowed), its bindings.
pub(super) fn layer_of(
    name: &str,
    id: Option<&str>,
    entries: &[(&str, &str)],
) -> heca_config::theme::SurfaceKeysConfig {
    heca_config::theme::SurfaceKeysConfig {
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
