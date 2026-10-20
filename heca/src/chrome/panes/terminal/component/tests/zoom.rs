//! Font size: the zoom keys reach the terminal that holds the keyboard, wherever it is placed.

use super::*;
use crate::input::FontZoomStep;

/// **The terminal the keyboard is on answers the zoom keys itself**: each step is said to its owner,
/// so a terminal in a dock or an overlay zooms the way a pane's does, and it takes the intent so
/// nothing behind it zooms too.
#[test]
fn the_zoom_keys_reach_the_terminal_that_holds_the_keyboard() {
    let (mut root, said) = focused_terminal();

    for step in [FontZoomStep::In, FontZoomStep::Out, FontZoomStep::Reset] {
        let taken = heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(step.intent()));
        assert_eq!(taken, Handled::Yes, "{step:?} is taken");
    }

    assert_eq!(
        pointer_input(&said),
        vec![
            TerminalInput::FontZoom(FontZoomStep::In),
            TerminalInput::FontZoom(FontZoomStep::Out),
            TerminalInput::FontZoom(FontZoomStep::Reset),
        ]
    );
}

/// An intent that is not a zoom is not one: the terminal says nothing and lets it go on.
#[test]
fn another_intent_is_not_a_zoom() {
    let (mut root, said) = focused_terminal();

    let taken = heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::Widget(heca_grid_ui::WidgetIntent::MenuDown),
    );

    assert_eq!(taken, Handled::No);
    assert!(pointer_input(&said).is_empty());
}
