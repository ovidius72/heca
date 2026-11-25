//! **Everything that moves with time reads one clock** — `heca_core::clock`, which a test can hold.
//!
//! A layout animation that read the system's clock directly could not be held: a test reading where
//! a pane is drawn before and after a swap saw the animation move between the two reads on a slow
//! machine, and failed on one CI runner in eight. A widget that measured "how long since" with
//! `Instant::elapsed` read the system's clock the same way, hidden inside a method call. This reads
//! the sources of everything that animates and fails on either, naming the line. It is a lint, not a
//! unit test: what it guards against is a call nobody notices, which no behaviour test can see.

use std::path::{Path, PathBuf};

/// Where things move with time: the widget library and the layout's animations.
const ANIMATED: &[&str] = &["../heca-grid-ui/src", "src/layout"];

/// What reads the system's clock behind `heca_core::clock`'s back.
const SYSTEM_CLOCK: &[&str] = &["Instant::now", ".elapsed()"];

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a source folder") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "tests") {
                sources(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn everything_that_moves_with_time_reads_the_one_clock() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ANIMATED {
        sources(&root.join(dir), &mut files);
    }
    let mut found = Vec::new();
    for file in files {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name == "clock.rs" || name.contains("test") {
            continue;
        }
        let text = std::fs::read_to_string(&file).expect("a readable source");
        // Test modules at the end of a file are not what the app runs.
        let code = text.split("#[cfg(test)]").next().unwrap_or_default();
        for (n, line) in code.lines().enumerate() {
            let line_code = line.split("//").next().unwrap_or_default();
            if SYSTEM_CLOCK.iter().any(|call| line_code.contains(call))
                && !line_code.contains("self.elapsed()")
            {
                found.push(format!("{}:{}: {}", file.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "read `heca_core::clock::now()` / `clock::since(..)` instead of the system's clock:\n{}",
        found.join("\n")
    );
}
