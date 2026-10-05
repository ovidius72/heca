use super::*;
use crate::builders::{LayoutExt, Parent};
use crate::style::Length;
use crate::widgets::Flex;

/// **Regression guard.** Taffy leaves the root at the origin of the space it is
/// given, so a margin on the root used to be silently dropped — it worked on every
/// child, and nothing distinguished the root. A caller positioning a box by margin
/// got no error and no warning; the box simply drew at the window's corner instead
/// of where it was put. That cost a mis-placed overlay a whole pane away from its
/// target before the engine applied it here.
#[test]
fn the_root_is_offset_by_its_own_margin() {
    let mut root = Flex::row()
        .width(100.0)
        .height(50.0)
        .margin_left(600.0)
        .margin_top(100.0);
    LayoutEngine::new().compute(&mut root, Size::new(1000.0, 800.0));
    assert_eq!(root.base().bounds.loc, Point::new(600.0, 100.0));
}

/// The offset carries into descendants — a child of an offset root must move with
/// it, not stay behind at the window's corner.
#[test]
fn a_root_margin_moves_the_whole_subtree() {
    let mut root = Flex::row()
        .width(100.0)
        .height(50.0)
        .margin_left(600.0)
        .margin_top(100.0)
        .child(Flex::row().width(20.0).height(20.0));
    LayoutEngine::new().compute(&mut root, Size::new(1000.0, 800.0));
    let child = root.base().children[0].base().bounds.loc;
    assert_eq!(child, Point::new(600.0, 100.0));
}

/// A root without a margin still starts at the origin — the common case must not
/// shift.
#[test]
fn a_root_without_a_margin_starts_at_the_origin() {
    let mut root = Flex::row().width(100.0).height(50.0);
    LayoutEngine::new().compute(&mut root, Size::new(1000.0, 800.0));
    assert_eq!(root.base().bounds.loc, Point::new(0.0, 0.0));
}

/// ⚠️ **A percentage margin resolves against the parent's WIDTH — on both axes.**
///
/// This is CSS's rule and taffy implements it faithfully, but it is the opposite of
/// what "a fraction of the parent" reads as on the vertical, and it is silent: a
/// `margin_top(Percent(0.25))` produces a number, just the wrong one, on any parent that
/// is not square. So a caller placing a box at a **fractional rect** can express its
/// `x` this way and **not** its `y` — the vertical fraction has to be a share
/// (`grow` weights) or a wrapper the engine can measure against the right axis.
///
/// Written down here because the exposé's float placement was designed around
/// percentage margins on both axes, and nothing in the API says which axis it means.
#[test]
fn a_percentage_margin_resolves_against_the_parents_width_on_both_axes() {
    let mut root = Flex::row().width(800.0).height(400.0).child(
        Flex::row()
            .width(10.0)
            .height(10.0)
            .margin_left(Length::Percent(0.25))
            .margin_top(Length::Percent(0.25)),
    );
    LayoutEngine::new().compute(&mut root, Size::new(800.0, 400.0));
    // A quarter of the width on **both** — not (200, 100), which is what a
    // per-axis reading would give.
    assert_eq!(
        root.base().children[0].base().bounds.loc,
        Point::new(200.0, 200.0)
    );
}

/// **`at_rect` is the answer the margin could not give**: a fractional rect where each
/// percentage resolves against its own axis, so a box lands where the caller said on a parent
/// of any shape. This is what places a floating pane over the workspace it belongs to.
#[test]
fn a_fractional_rect_resolves_each_percentage_against_its_own_axis() {
    let mut root = Flex::row()
        .width(800.0)
        .height(400.0)
        .child(Flex::row().at_rect(
            Length::Percent(0.25),
            Length::Percent(0.25),
            Length::Percent(0.5),
            Length::Percent(0.5),
        ));
    LayoutEngine::new().compute(&mut root, Size::new(800.0, 400.0));
    let placed = root.base().children[0].base().bounds;
    assert_eq!(placed.loc, Point::new(200.0, 100.0));
    assert_eq!(placed.size, Size::new(400.0, 200.0));
}

/// **Placing something is not resizing it.** An axis left `Auto` in a placement is a question
/// the caller did not answer, so the widget's own size stands there — which is what lets a
/// host seat a surface at the window origin without also deciding how big it is.
///
/// Reading `Auto` as "shrink to content" instead is how a menu seated as a surface ended up
/// stretched down the whole window: the seat handed it the viewport, and a menu is not a layer
/// — it *is* its panel. Both halves are here, because the trap is that one of them is silent:
/// a layer that declares its own `Percent(1.0)` must keep filling the window.
#[test]
fn a_placement_that_leaves_an_axis_auto_keeps_the_widgets_own_size() {
    let mut root = Flex::row()
        .width(800.0)
        .height(400.0)
        // Sizes itself, like a menu panel: the seat must not touch it.
        .child(Flex::row().width(220.0).height(90.0).at_rect(
            Length::Percent(0.0),
            Length::Percent(0.0),
            Length::Auto,
            Length::Auto,
        ))
        // Declares itself the whole window, like every layer-shaped surface.
        .child(
            Flex::row()
                .width(Length::FULL)
                .height(Length::FULL)
                .at_rect(
                    Length::Percent(0.0),
                    Length::Percent(0.0),
                    Length::Auto,
                    Length::Auto,
                ),
        );
    LayoutEngine::new().compute(&mut root, Size::new(800.0, 400.0));

    assert_eq!(
        root.base().children[0].base().bounds.size,
        Size::new(220.0, 90.0),
        "the panel kept the size it set on itself",
    );
    assert_eq!(
        root.base().children[1].base().bounds.size,
        Size::new(800.0, 400.0),
        "and the layer still fills the window",
    );
}

/// **A leading icon never pushes the text out of the row** (F003/P082/T481).
///
/// The row is willing to shrink and the label is willing to be cut, and it still overflowed:
/// flexbox floors every item at its own content width, so the icon kept its 40px, the label
/// kept its text width, and the sum was laid out past the row's right edge. Four widgets — a
/// dock header, a group header, a toast, a list row — showed it as text drawn across whatever
/// sat beside them.
#[test]
fn a_leading_slot_never_pushes_the_text_past_the_rows_edge() {
    use crate::widgets::{Ellipsis, Label};

    let mut row = Flex::row()
        .width(60.0)
        .height(30.0)
        .child(Flex::row().width(40.0).height(20.0))
        .child(Label::new("a title far too long for this").truncate(Ellipsis::End));
    LayoutEngine::new().compute(&mut row, Size::new(200.0, 100.0));

    for child in &row.base().children {
        let b = child.base().bounds;
        assert!(
            b.loc.x >= -0.5 && b.loc.x + b.size.w <= 60.5,
            "content laid out at {}..{} in a 60px row",
            b.loc.x,
            b.loc.x + b.size.w,
        );
    }
}

/// **A widget that must keep its size still keeps it.** The floor is removed by default, not
/// forbidden: `flex_shrink(0.0)` is how a control opts out, and it must survive the rule above
/// — otherwise every icon in a tight row would be squeezed to a smear instead of the text
/// giving way.
#[test]
fn a_widget_that_refuses_to_shrink_is_left_alone() {
    let mut row = Flex::row()
        .width(60.0)
        .height(30.0)
        .child(Flex::row().width(40.0).height(20.0).shrink(0.0))
        .child(Flex::row().width(40.0).height(20.0));
    LayoutEngine::new().compute(&mut row, Size::new(200.0, 100.0));
    assert_eq!(row.base().children[0].base().bounds.size.w, 40.0);
}

/// **A placed box is out of the flow** — it takes no space from its siblings and does not move
/// them, which is what makes it draw *over* the row rather than beside it. Without this a
/// floating pane would push the columns it floats above along the strip.
#[test]
fn a_placed_box_takes_no_space_from_its_siblings() {
    let mut root = Flex::row()
        .width(800.0)
        .height(400.0)
        .child(Flex::row().width(Length::FULL).height(Length::FULL))
        .child(Flex::row().at_rect(
            Length::Percent(0.5),
            Length::Percent(0.5),
            Length::Px(100.0),
            Length::Px(100.0),
        ));
    LayoutEngine::new().compute(&mut root, Size::new(800.0, 400.0));
    // The in-flow sibling still has the whole row…
    assert_eq!(
        root.base().children[0].base().bounds.size,
        Size::new(800.0, 400.0)
    );
    // …and the placed box sits on top of it at its own rect.
    assert_eq!(
        root.base().children[1].base().bounds.loc,
        Point::new(400.0, 200.0)
    );
}

// ── A tree is solved again only where it changed ─────────────────────────────────────────

/// A small tree to lay out: a column of three labelled rows.
fn tree() -> Flex {
    use crate::widgets::Label;
    Flex::column()
        .width(300.0)
        .height(200.0)
        .child(Flex::row().child(Label::new("one")))
        .child(Flex::row().child(Label::new("two")))
        .child(Flex::row().child(Label::new("three")))
}

fn compute(root: &mut Flex) -> usize {
    let mut engine = LayoutEngine::new();
    engine.compute(root, Size::new(400.0, 300.0));
    engine.updated_nodes()
}

/// **The same tree is the same layout.** A second pass changes no node, so taffy answers from its
/// own cache. This is what a frame caused only by a terminal printing needs: nothing in the chrome
/// changed, so nothing is solved.
#[test]
fn laying_out_an_unchanged_tree_again_changes_no_node() {
    let mut root = tree();
    let first = compute(&mut root);
    let before = root.base().bounds;
    let second = compute(&mut root);
    assert!(first > 0, "the first pass makes every node: {first}");
    assert_eq!(second, 0, "nothing changed, so nothing was updated");
    assert_eq!(root.base().bounds, before);
}

/// Only the widget that changed is touched: a label's text changing is that label's node, not its
/// row's, its column's or its siblings'.
#[test]
fn changing_one_text_updates_only_that_node() {
    use crate::reactive::SignalUpdate;
    use crate::widgets::Label;
    let label = Label::new("one");
    let text = label.text_signal();
    let mut root = Flex::column()
        .width(300.0)
        .height(200.0)
        .child(Flex::row().child(label))
        .child(Flex::row().child(Label::new("two")));
    compute(&mut root);

    text.set("a much longer line".to_string());
    let updated = compute(&mut root);

    assert_eq!(updated, 1, "the one label");
}

/// **What the pass lays out is still right**, not merely cheap: a changed text moves what follows
/// it, because taffy dirtied the path and re-solved it.
#[test]
fn a_changed_node_is_solved_again_and_what_follows_it_moves() {
    use crate::reactive::SignalUpdate;
    use crate::widgets::Label;
    let first = Label::new("one");
    let text = first.text_signal();
    let mut root = Flex::row()
        .width(400.0)
        .height(50.0)
        .child(first)
        .child(Label::new("two"));
    compute(&mut root);
    let second_x = root.base().children[1].base().bounds.loc.x;

    text.set("one two three four".to_string());
    compute(&mut root);

    assert!(
        root.base().children[1].base().bounds.loc.x > second_x,
        "the neighbour moved right with the longer text"
    );
}

/// A different room is a different layout even though no node changed.
#[test]
fn a_new_size_is_laid_out_again_with_nothing_updated() {
    let mut root = Flex::row().width(Length::Percent(1.0)).height(40.0);
    let mut engine = LayoutEngine::new();
    engine.compute(&mut root, Size::new(400.0, 300.0));
    assert_eq!(root.base().bounds.size.w, 400.0);

    engine.compute(&mut root, Size::new(250.0, 300.0));

    assert_eq!(engine.updated_nodes(), 0, "no node changed");
    assert_eq!(root.base().bounds.size.w, 250.0, "but the room did");
}

/// **Bounds are written every pass.** A host shifts a laid-out subtree to where it goes; the next
/// pass starts from the origin again, or the shift would pile up frame after frame.
#[test]
fn bounds_come_from_the_layout_each_pass_even_when_nothing_changed() {
    let mut root = tree();
    compute(&mut root);
    let at_origin = root.base().bounds.loc;
    crate::component::shift_subtree(&mut root, 50.0, 60.0);
    assert_ne!(root.base().bounds.loc, at_origin);

    compute(&mut root);

    assert_eq!(root.base().bounds.loc, at_origin);
}

/// A child that leaves the tree takes its node with it, and one that joins gets its own — no
/// node outlives its component.
#[test]
fn nodes_follow_the_components_that_come_and_go() {
    use crate::widgets::Label;
    let live = |root: &Flex| {
        root.base()
            .layout_cache
            .borrow()
            .as_ref()
            .map_or(0, |r| r.live.len())
    };
    let mut root = tree();
    compute(&mut root);
    let all = live(&root);

    root.base_mut().children.pop();
    compute(&mut root);
    assert!(live(&root) < all, "a row left with its label");

    root.base_mut()
        .children
        .push(Box::new(Flex::row().child(Label::new("again"))));
    let updated = compute(&mut root);
    assert_eq!(live(&root), all, "and a new one joined");
    assert!(updated > 0);
}

/// A component that was laid out as the root of its own tree and is then put inside another is a
/// new node there: the id it carries belongs to the tree it left.
#[test]
fn a_component_moved_in_from_another_tree_is_laid_out_in_its_new_one() {
    let mut inner = Flex::row().width(80.0).height(30.0);
    compute(&mut inner);
    let mut outer = Flex::column().width(300.0).height(200.0).child(inner);

    compute(&mut outer);

    let b = outer.base().children[0].base().bounds;
    assert_eq!((b.size.w, b.size.h), (80.0, 30.0));
}
