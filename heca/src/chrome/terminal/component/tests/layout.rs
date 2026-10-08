//! The box a terminal is given, and the picture it paints.

use super::*;

/// A terminal in a box learns the box's size — the only thing it needs to size its grid.
#[test]
fn a_terminal_learns_the_size_of_the_box_it_is_given() {
    let t = Terminal::new();
    assert_eq!(t.room(), None, "nothing is known before the first layout");

    let _root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);

    let size = t.room().expect("laid out");
    assert_eq!((size.w, size.h), (300.0, 200.0));
}

/// Below a header the terminal gets what the header leaves — not the whole pane. That is the
/// number the host used to compute by hand and got wrong for a tiled pane (two rows hidden
/// under the header).
#[test]
fn below_a_header_a_terminal_gets_what_the_header_leaves() {
    let t = Terminal::new();
    let _root = header_above(Box::new(t.clone()), 33.0, 308.0, 720.0);

    let size = t.room().expect("laid out");
    assert_eq!((size.w, size.h), (308.0, 687.0));
}

/// It never exceeds the box it is given, whatever the header takes.
#[test]
fn a_terminal_never_exceeds_its_box() {
    for (w, h, header) in [
        (100.0, 50.0, 10.0),
        (308.0, 720.0, 33.0),
        (40.0, 40.0, 60.0),
    ] {
        let t = Terminal::new();
        let _root = header_above(Box::new(t.clone()), header, w, h);
        let size = t.room().expect("laid out");
        assert!(size.w <= w && size.h <= h, "{size:?} exceeds {w}x{h}");
    }
}

/// A clone is the same terminal: what one learns the other reports.
#[test]
fn a_clone_is_the_same_terminal() {
    let t = Terminal::new();
    let placed = t.clone();
    let _root = laid_out_in(Box::new(placed), 120.0, 80.0);
    assert_eq!(t.room().map(|s| (s.w, s.h)), Some((120.0, 80.0)));
}

/// Painting is one command, and it is the terminal's box below the header — not the pane's.
#[test]
fn a_terminal_paints_one_surface_at_its_own_box() {
    let t = Terminal::new();
    t.attach(TerminalId(7));
    let root = header_above(Box::new(t.clone()), 33.0, 308.0, 720.0);

    let theme = Theme::default();
    let mut scene = Scene::new();
    heca_grid_ui::paint_child(root.as_ref(), &mut PaintCx::new(&mut scene, &theme));

    let surfaces: Vec<_> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Host(h) => Some(*h),
            _ => None,
        })
        .collect();
    assert_eq!(surfaces.len(), 1, "one command, nothing else");
    assert_eq!(surfaces[0].draw, HostDraw::Surface { id: 7 });
    assert_eq!(
        (
            surfaces[0].rect.loc.x,
            surfaces[0].rect.loc.y,
            surfaces[0].rect.size.w,
            surfaces[0].rect.size.h
        ),
        (0.0, 33.0, 308.0, 687.0),
    );
}

/// A terminal nobody has named a process for has nothing to show, and says so by drawing
/// nothing.
#[test]
fn a_terminal_with_no_process_paints_nothing() {
    let root = laid_out_in(Box::new(Terminal::new()), 100.0, 100.0);
    let theme = Theme::default();
    let mut scene = Scene::new();
    heca_grid_ui::paint_child(root.as_ref(), &mut PaintCx::new(&mut scene, &theme));
    assert!(scene.iter().all(|c| !matches!(c, DrawCommand::Host(_))));
}

/// **A frame caused only by a terminal printing lays nothing out.** What changes is the picture the
/// host draws for the surface and the viewport it shows back; no widget's style or text does. So a
/// second layout pass over the same tree, with the viewport shown again, updates no node and taffy
/// answers from its own cache.
#[test]
fn a_terminal_that_only_prints_changes_no_layout_node() {
    let t = Terminal::new();
    t.show(&scrolled(0));
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    let size = heca_grid_ui::Size::new(300.0, 200.0);
    let mut engine = heca_grid_ui::LayoutEngine::new();
    engine.compute(root.as_mut(), size);

    // A frame: the owner shows the viewport again, as it does after every burst of output.
    t.show(&scrolled(0));
    engine.compute(root.as_mut(), size);

    assert_eq!(engine.updated_nodes(), 0, "nothing in the tree changed");
}
