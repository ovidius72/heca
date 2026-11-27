//! **A row's own actions** — `Command::context_menu`: the ⋯ on the selected row, and the keyboard's
//! menu action opening the selected row's menu while the query field keeps the focus.

use super::*;
use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};
use heca_grid_ui::{Command, CommandPalette, PointerButton};
use std::cell::RefCell;
use std::rc::Rc;

type Log = Rc<RefCell<Vec<String>>>;

fn menu(log: &Log, entries: &[&'static str]) -> ContextMenu {
    let mut items = Menu::new("session", "");
    for &entry in entries {
        let log = log.clone();
        items = items.child(
            MenuItem::new()
                .label(entry)
                .on_click(move || log.borrow_mut().push(entry.to_string())),
        );
    }
    ContextMenu::new("row").child(items)
}

fn opened_menus() -> Rc<RefCell<Vec<ContextMenu>>> {
    let opened = Rc::new(RefCell::new(Vec::new()));
    let sink = opened.clone();
    heca_grid_ui::install_menu_sink(move |menu, _anchor, _subject| sink.borrow_mut().push(menu));
    opened
}

fn laid_out(mut p: CommandPalette) -> CommandPalette {
    let viewport = Size::new(1200.0, 800.0);
    heca_grid_ui::settle_focus(&mut p);
    LayoutEngine::new().compute(&mut p, viewport);
    p
}

/// **The keyboard's menu action, with the palette open, opens the selected row's menu** — not the
/// menu of whatever is behind the palette. The query field keeps the focus; it names the selected
/// row as the one the keyboard acts on.
#[test]
fn the_keyboard_menu_action_opens_the_selected_rows_menu() {
    let log: Log = Default::default();
    let opened = opened_menus();
    let p = laid_out(
        CommandPalette::new()
            .command(
                Command::new("work", || {})
                    .context_menu(menu(&log, &["Rename work", "Delete work"])),
            )
            .command(
                Command::new("home", || {})
                    .context_menu(menu(&log, &["Rename home", "Delete home"])),
            )
            .default_open(true),
    );
    assert!(heca_grid_ui::open_for_keyboard(&p, None));
    let opened = opened.borrow();
    assert_eq!(opened.len(), 1);
    assert_eq!(
        opened[0].menu().item_labels(),
        vec![
            Some("Rename work".to_string()),
            Some("Delete work".to_string())
        ],
        "the selected (first) row's menu"
    );
}

/// **A row with one action runs it from the ⋯** — no menu for a single choice — and the click does
/// not choose the row.
#[test]
fn the_more_button_of_a_row_with_one_action_runs_it_and_not_the_row() {
    let log: Log = Default::default();
    let ran = Rc::new(std::cell::Cell::new(false));
    let opened = opened_menus();
    let chosen = ran.clone();
    let mut p = laid_out(
        CommandPalette::new()
            .command(
                Command::new("work", move || chosen.set(true))
                    .context_menu(menu(&log, &["Delete work"])),
            )
            .default_open(true),
    );
    let row = heca_grid_ui::node_with_key(&p, "command-palette.row.0").expect("the row");
    let more = row
        .base()
        .children
        .last()
        .expect("the ⋯ column")
        .base()
        .bounds;
    assert!(more.size.w > 0.0, "the ⋯ is laid out on the selected row");
    let centre = heca_grid_ui::Point::new(
        more.loc.x + more.size.w / 2.0,
        more.loc.y + more.size.h / 2.0,
    );
    common::click_at(&mut p, centre, PointerButton::Left);
    // A row's choice is run on the palette's next look; give it one.
    heca_grid_ui::Component::tick(&mut p, 0.016);
    assert_eq!(*log.borrow(), ["Delete work"], "its one action ran");
    assert!(opened.borrow().is_empty(), "no menu for a single choice");
    assert!(!ran.get(), "the row itself was not chosen");
}

/// **A row without actions has no ⋯** — the column holds nothing.
#[test]
fn a_row_without_actions_has_no_more_button() {
    let p = laid_out(
        CommandPalette::new()
            .command(Command::new("work", || {}))
            .default_open(true),
    );
    let row = heca_grid_ui::node_with_key(&p, "command-palette.row.0").expect("the row");
    let last = row.base().children.last().unwrap();
    assert!(last.base().children.is_empty(), "nothing in the ⋯ column");
}

/// **The ⋯ shows on the selected row and on the row under the pointer** — the row's own `:hover` —
/// and nowhere else; its box is kept on the others so nothing moves.
#[test]
fn the_more_button_shows_on_the_selected_row_and_on_the_hovered_one() {
    use heca_grid_ui::reactive::SignalGet as _;
    use heca_grid_ui::{Event, RawPointer, RawPointerKind};
    let log: Log = Default::default();
    let mut p = laid_out(
        CommandPalette::new()
            .command(Command::new("work", || {}).context_menu(menu(&log, &["Rename", "Delete"])))
            .command(Command::new("home", || {}).context_menu(menu(&log, &["Rename", "Delete"])))
            .command(Command::new("misc", || {}).context_menu(menu(&log, &["Rename", "Delete"])))
            .default_open(true),
    );
    let shown = |p: &CommandPalette, i: usize| {
        let row = heca_grid_ui::node_with_key(p, &format!("command-palette.row.{i}")).unwrap();
        let more = row.base().children.last().unwrap();
        more.base().children[0].base().visible.get()
    };
    assert!(shown(&p, 0), "the selected row");
    assert!(!shown(&p, 1) && !shown(&p, 2), "not the others");
    let over = heca_grid_ui::node_with_key(&p, "command-palette.row.2")
        .unwrap()
        .base()
        .bounds;
    let at = heca_grid_ui::Point::new(over.loc.x + 10.0, over.loc.y + over.size.h / 2.0);
    heca_grid_ui::dispatch(
        &mut p,
        &Event::Raw(RawPointer::new(RawPointerKind::Moved, at)),
    );
    assert!(shown(&p, 2), "the row under the pointer");
    assert!(!shown(&p, 1), "still not a row the pointer is not on");
}
