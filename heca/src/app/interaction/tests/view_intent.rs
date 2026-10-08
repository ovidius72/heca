//! Intents of a described tree, resolved and judged like the built-in actions.

use super::*;


/// **The chain a `prefix+/` candidate is judged by, end to end** (F003/P082/T432).
///
/// With a floating pane active, `prefix+/` lettered every pane and
/// every sidebar row naming one, and pressing a letter did **nothing**. This is why — and now
/// it is asked one moment earlier, so the letter is never offered.
///
/// The test walks the real links: the intent a pane declares → the `WmAction` it resolves to →
/// the routing decision. `view_intent_allowed` is the same three steps with an `AppState` to
/// supply the domain, which a test cannot build (it needs a window), so the pure half is held
/// here and the pane's half of the declaration is held by
/// `chrome::pane::shell::tests::the_pane_says_what_picking_it_would_do`.
#[test]
fn a_pick_that_would_be_refused_resolves_to_a_refused_action() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;

    // What `PaneShell` declares — the same name and argument, built the same way a config
    // binding is.
    let args = std::collections::HashMap::from([("pane_id".to_string(), "99".to_string())]);
    let action = builtin_of("focus_pane", &args).expect("focus_pane is a built-in");
    assert_eq!(
        action,
        WmAction::FocusPane {
            pane_id: PaneId(99)
        }
    );

    let decision = route_in_domain(
        session.l(),
        session_domain(session.l()),
        InteractionSource::Keyboard,
        InteractionIntent::ActivateAction(action),
    );
    assert!(
        matches!(decision, RouteDecision::Block),
        "a pane that is not the active float cannot be focused, so its letter would do nothing"
    );
}


/// The args of a declarative `Intent` construct a parameterized built-in through the SAME
/// `build_action` a `config.toml` binding uses — so a menu item, an RPC call and a keybinding
/// all produce one identical `WmAction`. Before this, `dispatch_view_intent` dropped the args.
#[test]
fn intent_args_build_the_same_parameterized_action_as_a_config_binding() {
    use crate::chrome::{Intent, PropValue};

    let mut intent = Intent::new("scroll_to_offset");
    intent.args.insert("rows".to_string(), PropValue::Int(12));

    let from_intent =
        crate::input::resolve_action("scroll_to_offset", &intent_args_as_strings(&intent));

    let mut config_args = std::collections::HashMap::new();
    config_args.insert("rows".to_string(), "12".to_string());
    let from_config = crate::input::resolve_action("scroll_to_offset", &config_args);

    assert_eq!(from_intent, from_config);
    assert_eq!(from_intent, Some(WmAction::ScrollToOffset { rows: 12 }));
}


/// A unit built-in with no args still resolves by name (the `action_from_name` fallback), so the
/// existing name-dispatch behaviour is unchanged.
#[test]
fn a_unit_builtin_still_resolves_by_name_with_no_args() {
    use crate::chrome::Intent;
    let intent = Intent::new("reload_config");
    let args = intent_args_as_strings(&intent);
    assert!(args.is_empty());
    assert_eq!(
        crate::input::resolve_action("reload_config", &args),
        Some(WmAction::ReloadConfig)
    );
}
