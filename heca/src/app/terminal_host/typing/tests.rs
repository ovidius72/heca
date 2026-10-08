use super::*;

#[test]
fn a_key_is_named_in_the_backends_terms_with_what_was_held() {
    let ctrl = Modifiers {
        ctrl: true,
        ..Default::default()
    };
    let event = key_event(GridKey::Char('c'), ctrl);
    assert_eq!(event.code, BackendKeyCode::Char('c'));
    assert!(event.modifiers.ctrl && !event.modifiers.shift && !event.modifiers.alt);
}

#[test]
fn the_command_key_is_the_backends_super() {
    let meta = Modifiers {
        meta: true,
        ..Default::default()
    };
    assert!(key_event(GridKey::Enter, meta).modifiers.super_);
}

#[test]
fn every_key_the_tree_can_deliver_has_a_backend_name() {
    assert_eq!(key_code(GridKey::Space), BackendKeyCode::Char(' '));
    assert_eq!(key_code(GridKey::PageUp), BackendKeyCode::PageUp);
    assert_eq!(key_code(GridKey::Function(5)), BackendKeyCode::Function(5));
    assert_eq!(key_code(GridKey::ArrowLeft), BackendKeyCode::LeftArrow);
}
