//! `.order(..)` — CSS `order`: a child says where it is laid out among its siblings.

use heca_grid_ui::builders::{ComponentExt, LayoutExt, Parent, PlaceExt};
use heca_grid_ui::widgets::{Flex, Grid};
use heca_grid_ui::{Component, LayoutEngine, Size};

fn block() -> Flex {
    Flex::column().height(100.0)
}

fn tops(root: &dyn Component) -> Vec<f64> {
    root.base()
        .children
        .iter()
        .map(|c| c.base().bounds.loc.y)
        .collect()
}

/// Lower first, ties and silence in the order added — and the TREE is untouched, so paint, Tab and
/// the letter picker still walk children as they were added.
#[test]
fn order_moves_where_a_child_is_laid_out_and_not_where_it_is_in_the_tree() {
    let mut root: Box<dyn Component> = Box::new(Flex::column().child([
        block().key("a"),               // 0
        block().key("b").order(-1),     // first
        block().key("c").order([0, 5]), // after every plain 0, before 1
        block().key("d"),               // 0, after `a` — a tie keeps the order added
        block().key("e").order(1),      // last
    ]));
    LayoutEngine::new().compute(root.as_mut(), Size::new(200.0, 1000.0));

    let t = tops(root.as_ref());
    println!("tops by tree position a..e: {t:?}");
    // Laid out as b, a, d, c, e.
    assert_eq!(t, vec![100.0, 0.0, 300.0, 200.0, 400.0]);
    let keys: Vec<_> = root
        .base()
        .children
        .iter()
        .map(|c| c.base().key.clone().unwrap())
        .collect();
    assert_eq!(
        keys,
        ["a", "b", "c", "d", "e"],
        "the tree keeps the order added"
    );
}

/// Nobody says anything → exactly the order added, so every existing tree is unchanged.
#[test]
fn a_parent_where_nobody_says_an_order_lays_out_as_added() {
    let mut root: Box<dyn Component> = Box::new(Flex::column().child([block(), block(), block()]));
    LayoutEngine::new().compute(root.as_mut(), Size::new(200.0, 1000.0));
    assert_eq!(tops(root.as_ref()), vec![0.0, 100.0, 200.0]);
}

/// In a grid, a child that said where it goes stays there; `order` sorts only what the grid places
/// itself (CSS).
#[test]
fn in_a_grid_a_placed_child_stays_where_it_said() {
    let mut root: Box<dyn Component> =
        Box::new(Grid::new().template_row("100px 100px 100px").child([
            block().key("placed").row(1).order(9),
            block().key("late").order(1),
            block().key("early").order(-1),
        ]));
    LayoutEngine::new().compute(root.as_mut(), Size::new(200.0, 1000.0));
    let t = tops(root.as_ref());
    println!("placed / late / early: {t:?}");
    assert_eq!(t[0], 0.0, "placed in row 1, whatever its order");
    assert!(
        t[2] < t[1],
        "the auto-placed ones follow their order: {t:?}"
    );
}
