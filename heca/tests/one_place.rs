//! **Some things are said in one place, and a second hand-written copy fails here.**
//!
//! Each rule below was found written out by hand at several call sites, each copy working, none of
//! them failing anything — which is why a copy is never caught by a behaviour test. The owner now
//! says it once; these read the app's code and name any line that says it again.

mod common;

use common::app_code;

/// Every app line containing all of `needles`, outside the files in `owners`.
fn said_outside(needles: &[&str], owners: &[&str]) -> Vec<String> {
    app_code()
        .into_iter()
        .filter(|(file, _, line)| {
            !owners.iter().any(|owner| file.starts_with(owner))
                && needles.iter().all(|needle| line.contains(needle))
        })
        .map(|(file, n, line)| format!("heca/src/{file}:{n}  {}", line.trim()))
        .collect()
}

/// A model intent — an action name and its arguments — is sent through the chrome emitter's own
/// door, which knows how the app carries one. A caller that wraps it by hand has to know that, and
/// six callers did.
#[test]
fn a_model_intent_is_sent_by_the_emitter() {
    let found = said_outside(&["fire(", "InteractionIntent::View("], &["chrome/input/emitter.rs"]);
    assert!(
        found.is_empty(),
        "these wrap a model intent by hand:\n  {}\n\nSend it with `emit.fire_intent(intent)`, or \
         hand `emit.intent_sink()` to `realize`.",
        found.join("\n  "),
    );
}

/// Closing or answering an overlay is said by the overlay's id. A caller that spells the action
/// out has to know which action closes it and how it is addressed, and four callers did.
#[test]
fn an_overlay_is_closed_and_answered_through_its_id() {
    let found = said_outside(&["overlay: Some("], &["chrome/surfaces/overlay/spec.rs"]);
    assert!(
        found.is_empty(),
        "these address an overlay by hand:\n  {}\n\nUse `id.closer(&emit)` or \
         `id.submitter(action, &emit)`.",
        found.join("\n  "),
    );
}

/// A described tree is built with the typed builders (`heca_view::build`), which name every
/// property the kind takes. A raw node takes any string key, so a misspelt one is silently ignored.
#[test]
fn app_code_builds_described_nodes_with_the_typed_builders() {
    let found = said_outside(&["ViewNode::new("], &[]);
    assert!(
        found.is_empty(),
        "these build a raw node:\n  {}\n\nUse the kind's builder in `heca_view::build`.",
        found.join("\n  "),
    );
}
