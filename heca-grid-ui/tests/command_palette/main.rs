#[path = "../common/mod.rs"]
mod common;

use common::type_text;
use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, Event, LayoutEngine, Rectangle, Size, Theme};

// --- CommandPalette --------------------------------------------------------

fn palette_with_markers() -> (
    heca_grid_ui::CommandPalette,
    std::rc::Rc<std::cell::Cell<u8>>,
) {
    use heca_grid_ui::{Command, CommandPalette};
    let ran = std::rc::Rc::new(std::cell::Cell::new(0u8));
    let (r1, r2, r3) = (ran.clone(), ran.clone(), ran.clone());
    let p = CommandPalette::new()
        .command(Command::new("Split pane", move || r1.set(1)))
        .command(Command::new("Close pane", move || r2.set(2)))
        .command(Command::new("Toggle sidebar", move || r3.set(3)));
    (p, ran)
}

mod editing;
mod filtering;
mod history;
mod modes;
mod sizing;
