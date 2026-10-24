//! The guard that keeps `Base::focused` meaning one thing.
//!
//! `focused` is the browser's `document.activeElement`: the widget the keyboard is aimed at, and
//! nothing else. It used to be written from five places — a widget aliasing the host's signal into
//! it (so "this overlay is open" and "the keyboard is here" were the same value, and a click that
//! blurred the widget would have closed it), a host setting it by hand on a field it kept, tests
//! flipping it. Each meant something slightly different, and none of the differences showed.
//!
//! Now there is one door, in `heca-grid-ui/src/focus/door.rs`: `Base::focus` / `Base::blur`, and
//! `Base::follow_focus` / `follow_focus_modal` for a surface whose focus is decided by a host signal. This reads every
//! source file in the workspace and fails when anything else writes the flag. It is a lint, not a
//! unit test: what it guards against is a *second path*, which behaviour tests cannot see.

use std::path::{Path, PathBuf};

/// The only code allowed to write the flag.
const THE_DOOR: &str = "heca-grid-ui/src/focus";

/// What a writer looks like. (`focused: signal(false)` — a constructor — is not one of them.)
const WRITES: &[&str] = &[
    ".focused.set(",
    ".focused = ",
    ".focused.update(",
    ".focus_visible.set(",
];

fn writes_focus_flag(path: &Path, line: &str) -> bool {
    let code = line.split("//").next().unwrap_or("");
    // Input's private View stores a bool for painting, not Base's focus signal.
    let code = if path == Path::new("heca-grid-ui/src/widgets/input/component.rs") {
        code.replace("view.focused = focused;", "")
    } else {
        code.to_owned()
    };
    WRITES.iter().any(|write| code.contains(write))
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace root")
        .to_path_buf()
}

fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !matches!(
                name.as_ref(),
                "target" | ".git" | ".planner" | "node_modules"
            ) {
                sources(&path, found);
            }
        } else if name.ends_with(".rs") {
            found.push(path);
        }
    }
}

#[test]
fn only_the_focus_door_writes_the_focused_flag() {
    let root = workspace();
    let mut files = Vec::new();
    sources(&root, &mut files);
    assert!(files.len() > 100, "found the workspace's sources");

    let mut offenders = Vec::new();
    for file in files {
        let relative = file.strip_prefix(&root).unwrap_or(&file);
        if relative.starts_with(THE_DOOR) || relative.ends_with("tests/focus_door.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (n, line) in text.lines().enumerate() {
            if writes_focus_flag(relative, line) {
                offenders.push(format!("{}:{}: {}", relative.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these write `Base::focused` directly. Go through the door instead — `base.focus(visible)` / \
         `base.blur()`, or `base.follow_focus(signal)` for a surface whose focus a host decides — \
         so that who holds the keyboard is answered in one place:\n  {}",
        offenders.join("\n  "),
    );
}

#[test]
fn the_input_paint_copy_does_not_hide_focus_signal_writes() {
    let input = Path::new("heca-grid-ui/src/widgets/input/component.rs");
    assert!(!writes_focus_flag(input, "view.focused = focused;"));
    for write in [
        "self.base.focused = signal(false);",
        "self.base.focused.set(true);",
        "self.base.focused.update(|value| *value = true);",
        "self.base.focus_visible.set(true);",
        "view.focused = focused; self.base.focused.set(true);",
    ] {
        assert!(writes_focus_flag(input, write), "missed: {write}");
    }
    assert!(writes_focus_flag(
        Path::new("heca/src/other.rs"),
        "view.focused = focused;"
    ));
}
