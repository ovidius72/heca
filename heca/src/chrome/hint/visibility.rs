//! **The one visibility rule** (`docs/surface-compositor.md` §3), both halves of it, in the order
//! §2 keeps them: coarse (context activation) first, then fine (geometric occlusion).
//!
//! If you are special-casing a surface's hints, the surface is mis-modelled — fix the model, do not
//! add a filter.

use super::surfaces::HintTarget;
use heca_core::layout::Rectangle;

/// One layer of the on-screen surface stack (front → back) for resolving which hint
/// targets are reachable. See `docs/surface-compositor.md`: a surface owns its targets,
/// the opaque region(s) it paints over lower layers (from real layout — never hardcoded),
/// and whether it is `modal` (a blocking context that suppresses everything beneath it).
pub(super) struct HintLayer {
    pub(super) targets: Vec<(HintTarget, Rectangle)>,
    pub(super) occluders: Vec<Rectangle>,
    pub(super) modal: bool,
}

/// The single visibility rule (`docs/surface-compositor.md` §3), both halves of it, in the order
/// §2 keeps them: **coarse first, then fine.**
///
/// - **Coarse — context activation.** Walking front → back, the first **modal** layer is the active
///   context, and everything beneath it is dormant: the walk stops there. This is what makes
///   `prefix+/` under the exposé offer the map's cards and nothing else.
/// - **Fine — geometric occlusion.** *Within* what the coarse rule left eligible, a target survives
///   iff **the centre of the part of it that is on screen** is not covered by a higher surface's
///   occluder.
///
/// **The centre of the VISIBLE part, not of the target.** The two are the same until a target hangs
/// off an edge, and then they are not: a pane scrolled left until its centre passes x = 0 is not
/// covered by a sidebar occluder that starts at x = 0, so it survived — and drew its keycap at that
/// centre, a sliver of a letter clinging to the window's left margin (Antonio, driving,
/// 2026-08-24). Clipping to the viewport first asks about a point that is actually on screen, which
/// is the only kind of point an occluder can be asked about. It also *is* the "lies in `viewport`"
/// half: an empty intersection means nothing of it is on screen.
///
/// The fine rule subsumes every case — off-screen, hidden behind the sidebar, a zoomed/floating
/// pane drawn over another — and extends to new surfaces for free. The caller must hand `layers`
/// in true front → back order; there is no sort here, because a second ordering is a second answer
/// to "what is in front", and the two drifted once already (2026-08-11: input and painting
/// disagreed about which layer was frontmost).
pub(super) fn resolve_hint_layers(
    layers: Vec<HintLayer>,
    viewport: Rectangle,
) -> Vec<(HintTarget, Rectangle)> {
    let covers = |r: &Rectangle, x: f64, y: f64| {
        x >= r.loc.x && x < r.loc.x + r.size.w && y >= r.loc.y && y < r.loc.y + r.size.h
    };
    let mut kept = Vec::new();
    let mut occluders: Vec<Rectangle> = Vec::new();
    for layer in layers {
        for (target, b) in layer.targets {
            // **A box with no geometry has not been laid out yet.** No answer, never "invisible" —
            // the same rule the clip walk follows (`hint::collect::narrowed`), and it is not
            // optional now that this gate runs every frame: the chrome tree is rebuilt with zero
            // bounds and laid out afterwards, so judging it in between called every sidebar row and
            // top-bar button hidden and *withdrew* their letters (Antonio, driving, 2026-08-24).
            // A target with no bounds also draws no keycap (`fit_into_view` finds no room), so
            // keeping it costs nothing and waiting one frame for real geometry costs the letters.
            if b.size.w <= 0.0 || b.size.h <= 0.0 {
                kept.push((target, b));
                continue;
            }
            // Nothing of it on screen: not a target, and no point to ask an occluder about.
            let Some(seen) = b.intersection(viewport) else {
                continue;
            };
            let cx = seen.loc.x + seen.size.w / 2.0;
            let cy = seen.loc.y + seen.size.h / 2.0;
            if !occluders.iter().any(|o| covers(o, cx, cy)) {
                kept.push((target, b));
            }
        }
        occluders.extend(layer.occluders.iter().copied());
        if layer.modal {
            break;
        }
    }
    kept
}

/// The coarse half of §3 — the half that was missing, and the whole of F003/P082/T416's picker
/// defect. Held here rather than in a behaviour test because the rule is a pure function of the
/// stack, and because the failure it guards was invisible on screen: the letters were painted
/// *under* the exposé while the targets they named answered normally.
#[cfg(test)]
mod hint_visibility {
    use super::super::surfaces::HintSurface;
    use super::*;
    use heca_core::layout::{Point, Size};

    /// **A target whose centre has left the screen is judged where you can still see it**
    /// (F003/P082/T438).
    ///
    /// A pane scrolled left until its middle passes the window's edge is still half on screen —
    /// under the sidebar. Its centre is at a negative x, which no occluder starting at x = 0
    /// contains, so it survived the fine rule and drew its keycap there: a sliver of a letter
    /// clinging to the left margin.
    #[test]
    fn a_target_whose_centre_is_off_screen_is_judged_by_the_part_that_is_not() {
        let sidebar = Rectangle::new(Point::new(0.0, 0.0), Size::new(308.0, 800.0));
        let viewport = Rectangle::new(Point::new(0.0, 0.0), Size::new(1412.0, 800.0));

        // A pane 386 wide, scrolled until its centre sits 5px off the left edge: it spans
        // -198..188, so the part you can see is 0..188 — entirely under the sidebar.
        let pane = (
            HintTarget {
                surface: HintSurface::Pane(heca_core::layout::PaneId(2)),
                path: vec![],
                identity: None,
            },
            Rectangle::new(Point::new(-198.0, 40.0), Size::new(386.0, 728.0)),
        );

        let kept = resolve_hint_layers(
            vec![
                HintLayer {
                    targets: vec![],
                    occluders: vec![sidebar],
                    modal: false,
                },
                HintLayer {
                    targets: vec![pane],
                    occluders: vec![],
                    modal: false,
                },
            ],
            viewport,
        );
        assert!(
            kept.is_empty(),
            "the only part of it on screen is behind the sidebar"
        );
    }

    /// The same target, scrolled far enough right that its visible middle clears the sidebar, is a
    /// target again — the rule must not simply refuse anything that touches an edge.
    #[test]
    fn a_target_hanging_off_an_edge_is_kept_once_what_you_see_clears_the_occluder() {
        let sidebar = Rectangle::new(Point::new(0.0, 0.0), Size::new(308.0, 800.0));
        let viewport = Rectangle::new(Point::new(0.0, 0.0), Size::new(1412.0, 800.0));

        // Spans -50..336: the visible part is 0..336, whose centre (168) is still under the
        // sidebar — but move it right and the visible centre clears it.
        let clear = (
            HintTarget {
                surface: HintSurface::Pane(heca_core::layout::PaneId(3)),
                path: vec![],
                identity: None,
            },
            Rectangle::new(Point::new(200.0, 40.0), Size::new(386.0, 728.0)),
        );

        let kept = resolve_hint_layers(
            vec![
                HintLayer {
                    targets: vec![],
                    occluders: vec![sidebar],
                    modal: false,
                },
                HintLayer {
                    targets: vec![clear],
                    occluders: vec![],
                    modal: false,
                },
            ],
            viewport,
        );
        assert_eq!(
            kept.len(),
            1,
            "its visible middle (393) is past the sidebar's edge"
        );
    }

    /// **A tree that has not been laid out yet is not "hidden"** (F003/P082/T438).
    ///
    /// The chrome tree is rebuilt with zero bounds and laid out after; this gate runs every frame.
    /// Judging it in between answered "nothing here is visible", and because an unseen view is
    /// *withdrawn from* rather than skipped, every sidebar row and top-bar button lost its letter.
    #[test]
    fn a_target_with_no_geometry_yet_is_kept_rather_than_called_hidden() {
        let viewport = Rectangle::new(Point::new(0.0, 0.0), Size::new(1412.0, 800.0));
        let fresh = (
            HintTarget {
                surface: HintSurface::Window,
                path: vec![0],
                identity: None,
            },
            Rectangle::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0)),
        );

        let kept = resolve_hint_layers(
            vec![HintLayer {
                targets: vec![fresh],
                occluders: vec![],
                modal: false,
            }],
            viewport,
        );
        assert_eq!(
            kept.len(),
            1,
            "no bounds is 'no answer yet', never 'not visible'"
        );
    }

    /// A target somewhere harmless, named by a path so two of them are never equal.
    fn target(path: usize, x: f64) -> (HintTarget, Rectangle) {
        (
            HintTarget {
                surface: HintSurface::Window,
                path: vec![path],
                identity: None,
            },
            Rectangle::new(Point::new(x, 10.0), Size::new(20.0, 20.0)),
        )
    }

    fn viewport() -> Rectangle {
        Rectangle::new(Point::new(0.0, 0.0), Size::new(1000.0, 800.0))
    }

    /// **The defect this task exists for.** With the exposé up, `prefix+/` offered the sidebar's
    /// rows — the letters were invisible under the map, so a keystroke drove a surface the user
    /// could not see (Antonio, 2026-08-12). A modal layer is the active context: everything
    /// beneath it is dormant, and dormant surfaces are not pickable.
    #[test]
    fn a_modal_layer_is_the_active_context_and_nothing_beneath_it_is_pickable() {
        let stack = vec![
            HintLayer {
                targets: vec![target(0, 0.0)],
                occluders: vec![],
                modal: true,
            },
            HintLayer {
                targets: vec![target(1, 100.0)],
                occluders: vec![],
                modal: false,
            },
        ];
        let kept = resolve_hint_layers(stack, viewport());
        assert_eq!(
            kept.len(),
            1,
            "only the active context's own targets survive"
        );
        assert_eq!(kept[0].0.path, vec![0]);
    }

    /// The counterpart, so the rule above cannot be satisfied by suppressing everything: a layer
    /// that does **not** take the keyboard leaves the surfaces beneath it live. This is what keeps
    /// the sidebar hintable while a pane is zoomed or a toast is up.
    #[test]
    fn a_non_modal_layer_leaves_what_is_beneath_it_pickable() {
        let stack = vec![
            HintLayer {
                targets: vec![target(0, 0.0)],
                occluders: vec![],
                modal: false,
            },
            HintLayer {
                targets: vec![target(1, 100.0)],
                occluders: vec![],
                modal: false,
            },
        ];
        assert_eq!(resolve_hint_layers(stack, viewport()).len(), 2);
    }

    /// Coarse first, then fine — both, not one. Within the active context a target is still
    /// dropped when a surface in front of it covers its centre.
    #[test]
    fn occlusion_still_applies_inside_the_active_context() {
        let stack = vec![
            HintLayer {
                targets: vec![target(0, 0.0)],
                occluders: vec![Rectangle::new(
                    Point::new(90.0, 0.0),
                    Size::new(200.0, 100.0),
                )],
                modal: false,
            },
            HintLayer {
                targets: vec![target(1, 100.0)],
                occluders: vec![],
                modal: true,
            },
        ];
        let kept = resolve_hint_layers(stack, viewport());
        assert_eq!(
            kept.len(),
            1,
            "the covered target is dropped, the covering one kept"
        );
        assert_eq!(kept[0].0.path, vec![0]);
    }

    /// A target scrolled off the window is not pickable however live its surface is.
    #[test]
    fn a_target_outside_the_viewport_is_dropped() {
        let stack = vec![HintLayer {
            targets: vec![target(0, 5_000.0)],
            occluders: vec![],
            modal: false,
        }];
        assert!(resolve_hint_layers(stack, viewport()).is_empty());
    }
}
