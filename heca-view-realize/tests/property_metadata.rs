//! The registry cannot advertise a native widget while dropping its generated properties.
use std::{collections::{BTreeMap, BTreeSet}, path::Path};
use heca_view_realize::WidgetRegistry;

fn native_properties(directory: &Path, properties: &mut BTreeMap<String, BTreeSet<String>>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() { native_properties(&path, properties); continue; }
        if path.extension().is_none_or(|extension| extension != "rs") { continue; }
        let source = std::fs::read_to_string(path).unwrap();
        let mut owner = "";
        let mut property = false;
        for line in source.lines().map(str::trim) {
            if let Some(implementation) = line.strip_prefix("impl ") {
                owner = implementation.split([' ', '<', '{']).next().unwrap_or("");
            }
            if line == "#[heca_grid_ui_macros::prop]" { property = true; }
            else if let Some(method) = line.strip_prefix("pub fn ") {
                if property { properties.entry(owner.to_string()).or_default()
                    .insert(method.split(['(', '<']).next().unwrap().to_string()); }
                property = false;
            }
        }
    }
}

#[test]
fn each_registered_native_widgets_generated_properties_are_present_in_its_definition() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../heca-grid-ui/src/widgets");
    let mut properties = BTreeMap::new(); native_properties(&directory, &mut properties);
    let registry = WidgetRegistry::builtins().unwrap();
    let mut missing = Vec::new();
    for definition in registry.definitions() {
        let metadata = definition.metadata();
        for property in properties.get(metadata.native_name).into_iter().flatten() {
            if !metadata.properties.contains(property) {
                missing.push(format!("{}::{property}", metadata.native_name));
            }
        }
    }
    assert!(missing.is_empty(), "native properties omitted by registry metadata: {missing:?}");
}
