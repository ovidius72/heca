//! A modal shares the window, not the terminal's input or layout.

use super::*;
use heca_grid_ui::{Command, CommandPalette, KeyPress, Keymap, LayoutEngine, WidgetModel};
use heca_grid_ui::reactive::SignalGet;

#[test]
fn a_newly_mounted_active_pane_gives_its_content_real_keyboard_focus() {
    use crate::chrome::panes::pane::{shell::PaneShell, testing};
    let terminal = Terminal::unbound();
    let mut model = testing::model(1);
    model.active = true;
    let (callbacks, _) = testing::recording_callbacks();
    let pane = PaneShell { model: &model, cb: &callbacks, header: None,
        content: Some(WidgetModel::new(terminal.clone())) }.build();
    let mut window = crate::chrome::new_window_root();
    window.base_mut().children.push(WidgetModel::new(pane));
    LayoutEngine::new().compute(&mut window, Size::new(900.0, 600.0));
    let _ = painted(&window);
    heca_grid_ui::settle_focus(&mut window);
    assert_ne!(terminal.shared.node.get(), 0);
    assert_eq!(heca_grid_ui::keyboard_holder(&window), Some(terminal.shared.node.get()),
        "typing and feedback must agree after a new session mounts its terminal");
    let palette = CommandPalette::new().default_open(true);
    crate::chrome::place_surface(&mut window, "test.palette", WidgetModel::new(palette));
    heca_grid_ui::settle_focus(&mut window);
    assert_ne!(heca_grid_ui::keyboard_holder(&window), Some(terminal.shared.node.get()),
        "a selected pane behind a palette is not input-focused");
    crate::chrome::remove_surface(&mut window, "test.palette");
    assert_eq!(heca_grid_ui::keyboard_holder(&window), Some(terminal.shared.node.get()));
}

#[test]
fn a_palette_confirm_never_reaches_the_terminal_or_resizes_it() {
    use crate::chrome::panes::pane::{shell::PaneShell, testing};

    let (seams, said) = recording();
    let terminal = Terminal::unbound();
    terminal.bind(seams);
    terminal.show(&scrolled(0));
    let model = testing::model(1);
    let (callbacks, _) = testing::recording_callbacks();
    let pane = PaneShell {
        model: &model,
        cb: &callbacks,
        header: Some(crate::chrome::PaneHeader::showing(|_, _| {})),
        content: Some(WidgetModel::new(terminal.clone())),
    }.build();
    let mut window = crate::chrome::new_window_root();
    window.base_mut().children.push(WidgetModel::new(pane));
    let mut layout = LayoutEngine::new();
    let size = Size::new(900.0, 1400.0);
    layout.compute(&mut window, size);
    let _ = painted(&window);
    heca_grid_ui::focus_widget_by_id(&mut window, terminal.shared.node.get());
    heca_grid_ui::settle_focus(&mut window);
    assert_eq!(heca_grid_ui::keyboard_holder(&window), Some(terminal.shared.node.get()));
    let original_bounds = terminal.content_box();
    let original_grids = grids(&said);
    said.borrow_mut().clear();
    let selected = Rc::new(Cell::new(0));
    let count = selected.clone();
    let palette = CommandPalette::new()
        .command(Command::new("Session A", move || count.set(count.get() + 1)))
        .default_open(true);
    let open = palette.open_signal();
    crate::chrome::place_surface(&mut window, "test.palette", WidgetModel::new(palette));
    layout.compute(&mut window, size);
    assert_eq!(terminal.content_box(), original_bounds);
    assert!(grids(&said).is_empty(), "opening a palette must not resize the terminal");

    let keymap = Keymap::with_defaults();
    let handled = keymap.deliver_press(&KeyPress {
        key: GridKey::Enter,
        text: Some("\r".into()),
        mods: Default::default(),
    }, |event| heca_grid_ui::dispatch(&mut window, event));
    assert_eq!(handled, Handled::Yes);
    assert_eq!(selected.get(), 1);
    assert!(!open.get_untracked());
    assert!(said.borrow().is_empty(), "confirm belongs only to the palette: {:?}", said.borrow());

    // A host may unmount the closed surface before the platform delivers key-up.
    crate::chrome::remove_surface(&mut window, "test.palette");
    keymap.deliver_release(GridKey::Enter, |event| heca_grid_ui::dispatch(&mut window, event));
    layout.compute(&mut window, size);
    assert_eq!(terminal.content_box(), original_bounds);
    assert!(said.borrow().is_empty());
    assert_eq!(original_grids.len(), 1);
    keymap.deliver_press(&KeyPress {
        key: GridKey::Char('x'),
        text: Some("x".into()),
        mods: Default::default(),
    }, |event| heca_grid_ui::dispatch(&mut window, event));
    assert_eq!(*said.borrow(), vec![TerminalInput::Text("x".into())]);
}
