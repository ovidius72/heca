use super::*;

/// **A position is auto margins, not pixels** — so a card lands at the named corner of a container
/// of any size.
#[test]
fn a_positioned_toast_lands_at_the_named_corner() {
    use heca_grid_ui::{Component, Flex, LayoutExt, Length, Parent, Toast, ToastPosition};

    let mut host = Flex::row()
        .width(Length::Px(800.0))
        .height(Length::Px(600.0))
        .child(Toast::info("Saved").position(ToastPosition::BottomRight));
    LayoutEngine::new().compute(&mut host, Size::new(800.0, 600.0));

    let card = host.base().children[0].base().bounds;
    assert!(
        (card.loc.x + card.size.w - 800.0).abs() < 0.5,
        "pinned to the right edge, not at {}",
        card.loc.x + card.size.w,
    );
    assert!(
        (card.loc.y + card.size.h - 600.0).abs() < 0.5,
        "and to the bottom edge, not at {}",
        card.loc.y + card.size.h,
    );
}

/// **A corner means the WINDOW's corner, whatever slot a host gives the stack.**
///
/// A host mounts its layers however it likes — the showcase stacks them in a column, where
/// viewport-sized siblings each get a share of the height. A stack that trusted its own box put
/// "top right" two thirds of the way down the window (F003/P096/T486).
#[test]
fn the_cards_anchor_to_the_window_corner_not_to_the_slot_a_parent_gave() {
    use heca_grid_ui::{Overlay, ToastPosition, ToastSpec, ToastStack};

    let vp = Size::new(1000.0, 700.0);
    let margin = 16.0;

    for (corner, want) in [
        (ToastPosition::TopRight, (true, true)),
        (ToastPosition::BottomLeft, (false, false)),
    ] {
        let items = signal(vec![ToastSpec::new(1, "Saved").body("one")]);
        let stack = ToastStack::new(items).position(corner);
        // Two viewport-sized siblings above it, exactly as an overlay layer is mounted.
        let mut overlays = Flex::column()
            .child(Overlay::new())
            .child(Overlay::new())
            .child(stack);
        overlays.base_mut().style.layout.width = Length::Px(vp.w as f32);
        overlays.base_mut().style.layout.height = Length::Px(vp.h as f32);
        overlays.tick(0.0);
        while overlays.tick(1.0 / 60.0) {}
        LayoutEngine::new()
            .base_font(14.0)
            .compute(&mut overlays, vp);

        let card = overlays.base().children[2].base().children[0].base().bounds;
        let (right, top) = want;
        if right {
            let gap = vp.w - (card.loc.x + card.size.w);
            assert!(
                (gap - margin).abs() < 1.0,
                "{corner:?}: {gap}px from the right edge"
            );
        } else {
            assert!(
                (card.loc.x - margin).abs() < 1.0,
                "{corner:?}: {} from the left",
                card.loc.x
            );
        }
        if top {
            assert!(
                (card.loc.y - margin).abs() < 1.0,
                "{corner:?}: {} from the top",
                card.loc.y
            );
        } else {
            let gap = vp.h - (card.loc.y + card.size.h);
            assert!(
                (gap - margin).abs() < 1.0,
                "{corner:?}: {gap}px from the bottom edge"
            );
        }
    }
}

/// **One vocabulary places the stack at every one of its members**, including the centres a
/// four-member corner enum could not say (F003/P096/T487).
#[test]
fn the_position_vocabulary_places_the_stack_at_each_of_its_members() {
    use heca_grid_ui::{ToastPosition, ToastSpec, ToastStack};

    let vp = Size::new(1000.0, 700.0);
    let margin = 16.0;
    for pos in [
        ToastPosition::TopRight,
        ToastPosition::TopLeft,
        ToastPosition::TopCenter,
        ToastPosition::BottomRight,
        ToastPosition::BottomLeft,
        ToastPosition::BottomCenter,
    ] {
        let items = signal(vec![ToastSpec::new(1, "Saved")]);
        let mut stack = ToastStack::new(items).position(pos);
        settled_stack(&mut stack, vp);
        let c = stack.base().children[0].base().bounds;

        let top = c.loc.y;
        let bottom = vp.h - (c.loc.y + c.size.h);
        match pos {
            ToastPosition::TopRight | ToastPosition::TopLeft | ToastPosition::TopCenter => {
                assert!((top - margin).abs() < 1.0, "{pos:?}: {top} from the top")
            }
            _ => assert!(
                (bottom - margin).abs() < 1.0,
                "{pos:?}: {bottom} from the bottom"
            ),
        }

        let left = c.loc.x;
        let right = vp.w - (c.loc.x + c.size.w);
        match pos {
            ToastPosition::TopRight | ToastPosition::BottomRight => {
                assert!(
                    (right - margin).abs() < 1.0,
                    "{pos:?}: {right} from the right"
                )
            }
            ToastPosition::TopLeft | ToastPosition::BottomLeft => {
                assert!((left - margin).abs() < 1.0, "{pos:?}: {left} from the left")
            }
            // Centred: equal slack on both sides, which is what a corner enum could not express.
            _ => assert!(
                (left - right).abs() < 1.0,
                "{pos:?}: {left} left vs {right} right — not centred",
            ),
        }
    }
}
