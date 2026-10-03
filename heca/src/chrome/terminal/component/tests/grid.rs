//! The grid the terminal asks its process to have.

use super::*;

/// A terminal asks for the grid its box and font call for, and asks again only when that
/// changes — a frame that changes nothing says nothing.
#[test]
fn a_terminal_reports_its_grid_only_when_it_changes() {
    let (seams, said) = recording();
    let t = Terminal::new();
    t.bind(seams);
    t.show(&scrolled(0)); // cells are 10 x 20
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    let first = grids(&said);
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!((first[0].cols, first[0].rows), (30, 10));

    for _ in 0..3 {
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 200.0));
    }
    assert_eq!(grids(&said).len(), 1, "the same box says nothing again");

    heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), Size::new(400.0, 200.0));
    let after_resize = grids(&said);
    assert_eq!(after_resize.len(), 2);
    assert_eq!(after_resize[1].cols, 40);

    // A bigger font in the same box is a different grid too.
    let mut zoomed = scrolled(0);
    zoomed.nominal_cell = (20.0, 40.0);
    t.show(&zoomed);
    heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), Size::new(400.0, 200.0));
    let after_zoom = grids(&said);
    assert_eq!(after_zoom.len(), 3);
    assert_eq!((after_zoom[2].cols, after_zoom[2].rows), (20, 5));
}

/// Nobody listening, nothing is lost: the grid is said as soon as someone binds.
#[test]
fn a_grid_nobody_heard_is_said_when_someone_listens() {
    let t = Terminal::new();
    t.show(&scrolled(0));
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    let (seams, said) = recording();
    t.bind(seams);
    heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 200.0));
    assert_eq!(grids(&said).len(), 1);
}

/// Before the font is measured there is no grid to ask for.
#[test]
fn no_grid_is_asked_for_before_the_cell_size_is_known() {
    let (seams, said) = recording();
    let t = Terminal::new();
    t.bind(seams);
    let _root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    assert!(grids(&said).is_empty());
}
