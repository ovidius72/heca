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
///   iff it lies in `viewport` and its **centre** is not covered by a higher surface's occluder.
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
    let vr = viewport.loc.x + viewport.size.w;
    let vb = viewport.loc.y + viewport.size.h;
    let mut kept = Vec::new();
    let mut occluders: Vec<Rectangle> = Vec::new();
    for layer in layers {
        for (target, b) in layer.targets {
            let in_view = b.loc.x < vr
                && b.loc.x + b.size.w > viewport.loc.x
                && b.loc.y < vb
                && b.loc.y + b.size.h > viewport.loc.y;
            let cx = b.loc.x + b.size.w / 2.0;
            let cy = b.loc.y + b.size.h / 2.0;
            if in_view && !occluders.iter().any(|o| covers(o, cx, cy)) {
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
    use super::*;
    use super::super::surfaces::HintSurface;
    use heca_core::layout::{Point, Size};

    /// A target somewhere harmless, named by a path so two of them are never equal.
    fn target(path: usize, x: f64) -> (HintTarget, Rectangle) {
        (
            HintTarget { surface: HintSurface::Chrome, path: vec![path] },
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
            HintLayer { targets: vec![target(0, 0.0)], occluders: vec![], modal: true },
            HintLayer { targets: vec![target(1, 100.0)], occluders: vec![], modal: false },
        ];
        let kept = resolve_hint_layers(stack, viewport());
        assert_eq!(kept.len(), 1, "only the active context's own targets survive");
        assert_eq!(kept[0].0.path, vec![0]);
    }

    /// The counterpart, so the rule above cannot be satisfied by suppressing everything: a layer
    /// that does **not** take the keyboard leaves the surfaces beneath it live. This is what keeps
    /// the sidebar hintable while a pane is zoomed or a toast is up.
    #[test]
    fn a_non_modal_layer_leaves_what_is_beneath_it_pickable() {
        let stack = vec![
            HintLayer { targets: vec![target(0, 0.0)], occluders: vec![], modal: false },
            HintLayer { targets: vec![target(1, 100.0)], occluders: vec![], modal: false },
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
                occluders: vec![Rectangle::new(Point::new(90.0, 0.0), Size::new(200.0, 100.0))],
                modal: false,
            },
            HintLayer { targets: vec![target(1, 100.0)], occluders: vec![], modal: true },
        ];
        let kept = resolve_hint_layers(stack, viewport());
        assert_eq!(kept.len(), 1, "the covered target is dropped, the covering one kept");
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
