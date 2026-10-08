//! The handle: what a caller asks of a terminal by its id.

use super::*;

/// The handle's `run` and `kill` say what they mean, about the terminal they belong to — and say
/// nothing until there is a process to mean it about.
#[test]
fn the_handle_runs_and_kills_by_id() {
    let told: Rc<RefCell<Vec<(u64, TerminalCommand)>>> = Rc::default();
    let seen = told.clone();
    let t = Terminal::new();
    t.bind(Seams {
        scroll: ScrollIntents {
            to_bottom: Box::new(|| {}),
            to_offset: Box::new(|_| {}),
        },
        input: Box::new(|_| {}),
        command: Box::new(move |id, c| seen.borrow_mut().push((id.0, c))),
    });
    t.run("ls");
    assert!(
        told.borrow().is_empty(),
        "no process yet, so nothing to run in"
    );

    t.attach(TerminalId(5));
    t.run("ls -la");
    t.kill();
    assert_eq!(
        *told.borrow(),
        vec![
            (
                5,
                TerminalCommand::Run {
                    text: "ls -la".into(),
                    enter: true
                }
            ),
            (5, TerminalCommand::Kill),
        ]
    );
}
