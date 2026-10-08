//! **A name written in config → the action it means.**
//!
//! Owns resolving a binding's action name (and its `args`) at load, qualifying a surface's short
//! names, and reporting a binding's argument mistakes. Owns nothing about keys, layers or clashes.

use crate::input::resolve_action;
use crate::keymap::ActionRef;
use std::collections::HashMap;

/// Resolve a config action **name** (+ its args) to what the key should be bound to.
///
/// Built-in ⇒ [`ActionRef::Builtin`], resolved right here at load, exactly as before: a unit variant
/// through `action_from_name`, a parameterized one through `build_action` (so a malformed `args`
/// still fails at load, not at press).
///
/// Anything else ⇒ [`ActionRef::Dynamic`] — **not an error** (plugin-04 G3). Config is loaded before
/// any provider or plugin has registered its actions, so a binding to `plugin.docker.restart` cannot
/// possibly resolve yet; it resolves at press time through the one dispatch door. A binding to an
/// action that never turns up no-ops with a debug warning when pressed, which is the price of not
/// rejecting bindings to actions that legitimately do not exist yet.
///
/// A **misspelled argument** is a different matter and is now reported, loudly, right here — see
/// [`binding_arg_problems`]. It used to land in the same silence as an unknown action name: a
/// required argument spelled wrong made `build_action` return `None`, so `focus_pane` with a
/// `pane_i` fell all the way through to `Dynamic` and did nothing for the rest of the session,
/// without a word in a release build.
pub(super) fn action_ref_from_config(name: &str, args: &HashMap<String, String>) -> ActionRef {
    log_arg_problems(name, args);
    if let Some(built) = resolve_action(name, args) {
        return ActionRef::Builtin(built);
    }
    let mut intent = crate::chrome::Intent::new(name);
    for (k, v) in args {
        intent
            .args
            .insert(k.clone(), crate::chrome::PropValue::Text(v.clone()));
    }
    ActionRef::Dynamic(intent)
}

/// The action id a `[[keys.component]]` entry means, and the reference to bind (F003/P086/T362).
///
/// **Binding names are short; the action id is `<component>.<name>`.** A user writes `cursor_up`
/// under `name = "workspaces"` and means `workspaces.cursor_up` — they should never have to repeat
/// the component in every line of its own block.
///
/// The one exception is an id heca already knows: a component **binds** existing actions rather than
/// redeclaring them (`next_pane`, `zoom_column`), and those keep their catalog id. That is decided
/// by whether [`action_ref_from_config`] can resolve the name at load — a built-in resolves, a
/// component's or plugin's own name does not and becomes a `Dynamic` ref answered at press time.
///
/// An already-qualified name is left alone, so writing the id out in full is never wrong.
pub(super) fn component_action(
    component: &str,
    written: &str,
    args: &HashMap<String, String>,
) -> (String, ActionRef) {
    let id = surface_action_id(component, written, args);
    let action = action_ref_from_config(&id, args);
    (id, action)
}

/// **The action id a name written in a surface's entry means** — the naming half of
/// [`component_action`], on its own so nothing has to re-derive it.
///
/// A surface asking "what key runs this **in me**?" has to ask under the id the
/// [`BindingIndex`](crate::keymap::BindingIndex) actually recorded, which is this one and not the
/// short name the user wrote. Deriving it a second time is how the exposé's `x` / `r` / `d` stopped
/// working the moment they moved into config: the cards looked up `delete_pane`, the index held
/// `heca.expose.delete_pane`, the lookup came back empty, and the letters silently did nothing
/// One rule, both readers.
pub(crate) fn surface_action_id(
    surface: &str,
    written: &str,
    args: &HashMap<String, String>,
) -> String {
    if resolves_as_builtin(written, args) || written.starts_with(&format!("{surface}.")) {
        return written.to_string();
    }
    format!("{surface}.{written}")
}

/// Would this name bind as a built-in, with these arguments? **Asked without reporting anything.**
///
/// The question [`surface_action_id`] needs, and the reason it is not
/// [`action_ref_from_config`]: that one *reports* argument problems, which is right for the name a
/// binding actually lands on and wrong for a name merely being considered. Probing with it made the
/// exposé's `[[keys.surface]] delete_column = "r"` announce *"missing required argument 'ws_idx' …
/// cannot be built and will do nothing when pressed"* at every startup — about `delete_column`,
/// which was then **not** what got bound: the entry resolved to `heca.expose.delete_column`, whose
/// arguments the map's own cards supply. A warning about a name the loader rejected is worse than
/// no warning, because it sends the user to fix a line that is right.
pub(super) fn resolves_as_builtin(name: &str, args: &HashMap<String, String>) -> bool {
    resolve_action(name, args).is_some()
}

/// What is wrong with a binding's `args` table, judged against what the action declares it takes.
///
/// Empty for an action heca does not know (a provider's, a plugin's, or a typo in the action name)
/// — that name is resolved at press time, not here, so there is nothing yet to compare against.
pub(crate) fn binding_arg_problems(
    name: &str,
    args: &HashMap<String, String>,
) -> Vec<crate::args::ArgProblem> {
    match crate::actions::builtin_args(name) {
        Some(specs) => crate::args::check_args(&specs, args),
        None => Vec::new(),
    }
}

/// Report a binding's argument mistakes at config load, through the same channel as a keybinding
/// conflict — unconditionally, not only in a debug build. A wrong argument in `config.toml` is the
/// user's to fix, so the user has to hear about it.
pub(super) fn log_arg_problems(name: &str, args: &HashMap<String, String>) {
    if cfg!(test) {
        return;
    }
    let problems = binding_arg_problems(name, args);
    for problem in &problems {
        eprintln!("[heca] binding '{name}': {problem}");
    }
    if problems
        .iter()
        .any(|p| matches!(p, crate::args::ArgProblem::Missing { .. }))
    {
        eprintln!("[heca] binding '{name}' cannot be built and will do nothing when pressed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::input::WmAction;
    use crate::keymap::{ActionRef, KeyCombo, KeymapRegistry};

    use std::collections::HashMap;

    /// A binding to an action id that is UNKNOWN at load is **not** an error and is **not** dropped:
    /// it becomes a `Dynamic` ref that resolves at press. This is G3 — config is read before any
    /// provider or plugin has registered its actions.
    #[test]
    fn a_binding_to_an_unregistered_action_id_becomes_dynamic() {
        let action = action_ref_from_config("plugin.docker.restart", &HashMap::new());
        match action {
            ActionRef::Dynamic(intent) => {
                assert_eq!(intent.action, "plugin.docker.restart");
                assert!(intent.args.is_empty());
            }
            other => panic!("expected Dynamic, got {other:?}"),
        }
    }

    /// A dynamic binding carries its config `args` into the Intent, so the handler receives them at
    /// press exactly as a parameterized built-in would.
    #[test]
    fn a_dynamic_binding_carries_its_config_args_into_the_intent() {
        let mut args = HashMap::new();
        args.insert("container".to_string(), "web".to_string());
        match action_ref_from_config("plugin.docker.restart", &args) {
            ActionRef::Dynamic(intent) => {
                assert_eq!(
                    intent.args.get("container"),
                    Some(&crate::chrome::PropValue::Text("web".to_string()))
                );
            }
            other => panic!("expected Dynamic, got {other:?}"),
        }
    }

    /// A built-in name still resolves at LOAD, unit and parameterized alike — unchanged behaviour,
    /// including the arg parsing (a malformed arg still fails at load, not at press).
    #[test]
    fn builtin_names_still_resolve_at_load() {
        assert_eq!(
            action_ref_from_config("reload_config", &HashMap::new()),
            ActionRef::Builtin(WmAction::ReloadConfig)
        );

        let mut args = HashMap::new();
        args.insert("rows".to_string(), "5".to_string());
        assert_eq!(
            action_ref_from_config("scroll_to_offset", &args),
            ActionRef::Builtin(WmAction::ScrollToOffset { rows: 5 })
        );

        // A parameterized built-in with UNPARSEABLE args does not silently become a dynamic action
        // named `scroll_to_offset` — `action_from_name` catches it as the unit fallback, and when
        // there is no unit variant either it is a dynamic ref, which the press path warns about.
        let mut bad = HashMap::new();
        bad.insert("rows".to_string(), "not-a-number".to_string());
        assert!(matches!(
            action_ref_from_config("scroll_to_offset", &bad),
            ActionRef::Dynamic(_)
        ));
    }

    /// A binding whose `args` do not match what the action declares is **named** at load, instead
    /// of being left for the user to discover by pressing a key that does nothing.
    #[test]
    fn a_bindings_argument_mistakes_are_reported_at_load() {
        use crate::args::ArgProblem;

        // A well-formed binding says nothing.
        let mut good = HashMap::new();
        good.insert("ws_idx".to_string(), "2".to_string());
        assert!(binding_arg_problems("delete_workspace", &good).is_empty());

        // A misspelled required argument: both halves of the mistake are named.
        let mut typo = HashMap::new();
        typo.insert("ws_idxx".to_string(), "2".to_string());
        let problems = binding_arg_problems("delete_workspace", &typo);
        assert!(problems.contains(&ArgProblem::Missing {
            name: "ws_idx".to_string()
        }));
        assert!(problems.contains(&ArgProblem::Unknown {
            name: "ws_idxx".to_string(),
            did_you_mean: Some("ws_idx".to_string()),
        }));
        // …and, because it cannot be built, it is not quietly bound to workspace 0 either.
        assert!(matches!(
            action_ref_from_config("delete_workspace", &typo),
            ActionRef::Dynamic(_)
        ));

        // An action heca does not know yet is not judged here — it is resolved at press time, when
        // its provider may have registered it.
        assert!(binding_arg_problems("plugin.docker.restart", &typo).is_empty());
    }

    /// `[keys.unbind]` is keyed by the COMBO, so it retires a dynamic binding exactly as it retires
    /// a built-in one.
    #[test]
    fn unbind_retires_a_dynamic_binding_too() {
        let mut keymap = KeymapRegistry::new();
        let combo = KeyCombo::parse("g");
        keymap.bind(
            crate::keymap::LEADER_LAYER,
            combo.clone(),
            action_ref_from_config("plugin.docker.restart", &HashMap::new()),
        );
        assert!(
            keymap
                .resolve(crate::keymap::LEADER_LAYER, &combo)
                .is_some()
        );
        assert!(keymap.unbind(crate::keymap::LEADER_LAYER, &combo).is_some());
        assert!(
            keymap
                .resolve(crate::keymap::LEADER_LAYER, &combo)
                .is_none()
        );
    }

    /// Each placement id builds the `WmAction` it names, from its args — the same construction a
    /// plugin's Intent, an RPC call and a config binding all go through (`build_action`).
    #[test]
    fn every_chrome_placement_id_builds_its_action_from_args() {
        use crate::chrome::RegionId;
        let args = |pairs: &[(&str, &str)]| -> HashMap<String, String> {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };

        assert_eq!(
            action_ref_from_config(
                "chrome.container.move_to_region",
                &args(&[("container_id", "workspaces"), ("region", "right-sidebar")]),
            ),
            ActionRef::Builtin(WmAction::MoveContainerToRegion {
                container_id: "workspaces".to_string(),
                region: RegionId::RightSidebar,
            })
        );
        // The fixed-region conveniences.
        assert_eq!(
            action_ref_from_config(
                "chrome.container.move_left_sidebar",
                &args(&[("container_id", "workspaces")]),
            ),
            ActionRef::Builtin(WmAction::MoveContainerToRegion {
                container_id: "workspaces".to_string(),
                region: RegionId::LeftSidebar,
            })
        );
        assert_eq!(
            action_ref_from_config(
                "chrome.container.move_right_sidebar",
                &args(&[("container_id", "workspaces")]),
            ),
            ActionRef::Builtin(WmAction::MoveContainerToRegion {
                container_id: "workspaces".to_string(),
                region: RegionId::RightSidebar,
            })
        );
        // `before_id` is optional — omitted ⇒ move to the end of the region.
        assert_eq!(
            action_ref_from_config(
                "chrome.container.reorder_before",
                &args(&[("container_id", "workspaces")]),
            ),
            ActionRef::Builtin(WmAction::ReorderContainerBefore {
                container_id: "workspaces".to_string(),
                before_id: None,
            })
        );
        assert_eq!(
            action_ref_from_config(
                "chrome.container.reorder_after",
                &args(&[("container_id", "workspaces"), ("after_id", "agents")]),
            ),
            ActionRef::Builtin(WmAction::ReorderContainerAfter {
                container_id: "workspaces".to_string(),
                after_id: "agents".to_string(),
            })
        );
    }

    /// The dotted ids are the ONLY built-ins with a dotted name — no snake_case alias was quietly
    /// added for them, and no existing snake_case built-in was quietly renamed to a dotted id.
    /// (Renaming the existing ~115 is a migration nobody has decided on.)
    #[test]
    fn only_the_chrome_placement_builtins_carry_dotted_names() {
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let dotted: Vec<&str> = crate::actions::builtins()
            .map(|d| d.name)
            .filter(|n| n.contains('.'))
            .collect();
        assert_eq!(
            dotted,
            [
                "chrome.container.move_to_region",
                "chrome.container.move_left_sidebar",
                "chrome.container.move_right_sidebar",
                "chrome.container.reorder_before",
                "chrome.container.reorder_after",
            ]
        );
        // No snake_case alias for the same capability.
        assert!(catalog.find("move_container_to_region").is_none());
        assert!(catalog.find("reorder_container_before").is_none());
    }
}
