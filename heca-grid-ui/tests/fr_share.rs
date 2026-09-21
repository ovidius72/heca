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
