//! Tests across the whole input module.

use super::*;

/// Every `WmAction` is reachable by **name** — a unit variant through `action_from_name`, a
/// parameterized one through `build_action` fed from the arguments its descriptor **declares**.
///
/// This is the direction the round-trip tests in `actions.rs` cannot cover. They walk the
/// declarations and check the code agrees; this walks the variants and checks a declaration
/// exists. Add a parameterized arm to `build_action` and forget its `ActionDescriptor` and the
/// new variant lands here with nothing to build it — which is how twenty-three argument-taking
/// actions went uncatalogued for months.
#[test]
fn every_wm_action_variant_is_reachable_by_name() {
    use crate::actions::builtins;
    use crate::args::{ArgSpec, sample_args};

    use strum::IntoEnumIterator;

    let mut reachable: std::collections::HashSet<WmActionKind> = std::collections::HashSet::new();
    for descriptor in builtins() {
        let args: Vec<ArgSpec> = descriptor
            .args
            .iter()
            .map(ArgSpec::from_descriptor)
            .collect();
        let built = resolve_action(descriptor.name, &sample_args(&args));
        let built = built.unwrap_or_else(|| {
            panic!(
                "action {:?} builds from neither its name nor its declared arguments",
                descriptor.name
            )
        });
        reachable.insert(built.kind());
    }

    let missing: Vec<String> = WmActionKind::iter()
        .filter(|k| !reachable.contains(k))
        .map(|k| format!("{k:?}"))
        .collect();
    assert!(
        missing.is_empty(),
        "these actions cannot be reached by name — each needs an `ActionDescriptor` (with its \
         `args` declared, if it takes any): {missing:#?}"
    );
}
