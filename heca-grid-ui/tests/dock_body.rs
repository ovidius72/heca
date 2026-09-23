// A dock given half a sidebar keeps its rows inside itself, and scrolls the ones that do not fit.
#[test]
fn a_dock_body_takes_what_the_title_leaves_and_scrolls_the_rest() {
    use heca_grid_ui::builders::{LayoutExt, Parent, PlaceExt};
    use heca_grid_ui::widgets::{DockFrame, Flex, Grid, ScrollRegion};
    use heca_grid_ui::{Component, LayoutEngine, Size};

    let tall = |n: usize| {
        let mut f = Flex::column();
        for _ in 0..n {
            f = f.child(Flex::column().height(100.0));
        }
        f
    };
    // What the app builds: a dock whose body is its own scroll area holding far too much.
    let dock = |n: usize| {
        DockFrame::new("WORKSPACES")
            .frameless(true)
            .child(ScrollRegion::new().grow(1.0).child(tall(n)))
    };

    // Two docks sharing the sidebar, exactly as the region templates them.
    let mut root: Box<dyn Component> = Box::new(
        Grid::new()
            .template_row("1fr 1fr")
            .height(heca_grid_ui::Length::Percent(1.0))
            .width(heca_grid_ui::Length::Percent(1.0))
            .child([dock(9).row(1), dock(2).row(2)]),
    );
    LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 900.0));

    fn dump(c: &dyn Component, depth: usize, out: &mut String) {
        let b = c.base().bounds;
        out.push_str(&format!(
            "{:indent$}{} y={:.0} h={:.0} w={:.0}\n",
            "",
            c.base().grid_area.as_deref().unwrap_or("-"),
            b.loc.y,
            b.size.h,
            b.size.w,
            indent = depth * 2
        ));
        for k in &c.base().children {
            dump(k.as_ref(), depth + 1, out);
        }
    }
    let mut out = String::new();
    dump(root.as_ref(), 0, &mut out);
    println!("{out}");

    let frames: Vec<(f64, f64)> = root
        .base()
        .children
        .iter()
        .map(|c| (c.base().bounds.loc.y, c.base().bounds.size.h))
        .collect();
    assert!(
        (frames[0].1 - 450.0).abs() < 2.0,
        "the first dock takes only its half: {frames:?}",
    );
    assert!(
        frames[1].0 + frames[1].1 <= 901.0,
        "the second dock stays inside the sidebar: {frames:?}",
    );

    // **The body must END inside the frame** — it is what the title leaves, not what its content
    // wants. Asserting only that it is "tall enough" passes on the overflow itself.
    let frame = root.base().children[0].as_ref();
    let body =
        heca_grid_ui::component::area(frame, "body").expect("the frame always holds its body");
    let (fy, fh) = (frame.base().bounds.loc.y, frame.base().bounds.size.h);
    let (by, bh) = (body.base().bounds.loc.y, body.base().bounds.size.h);
    println!("frame y={fy:.0} h={fh:.0} | body y={by:.0} h={bh:.0}");
    assert!(
        by + bh <= fy + fh + 1.0,
        "the body stays inside its frame: body ends at {:.0}, frame ends at {:.0}",
        by + bh,
        fy + fh,
    );

    // And the scroll area inside it must be handed LESS than its content, or there is nothing to
    // scroll and the rows simply run off the frame.
    let scroll = body.base().children[0].as_ref();
    let content = scroll.base().children[0].base().bounds.size.h;
    println!(
        "scroll h={:.0} content h={:.0}",
        scroll.base().bounds.size.h,
        content
    );
    assert!(
        scroll.base().bounds.size.h < content,
        "the scroll area is smaller than what it holds, so it can scroll: {:.0} vs {:.0}",
        scroll.base().bounds.size.h,
        content,
    );
}
