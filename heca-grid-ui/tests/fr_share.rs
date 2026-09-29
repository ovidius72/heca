#[test]
fn a_fraction_holds_its_share_however_tall_its_content() {
    use heca_grid_ui::builders::{LayoutExt, Parent, PlaceExt};
    use heca_grid_ui::widgets::{Flex, Grid};
    use heca_grid_ui::{Component, LayoutEngine, Size};

    // Two rows, the first holding far more than its share.
    let tall = |n: usize| {
        let mut f = Flex::column();
        for _ in 0..n {
            f = f.child(Flex::column().height(100.0));
        }
        f
    };
    let mut root: Box<dyn Component> = Box::new(
        Grid::new()
            .template_row("1fr 1fr")
            .height(heca_grid_ui::Length::Percent(1.0))
            .width(heca_grid_ui::Length::Percent(1.0))
            .child([tall(9).row(1), tall(2).row(2)]),
    );
    LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 900.0));

    let rows: Vec<(f64, f64)> = root
        .base()
        .children
        .iter()
        .map(|c| (c.base().bounds.loc.y, c.base().bounds.size.h))
        .collect();
    println!("rows: {rows:?}");
    assert!(
        rows[1].0 + rows[1].1 <= 901.0,
        "the second row stays inside the box: {rows:?}",
    );
    assert!(
        (rows[0].1 - 450.0).abs() < 2.0,
        "a tall first row still takes only its half: {rows:?}",
    );

    // **And the axis nobody templated fills the box too.** Only the rows were written here, so the
    // column is implicit — and an implicit track used to be `auto`, which sizes to its content, so
    // a row-templated grid drew everything at its natural width and left the rest empty.
    let widths: Vec<f64> = root
        .base()
        .children
        .iter()
        .map(|c| c.base().bounds.size.w)
        .collect();
    println!("widths: {widths:?}");
    assert!(
        widths.iter().all(|w| (*w - 300.0).abs() < 1.0),
        "every row is as wide as the box, not as wide as its content: {widths:?}",
    );
}

/// T518's first claim: **a grid with no template needs none.** Two children, nothing declared, and
/// each should get an equal half of the box — the implicit tracks do it, so nothing has to count the
/// children to write a template for them.
#[test]
fn a_grid_with_no_template_splits_its_children_evenly() {
    use heca_grid_ui::builders::{LayoutExt, Parent};
    use heca_grid_ui::widgets::{Flex, Grid};
    use heca_grid_ui::{Component, LayoutEngine, Size};

    let tall = |n: usize| {
        let mut f = Flex::column();
        for _ in 0..n {
            f = f.child(Flex::column().height(100.0));
        }
        f
    };
    let mut root: Box<dyn Component> = Box::new(
        Grid::new()
            .height(heca_grid_ui::Length::Percent(1.0))
            .width(heca_grid_ui::Length::Percent(1.0))
            .child([tall(9), tall(1)]),
    );
    LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 900.0));

    let boxes: Vec<(f64, f64, f64)> = root
        .base()
        .children
        .iter()
        .map(|c| {
            (
                c.base().bounds.loc.y,
                c.base().bounds.size.h,
                c.base().bounds.size.w,
            )
        })
        .collect();
    println!("no template: {boxes:?}");
    assert_eq!(boxes.len(), 2);
    for (y, h, w) in &boxes {
        assert!(
            (h - 450.0).abs() < 2.0,
            "each child takes half the height: {boxes:?}"
        );
        assert!(
            (w - 300.0).abs() < 1.0,
            "each child fills the width: {boxes:?}"
        );
        assert!(y + h <= 901.0, "nothing spills past the box: {boxes:?}");
    }
}

/// T518's second claim: a child that asks for a row the template did not write **still gets one**.
/// Records what that row is sized as, since the template covers only the rows it names.
#[test]
fn a_child_placed_past_the_template_gets_its_own_row() {
    use heca_grid_ui::builders::{LayoutExt, Parent, PlaceExt};
    use heca_grid_ui::widgets::{Flex, Grid};
    use heca_grid_ui::{Component, LayoutEngine, Size};

    let block = || Flex::column().child(Flex::column().height(100.0));
    let mut root: Box<dyn Component> = Box::new(
        Grid::new()
            .template_row("1fr 1fr")
            .height(heca_grid_ui::Length::Percent(1.0))
            .width(heca_grid_ui::Length::Percent(1.0))
            .child([block().row(1), block().row(2), block().row(3)]),
    );
    LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 900.0));

    let rows: Vec<(f64, f64)> = root
        .base()
        .children
        .iter()
        .map(|c| (c.base().bounds.loc.y, c.base().bounds.size.h))
        .collect();
    println!("row(3) in a two-row template: {rows:?}");
    assert!(
        rows[2].0 >= rows[1].0 + rows[1].1 - 1.0,
        "the third child sits below the second: {rows:?}"
    );
    assert!(rows[2].1 > 0.0, "the third row exists: {rows:?}");
    assert!(
        rows[2].0 + rows[2].1 <= 901.0,
        "the third row stays inside the box: {rows:?}"
    );
}

/// `.flex(n)` splits a **row** by its parts too, however wide each child's content is.
#[test]
fn flex_parts_split_a_row_whatever_each_child_holds() {
    use heca_grid_ui::builders::{LayoutExt, Parent};
    use heca_grid_ui::widgets::Flex;
    use heca_grid_ui::{Component, LayoutEngine, Size};

    let wide = |w: f32| Flex::row().child(Flex::row().width(w));
    let mut root: Box<dyn Component> = Box::new(
        Flex::row()
            .width(heca_grid_ui::Length::Percent(1.0))
            .height(heca_grid_ui::Length::Percent(1.0))
            .child([wide(900.0).flex(1.0), wide(10.0).flex(3.0)]),
    );
    LayoutEngine::new().compute(root.as_mut(), Size::new(400.0, 100.0));
    let cols: Vec<(f64, f64)> = root
        .base()
        .children
        .iter()
        .map(|c| (c.base().bounds.loc.x, c.base().bounds.size.w))
        .collect();
    println!("row 1:3: {cols:?}");
    assert!(
        (cols[0].1 - 100.0).abs() < 1.0,
        "a wide child keeps its one part: {cols:?}"
    );
    assert!(
        (cols[1].1 - 300.0).abs() < 1.0,
        "the other gets three parts: {cols:?}"
    );
}
