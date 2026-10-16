//! Reading the app's own source, for the lints that guard a shape no unit test can reach.

// Every test file that says `mod common;` compiles its own copy of this module and calls only the
// part it needs, so in any one of them the rest reads as unused.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// All the source of the module at `module` — a path under `heca/src` with no extension, such as
/// `"handlers"` or `"app/events"`.
///
/// That is the file `module.rs` **and**, once the module has been split into a folder, every `.rs`
/// file under `module/`. A lint asks for a module rather than a file so that splitting the module
/// keeps it checking the same code: when `handlers.rs` became `handlers/`, two lints that read the
/// file by path would otherwise have failed to find it, or — worse, for one that looks for a
/// forbidden pattern — found nothing and passed.
pub fn module_source(module: &str) -> String {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    let single = src.join(format!("{module}.rs"));
    if single.is_file() {
        files.push(single);
    }
    let mut nested = Vec::new();
    collect_rs(&src.join(module), &mut nested);
    nested.sort();
    files.extend(nested);
    assert!(
        !files.is_empty(),
        "no source for module `{module}` under heca/src — if it moved, point the lint at its new home"
    );
    files
        .iter()
        .map(|f| std::fs::read_to_string(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display())))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every `.rs` file under `heca/src`, labelled by its path relative to `src` so a failure names
/// something you can open.
pub fn all_sources() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs(&root, &mut files);
    files.sort();
    files
        .into_iter()
        .map(|f| {
            let label = f
                .strip_prefix(&root)
                .unwrap_or(&f)
                .to_string_lossy()
                .replace('\\', "/");
            let src = std::fs::read_to_string(&f).unwrap_or_else(|e| panic!("read {}: {e}", f.display()));
            (label, src)
        })
        .collect()
}

/// The one file under `heca/src` that contains `definition` — a line such as
/// `"pub(crate) fn deliver("` — with its label.
///
/// A lint that guards one function asks for the function, not for the file it lives in today, so
/// moving the file into another folder keeps the lint checking the same code. Exactly one file must
/// match: none means the definition is gone (move the lint with it, never delete it), and several
/// means the marker is not specific enough to name one thing.
pub fn source_defining(definition: &str) -> (String, String) {
    let mut found: Vec<(String, String)> = all_sources()
        .into_iter()
        .filter(|(_, src)| src.contains(definition))
        .collect();
    assert!(
        found.len() == 1,
        "expected exactly one file under heca/src containing `{definition}`, found {}: {:?}",
        found.len(),
        found.iter().map(|(label, _)| label.as_str()).collect::<Vec<_>>(),
    );
    found.remove(0)
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}
