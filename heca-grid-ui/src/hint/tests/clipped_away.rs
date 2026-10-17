//! **A letter goes only where it can be seen**.
//!
//! The picker was the one tree walk that never asked `Component::clips_children`. Paint honours it,
//! input honours it (`pointer::hit_test`), and the picker handed letters to rows scrolled past a
//! sidebar's fold — whose keycaps then painted over the top bar and the status bar, because a cap
//! goes into the overlay band and an overlay segment starts unclipped so a dropdown can escape a
//! scroll region.

use crate::LayoutEngine;
use crate::builders::{HintExt as _, IdentityExt as _, LayoutExt as _, Parent as _};
use crate::component::Component;
use crate::hint::*;
use crate::reactive::SignalGet;
use crate::widgets::{Flex, Label, Row, ScrollRegion};
use heca_core::layout::Size;

/// Six 40px rows in a 100px viewport, scrolled to the bottom: the first rows are above the
/// fold, the last are on screen. The same shape as a sidebar with more panes than room.
fn scrolled_sidebar() -> ScrollRegion {
    let rows = (0..6).fold(Flex::column(), |c, i| {
        c.child(
            Row::new()
                .height(40.0)
                .key(format!("pane:{i}"))
                .child(Label::new(format!("zsh {i}")))
                .on_hint(|| {}),
        )
    });
    let mut region = ScrollRegion::new().width(200.0).height(100.0).child(rows);
    LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));
    region.scroll_to(120.0);
    LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));
    region
}

fn row(region: &ScrollRegion, i: usize) -> &dyn Component {
    region.base().children[0].base().children[i].as_ref()
}

fn letter(region: &ScrollRegion, i: usize) -> Option<String> {
    row(region, i).base().hint_label.get_untracked()
}

/// A row scrolled clean past the fold gets **no letter** — by key, which is the `prefix+q`
/// path, where the candidates come from the session model and nothing geometric is consulted.
#[test]
fn a_row_past_the_fold_is_not_lettered() {
    let region = scrolled_sidebar();
    assert!(
        row(&region, 0).base().bounds.loc.y + row(&region, 0).base().bounds.size.h < 0.0,
        "row 0 is entirely above the viewport, which is the case under test",
    );

    assert!(!offer_hint_by_key(&region, "pane:0", Some("a".into())));
    assert_eq!(
        letter(&region, 0),
        None,
        "no keycap to paint over the chrome above"
    );
}

/// **A row you can half see keeps its letter**, so you can peek at it. Any overlap at all is
/// visible.
#[test]
fn a_half_visible_row_keeps_its_letter() {
    let region = scrolled_sidebar();
    let straddling = (0..6)
        .find(|&i| {
            let b = row(&region, i).base().bounds;
            b.loc.y < 0.0 && b.loc.y + b.size.h > 0.0
        })
        .expect("one row straddles the top of the viewport");

    assert!(offer_hint_by_key(
        &region,
        &format!("pane:{straddling}"),
        Some("s".into())
    ));
    assert_eq!(letter(&region, straddling).as_deref(), Some("s"));
}

/// A row in full view is untouched by any of this.
#[test]
fn a_visible_row_is_lettered_as_before() {
    let region = scrolled_sidebar();
    assert!(offer_hint_by_key(&region, "pane:5", Some("d".into())));
    assert_eq!(letter(&region, 5).as_deref(), Some("d"));
}

/// **Withdrawal is never refused.** A row that scrolled out of view *after* being lettered must
/// still lose its keycap, or the cap outlives the picker that put it up.
#[test]
fn a_letter_is_always_withdrawable_wherever_the_row_has_gone() {
    let mut region = scrolled_sidebar();
    assert!(offer_hint_by_key(&region, "pane:5", Some("d".into())));

    region.scroll_to(0.0);
    LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));
    assert!(
        row(&region, 5).base().bounds.loc.y > 100.0,
        "row 5 has scrolled below the fold while wearing its letter",
    );

    assert!(
        offer_hint_by_key(&region, "pane:5", None),
        "the withdrawal still lands"
    );
    assert_eq!(letter(&region, 5), None);
}

/// **A tree that has never been laid out still gets its letters.** Every widget's bounds are
/// zero before the first layout, so a clipping widget with no geometry would clip everything
/// away — which took every letter off the workspaces sidebar in the provider's own tests, where
/// the body is built and collected without being laid out. Degenerate bounds are *"no answer
/// yet"*, never *"nothing is visible"*.
#[test]
fn a_tree_with_no_layout_yet_is_not_clipped_to_nothing() {
    let rows = (0..3).fold(Flex::column(), |c, i| {
        c.child(
            Row::new()
                .key(format!("pane:{i}"))
                .child(Label::new("zsh"))
                .on_hint(|| {}),
        )
    });
    let region = ScrollRegion::new().child(rows); // never laid out: all bounds are 0x0

    assert_eq!(
        collect_hints(&region).len(),
        3,
        "the rows are still candidates"
    );
    assert!(offer_hint_by_key(&region, "pane:1", Some("a".into())));
}

/// The same rule on the other walk: `collect_hints` drops a clipped-away candidate, so it does
/// not spend one of the 52 either.
#[test]
fn a_clipped_away_candidate_is_not_collected() {
    let region = scrolled_sidebar();
    let found = collect_hints(&region);
    for (path, bounds) in &found {
        assert!(
            bounds.loc.y + bounds.size.h > 0.0 && bounds.loc.y < 100.0,
            "a candidate outside the viewport was collected at {path:?} ({bounds:?})",
        );
    }
    assert!(
        !found.is_empty(),
        "the rows still in view are still candidates"
    );
}
