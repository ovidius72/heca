//! Where a frame says a terminal was drawn: from the surfaces its scenes asked for, whichever scene
//! held them.

use crate::chrome::terminal::{Drawn, settle};
use super::*;

/// What the flush does with a scene: note every terminal surface it reaches.
fn flush(scene: &Scene, drawn: &mut Drawn) {
    for run in scene.base_runs() {
        if let Some(surface) = run.then {
            drawn.record(&surface);
        }
    }
}

/// **A terminal that only one scene holds is placed where that scene drew it** — and one the frame
/// did not draw is not placed, whatever it was the frame before.
#[test]
fn a_terminal_is_placed_where_the_scene_that_held_it_drew_it() {
    let docked = Terminal::new();
    docked.attach(TerminalId(2));
    let pane = Terminal::new();
    pane.attach(TerminalId(1));

    let root = header_above(Box::new(docked.clone()), 33.0, 308.0, 720.0);
    let chrome = painted(root.as_ref());
    let mut drawn = Drawn::default();
    flush(&chrome, &mut drawn);
    settle([&docked, &pane].into_iter(), &drawn);

    let at = docked.placed().expect("the scene that held it drew it");
    assert_eq!((at.loc.y, at.size.h), (33.0, 687.0));
    assert_eq!(pane.placed(), None, "no scene drew the other one");

    settle([&docked, &pane].into_iter(), &Drawn::default());
    assert_eq!(docked.placed(), None, "a frame that did not draw it says so");
}
