//! Native construction and owner-scoped sharing of terminal sessions.

use super::*;

#[test]
fn native_construction_and_cloning_control_the_same_session() {
    let first = Terminal::new("native-sharing").command("pwd").cwd("/work");
    let again = Terminal::new("native-sharing");
    let clone = first.clone();
    assert!(Rc::ptr_eq(&first.shared, &again.shared));
    assert!(Rc::ptr_eq(&first.shared, &clone.shared));
    assert_eq!(again.to_start(), first.to_start());

    let told = Rc::new(RefCell::new(Vec::new()));
    let seen = told.clone();
    let mut callbacks = recording().0;
    callbacks.command = Box::new(move |id, command| seen.borrow_mut().push((id.0, command)));
    first.bind(callbacks);
    first.attach(TerminalId(71));
    first.resolved();
    again.run("pwd");
    clone.kill();
    assert_eq!(*told.borrow(), vec![
        (71, TerminalCommand::Run { text: "pwd".into(), enter: true }),
        (71, TerminalCommand::Kill),
    ]);
    assert!(again.to_start().is_none(), "a shared session is started only once");
}

#[test]
fn native_names_and_extension_owners_have_distinct_sessions() {
    let native = Terminal::new("native-owner");
    let other = Terminal::new("other-native-owner");
    let plugin = crate::extension("native_terminal_owner_test").terminal("native-owner");
    assert_eq!(native.shown_title().as_deref(), Some("heca.native-owner"));
    assert_eq!(plugin.shown_title().as_deref(), Some("native_terminal_owner_test.native-owner"));
    assert!(!Rc::ptr_eq(&native.shared, &other.shared));
    assert!(!Rc::ptr_eq(&native.shared, &plugin.shared));
}

#[test]
fn invalid_native_names_never_declare_a_process() {
    for name in ["", "another.owner"] {
        let terminal = Terminal::new(name);
        assert_eq!(terminal.shown_title(), None);
        assert!(terminal.to_start().is_none());
    }
}
