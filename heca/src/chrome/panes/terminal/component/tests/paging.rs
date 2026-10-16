//! Paging: the scroll keys reach the terminal that holds the keyboard, wherever it is placed.

use super::*;
use crate::chrome::panes::terminal::Page;

/// **The terminal the keyboard is on is a scroll area to the paging keys**: each step is said to its
/// owner, so a terminal in a dock or an overlay pages the way a pane's does, and it takes the intent
/// so the scroll region around it does not page too.
#[test]
fn the_paging_keys_reach_the_terminal_that_holds_the_keyboard() {
    let (mut root, said) = focused_terminal();
    let pages = [Page::Up, Page::Down, Page::Top, Page::Bottom];

    for page in pages {
        let taken = heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(page.intent()));
        assert_eq!(taken, Handled::Yes, "{page:?} is taken");
    }

    assert_eq!(
        pointer_input(&said),
        pages.map(TerminalInput::Page).to_vec()
    );
}

/// A sideways page is not the terminal's: its history has one axis, so the intent goes on to
/// whatever scrolls sideways around it.
#[test]
fn a_sideways_page_goes_on() {
    let (mut root, said) = focused_terminal();

    let taken = heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::Widget(heca_grid_ui::WidgetIntent::ScrollPageLeft),
    );

    assert_eq!(taken, Handled::No);
    assert!(pointer_input(&said).is_empty());
}
