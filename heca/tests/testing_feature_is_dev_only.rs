//! The `testing` feature of `heca-core` is a door for tests, and only tests may open it.
//!
//! It adds fixtures (`heca_core::layout::testing`) that build a session and a window view without a
//! window. If a normal `[dependencies]` entry asked for it, those fixtures would ship in the
//! program. Cargo would not object, so this reads every manifest and fails when the feature is
//! turned on anywhere but `[dev-dependencies]`. A lint rather than a unit test: what it guards
//! against is a line in a manifest, which behaviour cannot see.

use std::path::{Path, PathBuf};

fn manifests(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if name != "target" && !name.to_string_lossy().starts_with('.') {
                manifests(&path, out);
            }
        } else if name == "Cargo.toml" {
            out.push(path);
        }
    }
}

#[test]
fn only_dev_dependencies_turn_the_testing_feature_on() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found = Vec::new();
    manifests(&root, &mut found);
    assert!(!found.is_empty(), "no manifests found under {root:?}");

    let mut offenders = Vec::new();
    for manifest in found {
        let text = std::fs::read_to_string(&manifest).expect("a manifest is readable");
        let mut section = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                section = line.to_string();
            }
            let asks = line.contains("heca-core") && line.contains("\"testing\"");
            if asks && !section.contains("dev-dependencies") {
                offenders.push(format!("{}: {line}", manifest.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "heca-core's `testing` feature may only be enabled from [dev-dependencies]:\n{}",
        offenders.join("\n")
    );
}
