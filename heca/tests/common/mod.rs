//! Reading the app's own source, for the lints that guard a shape no unit test can reach.

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
