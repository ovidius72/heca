use super::*;

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

/// **A group is right on the FIRST frame it is laid out** (F003/P097/T500).
///
/// It decides what fits by reading the room it was given, and that answer only exists once the
/// layout has run — so the decision lands in `on_layout`, *after* the pass that informed it. Left
/// there, the arrangement being replaced is what gets painted and the corrected one appears a frame
/// later: a visible flash on every rebuild.
///
/// No caller can prevent that or is even in a position to know about it, which is why the layout
/// settles before anything is painted rather than every host learning to re-run it. The pane
/// header's buttons blinked on every terminal command, on a focus
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
