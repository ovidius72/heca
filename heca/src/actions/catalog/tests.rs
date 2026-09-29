use super::*;
use crate::args::{ArgKind, sample_args};
use crate::input::WmAction;

// ══════════════════════════════════════════════════════════════════════════
//  Declared arguments (action-task-E)
//
//  An action's `args` are a *claim* about code that lives somewhere else —
//  `build_action`'s match arm, which reads each name as a string literal. These four tests are
//  what makes the claim true, in both directions: the three below walk every declaration and
//  check the code agrees, and `every_wm_action_variant_is_reachable_by_name` (in `input.rs`,
//  where the exhaustive variant list lives) walks every action and checks a declaration exists.
//
//  To see them work, misspell one name in a `build_action` arm — `pane_id` → `paneid` — and
//  `every_declared_argument_is_read_by_the_action` fails on that action.
// ══════════════════════════════════════════════════════════════════════════

/// The declarations of every action that takes arguments, as the catalog holds them.
fn declared_args() -> Vec<(&'static str, Vec<ArgSpec>)> {
    builtins()
        .filter(|d| !d.args.is_empty())
        .map(|d| {
            (
                d.name,
                d.args.iter().map(ArgSpec::from_descriptor).collect(),
            )
        })
        .collect()
}

/// Supply exactly what an action declares and it builds. Fails when a declaration names an
/// argument the code does not read, or misses one it requires.
#[test]
fn every_declared_argument_is_read_by_the_action() {
    for (name, specs) in declared_args() {
        let args = sample_args(&specs);
        assert!(
            crate::input::resolve_action(name, &args).is_some(),
            "{name} declares {:?} but build_action refuses that exact call — the declaration \
                 and the arm that reads it have drifted",
            specs.iter().map(|s| &s.name).collect::<Vec<_>>(),
        );
    }
}

/// `required: true` means it. Drop each required argument on its own and the action must
/// refuse to build.
#[test]
fn every_required_argument_is_actually_required() {
    for (name, specs) in declared_args() {
        for spec in specs.iter().filter(|s| s.required) {
            let mut args = sample_args(&specs);
            args.remove(&spec.name);
            assert!(
                crate::input::resolve_action(name, &args).is_none(),
                "{name} builds without '{}', so that argument is not required — declare it \
                     optional, or the caller will never learn it was ignored",
                spec.name,
            );
        }
    }
}

/// `required: false` means it too. An optional argument can be left out and the action still
/// builds, on its declared default.
#[test]
fn every_optional_argument_is_actually_optional() {
    for (name, specs) in declared_args() {
        for spec in specs.iter().filter(|s| !s.required) {
            let mut args = sample_args(&specs);
            args.remove(&spec.name);
            assert!(
                crate::input::resolve_action(name, &args).is_some(),
                "{name} refuses to build without '{}', so that argument is required — say so, \
                     or a caller that omits it gets nothing and no reason",
                spec.name,
            );
        }
    }
}

/// An action that **needs** a target must not be reachable from its bare name.
///
/// This is the line that stops the defect coming back. `action_from_name` used to answer
/// `delete_workspace` with `DeleteWorkspace { ws_idx: 0 }`, so binding that name to a key
/// deleted the *first* workspace — not the focused one, not nothing. Eight actions did that.
/// Now a required argument has to be supplied, and the caller hears about it if it is not.
#[test]
fn every_action_that_needs_a_target_refuses_to_default_it() {
    for d in builtins() {
        if d.args.iter().any(|a| a.required) {
            assert!(
                crate::input::resolve_action(d.name, &std::collections::HashMap::new()).is_none(),
                "{} requires an argument but resolves from its bare name alone — that answer \
                     is a guess, and a silent one",
                d.name,
            );
        }
    }
}

/// Every action that takes arguments says so where a caller can read it — `list-actions` and
/// `describe-action` carry the list, not just the name.
#[test]
fn introspection_carries_the_arguments() {
    let catalog = ActionCatalog::with_builtins();
    let info = catalog.describe("resize").unwrap();
    let names: Vec<&str> = info.args.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["target", "amount", "edge"]);

    let target = &info.args[0];
    assert_eq!(target.kind, ArgKind::Enum);
    assert!(target.values.contains(&"column".to_string()));
    assert!(target.required);

    // **An optional argument says so, and still lists its vocabulary.** `edge` chooses which of
    // a pane's two boundaries a resize moves; omitting it keeps the one the target already
    // owned, so every binding written before it existed is unaffected.
    let edge = info.args.last().expect("resize declares an edge");
    assert_eq!(edge.kind, ArgKind::Enum);
    assert!(!edge.required, "omitting it is what every old binding does");
    assert!(edge.values.contains(&"top".to_string()));

    let json = serde_json::to_string(&info).unwrap();
    let back: ActionInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.args, info.args, "the declaration survives the wire");
}

#[test]
fn test_registry_has_actions() {
    assert!(
        ActionCatalog::with_builtins().all().count() > 0,
        "catalog should not be empty"
    );
}

#[test]
fn test_find_known_action() {
    let catalog = ActionCatalog::with_builtins();
    let desc = catalog.find("focus_left");
    assert!(desc.is_some(), "should find focus_left");
    let desc = desc.unwrap();
    assert_eq!(desc.label, "Focus Column Left");
    assert!(matches!(desc.category, ActionCategory::Navigation));
}

#[test]
fn test_find_unknown_action() {
    assert!(ActionCatalog::with_builtins().find("nonexistent").is_none());
}

#[test]
fn test_selection_action_descriptors_exist() {
    let catalog = ActionCatalog::with_builtins();
    for name in [
        "enter_selection_mode",
        "selection_left",
        "selection_right",
        "selection_up",
        "selection_down",
        "clear_selection",
        "copy_selection",
        "paste_clipboard",
    ] {
        let desc = catalog.find(name);
        assert!(desc.is_some(), "missing descriptor for {name}");
        let desc = desc.unwrap();
        assert!(!desc.label.is_empty(), "{name} label must be set");
        assert!(
            !desc.description.is_empty(),
            "{name} description must be set"
        );
    }
}

#[test]
fn test_by_category() {
    let catalog = ActionCatalog::with_builtins();
    let in_category = |c: ActionCategory| catalog.all().filter(|m| m.category == c).count();
    let nav_count = in_category(ActionCategory::Navigation);
    assert!(nav_count > 0, "should have navigation actions");

    let sys_count = in_category(ActionCategory::System);
    assert!(sys_count > 0, "should have system actions");
}

#[test]
fn test_all_names_unique() {
    let mut names: Vec<&str> = builtins().map(|d| d.name).collect();
    let original_len = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), original_len, "all action names must be unique");
}

// ── plugin-04 / T3: the one runtime registry ──

/// The built-in metadata after seeding is IDENTICAL to the [`builtins`] descriptors it came
/// from — the owned-`String` ripple must not have changed a single value (the non-regression
/// test the task asks for).
#[test]
fn builtin_metadata_survives_the_owned_ripple_unchanged() {
    let catalog = ActionCatalog::with_builtins();
    assert_eq!(catalog.all().count(), builtins().count());
    for (category, d) in builtins_by_category() {
        let m = catalog
            .find(d.name)
            .unwrap_or_else(|| panic!("missing meta for {}", d.name));
        assert_eq!(m.name, d.name);
        assert_eq!(m.label, d.label);
        assert_eq!(m.description, d.description);
        assert_eq!(
            m.category, category,
            "{} is listed with the {category:?} actions, so that is its category",
            d.name,
        );
        assert_eq!(m.icon, d.icon);
        assert!(catalog.is_builtin(d.name));
    }
}

/// Every built-in's `policy` is COMPUTED from `action_policy`'s exhaustive match, never
/// hand-written — so the match stays the single authority and the two cannot drift. This also
/// proves `builtin_policy` resolves all 115 names (it would `unreachable!` otherwise).
#[test]
fn builtin_policy_is_derived_from_the_exhaustive_match() {
    use crate::app::interaction::{ActionPolicy, action_policy};
    let catalog = ActionCatalog::with_builtins();
    for d in builtins() {
        let meta = catalog.find(d.name).unwrap();
        if let Some(action) =
            crate::input::resolve_action(d.name, &std::collections::HashMap::new())
        {
            assert_eq!(
                meta.policy,
                action_policy(&action),
                "{}: catalog policy diverged from action_policy()",
                d.name
            );
        }
    }
    // Spot-check the two parameterized names that have no bare-name variant.
    assert_eq!(
        catalog.find("scroll_to_offset").unwrap().policy,
        ActionPolicy::FocusedPaneLocal
    );
    assert_eq!(
        catalog.find("open_link").unwrap().policy,
        action_policy(&WmAction::OpenLink { url: String::new() })
    );
}

/// **Every action renders with an icon** — its own, or the one generic mark.
///
/// A per-category fallback was tried first and removed the same day: it filled every row, but
/// all four `focus_*` then wore the same arrow, which reads as a wrong meaning rather than as
/// no meaning. A plain circle claims nothing.
#[test]
fn an_action_without_its_own_icon_shows_the_generic_mark() {
    let catalog = ActionCatalog::with_builtins();
    assert_eq!(
        catalog.icon("close"),
        Some(Glyph::FolderSimpleMinus),
        "an action that declares its own keeps it",
    );
    assert_eq!(
        catalog.icon("focus_toggle_local"),
        Some(GENERIC_ACTION_ICON),
        "…and one that declares none shows the generic mark, not a family glyph",
    );
    assert!(
        catalog.all().all(|m| catalog.icon(&m.name).is_some()),
        "no catalogued action may render blank",
    );
    assert_eq!(
        catalog.icon("nope.not.a.thing"),
        None,
        "only an unknown name has none"
    );
}

#[test]
fn test_descriptors_are_populated() {
    for (category, desc) in builtins_by_category() {
        assert!(!desc.name.is_empty(), "name must not be empty");
        assert!(!desc.label.is_empty(), "label must not be empty");
        assert!(
            !desc.description.is_empty(),
            "description must not be empty"
        );
        assert!(
            !category.label().is_empty(),
            "category label must not be empty"
        );
    }
}
