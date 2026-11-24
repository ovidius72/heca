//! A size that follows a value.

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::Flex;
use heca_grid_ui::{LayoutEngine, Length, Size};

/// **A width given as a value that changes is followed** — the next layout lays the box out at the
/// new width, nothing rebuilt; a fixed width given after it stops the following.
#[test]
fn a_width_that_follows_a_value_is_laid_out_at_each_new_value() {
    let wide = signal(false);
    let mut root = Flex::column().child(
        Flex::row()
            .width(move || {
                if wide.get() {
                    Length::Px(300.0)
                } else {
                    Length::Px(100.0)
                }
            })
            .max_height(Length::Px(40.0))
            .height(Length::Px(80.0)),
    );
    let viewport = Size::new(800.0, 600.0);
    let width = |root: &mut Flex| {
        LayoutEngine::new().compute(root, viewport);
        root.base().children[0].base().bounds.size
    };
    assert_eq!(width(&mut root), Size::new(100.0, 40.0));
    wide.set(true);
    assert_eq!(width(&mut root).w, 300.0, "followed into the next layout");
    wide.set(false);
    assert_eq!(width(&mut root).w, 100.0, "and back");
}
