//! [`ButtonGroup`] — the row of actions that fits the space it is given.
//!
//! Every assertion is made on the **laid-out tree** inside a parent of a known width, because that
//! is where a toolbar lives — a strip, a header, a card — and a group laid out as a root hugs its
//! content and can never learn the room was short.

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{ButtonGroup, Display};
use heca_grid_ui::{LayoutEngine, Length, Size, WidgetSize};

fn group() -> ButtonGroup {
    ButtonGroup::new()
        .size(WidgetSize::Small)
        .child(Button::new("Split").icon(Glyph::Plus))
        .child(Button::new("Zoom").icon(Glyph::FrameCorners))
        .child(Button::new("Close").icon(Glyph::Minus))
}

/// Lay `g` out inside a parent `w` wide.
fn lay(g: ButtonGroup, w: f64) -> Flex {
    lay_n(g, w, 4)
}

fn lay_n(g: ButtonGroup, w: f64, passes: usize) -> Flex {
    let mut parent = Flex::row().width(Length::Px(w as f32)).child(g);
    for _ in 0..passes {
        LayoutEngine::new()
            .base_font(13.0)
            .compute(&mut parent, Size::new(w, 60.0));
    }
    parent
}

/// The buttons' row: parent → group → row.
fn row(parent: &Flex) -> &dyn Component {
    // The group **is** the row that arranges the buttons — they are its own children.
    parent.base().children[0].as_ref()
}

/// How many children are still laid out as buttons.
fn visible(parent: &Flex) -> usize {
    row(parent)
        .base()
        .children
        .iter()
        .filter(|c| !c.base().style.layout.hidden)
        .count()
}

/// Widths of the children still shown.
fn widths(parent: &Flex) -> Vec<f64> {
    row(parent)
        .base()
        .children
        .iter()
        .filter(|c| !c.base().style.layout.hidden)
        .map(|c| c.base().bounds.size.w)
        .collect()
}

/// **Wide enough, and nothing is given up.** Three buttons, no ⋮.
#[test]
fn every_button_is_shown_when_they_all_fit() {
    let parent = lay(group(), 900.0);
    assert_eq!(visible(&parent), 3, "three buttons and no overflow trigger");
}

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
    assert!(hidden > 0, "what cannot fit leaves the row rather than being squashed");
    assert!(
        row(&parent).base().children.len() > 3,
        "…and a trigger appears for it"
    );
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

/// **A held-on button shows it, whatever its variant** — the state a pane's zoom and float buttons
/// carry while engaged. Drawn once beneath the variant, so a new variant cannot forget it.
#[test]
fn a_held_on_button_paints_its_status() {
    use heca_grid_ui::scene::DrawCommand;
    use heca_grid_ui::{PaintCx, Scene, Theme};

    let painted = |active: bool| -> usize {
        let mut b = Button::new("Zoom").icon(Glyph::FrameCorners).active(active);
        LayoutEngine::new().base_font(13.0).compute(&mut b, Size::new(300.0, 60.0));
        let theme = Theme::default();
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            heca_grid_ui::paint_child(&b, &mut cx);
        }
        scene.iter().filter(|c| matches!(c, DrawCommand::Rect(_))).count()
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
    LayoutEngine::new().base_font(13.0).compute(&mut parent, Size::new(900.0, 60.0));

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
        LayoutEngine::new().base_font(13.0).compute(&mut b, Size::new(400.0, 60.0));
        b.base().style.layout.pad_left(b.base().font)
    };
    let snug = {
        let mut b = Button::new("Zoom").icon(Glyph::FrameCorners).icon_only(true);
        LayoutEngine::new().base_font(13.0).compute(&mut b, Size::new(400.0, 60.0));
        b.base().style.layout.pad_left(b.base().font)
    };
    assert!(
        snug < wide,
        "an icon-only button drops the room reserved for words ({snug:?} vs {wide:?})"
    );
}

/// **A button with no icon keeps its words** whatever it is told — hiding them would leave an empty
/// box.
#[test]
fn a_wordless_button_is_refused_rather_than_emptied() {
    let mut b = Button::new("Rename").icon_only(true);
    LayoutEngine::new().base_font(13.0).compute(&mut b, Size::new(400.0, 60.0));
    assert!(
        b.base().children.iter().any(|c| !c.base().style.layout.hidden),
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

/// **A tooltip is the surface's size, not the control's.** A size variant scales what a widget draws
/// as its own content; a bubble floating beside it belongs to the surface, so an emphasized button
/// must not get emphasized words hovering over it.
#[test]
fn a_tooltip_does_not_inherit_an_emphasised_control_size() {
    use heca_grid_ui::ComponentExt;
    let mut big = Button::new("Zoom").tooltip("Zoom").size(WidgetSize::Header);
    LayoutEngine::new().base_font(13.0).compute(&mut big, Size::new(400.0, 60.0));
    assert!(
        big.base().font > big.base().root_font,
        "the control itself is scaled up by its variant"
    );
    assert_eq!(
        big.base().root_font, 13.0,
        "…and its bubble reads the tree's own base font instead"
    );
}

/// **It settles in one pass, and that is the whole point.** Learning each width by rendering in that
/// mode costs a frame per step — choose the words, find out what the icons measure, place what
/// survived, then measure the ⋮ — and every one of them is a frame the user watches. That is what
/// made a pane header flicker while a divider was dragged.
#[test]
fn the_arrangement_does_not_settle_over_several_frames() {
    for w in [900.0, 300.0, 120.0, 40.0] {
        let settled = visible(&lay_n(group(), w, 6));
        let first = visible(&lay_n(group(), w, 2));
        assert_eq!(
            first, settled,
            "at {w}px the second frame already shows what the sixth does"
        );
    }
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

/// **Measuring a button must not leave it where it was measured.** Laying a widget out on its own
/// places it at the origin, and this happens inside the parent's layout — so without putting the
/// bounds back, the frame that paints next draws the whole cluster in the header's top-left corner.
#[test]
fn measuring_the_buttons_does_not_move_them() {
    let width = 600.0;
    let mut header = Flex::row()
        .width(Length::Px(width as f32))
        .child(Label::new("title"))
        .child(group());
    // One pass: the first frame, which is the one that showed the flash.
    LayoutEngine::new()
        .base_font(13.0)
        .compute(&mut header, Size::new(width, 60.0));

    let group_node = &header.base().children[1];
    for child in group_node.base().children.iter() {
        let b = child.base().bounds;
        if child.base().style.layout.hidden {
            continue;
        }
        assert!(
            b.loc.x > 0.0,
            "a shown button was left at the origin by the measuring pass (x={})",
            b.loc.x
        );
    }
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

/// **Nothing is added to the tree while it is being laid out.**
///
/// A widget created inside the layout walk has no bounds yet, and the frame that paints next draws
/// it at the window's origin — a small thing flickering in the corner whenever the header was
/// rebuilt, which a resize does continuously. Everything the group can show exists before the first
/// layout; only its visibility changes.
#[test]
fn the_group_adds_nothing_to_the_tree_after_the_first_layout() {
    let g = group();
    let before = g.base().children.len();

    let parent = lay(g, 40.0);
    let after = row(&parent).base().children.len();
    assert_eq!(
        before, after,
        "the group had {before} children before any layout and {after} after collapsing — \
         something was created during the walk"
    );
}

/// **Building a group does not ask for a frame.**
///
/// `set_hidden` asks for the layout that has to follow a change, and asking for a layout asks for a
/// frame. A widget being *constructed* has no layout to redo — and the pane headers are rebuilt
/// every frame to work out their key, so a constructor that asked for a frame asked for one every
/// frame: the app never went idle and sat at a full core with nothing happening.
#[test]
fn constructing_a_group_does_not_request_a_frame() {
    use std::cell::Cell;
    use std::rc::Rc;

    let frames = Rc::new(Cell::new(0u32));
    let seen = frames.clone();
    heca_grid_ui::install_frame_request(move || seen.set(seen.get() + 1));

    let _g = ButtonGroup::new()
        .size(WidgetSize::Small)
        .child(Button::new("Zoom").icon(Glyph::FrameCorners))
        .child(Button::new("Close").icon(Glyph::Minus));

    assert_eq!(
        frames.get(),
        0,
        "building a group asked for {} frame(s); nothing is being changed, only made",
        frames.get()
    );
}


/// **Giving way is not one-way** — the room coming back brings the buttons back.
#[test]
fn a_collapsed_group_fills_up_again_when_the_room_returns() {
    let g = group();
    // Collapse it hard, then widen the same group and let it settle.
    let mut parent = Flex::row().width(Length::Px(40.0)).child(g);
    for _ in 0..6 {
        LayoutEngine::new().base_font(13.0).compute(&mut parent, Size::new(40.0, 60.0));
    }
    let collapsed = row(&parent)
        .base()
        .children
        .iter()
        .filter(|c| !c.base().style.layout.hidden)
        .count();

    parent.base_mut().style.layout.width = Length::Px(900.0);
    for _ in 0..12 {
        LayoutEngine::new().base_font(13.0).compute(&mut parent, Size::new(900.0, 60.0));
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
    use heca_grid_ui::scene::DrawCommand;
    use heca_grid_ui::{PaintCx, Scene, Theme};

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
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        heca_grid_ui::paint_child(&parent, &mut cx);
    }

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

/// **A group is right on the FIRST frame it is laid out** (F003/P097/T500).
///
/// It decides what fits by reading the room it was given, and that answer only exists once the
/// layout has run — so the decision lands in `on_layout`, *after* the pass that informed it. Left
/// there, the arrangement being replaced is what gets painted and the corrected one appears a frame
/// later: a visible flash on every rebuild.
///
/// No caller can prevent that or is even in a position to know about it, which is why the layout
/// settles before anything is painted rather than every host learning to re-run it. Antonio,
/// driving (2026-09-05): the pane header's buttons blinked on every terminal command, on a focus
/// change, on a split, and when the working directory was detected — four symptoms, one widget.
///
/// ⚠️ **The helper above lays out FOUR times**, which is this defect written into the tests: they
/// could not see it because they always gave the group the extra passes a real frame never does.
#[test]
fn a_group_holds_its_final_arrangement_after_a_single_layout() {
    for w in [90.0, 120.0, 149.0, 200.0] {
        let once = lay_n(group(), w, 1);
        let settled = lay_n(group(), w, 4);
        assert_eq!(
            (visible(&once), widths(&once)),
            (visible(&settled), widths(&settled)),
            "at {w}px the first frame showed a different row than the settled one — that \
             difference IS the flash",
        );
    }
}
