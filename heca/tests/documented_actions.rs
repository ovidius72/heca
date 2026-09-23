//! **Every action the docs name must exist.**
//!
//! A binding to an unknown action is deliberately not an error — names resolve at press time so a
//! plugin's action can be bound before the plugin loads. The cost is that a wrong name in the docs
//! produces a key that silently does nothing, and nothing anywhere says so.
//!
//! That is not hypothetical: the README documented `spawn_pane` and `float_active_at` in its live
//! configuration examples for months. Neither has ever existed — the README itself called
//! `spawn_pane` "planned" a thousand lines further down — and every reader who copied one got a
//! dead key with no error to search for. Bindings in `keybindings.default.toml` are checked against
//! the registry at startup; the prose was checked by nobody.
//!
//! A **lint**, because there is nothing to unit-test: the defect is a string in a Markdown file.
//! Same shape as `by_id_actions.rs` and `pick_refusal.rs`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crate sits in the workspace")
        .join(rel)
}

/// Every action name the registry declares.
fn known_actions() -> HashSet<String> {
    let src = std::fs::read_to_string(repo("heca/src/actions.rs")).expect("read the catalog");
    src.lines()
        .filter_map(|l| l.trim().strip_prefix("name: \""))
        .filter_map(|l| l.split('"').next())
        .map(str::to_string)
        .collect()
}

/// Every `action = "…"` written in a doc, with the line it is on.
fn actions_named_in(doc: &str) -> Vec<(usize, String)> {
    let src = std::fs::read_to_string(repo(doc)).unwrap_or_else(|e| panic!("read {doc}: {e}"));
    src.lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let rest = line.trim().strip_prefix("action = \"")?;
            Some((i + 1, rest.split('"').next()?.to_string()))
        })
        .collect()
}

/// A doc may describe an action that is deliberately not built — but it has to SAY so, in the
/// section the reader is in. These are the ones marked as planned; everything else must be real.
const KNOWN_PLANNED: &[&str] = &["spawn_pane", "float_active_at"];

#[test]
fn every_action_named_in_the_docs_exists() {
    let known = known_actions();
    assert!(
        known.contains("spawn_command"),
        "the catalog could not be read — this lint is only as good as that scrape",
    );

    let mut dead = Vec::new();
    for doc in ["README.md", "docs/keybindings.md"] {
        for (line, action) in actions_named_in(doc) {
            // A dotted id belongs to a plugin or provider and registers at runtime, so the static
            // catalog cannot know it — that is the whole point of late resolution.
            if action.contains('.') || known.contains(&action) {
                continue;
            }
            if KNOWN_PLANNED.contains(&action.as_str()) {
                continue;
            }
            dead.push(format!("{doc}:{line} action = \"{action}\""));
        }
    }

    assert!(
        dead.is_empty(),
        "these docs name actions that do not exist, so copying them produces a key that silently \
         does nothing:\n  {}\n\
         Either the name is wrong, or the action is unbuilt — in which case say so where the reader \
         is, and add it to KNOWN_PLANNED here.",
        dead.join("\n  "),
    );
}

/// The planned ones must stay clearly marked as unbuilt, or the exemption above quietly becomes a
/// licence to document fiction.
#[test]
fn an_unbuilt_action_is_marked_as_unbuilt_where_it_is_documented() {
    let readme = std::fs::read_to_string(repo("README.md")).expect("read the README");
    assert!(
        readme.contains("`spawn_pane` DOES NOT EXIST"),
        "the README documents `spawn_pane` with no warning that it is unbuilt — that is how it came \
         to sit in copyable examples for months",
    );
}
