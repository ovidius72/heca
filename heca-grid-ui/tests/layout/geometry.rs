use super::*;

/// A state highlight must never be smaller than the content it highlights.
///
/// The hover/active pill is drawn inset so its rounded corners never contend with a rounded
/// container's, but the inset may only spend space the widget's own padding already reserves. An
/// unpadded row's content reaches its edges, so there is nothing to give: taking the inset anyway
/// drew a pill *shorter than its own content*, and a badge inside it stuck out above and below.
#[test]
fn a_state_highlight_never_crops_the_content_it_covers() {
    let theme = Theme::default();
    // The pill only paints for a hovered (or active) row, so hover it before painting. An unfilled
    // row draws no surface of its own, which makes the pill the only rect in the scene.
    let hovered_pill = |padding: f32| {
        let mut row = Row::new()
            .padding(padding)
            .on_activate(|| {})
            .child(fixed_box(60.0, 24.0));
        LayoutEngine::new().compute(&mut row, Size::new(200.0, 60.0));
        let b = row.base().bounds;
        heca_grid_ui::dispatch(
            &mut row,
            &Event::pointer_moved(Point::new(
                b.loc.x + b.size.w / 2.0,
                b.loc.y + b.size.h / 2.0,
            )),
        );
        let scene = common::paint(&row, &theme);
        let rects: Vec<Rectangle> = scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Rect(r) => Some(r.rect),
                _ => None,
            })
            .collect();
        assert_eq!(rects.len(), 1, "the hover pill is the only rect: {rects:?}");
        (rects[0], row.base().children[0].base().bounds, b)
    };

    // No padding: the content reaches the row's edges, so the pill covers the row exactly.
    let (sel, content, bounds) = hovered_pill(0.0);
    assert_eq!(
        sel.loc.y, bounds.loc.y,
        "unpadded: no vertical inset to take"
    );
    assert_eq!(
        sel.size.h, bounds.size.h,
        "unpadded: the pill is the full row height"
    );
    assert!(
        sel.loc.y <= content.loc.y && sel.loc.y + sel.size.h >= content.loc.y + content.size.h,
        "pill {sel:?} crops content {content:?}",
    );

    // With padding the inset costs nothing, so it still happens: the pill stays off the edges and
    // still clears the content.
    let (sel, content, bounds) = hovered_pill(10.0);
    assert!(
        sel.loc.y > bounds.loc.y && sel.size.h < bounds.size.h,
        "padded: the pill stays inset from the row's edges ({sel:?} vs {bounds:?})",
    );
    assert!(
        sel.loc.y <= content.loc.y && sel.loc.y + sel.size.h >= content.loc.y + content.size.h,
        "pill {sel:?} crops content {content:?}",
    );
}

/// **An overlay must not eat a key its panel did not want.** `Keymap::dispatch` offers the raw key
/// first and the semantic intent second, so claiming the key stops the walk before the intent
/// arrives — which is how Esc stopped closing an overlay and `Ctrl+h` never reached `item_previous`.
#[test]
fn a_blocking_overlay_reports_a_key_its_panel_ignored_as_unhandled() {
    use heca_grid_ui::WidgetIntent;
    use heca_grid_ui::reactive::signal;
    use heca_grid_ui::widgets::{CardGrid, Flex, GridCell, Label, Overlay};

    let dismissed = std::rc::Rc::new(std::cell::Cell::new(false));
    let d = dismissed.clone();
    let lit = signal(false);
    let grid = CardGrid::new()
        .row(
            vec![vec![GridCell::new("a", lit)]],
            Flex::row().child(Label::new("a")),
        )
        .on_dismiss(move || d.set(true));
    let mut overlay = Overlay::new().blocking(true).panel(grid).default_open(true);

    // A raw key the panel has no use for: the overlay must NOT claim it, or the keymap stops here.
    let handled = heca_grid_ui::dispatch(
        &mut overlay,
        &Event::Key {
            key: GridKey::Escape,
            pressed: true,
        },
    );
    assert_eq!(handled, Handled::No, "an unwanted key is not swallowed");

    // …so the intent the same chord resolves to still arrives, and closes the layer.
    heca_grid_ui::dispatch(&mut overlay, &Event::Widget(WidgetIntent::Dismiss));
    assert!(dismissed.get(), "Dismiss reached the panel");
}

/// **A fixed-size box keeps its shape when the row runs out of room.** Everything gives way by
/// default, which is right for text and for a card carrying a design width — and wrong for a
/// circle: a 9px status dot in a tight sidebar row was squeezed to a 3px sliver, because there is
/// no narrower version of a dot, only a deformed one (F003/P096/T483).
#[test]
fn a_dot_stays_round_in_a_row_too_narrow_for_it() {
    use heca_grid_ui::{Component, DotStatus, Flex, Label, LayoutExt, Length, Parent, StatusDot};

    let mut row = Flex::row()
        .width(Length::Px(120.0))
        .gap(8.0)
        .child(StatusDot::new(DotStatus::Online))
        .child(Label::new("~/projects/hype/HypeSiteNext"))
        .child(Label::new("master"));
    LayoutEngine::new().compute(&mut row, Size::new(120.0, 40.0));

    let dot = row.base().children[0].base().bounds.size;
    assert_eq!(
        dot.w, dot.h,
        "the dot is {dot:?} — round, or it is not a dot"
    );
}

/// **A wrapper gives way exactly as much as what it wraps.** The app shows a pane's status through
/// a `Visibility`, and the wrapper was squeezed where its content would not be — so the dot inside
/// it was drawn as a sliver. A wrapper has no opinion of its own (F003/P096/T483).
#[test]
fn a_wrapped_dot_is_still_round() {
    use heca_grid_ui::{
        Component, DotStatus, Flex, Label, LayoutExt, Length, Parent, StatusDot, Visibility,
    };

    let mut row = Flex::row()
        .width(Length::Px(120.0))
        .gap(8.0)
        .child(Visibility::new(StatusDot::new(DotStatus::Online), true))
        .child(Label::new("~/projects/hype/HypeSiteNext"))
        .child(Label::new("master"));
    LayoutEngine::new().compute(&mut row, Size::new(120.0, 40.0));

    let dot = row.base().children[0].base().children[0].base().bounds.size;
    assert_eq!(dot.w, dot.h, "the wrapped dot is {dot:?}");
}

/// **A hidden wrapper takes no space.** A sidebar row carries four status dots and shows one; a
/// wrapper takes its size from what it wraps, so the three hidden ones went on occupying a dot's
/// width each — four dots in a slot sized for one, and the visible dot drawn over the icon beside
/// it (F003/P096/T483).
#[test]
fn a_hidden_wrapper_leaves_the_slot_to_the_visible_one() {
    use heca_grid_ui::{
        Align, Component, DotStatus, Flex, LayoutExt, Length, Parent, StatusDot, Visibility,
    };

    let mut slot = Flex::row()
        .align(Align::Center)
        .width(Length::Px(12.0))
        .child(Visibility::new(StatusDot::new(DotStatus::Offline), false))
        .child(Visibility::new(StatusDot::new(DotStatus::Online), true))
        .child(Visibility::new(StatusDot::new(DotStatus::Error), false));
    LayoutEngine::new().compute(&mut slot, Size::new(200.0, 40.0));

    let shown = slot.base().children[1].base().bounds;
    assert_eq!(
        shown.size.w, shown.size.h,
        "the visible dot is {:?}",
        shown.size
    );
    assert!(
        shown.loc.x + shown.size.w <= 12.5,
        "it spans {}..{} of a 12px slot — the hidden ones are still taking room",
        shown.loc.x,
        shown.loc.x + shown.size.w,
    );
}

/// **Nothing is painted before the layout has given it a box.**
///
/// A widget's layout node is written by the engine as it walks, so one the walk has never reached
/// still has none — and its default bounds sit at the window's origin with no size. Drawing it
/// there is a real defect, not a theoretical one: a widget added *while* its tree is being laid out
/// (a button's words put back when a pane widens, an overflow trigger built inside the decision that
/// needed it) was drawn in the top-left corner of the screen for the frame before the next layout
/// reached it — a red flicker in the corner throughout a divider drag (Antonio, driving,
/// 2026-09-03).
///
/// The rule is in the one place every widget's paint passes through, so no widget opts in and none
/// can forget it. The next layout gives the new child a box and it is drawn from then on.
#[test]
fn a_widget_that_has_never_been_laid_out_paints_nothing() {
    let theme = Theme::default();

    let painted = |laid_out: bool| -> usize {
        let mut button = Button::new("Close").icon(Glyph::Minus);
        if laid_out {
            LayoutEngine::new()
                .base_font(13.0)
                .compute(&mut button, Size::new(300.0, 60.0));
        }
        let scene = common::paint_via_child(&button, &theme);
        scene.iter().count()
    };

    assert_eq!(
        painted(false),
        0,
        "a widget the layout has never reached has no box, so it draws nothing"
    );
    assert!(
        painted(true) > 0,
        "…and the same widget draws normally once it has been laid out"
    );
}
