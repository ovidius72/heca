//! What a region's body actually measures when it is given a template.
use heca_grid_ui::builders::{LayoutExt, Parent};
use heca_grid_ui::widgets::{Flex, Grid};
use heca_grid_ui::{Component, LayoutEngine, Size};

/// Two dock-shaped children in a region-shaped box: each should take half the height.
#[test]
fn two_containers_under_a_template_take_half_the_region_each() {
    // A dock body as the region gets it: a share, willing to shrink.
    let dock = || {
        Flex::column()
            .grow(1.0)
            .basis(0.0)
            .shrink(1.0)
            .child(Flex::column().height(40.0))
    };

    let mut grid = Grid::new().template_row("1fr 1fr").gap(8.0).grow(1.0);
    {
        let l = &mut grid.base_mut().style.layout;
        l.min_height = Some(heca_grid_ui::Length::Px(0.0));
        l.flex_shrink = Some(1.0);
    }
    let grid = grid.child([dock(), dock()]);

    // The sidebar shell: a full-height column holding the body.
    let mut shell: Box<dyn Component> = Box::new(
        Flex::column()
            .height(heca_grid_ui::Length::Percent(1.0))
            .child(grid),
    );
    LayoutEngine::new().compute(shell.as_mut(), Size::new(300.0, 900.0));

    let body = &shell.base().children[0];
    println!("region body h = {:.1}", body.base().bounds.size.h);
    for (i, c) in body.base().children.iter().enumerate() {
        println!(
            "  dock {i}: y={:.1} h={:.1}",
            c.base().bounds.loc.y,
            c.base().bounds.size.h
        );
    }
    let heights: Vec<f64> = body
        .base()
        .children
        .iter()
        .map(|c| c.base().bounds.size.h)
        .collect();
    assert!(
        heights.iter().all(|h| *h > 400.0),
        "each dock takes about half of 900, got {heights:?}",
    );
}
