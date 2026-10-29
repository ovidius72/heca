//! Shipped authoring examples must use the same plain composition promised to consumers.

use std::path::{Path, PathBuf};

fn rust_sources(directory: &Path, sources: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).expect("authoring example directory") {
        let path = entry.expect("example entry").path();
        if path.is_dir() {
            rust_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
}

#[test]
fn shipped_authoring_examples_do_not_require_storage_conversions_or_child_list_mutation() {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    rust_sources(&package.join("examples"), &mut sources);
    rust_sources(&package.join("../heca-renderer/examples/showcase"), &mut sources);
    assert!(sources.len() > 30, "the guard must read the complete example tree");

    for path in sources {
        let source = std::fs::read_to_string(&path).expect("authoring example source");
        // Ignore formatting so splitting a conversion over several lines cannot evade the guard.
        let compact: String = source.chars().filter(|character| !character.is_whitespace()).collect();
        for storage in ["Box::new(", "asBox<", "WidgetModel::new(",
            ".into_component(", ".into_node(", ".base_mut().children.push(",
            ".base_mut().children.extend(", ".base_mut().children.insert("] {
            assert!(!compact.contains(storage),
                "{} requires {storage}; use ordinary constructors and child/factory builders",
                path.display());
        }
    }
}

#[test]
fn public_widget_and_factory_method_signatures_never_expose_box_storage() {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    for directory in ["../heca-grid-ui/src", "../heca-view/src", "../heca-view-realize/src", "src/entry"] {
        rust_sources(&package.join(directory), &mut sources);
    }
    sources.push(package.join("src/chrome/docks/contribution.rs"));
    for path in sources {
        if path.components().any(|part| part.as_os_str() == "tests")
            || path.file_stem().is_some_and(|name| name == "tests" || name == "testing") { continue; }
        let source = std::fs::read_to_string(&path).expect("public authoring source");
        let compact: String = source.chars().filter(|character| !character.is_whitespace()).collect();
        for declaration in compact.split("pubfn").skip(1) {
            let signature = declaration.split('{').next().unwrap();
            assert!(!signature.contains("Box<"), "{} exposes storage in pub fn {signature}", path.display());
        }
    }
}
