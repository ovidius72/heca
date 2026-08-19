//! The pane shell — the frame around whatever app is running inside, and the one thing that
//! carries the pane's identity and its pick letter.
//!
//! Antonio, 2026-08-18: *"the only responsibility of the pane is to draw the key hint letter, zoom
//! in/out, float/unfloat, split/unsplit, remove"*, and *"terminal should not be responsible to
//! render letters. They are programs that run inside pane. So pane is the container for every
//! future app."*
//!
//! So the shell owns **no content**. A terminal today, a browser or a Neovim GUI later, or a
//! plugin's own tree — each is a child, and none of them has anything to do with the letter.

use heca_core::layout::PaneId;
use heca_grid_ui::widgets::{HintPlacement, KeyHint, Pane as UiPane};
use heca_grid_ui::{ComponentExt, LayoutExt, StyleExt};

use super::model::PaneShellModel;

/// The app seams the shell binds, travelling as **one group** rather than one argument at a time
/// (AGENTS.md § 0b-bis rule 3) — so a test can hand it the app's edges and nothing else.
pub(crate) struct PaneCallbacks {
    /// What picking this pane does: focus it. Emitted as an `InteractionIntent`, never performed
    /// here — the pane asks, the core does.
    pub(crate) pick: std::rc::Rc<dyn Fn(PaneId)>,
}

/// The pane shell component. Properties are struct fields and the constructor is a struct literal
/// (AGENTS.md § 0b-bis rule 2), so a call site reads without counting arguments.
pub(crate) struct PaneShell<'a> {
    pub(crate) model: &'a PaneShellModel,
    pub(crate) cb: &'a PaneCallbacks,
}

impl PaneShell<'_> {
    /// Build the retained tree for one pane.
    ///
    /// Returns the `KeyHint` wrapper rather than the `Pane` inside it: the wrapper is what carries
    /// the pick declaration, and it is transparent to layout, focus and events, so the pane below
    /// stays an ordinary widget. (`ComponentExt::on_hint` — which would let the `Pane` declare it
    /// directly and drop the wrapper — is F003/P082/T432 and is not built yet.)
    ///
    /// **The letter is drawn by the framework**, inside `heca_grid_ui::paint_child`, from
    /// `Base::hint_label`. Anything that paints this tree with a direct `.paint(cx)` will show no
    /// letter at all — which is exactly the defect this task exists to end.
    pub(crate) fn build(self) -> KeyHint {
        let PaneShellModel {
            pane_id,
            active,
            frame,
            border_color,
            border_width,
            border_radius,
            content_inset,
            accent,
            ..
        } = *self.model;

        // **The pane fills whatever rect it is given — it never carries one.** A share, not a
        // measure: the WM layout engine owns a pane's geometry, and `sync_panes` writes that rect
        // onto this tree's root every frame. Baking `Px(w)` in here instead meant the tree kept
        // the width it was first built at, so zooming or resizing left the frame at the old size
        // while the content moved (found by tracing: asked 648 wide, got 380, forever).
        let mut pane = crate::chrome::apply_pane_frame(UiPane::new(), frame)
            .width(heca_grid_ui::Length::Pct(1.0))
            .height(heca_grid_ui::Length::Pct(1.0))
            .padding(content_inset)
            .border(to_gui_color(border_color), border_width)
            .radius(border_radius);
        if active {
            pane = pane.glow_with(to_gui_color(border_color), ACTIVE_GLOW_RADIUS, ACTIVE_GLOW_STRENGTH);
        }

        // The pane's own identity, from the data — never a counter, never a position. A pane id is
        // stable across every rebuild, which is what lets a letter stay with the same pane between
        // openings of the picker (F003/P082/T445).
        let pane = pane.key(crate::chrome::pane_key(pane_id));

        let pick = self.cb.pick.clone();
        KeyHint::new(pane)
            // Centred over the pane, which is where the letter is today and what the maintainer
            // expects. `TopCenter` is for compact square targets; a pane is the large-target case.
            .placement(HintPlacement::Center)
            // The theme accent, not the ambient one: see `PaneShellModel::accent`.
            .color(to_gui_color(accent))
            .on_hint(move || pick(pane_id))
    }
}

/// **Give the shell the rect the WM assigned it.** Size is a per-frame input, never part of the
/// built tree: the layout engine owns a pane's geometry, and a zoom, a resize or a float must move
/// the frame without rebuilding the widget and throwing away its signals.
///
/// Baking `Px(w)` into `build` instead is the regression this exists to stop — the tree kept the
/// width it was first built at, so the border stayed put while the content moved (traced: asked
/// 648 wide, got 380, every frame).
pub(crate) fn size_to(root: &mut KeyHint, w: f32, h: f32) {
    use heca_grid_ui::Component;
    let style = &mut root.base_mut().style.layout;
    style.width = heca_grid_ui::Length::Px(w);
    style.height = heca_grid_ui::Length::Px(h);
}

/// Halo radius + strength for the active pane's frame. Kept here beside the only widget that draws
/// it, and scaled by the theme's `glow_size` at the single `PaintCx` chokepoint like every other
/// glow — so `glow_size = none` removes it with the rest.
const ACTIVE_GLOW_RADIUS: f32 = 10.0;
const ACTIVE_GLOW_STRENGTH: f32 = 0.55;

fn to_gui_color(color: [f32; 4]) -> heca_grid_ui::Color {
    heca_grid_ui::Color::new(
        (color[0] * 255.0) as u8,
        (color[1] * 255.0) as u8,
        (color[2] * 255.0) as u8,
        (color[3] * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::pane::testing::{model, recording_callbacks};
    use heca_grid_ui::Component;
    use heca_grid_ui::{LayoutEngine, Size};

    /// It renders what it was given: the pane's own identity, from its id.
    #[test]
    fn the_pane_declares_its_own_identity() {
        let (cb, _) = recording_callbacks();
        let m = model(7);
        let tree = PaneShell { model: &m, cb: &cb }.build();
        // The wrapper is transparent; the identity belongs to the pane inside it.
        let pane = &tree.base().children[0];
        assert_eq!(pane.base().key.as_deref(), Some("pane:7"));
    }

    /// It answers what it binds: picking it focuses that pane, and nothing else.
    #[test]
    fn picking_the_pane_focuses_that_pane() {
        let (cb, picked) = recording_callbacks();
        let m = model(3);
        let tree = PaneShell { model: &m, cb: &cb }.build();
        let hint = tree.base().hint.as_ref().expect("the pane declares a pick");
        hint();
        assert_eq!(*picked.borrow(), vec![heca_core::layout::PaneId(3)]);
    }

    /// The letter sits centred over the pane — the large-target placement, which is where it is
    /// drawn today and what the maintainer expects.
    #[test]
    fn the_letter_is_centred_over_the_pane() {
        let (cb, _) = recording_callbacks();
        let m = model(1);
        let tree = PaneShell { model: &m, cb: &cb }.build();
        assert_eq!(tree.base().hint_style.placement, HintPlacement::Center);
    }

    /// **A zoom resizes the pane without rebuilding it** — the regression that shipped.
    ///
    /// The retained tree is deliberately NOT rebuilt when the rect changes (a rebuild mid-drag
    /// throws away the widget's signals), so the size must be a per-frame input. When it was baked
    /// into `build` instead, the frame kept its first width forever: traced live as asked 648 wide,
    /// got 380, every frame, while the terminal content moved to the new rect.
    #[test]
    fn resizing_an_already_built_shell_moves_the_frame() {
        let (cb, _) = recording_callbacks();
        let m = model(1);
        let mut tree = PaneShell { model: &m, cb: &cb }.build();

        super::size_to(&mut tree, 320.0, 728.0);
        LayoutEngine::new().compute(&mut tree, Size::new(320.0, 728.0));
        assert!((tree.base().bounds.size.w - 320.0).abs() < 0.5);

        // …now zoom it. Same tree, no rebuild.
        super::size_to(&mut tree, 648.0, 728.0);
        LayoutEngine::new().compute(&mut tree, Size::new(648.0, 728.0));
        assert!(
            (tree.base().bounds.size.w - 648.0).abs() < 0.5,
            "the frame stayed at {} after the pane grew to 648",
            tree.base().bounds.size.w
        );
    }

    /// The box test: laid out at the rect it was given, the shell never exceeds it. This is the one
    /// assertion that catches a component computing geometry of its own.
    #[test]
    fn the_shell_never_exceeds_the_box_it_is_given() {
        let (cb, _) = recording_callbacks();
        for (w, h) in [(400.0, 300.0), (120.0, 80.0), (1600.0, 900.0)] {
            let mut m = model(1);
            m.w = w;
            m.h = h;
            let mut tree = PaneShell { model: &m, cb: &cb }.build();
            super::size_to(&mut tree, w, h);
            LayoutEngine::new().compute(&mut tree, Size::new(w as f64, h as f64));
            let b = tree.base().bounds;
            // EXACT, not "within": the pane's rect is given by the WM layout engine, so the frame
            // must land on it to the pixel. `<=` would pass while the border was drawn short —
            // which is exactly the regression this assertion was added for.
            assert!(
                (b.size.w - w as f64).abs() < 0.5 && (b.size.h - h as f64).abs() < 0.5,
                "shell laid out {}x{} for a {w}x{h} pane",
                b.size.w,
                b.size.h
            );
        }
    }
}
