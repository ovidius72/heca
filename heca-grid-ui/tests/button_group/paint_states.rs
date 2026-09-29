use super::*;

/// **A held-on button shows it, whatever its variant** — the state a pane's zoom and float buttons
/// carry while engaged. Drawn once beneath the variant, so a new variant cannot forget it.
#[test]
fn a_held_on_button_paints_its_status() {
    use heca_grid_ui::Theme;

    let painted = |active: bool| -> usize {
        let mut b = Button::new("Zoom").icon(Glyph::FrameCorners).active(active);
        LayoutEngine::new()
            .base_font(13.0)
            .compute(&mut b, Size::new(300.0, 60.0));
        let theme = Theme::default();
        let scene = common::paint_via_child(&b, &theme);
        common::rects(&scene).len()
    };

    assert!(
        painted(true) > painted(false),
        "held on draws a status wash the resting button does not ({} vs {})",
        painted(true),
        painted(false)
    );
}

/// **Pinned to icons, it never shows words — not even on its first frame.** Starting in the other
/// mode and correcting on the next pass is a visible flash, once at startup and again on every
/// rebuild.
#[test]
fn a_group_pinned_to_icons_never_renders_its_words() {
    let g = ButtonGroup::new()
        .display(Display::IconOnly)
        .size(WidgetSize::Small)
        .child(Button::new("Split").icon(Glyph::Plus))
        .child(Button::new("Zoom").icon(Glyph::FrameCorners));

    // ONE pass — the state the very first frame paints.
    let mut parent = Flex::row().width(Length::Px(900.0)).child(g);
    LayoutEngine::new()
        .base_font(13.0)
        .compute(&mut parent, Size::new(900.0, 60.0));

    let row = row(&parent);
    for child in row.base().children.iter() {
        for grandchild in child.base().children.iter() {
            if grandchild.text_summary().is_some() {
                assert!(
                    grandchild.base().style.layout.hidden,
                    "a pinned-to-icons group showed its words on the first frame"
                );
            }
        }
    }
}

/// **An icon-only button is square.** The horizontal room a button reserves is there for text, so
/// one with none is too wide by exactly that much — which is what made a pane's header buttons look
/// oversized (Antonio, driving, 2026-09-03).
#[test]
fn a_button_with_no_words_is_not_padded_for_them() {
    let wide = {
        let mut b = Button::new("Zoom").icon(Glyph::FrameCorners);
        LayoutEngine::new()
            .base_font(13.0)
            .compute(&mut b, Size::new(400.0, 60.0));
        b.base().style.layout.pad_left(b.base().font)
    };
    let snug = {
        let mut b = Button::new("Zoom")
            .icon(Glyph::FrameCorners)
            .icon_only(true);
        LayoutEngine::new()
            .base_font(13.0)
            .compute(&mut b, Size::new(400.0, 60.0));
        b.base().style.layout.pad_left(b.base().font)
    };
    assert!(
        snug < wide,
        "an icon-only button drops the room reserved for words ({snug:?} vs {wide:?})"
    );
}

/// **A tooltip is the surface's size, not the control's.** A size variant scales what a widget draws
/// as its own content; a bubble floating beside it belongs to the surface, so an emphasized button
/// must not get emphasized words hovering over it.
#[test]
fn a_tooltip_does_not_inherit_an_emphasised_control_size() {
    use heca_grid_ui::ComponentExt;
    let mut big = Button::new("Zoom").tooltip("Zoom").size(WidgetSize::Header);
    LayoutEngine::new()
        .base_font(13.0)
        .compute(&mut big, Size::new(400.0, 60.0));
    assert!(
        big.base().font > big.base().root_font,
        "the control itself is scaled up by its variant"
    );
    assert_eq!(
        big.base().root_font,
        13.0,
        "…and its bubble reads the tree's own base font instead"
    );
}

/// **An icon-only button sits where its row puts it** — and so does anything holding one.
///
/// Hiding the label with `display: none` left the button's box answered differently by the two
/// passes the layout makes: it reported its full height and was then placed as though it had almost
/// none, so it hung below its row and took the picker's letters with it. Found by reproducing it
/// with a plain `Flex` holding one such button, which is why this guards the button rather than the
/// group (Antonio, driving, 2026-09-03).
#[test]
fn an_icon_only_button_is_placed_where_its_row_puts_it() {
    use heca_grid_ui::Align;
    for size in [
        WidgetSize::Small,
        WidgetSize::Normal,
        WidgetSize::Large,
        WidgetSize::Header,
    ] {
        let mut row = Flex::row()
            .width(Length::Px(400.0))
            .align(Align::Center)
            .child(Label::new("title"))
            .child(
                Button::new("Zoom")
                    .icon(Glyph::FrameCorners)
                    .size(size)
                    .icon_only(true),
            );
        for _ in 0..3 {
            LayoutEngine::new()
                .base_font(13.0)
                .compute(&mut row, Size::new(400.0, 200.0));
        }
        let r = row.base().bounds;
        let b = row.base().children[1].base().bounds;
        let inside = b.loc.y >= r.loc.y - 0.5 && b.loc.y + b.size.h <= r.loc.y + r.size.h + 0.5;
        assert!(
            inside,
            "{size:?}: the button sits inside its row (row {}..{}, button {}..{})",
            r.loc.y,
            r.loc.y + r.size.h,
            b.loc.y,
            b.loc.y + b.size.h
        );
    }
}

/// **A button that is showing only its icon still knows its words** — they are what it says on
/// hover and what its row reads in the menu. Removing the label from the tree must not lose them.
#[test]
fn an_icon_only_button_still_knows_its_words() {
    let b = Button::new("Close the pane")
        .icon(Glyph::Minus)
        .icon_only(true);
    assert_eq!(b.label(), "Close the pane");
}
