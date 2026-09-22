//! **A container that is handed its children's rects keeps them, and paints each child once.**
//!
//! These two pin the traps that made the column-owns-its-panes move fail before (F003/P082/T474
//! Part A). Neither depends on that move having happened: they are about the mechanism it will use,
//! so they hold today and break the moment it is done the wrong way.

use heca_grid_ui::builders::{LayoutExt, Parent, StyleExt};
use heca_grid_ui::component::{PaintCx, paint_child};
use heca_grid_ui::scene::{DrawCommand, Scene};
use heca_grid_ui::theme::Theme;
use heca_grid_ui::widgets::{Flex, Surface};
use heca_grid_ui::{Color, Component, LayoutEngine, Size};

/// The window manager owns pane geometry, so a column is told where each pane goes — including the
/// space between them.
const SLOTS: [(f32, f32); 3] = [(0.0, 100.0), (110.0, 100.0), (220.0, 100.0)];
/// What the WM left between two panes. This is the thing that goes missing.
const GAP: f64 = 10.0;

fn child_tops(parent: &dyn Component) -> Vec<(f64, f64)> {
    parent
        .base()
        .children
        .iter()
        .map(|c| (c.base().bounds.loc.y, c.base().bounds.size.h))
        .collect()
}

/// **Placed at the rects it was given, a container keeps the gaps.**
///
/// A pane below the first drawn even a few pixels high sits over its own terminal text, which is
/// what this costs when it goes wrong.
#[test]
fn a_container_told_where_its_children_go_keeps_the_space_between_them() {
    let mut parent: Box<dyn Component> = Box::new(
        Flex::column().width(400.0).height(400.0).child(
            SLOTS
                .iter()
                .map(|(top, h)| Flex::column().at_rect(0.0, *top, 400.0, *h))
                .collect::<Vec<_>>(),
        ),
    );
    LayoutEngine::new().compute(parent.as_mut(), Size::new(400.0, 400.0));

    let got = child_tops(parent.as_ref());
    println!("placed: {got:?}");
    for (i, (top, h)) in SLOTS.iter().enumerate() {
        assert!(
            (got[i].0 - *top as f64).abs() < 1.0 && (got[i].1 - *h as f64).abs() < 1.0,
            "child {i} is not at the rect it was given: {got:?}",
        );
    }
    for i in 1..got.len() {
        let space = got[i].0 - (got[i - 1].0 + got[i - 1].1);
        assert!(
            (space - GAP).abs() < 1.0,
            "the gap before child {i} is {space}, not {GAP}: {got:?}",
        );
    }
}

/// **And stacking the same children loses them**, which is why the rects have to be used.
///
/// Not a bug in flex — it is the right answer to a different question. It is here so the next
/// person can see what the test above is protecting against rather than take it on trust.
#[test]
fn stacking_the_same_children_closes_the_gaps() {
    let mut parent: Box<dyn Component> = Box::new(
        Flex::column().width(400.0).height(400.0).child(
            SLOTS
                .iter()
                .map(|(_, h)| Flex::column().height(*h))
                .collect::<Vec<_>>(),
        ),
    );
    LayoutEngine::new().compute(parent.as_mut(), Size::new(400.0, 400.0));

    let got = child_tops(parent.as_ref());
    println!("stacked: {got:?}");
    let space = got[1].0 - (got[0].0 + got[0].1);
    assert!(
        space.abs() < 1.0,
        "stacked children were expected to touch, but there is {space} between them: {got:?}",
    );
}

/// **A parent's paint walk draws each child exactly once.**
///
/// The moment panes become a column's children they are painted by its walk, and anything still
/// painting them one by one paints them twice. Twice is not always visible — two identical opaque
/// rects look like one — so it is counted rather than looked at.
#[test]
fn a_parents_walk_paints_each_child_exactly_once() {
    let colours = [
        Color::new(255, 0, 0, 255),
        Color::new(0, 255, 0, 255),
        Color::new(0, 0, 255, 255),
    ];
    let mut parent: Box<dyn Component> = Box::new(
        Flex::column().width(400.0).height(400.0).child(
            SLOTS
                .iter()
                .zip(colours)
                .map(|((top, h), c)| Surface::new().at_rect(0.0, *top, 400.0, *h).background(c))
                .collect::<Vec<_>>(),
        ),
    );
    LayoutEngine::new().compute(parent.as_mut(), Size::new(400.0, 400.0));

    let theme = Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        paint_child(parent.as_ref(), &mut cx);
    }

    for (i, c) in colours.iter().enumerate() {
        let drawn = scene
            .iter()
            .filter(|cmd| matches!(cmd, DrawCommand::Rect(r) if r.fill == *c))
            .count();
        assert_eq!(drawn, 1, "child {i} was painted {drawn} times, not once");
    }
}
