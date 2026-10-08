//! **The terminals extensions placed by name.**
//!
//! `extension("demo").terminal("shell")` names one terminal, `demo.shell`. Every call with that name
//! hands back a handle on the **same** terminal, so a dock that is built again, or an overlay that is
//! opened again, shows the process that was already running instead of asking for a second one. The
//! author never sees this table: it is how "the same name" is the same terminal.

use std::cell::RefCell;
use std::collections::HashMap;

use super::Terminal;

thread_local! {
    /// The declared terminals by qualified name. Thread-local like the other queues extensions feed.
    static DECLARED: RefCell<HashMap<String, Terminal>> = RefCell::new(HashMap::new());
}

/// The terminal declared as `name`, made the first time and the same one after.
pub(super) fn of(name: String) -> Terminal {
    DECLARED.with(|table| {
        let mut table = table.borrow_mut();
        if let Some(terminal) = table.get(&name) {
            return terminal.clone();
        }
        let terminal = Terminal::new();
        terminal.declare(name.clone());
        table.insert(name, terminal.clone());
        terminal
    })
}

/// The declared terminals the host has not started yet.
pub(super) fn waiting() -> Vec<Terminal> {
    DECLARED.with(|table| {
        table
            .borrow()
            .values()
            .filter(|terminal| terminal.to_start().is_some())
            .cloned()
            .collect()
    })
}

/// Whether any declared terminal has not been started yet. Stops at the first and copies nothing:
/// a frame asks it every time.
pub(super) fn any_waiting() -> bool {
    DECLARED.with(|table| {
        table
            .borrow()
            .values()
            .any(|terminal| terminal.is_waiting())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_name_is_the_same_terminal() {
        let first = of("demo.shell".into());
        // Said through one handle, seen through the other: they are one terminal.
        let _ = first.clone().title("Scratch");
        let again = of("demo.shell".into());
        assert_eq!(again.shown_title().as_deref(), Some("Scratch"));
        assert_ne!(
            of("other.shell".into()).shown_title().as_deref(),
            Some("Scratch"),
            "another name is another terminal"
        );
    }

    #[test]
    fn the_title_is_the_name_until_one_is_given() {
        assert_eq!(
            of("demo.logs".into()).shown_title().as_deref(),
            Some("demo.logs")
        );
        assert_eq!(Terminal::new().shown_title(), None, "a pane's has none");
    }

    #[test]
    fn a_declared_terminal_waits_to_be_started_until_the_host_has_done_it_once() {
        let terminal = of("demo.once".into()).command("htop").cwd("/work");
        assert!(waiting().iter().any(|t| t.to_start().is_some()));
        let (name, program, cwd) = terminal.to_start().expect("it is waiting");
        assert_eq!(name, "demo.once");
        assert_eq!(
            program,
            crate::app::backend_store::Program::Command("htop".into())
        );
        assert_eq!(cwd, Some("/work".into()));
        terminal.resolved();
        assert!(
            terminal.to_start().is_none(),
            "started once, never again — a killed one stays ended"
        );
    }
}
