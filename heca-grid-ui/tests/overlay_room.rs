//! What an overlay does for any panel: keep it inside the window; the arrows across a row of
//! buttons, when asked.

mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{Dialog, Flex, Overlay};
use heca_grid_ui::{Event, LayoutEngine, Size, WidgetIntent, holds_keyboard};

/// **A panel never needs more room than the window has** — a dialog whose `22em` floor is wider
/// than a narrow window is narrowed to it, and gets its floor back when the window grows.
#[test]
fn a_panel_wider_than_the_window_is_narrowed_to_it_and_given_back() {
    let confirm = || {
        Dialog::new("Delete pane?")
            .action(Button::new("Cancel"))
            .action(Button::new("Delete"))
            .default_open(true)
    };
    let mut dialog = confirm();
    let frame = |dialog: &mut Dialog, w: f64| {
        for _ in 0..3 {
            LayoutEngine::new()
                .base_font(20.0)
                .compute(dialog, Size::new(w, 600.0));
        }
        let overlay = &dialog.base().children[0];
        overlay.base().children[0].base().bounds
    };
    let narrow = frame(&mut dialog, 300.0);
    assert!(
        narrow.loc.x >= 0.0 && narrow.loc.x + narrow.size.w <= 300.0,
        "inside a 300px window: {narrow:?}"
    );
    let wide = frame(&mut dialog, 1600.0);
    let fresh = frame(&mut confirm(), 1600.0);
    assert!(
        narrow.size.w < fresh.size.w,
        "it was narrowed: {} < {}",
        narrow.size.w,
        fresh.size.w
    );
    assert_eq!(
        wide.size.w, fresh.size.w,
        "and given its floor back when the window grew"
    );
}

/// **`item_keys(true)` makes the arrows across a row of buttons move the keyboard**, as Tab does;
/// without it the arrows are left for whatever else wants them.
#[test]
fn item_keys_move_the_keyboard_across_a_row_only_when_asked() {
    for asked in [true, false] {
        let mut overlay = Overlay::new()
            .item_keys(asked)
            .default_open(true)
            .panel(Flex::row().child([Button::new("A"), Button::new("B")]));
        LayoutEngine::new().compute(&mut overlay, Size::new(800.0, 600.0));
        heca_grid_ui::dispatch(&mut overlay, &Event::Widget(WidgetIntent::ItemNext));
        heca_grid_ui::dispatch(&mut overlay, &Event::Widget(WidgetIntent::ItemNext));
        let row = &overlay.base().children[0];
        assert_eq!(
            holds_keyboard(row.base().children[1].as_ref()),
            asked,
            "asked={asked}: the second button holds the keyboard after two moves"
        );
    }
}
