//! **A key's `prefix+` is read in one place.**
//!
//! "Is this key after the leader, and in which layer does it bind?" was answered by hand in ten
//! places across five files — each one a `strip_prefix("prefix+")` and a choice between the
//! `"normal"` and `"global"` layers (F003/P082/T509). `keymap::WrittenKey` and
//! `keymap::split_leader` are the one reader now; this keeps it that way.

use std::path::Path;

#[test]
fn only_the_keymap_reads_the_leader_prefix() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") || path.ends_with("keymap.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("readable");
            for (n, line) in text.lines().enumerate() {
                if line.contains(r#"strip_prefix("prefix+")"#)
                    || line.contains(r#"starts_with("prefix+")"#)
                {
                    offenders.push(format!(
                        "{}:{}",
                        path.strip_prefix(&src).unwrap().display(),
                        n + 1
                    ));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these read `prefix+` by hand: {offenders:?}\n\
         Use `keymap::WrittenKey::parse` (to bind — it also names the layer) or \
         `keymap::split_leader` (to show a binding).",
    );
}
