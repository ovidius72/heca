use super::*;

/// **Wide enough, and nothing is given up.** Three buttons, no ⋮.
#[test]
fn every_button_is_shown_when_they_all_fit() {
    let parent = lay(group(), 900.0);
    assert_eq!(visible(&parent), 3, "three buttons and no overflow trigger");
}

/// **A button that is still shown keeps its size.** The defect this replaces: a pane's header
/// buttons fell to seven pixels in a narrow pane while the title beside them ellipsed correctly
/// (Antonio, driving, 2026-09-02).
#[test]
fn a_shown_button_is_never_squashed() {
    // Compared **within one display**, because the words coming off is not squashing — it is the
    // group giving up the cheapest thing first, and a narrower icon button is the intended result.
    let roomy = ButtonGroup::new()
        .display(Display::IconOnly)
        .size(WidgetSize::Small)
        .child(Button::new("Split").icon(Glyph::Plus))
        .child(Button::new("Zoom").icon(Glyph::FrameCorners))
        .child(Button::new("Close").icon(Glyph::Minus));
    let wide = widths(&lay(roomy, 900.0));

    let tight = ButtonGroup::new()
        .display(Display::IconOnly)
        .size(WidgetSize::Small)
        .child(Button::new("Split").icon(Glyph::Plus))
        .child(Button::new("Zoom").icon(Glyph::FrameCorners))
        .child(Button::new("Close").icon(Glyph::Minus));
    let narrow = widths(&lay(tight, 200.0));

    let (Some(&full), Some(&squeezed)) = (wide.first(), narrow.first()) else {
        panic!("both layouts show at least one button");
    };
    assert!(
        (full - squeezed).abs() < 0.5,
        "a shown icon button is the same width in a 200px strip as in a 900px one \
         ({full} then {squeezed}) — it leaves the row rather than being squashed"
    );
}

/// **In a `SpaceBetween` row it sits at the right end** — the shape a header is: a title at one end,
/// its actions at the other.
///
/// It fails if the group ever fills the width it is offered again. Filling leaves the parent with no
/// free space to distribute, so the title and the actions end up side by side at the left, which is
/// what a pane header looked like (Antonio, driving, 2026-09-03).
#[test]
fn in_a_space_between_row_the_group_sits_at_the_far_end() {
    use heca_grid_ui::{Align, Justify};
    let g = ButtonGroup::new()
        .display(Display::IconOnly)
        .size(WidgetSize::Small)
        .child(Button::new("Zoom").icon(Glyph::FrameCorners))
        .child(Button::new("Close").icon(Glyph::Minus));

    let width = 600.0;
    let mut header = Flex::row()
        .width(Length::Px(width as f32))
        .align(Align::Center)
        .justify(Justify::SpaceBetween)
        .child(Label::new("~/projects/heca"))
        .child(g);
    for _ in 0..3 {
        LayoutEngine::new()
            .base_font(13.0)
            .compute(&mut header, Size::new(width, 60.0));
    }

    // The group takes the room that is left — that is how it knows how much there is — and puts its
    // buttons at the **end** of it, which is what makes the header read as title-left, actions-right.
    let buttons = &header.base().children[1];
    let last = buttons
        .base()
        .children
        .iter()
        .rfind(|c| !c.base().style.layout.hidden && c.base().bounds.size.w > 0.0)
        .expect("something is shown");
    let right_edge = last.base().bounds.loc.x + last.base().bounds.size.w;
    assert!(
        (right_edge - width).abs() < 1.0,
        "the last action sits against the row's right edge (ended at {right_edge} of {width})"
    );
    let first = buttons
        .base()
        .children
        .iter()
        .find(|c| !c.base().style.layout.hidden && c.base().bounds.size.w > 0.0)
        .expect("something is shown");
    assert!(
        first.base().bounds.loc.x > width / 2.0,
        "…and the cluster is at that end, not spread across the row (starts at {})",
        first.base().bounds.loc.x
    );
}
