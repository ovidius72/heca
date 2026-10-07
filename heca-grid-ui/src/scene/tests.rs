use super::*;
use crate::color::Color;
use heca_core::layout::{Point, Size};

fn clip(w: f64) -> DrawCommand {
    DrawCommand::PushClip(Rectangle::new(Point::default(), Size::new(w, w)))
}

/// A neutral, distinguishable command for the SEGMENTATION tests. They only
/// need to tell commands apart; using `PushClip` for that would entangle them
/// with real clip semantics (an unpopped clip is restored on the parent's
/// continuation segment — see the clip test below).
fn marker(n: f64) -> DrawCommand {
    DrawCommand::Rect(RectCmd {
        rect: Rectangle::new(Point::default(), Size::new(n, n)),
        fill: Color::rgb(1, 2, 3),
        border: None,
        radius: 0.0,
        glow: None,
        shadow: None,
    })
}

fn at(x: f64, w: f64) -> DrawCommand {
    DrawCommand::Rect(RectCmd {
        rect: Rectangle::new(Point::new(x, 0.0), Size::new(w, 10.0)),
        fill: Color::rgb(1, 2, 3),
        border: None,
        radius: 0.0,
        glow: None,
        shadow: None,
    })
}

fn window() -> Rectangle {
    Rectangle::new(Point::default(), Size::new(320.0, 900.0))
}

/// **A draw the clips have already thrown away is not an escape** (F003/P082/T481).
///
/// A widget laid out past a scroll viewport's edge opens its own clip out there, entirely
/// outside the viewport's. Intersecting the two gives nothing — nothing inside reaches the
/// screen — but both sweeps read "no intersection" as "no clip in force" and reported every
/// draw inside it at its raw position. Five phantom escapes, and the reason the catalog sweep
/// was committed ignored rather than passing.
#[test]
fn a_draw_inside_a_clip_that_is_itself_clipped_away_is_not_an_escape() {
    let mut scene = Scene::new();
    scene.push(DrawCommand::PushClip(Rectangle::new(
        Point::default(),
        Size::new(304.0, 900.0),
    )));
    scene.push(DrawCommand::PushClip(Rectangle::new(
        Point::new(536.0, 473.0),
        Size::new(19.0, 19.0),
    )));
    scene.push(at(536.0, 16.0));
    assert!(scene.draws_outside(window()).is_empty());
}

/// The other half: an unclipped draw past the edge **is** reported, and it says what it was.
#[test]
fn a_draw_past_the_edge_is_reported_with_what_it_drew() {
    let mut scene = Scene::new();
    scene.push(at(300.0, 40.0));
    let escapes = scene.draws_outside(window());
    assert_eq!(escapes.len(), 1);
    assert_eq!(escapes[0].to_string(), "rect spans 300..340");
}

/// A clip that only **narrows** a draw is honoured, not ignored: the visible part is what is
/// judged, so a row cut off at a viewport's edge is inside its box, not outside it.
#[test]
fn a_clip_that_narrows_a_draw_leaves_it_inside_the_box() {
    let mut scene = Scene::new();
    scene.push(DrawCommand::PushClip(Rectangle::new(
        Point::default(),
        Size::new(320.0, 900.0),
    )));
    scene.push(at(300.0, 100.0));
    assert!(scene.draws_outside(window()).is_empty());
}

/// **Regression guard.** A nested overlay splits the parent's segment, and
/// segments render independently with a fresh clip stack — so a clip opened
/// *before* the nested overlay must be re-established on the parent's
/// continuation segment, or everything the parent draws afterwards is
/// unclipped.
///
/// Real symptom: opening a `Select` inside a scrolled `Dialog` body made the
/// rows painted after it escape the `ScrollRegion`'s clip and draw outside the
/// modal. The nested overlay itself must stay UNCLIPPED (a dropdown legitimately
/// extends past the region it lives in).
#[test]
fn a_clip_open_around_a_nested_overlay_is_restored_after_it() {
    let row = || {
        DrawCommand::Rect(RectCmd {
            rect: Rectangle::new(Point::default(), Size::new(10.0, 10.0)),
            fill: Color::rgb(1, 2, 3),
            border: None,
            radius: 0.0,
            glow: None,
            shadow: None,
        })
    };

    let mut s = Scene::new();
    s.begin_overlay(); // the Dialog's layer
    s.push(clip(100.0)); // the ScrollRegion clips its body
    s.push(row()); // a row, clipped
    s.begin_overlay(); // a Select opens INSIDE the clipped body
    s.push(row()); // the dropdown panel
    s.end_overlay();
    s.push(row()); // the rows the parent draws AFTER the dropdown
    s.push(DrawCommand::PopClip);
    s.end_overlay();

    let segments: Vec<Scene> = s.overlay_segments().collect();

    // The nested dropdown must NOT inherit the clip — it legitimately extends
    // beyond the region it was opened inside.
    let nested = segments
        .iter()
        .find(|seg| seg.iter().count() == 1)
        .expect("the nested overlay is its own single-command segment");
    assert!(
        !nested.iter().any(|c| matches!(c, DrawCommand::PushClip(_))),
        "a nested overlay starts unclipped so a dropdown can escape its region"
    );

    // …but the parent's continuation must re-open it, or those later rows
    // paint outside the modal (the reported bug). NB segments are yielded
    // depth-ordered, so the continuation is *not* simply the last one — find it
    // by the `PopClip` that closes the region.
    let continuation = segments
        .iter()
        .find(|seg| seg.iter().any(|c| matches!(c, DrawCommand::PopClip)))
        .expect("the parent's continuation segment closes the clip");
    assert!(
        continuation
            .iter()
            .any(|c| matches!(c, DrawCommand::PushClip(_))),
        "the clip open before the nested overlay must be re-established, else \
         everything the parent draws after the dropdown escapes it"
    );
}

#[test]
fn overlay_segments_yields_one_per_nonempty_begin_end_pair() {
    let mut s = Scene::new();
    s.begin_overlay(); // overlay A: two commands
    s.push(marker(1.0));
    s.push(marker(1.5));
    s.end_overlay();
    s.begin_overlay(); // overlay B: one command
    s.push(marker(2.0));
    s.end_overlay();

    let segs: Vec<Scene> = s.overlay_segments().collect();
    assert_eq!(
        segs.len(),
        2,
        "two non-empty overlays should yield two segments, got {}",
        segs.len()
    );
    // Each segment holds exactly its own commands, in paint (z) order: A then B.
    assert_eq!(
        segs[0].iter().cloned().collect::<Vec<_>>(),
        vec![marker(1.0), marker(1.5)],
        "first segment should hold overlay A's commands"
    );
    assert_eq!(
        segs[1].iter().cloned().collect::<Vec<_>>(),
        vec![marker(2.0)],
        "second segment should hold overlay B's command"
    );
}

#[test]
fn empty_begin_end_pair_records_no_segment() {
    let mut s = Scene::new();
    s.begin_overlay();
    s.end_overlay();
    assert_eq!(
        s.overlay_segments().count(),
        0,
        "a begin/end pair that pushed nothing should record no segment"
    );
}

#[test]
fn base_layer_excludes_overlay_commands() {
    let mut s = Scene::new();
    s.push(marker(0.0)); // base
    s.begin_overlay();
    s.push(marker(1.0)); // overlay
    s.end_overlay();
    assert_eq!(
        s.base_layer().iter().cloned().collect::<Vec<_>>(),
        vec![marker(0.0)],
        "base layer should hold only base commands, not overlay ones"
    );
}

#[test]
fn nested_overlay_keeps_parent_segment_and_composites_on_top() {
    // A `Dialog` paints its panel inside `with_overlay`; a child overlay (a `Select`
    // dropdown, a `Tooltip`) paints inside its own `with_overlay` — nested. Two past bugs:
    // (1) the inner `end_overlay` dropped the parent's segment (scrim/panel vanished);
    // (2) segments rendered in RECORD order, so the parent's draws AFTER the nest (its
    // action buttons) painted over the nested panel (the Select-in-Dialog show-through,
    // T009 BUG A). Nesting must yield three segments with the nested one LAST (on top):
    // parent-before, parent-after, then the deeper child.
    let mut s = Scene::new();
    s.begin_overlay(); // outer (Dialog panel)
    s.push(marker(1.0)); // panel, before the nested overlay
    s.begin_overlay(); // inner (Select dropdown)
    s.push(marker(2.0)); // dropdown list
    s.end_overlay(); // close inner — must NOT drop the outer
    s.push(marker(3.0)); // outer continues (action buttons, drawn after the nest)
    s.end_overlay(); // close outer

    let segs: Vec<Scene> = s.overlay_segments().collect();
    assert_eq!(
        segs.len(),
        3,
        "parent segment must survive the nested child"
    );
    assert_eq!(
        segs[0].iter().cloned().collect::<Vec<_>>(),
        vec![marker(1.0)],
        "segment 0 = parent content before the nest"
    );
    assert_eq!(
        segs[1].iter().cloned().collect::<Vec<_>>(),
        vec![marker(3.0)],
        "segment 1 = parent content after the nest (same depth as segment 0)"
    );
    assert_eq!(
        segs[2].iter().cloned().collect::<Vec<_>>(),
        vec![marker(2.0)],
        "segment 2 = the nested overlay, rendered LAST so it occludes the whole parent"
    );
}

#[test]
fn sibling_overlays_keep_record_order_within_a_depth() {
    // Depth ordering must be STABLE: two top-level overlays (a dropdown, then the toast
    // stack painted after it) keep record order — the later one still occludes the earlier.
    let mut s = Scene::new();
    s.begin_overlay();
    s.push(marker(1.0));
    s.end_overlay();
    s.begin_overlay();
    s.push(marker(2.0));
    s.end_overlay();
    let segs: Vec<Scene> = s.overlay_segments().collect();
    assert_eq!(segs.len(), 2);
    assert_eq!(
        segs[0].iter().cloned().collect::<Vec<_>>(),
        vec![marker(1.0)]
    );
    assert_eq!(
        segs[1].iter().cloned().collect::<Vec<_>>(),
        vec![marker(2.0)],
        "same-depth overlays render in record order (later on top)"
    );
}

#[test]
fn nested_overlay_stays_in_overlay_until_outermost_close() {
    // Only the OUTERMOST `end_overlay` returns to the base layer. A push between the inner
    // close and the outer close must land in the overlay layer, never leak to base.
    let mut s = Scene::new();
    s.begin_overlay();
    s.begin_overlay();
    s.end_overlay(); // inner closed, but still inside the outer overlay
    s.push(marker(9.0)); // still overlay-targeted
    s.end_overlay(); // outer closed → back to base
    s.push(marker(8.0)); // base
    assert_eq!(
        s.base_layer().iter().cloned().collect::<Vec<_>>(),
        vec![marker(8.0)],
        "the mid-nest push must not leak to base; only post-outer-close pushes are base"
    );
}

// ── the outline band: painted over what the widget's children drew ──

#[test]
fn an_outline_is_drawn_after_what_was_pushed_after_it_was_asked_for() {
    let mut s = Scene::new();
    s.push(marker(1.0)); // the widget's own fill
    s.begin_outline();
    s.push(marker(2.0)); // its border
    s.end_outline();
    s.push(marker(3.0)); // its child, painted after the widget
    let drawn: Vec<_> = s.iter().cloned().collect();
    assert_eq!(drawn, vec![marker(1.0), marker(3.0), marker(2.0)]);
    assert_eq!(
        s.base_layer().len(),
        3,
        "the base layer carries the outline"
    );
}

#[test]
fn an_outline_keeps_every_clip_open_around_it_and_closes_them_again() {
    let mut s = Scene::new();
    s.push(clip(100.0)); // an outer pane's region
    s.push(clip(50.0)); // a nested pane's region
    s.begin_outline();
    s.push(marker(2.0));
    s.end_outline();
    s.push(DrawCommand::PopClip);
    s.push(DrawCommand::PopClip);
    // A second outline, after the clips closed, must not inherit them.
    s.begin_outline();
    s.push(marker(4.0));
    s.end_outline();

    let drawn: Vec<_> = s.iter().cloned().collect();
    assert_eq!(
        drawn,
        vec![
            clip(100.0),
            clip(50.0),
            DrawCommand::PopClip,
            DrawCommand::PopClip,
            // outline piece one: the same two clips, then closed
            clip(100.0),
            clip(50.0),
            marker(2.0),
            DrawCommand::PopClip,
            DrawCommand::PopClip,
            // outline piece two: no clips were open
            marker(4.0),
        ]
    );
}

#[test]
fn an_outline_inside_an_overlay_or_another_outline_paints_in_place() {
    let mut s = Scene::new();
    s.begin_overlay();
    s.begin_outline();
    s.push(marker(1.0));
    s.end_outline();
    s.end_overlay();
    assert_eq!(s.overlay_segments().count(), 1, "it stayed in the overlay");
    assert!(s.outline.is_empty());

    s.begin_outline();
    s.begin_outline();
    s.push(marker(2.0));
    s.end_outline();
    s.push(marker(3.0));
    s.end_outline();
    assert_eq!(s.outline, vec![marker(2.0), marker(3.0)]);
    s.push(marker(4.0));
    assert_eq!(s.commands, vec![marker(4.0)], "the pair closed: base again");
}

// ── base runs: the scene cut at its terminal surfaces ──

fn surface(id: u64) -> DrawCommand {
    DrawCommand::Host(HostCmd {
        draw: HostDraw::Surface { id },
        rect: Rectangle::new(Point::default(), Size::new(5.0, 5.0)),
        alpha: 1.0,
        echo: false,
    })
}

#[test]
fn a_scene_without_a_surface_is_one_run_equal_to_its_base_layer() {
    let mut s = Scene::new();
    s.push(marker(1.0));
    s.begin_outline();
    s.push(marker(2.0));
    s.end_outline();
    let runs = s.base_runs();
    assert_eq!(runs.len(), 1);
    assert!(runs[0].then.is_none());
    assert_eq!(
        runs[0].draws.iter().cloned().collect::<Vec<_>>(),
        s.base_layer().iter().cloned().collect::<Vec<_>>()
    );
}

#[test]
fn a_surface_cuts_the_runs_and_the_outline_stays_after_it() {
    let mut s = Scene::new();
    s.push(marker(1.0)); // a pane's fill
    s.begin_outline();
    s.push(marker(9.0)); // its border, asked for before its child
    s.end_outline();
    s.push(surface(7)); // its terminal
    s.push(marker(3.0)); // the chip, over the terminal
    let runs = s.base_runs();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].surface().map(|at| at.id), Some(7));
    assert_eq!(
        runs[0].draws.iter().cloned().collect::<Vec<_>>(),
        vec![marker(1.0)]
    );
    assert_eq!(
        runs[1].draws.iter().cloned().collect::<Vec<_>>(),
        vec![marker(3.0), marker(9.0)],
        "the chip, then the border: both over the surface, the border last"
    );
}

#[test]
fn every_run_closes_the_clips_it_opened_and_the_next_one_opens_them_again() {
    let mut s = Scene::new();
    s.push(clip(100.0));
    s.push(clip(50.0));
    s.push(marker(1.0));
    s.push(surface(1));
    s.push(marker(2.0));
    s.push(DrawCommand::PopClip);
    s.push(DrawCommand::PopClip);
    let runs = s.base_runs();
    let drawn = |r: &BaseRun| r.draws.iter().cloned().collect::<Vec<_>>();
    assert_eq!(
        drawn(&runs[0]),
        vec![
            clip(100.0),
            clip(50.0),
            marker(1.0),
            DrawCommand::PopClip,
            DrawCommand::PopClip
        ]
    );
    assert_eq!(
        drawn(&runs[1]),
        vec![
            clip(100.0),
            clip(50.0),
            marker(2.0),
            DrawCommand::PopClip,
            DrawCommand::PopClip
        ]
    );
}

#[test]
fn a_surface_knows_every_clip_around_it() {
    let mut s = Scene::new();
    s.push(surface(1)); // nothing clips this one
    s.push(clip(100.0));
    s.push(clip(50.0));
    s.push(surface(2)); // clipped by the smaller of the two
    let runs = s.base_runs();
    assert_eq!(runs[0].surface().unwrap().clip, None);
    assert_eq!(
        runs[1].surface().unwrap().clip,
        Some(Rectangle::new(Point::default(), Size::new(50.0, 50.0)))
    );
}

/// **A backdrop is a cut**: what it blurs is exactly the runs before it, and what is drawn after it
/// is not in the blur.
#[test]
fn a_backdrop_is_a_cut_between_what_it_blurs_and_what_is_over_it() {
    let mut s = Scene::new();
    s.push(marker(1.0));
    s.push(DrawCommand::Host(HostCmd {
        draw: HostDraw::Backdrop {
            radius: 4.0,
            corner: 0.0,
        },
        rect: Rectangle::new(Point::default(), Size::new(5.0, 5.0)),
        alpha: 0.5,
        echo: false,
    }));
    s.push(marker(2.0));
    let runs = s.base_runs();
    assert_eq!(runs.len(), 2);
    assert!(matches!(
        runs[0].then,
        Some(crate::scene::HostWork::Backdrop(b)) if b.radius == 4.0 && b.alpha == 0.5
    ));
    assert_eq!(runs[0].draws.iter().cloned().collect::<Vec<_>>(), vec![marker(1.0)]);
    assert_eq!(runs[1].draws.iter().cloned().collect::<Vec<_>>(), vec![marker(2.0)]);
}
