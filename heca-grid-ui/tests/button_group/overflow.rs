use super::*;

/// **Too narrow, and what does not fit leaves the row** — rather than every button being squashed,
/// which is what a plain row does and what this widget exists to replace.
#[test]
fn what_does_not_fit_leaves_the_row() {
    // 40px: narrow enough that three buttons cannot fit even once they are down to their icons.
    // A wider strip is not a test — three icon buttons genuinely *do* fit in 100px, and asserting
    // they collapse there passes only because the assertion was too weak to notice.
    let parent = lay(group(), 40.0);
    let hidden = row(&parent)
        .base()
        .children
        .iter()
        .filter(|c| c.base().style.layout.hidden)
        .count();
    assert!(
        hidden > 0,
        "what cannot fit leaves the row rather than being squashed"
    );
    assert!(
        row(&parent).base().children.len() > 3,
        "…and a trigger appears for it"
    );
}

/// **The group never exceeds the box it is given** — the assertion a component computing geometry
/// of its own always fails, and the one the pane header needed.
#[test]
fn the_group_never_exceeds_the_box_it_is_given() {
    for w in [900.0, 400.0, 220.0, 120.0, 70.0] {
        let parent = lay(group(), w);
        let got = parent.base().children[0].base().bounds.size.w;
        assert!(got <= w + 0.5, "group laid out {got} wide in a {w} box");
    }
}

/// **Pinning the display governs the words, never the collapsing.** `Full` does not mean "overflow
/// anyway" — that would bring back the squashing.
#[test]
fn a_pinned_display_still_gives_way_when_there_is_no_room() {
    let g = ButtonGroup::new()
        .display(Display::Full)
        .child(Button::new("Split").icon(Glyph::Plus))
        .child(Button::new("Zoom").icon(Glyph::FrameCorners))
        .child(Button::new("Close").icon(Glyph::Minus));
    let parent = lay(g, 40.0);
    assert!(visible(&parent) < 3, "Full still gives way when it must");
}

/// **A button with no icon keeps its words** whatever it is told — hiding them would leave an empty
/// box.
#[test]
fn a_wordless_button_is_refused_rather_than_emptied() {
    let mut b = Button::new("Rename").icon_only(true);
    LayoutEngine::new()
        .base_font(13.0)
        .compute(&mut b, Size::new(400.0, 60.0));
    assert!(
        b.base()
            .children
            .iter()
            .any(|c| !c.base().style.layout.hidden),
        "a button with no icon still shows something"
    );
}

/// **The ⋮ gets a letter for being a button, having declared nothing.**
///
/// It used to have to declare a pick of its own, because the picker switched off letters for
/// anything inside something that had declared one — and a pane's shell declares. That made a
/// button's letter depend on where it had been put, which is the one thing a widget must never care
/// about, and it left this control — the only one the widget builds for itself, so with no author to
/// write the declaration — silently unpickable (Antonio, 2026-09-04: *"Users/Developers MUST not
/// think where a widget is"*).
#[test]
fn the_overflow_trigger_says_what_picking_it_does() {
    let parent = lay(group(), 40.0);
    let targets = heca_grid_ui::collect_hints(row(&parent));
    let trigger = row(&parent).base().children.len() - 1;
    assert!(
        targets.iter().any(|(path, _)| path == &vec![trigger]),
        "the ⋮ is offered a letter, with nothing declared on it: {targets:?}"
    );
}

/// **Giving way is not one-way** — the room coming back brings the buttons back.
#[test]
fn a_collapsed_group_fills_up_again_when_the_room_returns() {
    let g = group();
    // Collapse it hard, then widen the same group and let it settle.
    let mut parent = Flex::row().width(Length::Px(40.0)).child(g);
    for _ in 0..6 {
        LayoutEngine::new()
            .base_font(13.0)
            .compute(&mut parent, Size::new(40.0, 60.0));
    }
    let collapsed = row(&parent)
        .base()
        .children
        .iter()
        .filter(|c| !c.base().style.layout.hidden)
        .count();

    parent.base_mut().style.layout.width = Length::Px(900.0);
    for _ in 0..12 {
        LayoutEngine::new()
            .base_font(13.0)
            .compute(&mut parent, Size::new(900.0, 60.0));
    }
    let reopened = row(&parent)
        .base()
        .children
        .iter()
        .filter(|c| !c.base().style.layout.hidden)
        .count();

    assert!(
        reopened > collapsed,
        "the group stayed collapsed at 900px after being squeezed at 40px ({collapsed} shown, then {reopened})"
    );
}

/// **The ⋮ is a button like the others, and keeps a button's width.**
///
/// A group hides what does not fit rather than squashing anything — but that only holds if nothing
/// in the row can give way. The ⋮ was the one child that could, and it was therefore the one the
/// layout took the room from: with a title beside the group competing for the same strip, the row
/// fitted by squeezing the ⋮ instead of pushing a button out, so the group never saw an overflow,
/// never moved anything into its menu, and its own affordance shrank to a sliver — four to eight
/// pixels wide beside a twenty-four pixel sibling, and barely clickable (Antonio, driving,
/// 2026-09-04).
///
/// Laid out in the pane header's actual shape, because that is what exposes it: a title that wants
/// room and the actions at the far end. A group alone in its parent never reproduces it.
#[test]
fn the_overflow_trigger_is_never_squeezed_by_a_title_beside_it() {
    for i in 40..200 {
        let w = i as f64;
        let g = ButtonGroup::new()
            .size(WidgetSize::Header)
            .display(Display::IconOnly)
            .child(Button::new("Split").icon(Glyph::Plus))
            .child(Button::new("Zoom").icon(Glyph::FrameCorners))
            .child(Button::new("Close").icon(Glyph::Minus));
        let mut parent = Flex::row()
            .justify(Justify::SpaceBetween)
            .align(Align::Center)
            .width(Length::Px(w as f32))
            .child(Label::new("a rather long pane title here"))
            .child(g);
        for _ in 0..4 {
            LayoutEngine::new()
                .base_font(13.0)
                .compute(&mut parent, Size::new(w, 60.0));
        }

        let group = parent.base().children[1].as_ref();
        let shown: Vec<f64> = group
            .base()
            .children
            .iter()
            .filter(|c| !c.base().style.layout.hidden)
            .map(|c| c.base().bounds.size.w)
            .collect();
        let (Some(&trigger), Some(&widest)) =
            (shown.last(), shown.iter().max_by(|a, b| a.total_cmp(b)))
        else {
            continue;
        };
        // Whole-pixel rounding of a fractional width moves a box by at most one pixel.
        assert!(
            widest - trigger <= 1.5,
            "at {w}px the row is {shown:?} — the ⋮ gave way where a button would not"
        );
    }
}

/// **A button coming back into the row does not appear in the corner of the screen first.**
///
/// Hidden is `display: none`, so the layout leaves the widget a zero rect at the origin, and
/// revealing it does not undo that. Most of a widget draws from its bounds and so draws nothing —
/// but an icon is a glyph at the font's size, painted wherever its box says it is. So a revealed
/// button dropped its icon at the window's top-left corner for the frame before the next layout
/// reached it: continuous while a divider is dragged, and red when the button is the destructive one
/// (Antonio, driving, 2026-09-04).
///
/// The assertion is on the painted scene rather than on the bounds, because it is the *drawing* that
/// was wrong — the bounds were honestly reporting that nothing had placed the widget yet.
#[test]
fn a_button_the_room_brings_back_paints_nothing_until_it_has_a_box() {
    use heca_grid_ui::Theme;
    use heca_grid_ui::scene::DrawCommand;

    let g = group();
    let mut parent = Flex::row().width(Length::Px(40.0)).child(g);
    for _ in 0..6 {
        LayoutEngine::new()
            .base_font(13.0)
            .compute(&mut parent, Size::new(40.0, 60.0));
    }

    // The room returns. ONE layout — the pass that reveals — then the frame that paints it.
    parent.base_mut().style.layout.width = Length::Px(900.0);
    LayoutEngine::new()
        .base_font(13.0)
        .compute(&mut parent, Size::new(900.0, 60.0));

    let theme = Theme::default();
    let scene = common::paint_via_child(&parent, &theme);

    // The three buttons that were squeezed out at 40px — check each got its own new box painted,
    // not just that nothing landed at the corner.
    let painted_in_own_box = row(&parent).base().children.iter().take(3).all(|c| {
        let b = c.base().bounds;
        scene.iter().any(|d| match d {
            DrawCommand::Text(t) => {
                t.rect.loc.x >= b.loc.x - 0.5
                    && t.rect.loc.x <= b.loc.x + b.size.w + 0.5
                    && t.rect.loc.y >= b.loc.y - 0.5
                    && t.rect.loc.y <= b.loc.y + b.size.h + 0.5
            }
            _ => false,
        })
    });
    assert!(
        painted_in_own_box,
        "a returning button must paint inside its own new box, not just avoid the corner"
    );

    let corner: Vec<String> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) if t.rect.loc.x < 1.0 && t.rect.loc.y < 1.0 => {
                Some(format!("{:?} at {:?}", t.text, t.rect))
            }
            _ => None,
        })
        .collect();
    assert!(
        corner.is_empty(),
        "a widget with no box yet drew in the window corner: {corner:?}"
    );
}
