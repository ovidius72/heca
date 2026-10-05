//! **A handler does its action and nothing else.**
//!
//! What every action is followed by — re-syncing focus, asking for a frame, rebuilding what lists
//! the session when its structure changed — is run once by the dispatcher
//! (`ActionRegistry::execute`, through `mutations::after_action`), after any handler. It used to be
//! written into each handler by hand: 52 of them asked for a redraw and 50 called a follow-up
//! themselves, and a handler that forgot one left the screen or the exposé stale with nothing to
//! notice (F003/P082/T509).
//!
//! A **lint**, for the same reason as `by_id_actions.rs`: a handler takes `&mut AppState`, which
//! needs a window, so there is no headless call to make. It reads every `handle_*` body.
//!
//! Not covered, on purpose: a helper whose work also runs later, outside any dispatch — the
//! confirm dialog, or `apply_rename` from the rename dialog's OK — still asks for its own frame.

mod common;

/// Every `pub fn handle_*` in the handlers, as `(name, body)`.
fn handler_bodies(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(at) = rest.find("\npub fn handle_") {
        let from = &rest[at + 1..];
        let name_end = from.find('(').unwrap_or(from.len());
        let name = from["pub fn ".len()..name_end].to_string();
        let end = from.find("\n}\n").map(|e| e + 2).unwrap_or(from.len());
        out.push((name, from[..end].to_string()));
        rest = &from[end..];
    }
    out
}

#[test]
fn no_handler_runs_the_follow_up_itself() {
    let src = common::module_source("handlers");
    let bodies = handler_bodies(&src);
    assert!(
        bodies.len() > 50,
        "found only {} handlers — the reader no longer understands the source",
        bodies.len()
    );
    let manual = [
        "needs_redraw = true",
        "after_layout_change(",
        "after_metadata_change(",
        "after_config_change(",
        "after_mutation_change(",
    ];
    let offenders: Vec<String> = bodies
        .iter()
        .flat_map(|(name, body)| {
            manual
                .iter()
                .filter(|m| body.contains(**m))
                .map(move |m| format!("{name}: {m}"))
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "these handlers run the follow-up themselves: {offenders:?}\n\
         The dispatcher already does it after every action — a redraw, a focus re-sync, and a \
         rebuild of what lists the session when its structure changed. Delete the line. If the \
         work also runs later outside any dispatch (a dialog's callback), put it in a helper that \
         the callback calls, and give the helper its own follow-up.",
    );
}

/// **Only the dispatcher runs a handler.** Code that called one directly — the mouse's drops did —
/// skipped the interaction policy and the follow-up, and had to repeat the follow-up by hand. An
/// action is posted instead (`mouse::dispatch_drop`, a chrome widget's emitter), and the registry
/// is the only place that names a handler.
#[test]
fn nothing_outside_the_registry_calls_a_handler() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable").flatten() {
            let path = entry.path();
            let rel = path
                .strip_prefix(&src)
                .unwrap()
                .to_string_lossy()
                .to_string();
            if path.is_dir() {
                if rel != "handlers" && rel != "app/registry" {
                    stack.push(path);
                }
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") || rel == "handlers.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("readable");
            for (n, line) in text.lines().enumerate() {
                if line.contains("handlers::handle_") {
                    offenders.push(format!("{rel}:{}", n + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these run a handler directly: {offenders:?}\n\
         Post the action instead — `mouse::dispatch_drop` from a gesture, the widget's emitter from \
         chrome — so it goes through the interaction policy and the dispatcher's follow-up.",
    );
}

/// **The follow-up is written once.** A built-in (run by its `WmAction`) and a name-keyed action
/// (run by name) are two ways in, and each used to carry its own copy of "note the session's
/// shape, run, then follow up" — so a change to one would silently miss the other.
#[test]
fn both_ways_to_run_an_action_share_one_follow_up() {
    let src = common::module_source("actions");
    let calls = src.matches("after_action(").count();
    assert_eq!(
        calls, 1,
        "the actions module calls the follow-up in {calls} places — every way to run an action \
         goes through the one step that does it",
    );
}
