//! **A target you cannot see gets no letter** — the second half of the visibility
//! rule, beside the clip. A widget squeezed to nothing draws nothing, so there is nothing to aim at.

use crate::builders::{EventExt as _, IdentityExt as _};
use crate::component::Component;
use crate::hint::*;
use crate::reactive::SignalGet as _;
use crate::widgets::{Flex, Surface};
use heca_core::layout::{Point, Rectangle, Size};

/// A clickable widget laid out at `x`,`y` with size `w`x`h`.
fn target_at(x: f64, y: f64, w: f64, h: f64) -> crate::WidgetModel {
    let mut s = Surface::new().on_click(|_| {});
    s.base_mut().bounds = Rectangle::new(Point::new(x, y), Size::new(w, h));
    crate::WidgetModel::new(s)
}

/// A laid-out page holding one target — so each test below reads as its assertion.
fn page_with(child: crate::WidgetModel) -> Flex {
    let mut root = Flex::column();
    root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(600.0, 800.0));
    root.base_mut().children.push(child);
    root
}

/// The defect: two sidebars in a narrow window squeezed the scrolling area to nothing, and every
/// pane in it laid out at 0x728 — still lettered, its keycap drawn half a cap to the left of a
/// pane with no inside, several of them stacked in the gutter.
#[test]
fn a_target_squeezed_to_no_width_is_not_lettered() {
    let root = page_with(target_at(318.0, 40.0, 0.0, 728.0));

    assert!(
        collect_hints(&root).is_empty(),
        "a pane with no width has nothing to aim at, and must not spend one of the 52",
    );
}

/// The same for the other axis — a row collapsed to a line.
#[test]
fn a_target_squeezed_to_no_height_is_not_lettered() {
    let root = page_with(target_at(10.0, 40.0, 280.0, 0.0));

    assert!(collect_hints(&root).is_empty());
}

/// The counterpart, so the rule cannot be satisfied by lettering nothing: a target with real
/// geometry is still a target.
#[test]
fn a_target_with_a_real_box_is_still_lettered() {
    let root = page_with(target_at(318.0, 40.0, 386.0, 728.0));

    assert_eq!(collect_hints(&root).len(), 1);
}

/// ⚠️ **A tree that has not been laid out yet is not "invisible"** — no position and no size is
/// *no answer yet*. The chrome tree is rebuilt with zero bounds and laid out afterwards; judging
/// it in between called every sidebar row hidden and took back its letter.
#[test]
fn a_target_that_has_not_been_laid_out_keeps_its_candidacy() {
    // Deliberately NOT `page_with`: nothing here has been laid out, root included.
    let mut root = Flex::column();
    root.base_mut().children.push(target_at(0.0, 0.0, 0.0, 0.0));

    assert_eq!(
        collect_hints(&root).len(),
        1,
        "no bounds at all is 'not laid out yet', never 'not visible'",
    );
}

/// The offer walk asks the **same** question, so a letter aimed at a squeezed view by name is
/// refused exactly as the collector would have refused to spend one on it.
#[test]
fn a_letter_is_not_handed_to_a_squeezed_view_by_name() {
    let mut row = Surface::new().key("pane:7").on_click(|_| {});
    row.base_mut().bounds = Rectangle::new(Point::new(318.0, 40.0), Size::new(0.0, 728.0));
    let root = page_with(crate::WidgetModel::new(row));

    assert!(
        !offer_hint_by_key(&root, "pane:7", Some("a".into())),
        "nothing here can show it",
    );
    assert!(
        root.base().children[0]
            .base()
            .hint_label
            .get_untracked()
            .is_none(),
    );
}

/// **A pane you cannot see is still pickable — through its other views**.
///
/// One pane is shown in several places: its own view in the scrolling area, its row in the
/// sidebar, its card in the exposé. They all name it. The visibility rule filters the **view**,
/// never the candidate — so the squeezed view loses the letter and the sidebar row wears it, and
/// the pane stays reachable by keyboard however narrow the window.
#[test]
fn a_squeezed_view_loses_its_letter_and_the_other_views_keep_it() {
    // The pane itself, squeezed to nothing by two sidebars in a narrow window.
    let mut pane = Surface::new().key("pane:7").on_click(|_| {});
    pane.base_mut().bounds = Rectangle::new(Point::new(318.0, 40.0), Size::new(0.0, 728.0));
    // …and its sidebar row, which is perfectly visible.
    let mut row = Surface::new().key("pane:7").on_click(|_| {});
    row.base_mut().bounds = Rectangle::new(Point::new(6.0, 90.0), Size::new(276.0, 28.0));

    let mut root = Flex::column();
    root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(600.0, 800.0));
    root.base_mut().children.push(crate::WidgetModel::new(pane));
    root.base_mut().children.push(crate::WidgetModel::new(row));

    assert!(
        offer_hint_by_key(&root, "pane:7", Some("a".into())),
        "the pane is still pickable — one of its views can show the letter",
    );
    assert!(
        root.base().children[0]
            .base()
            .hint_label
            .get_untracked()
            .is_none(),
        "not on the view squeezed to nothing, where the cap would land outside the pane",
    );
    assert_eq!(
        root.base().children[1]
            .base()
            .hint_label
            .get_untracked()
            .as_deref(),
        Some("a"),
        "on the sidebar row, which you can see and can aim at",
    );
}

/// **Withdrawal is never refused.** A view that was lettered and has since collapsed must still
/// give the letter back, or the keycap outlives the picker that put it up.
#[test]
fn a_squeezed_view_still_gives_its_letter_back() {
    let mut row = Surface::new().key("pane:7").on_click(|_| {});
    row.base_mut().bounds = Rectangle::new(Point::new(318.0, 40.0), Size::new(386.0, 728.0));
    let mut root = page_with(crate::WidgetModel::new(row));

    assert!(offer_hint_by_key(&root, "pane:7", Some("a".into())));
    // …and now the window narrows and the pane is squeezed away.
    root.base_mut().children[0].base_mut().bounds =
        Rectangle::new(Point::new(318.0, 40.0), Size::new(0.0, 728.0));

    assert!(
        offer_hint_by_key(&root, "pane:7", None),
        "taking a letter back must reach a view wherever it has gone",
    );
    assert!(
        root.base().children[0]
            .base()
            .hint_label
            .get_untracked()
            .is_none(),
    );
}
