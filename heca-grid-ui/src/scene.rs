//! The display list — `heca-grid-ui`'s GPU-free visual vocabulary.
//!
//! Components never call the GPU. Instead they emit [`DrawCommand`]s into a
//! [`Scene`], and `heca-renderer` rasterizes them (SDF glow, scanline, and text
//! pipelines). This keeps the component crate headless and unit-testable while
//! still driving real GPU effects: a component states the *intent*
//! (`Rect { glow: Some(..) }`), the renderer owns the *shader*.
//!
//! Coordinates are **f64 logical pixels** (reusing `heca-core` geometry); the
//! renderer scales to physical pixels by the display `scale_factor`, preserving
//! HiDPI crispness — consistent with the project's "f64 logical, f32 at the GPU
//! boundary" rule.

use crate::color::Color;
use heca_core::layout::Rectangle;

/// A retained list of draw commands, rebuilt each repaint.
///
/// Commands are split into two layers: the **base** layer and an **overlay**
/// layer drawn entirely on top of it (popovers/dropdowns). [`iter`](Scene::iter)
/// yields base commands first, then overlay — so the renderer naturally draws
/// overlays last. Widgets route draws to the overlay layer via
/// [`PaintCx::with_overlay`](crate::component::PaintCx::with_overlay).
#[derive(Debug, Default, Clone)]
pub struct Scene {
    commands: Vec<DrawCommand>,
    overlay: Vec<DrawCommand>,
    /// `(start, end, depth)` ranges into `overlay`, one per [`begin_overlay`](Scene::begin_overlay)/
    /// [`end_overlay`](Scene::end_overlay) pair that produced commands. Each is a
    /// distinct overlay (a dropdown, a toast stack, …) tagged with its **nesting
    /// depth** (1 = top-level `with_overlay`, 2 = an overlay painted inside another
    /// overlay's paint, …); [`overlay_segments`](Scene::overlay_segments) hands them
    /// back depth-ordered so a host can flush each as its own rects→text pass with
    /// nested overlays occluding their parents (and later overlays occluding
    /// earlier ones at the same depth — no overlapping-overlay text bleed).
    overlay_segs: Vec<(usize, usize, usize)>,
    /// When set, [`push`](Scene::push) targets the overlay layer.
    to_overlay: bool,
    /// Start index in `overlay` of the currently open segment.
    seg_start: usize,
    /// Nesting depth of open [`begin_overlay`](Scene::begin_overlay) pairs. An overlay widget
    /// (a `Tooltip`, a `Select` dropdown) painted **inside** another overlay's paint (a
    /// `Dialog`) nests `with_overlay`; without tracking depth the inner `end_overlay` would
    /// clear `to_overlay` mid-parent and drop the parent's segment (its scrim/panel would never
    /// be recorded → not drawn). Depth lets nesting close each segment in order and only leave
    /// overlay mode at depth 0.
    overlay_depth: usize,
    /// Clip rects currently open in the **overlay** layer (pushed by `PushClip`,
    /// popped by `PopClip`).
    ///
    /// Segments are rendered independently, each starting with an empty clip stack,
    /// so a clip that was opened *before* a nested overlay split the stream would be
    /// silently lost for the parent's remaining draws. That is exactly what happened
    /// with a `Select` opened inside a scrolled `Dialog` body: the rows painted after
    /// the dropdown escaped the `ScrollRegion`'s clip and drew outside the modal.
    /// Tracking the open clips lets [`end_overlay`](Scene::end_overlay) re-establish
    /// them on the continuation segment, keeping every segment self-contained.
    overlay_clips: Vec<Rectangle>,
    /// `overlay_clips` depth saved at each [`begin_overlay`](Scene::begin_overlay),
    /// so a nested overlay's own clips can't leak into the parent's continuation.
    clip_depth_stack: Vec<usize>,
}

impl Scene {
    /// An empty scene.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a command to the active layer (base, or overlay while in
    /// [`begin_overlay`](Scene::begin_overlay)).
    pub fn push(&mut self, cmd: DrawCommand) {
        if self.to_overlay {
            // Track clips open in this layer so a nested overlay splitting the
            // stream can't lose them (see `overlay_clips`).
            match &cmd {
                DrawCommand::PushClip(r) => self.overlay_clips.push(*r),
                DrawCommand::PopClip => {
                    self.overlay_clips.pop();
                }
                _ => {}
            }
            self.overlay.push(cmd);
        } else {
            self.commands.push(cmd);
        }
    }

    /// Route subsequent pushes to the overlay layer (drawn on top), starting a new
    /// overlay segment. Each segment becomes a self-contained occluding unit (see
    /// [`overlay_segments`](Scene::overlay_segments)). **Re-entrant**: called inside another
    /// open overlay (a `Tooltip` painted within a `Dialog`), it first closes the parent's
    /// open segment, then opens a nested one — so the deeper overlay draws on top as its own
    /// segment and the parent's earlier draws are preserved.
    pub fn begin_overlay(&mut self) {
        // Descending into a nested overlay: bank the parent's segment so far (it
        // carries the PARENT's depth — the current one, before the increment).
        if self.to_overlay && self.overlay.len() > self.seg_start {
            self.overlay_segs
                .push((self.seg_start, self.overlay.len(), self.overlay_depth));
        }
        self.to_overlay = true;
        self.seg_start = self.overlay.len();
        self.overlay_depth += 1;
        // Remember how many clips were open, so `end_overlay` restores exactly
        // those and not any the nested overlay opened. The nested segment itself
        // starts UNCLIPPED on purpose: a dropdown opened inside a scroll region
        // must be able to extend beyond it.
        self.clip_depth_stack.push(self.overlay_clips.len());
    }

    /// Stop routing to the overlay layer for this level, closing the current segment (recorded
    /// only if it produced any commands). While still nested inside a parent overlay, routing
    /// stays on the overlay layer and a fresh segment opens for the parent's remaining draws;
    /// overlay mode ends only when the outermost pair closes.
    pub fn end_overlay(&mut self) {
        if self.overlay.len() > self.seg_start {
            self.overlay_segs
                .push((self.seg_start, self.overlay.len(), self.overlay_depth));
        }
        self.overlay_depth = self.overlay_depth.saturating_sub(1);
        // A new segment begins here for whatever the parent overlay draws next.
        self.seg_start = self.overlay.len();
        if self.overlay_depth == 0 {
            self.to_overlay = false;
        }
        // Drop any clip the nested overlay left open, then RE-ESTABLISH the ones
        // that were open around it. Segments render independently with a fresh
        // clip stack, so without this the parent's remaining draws would be
        // unclipped — the rows of a scrolled dialog body painting outside the
        // modal after a Select was opened inside it.
        let restore_to = self.clip_depth_stack.pop().unwrap_or(0);
        self.overlay_clips.truncate(restore_to);
        if self.to_overlay && !self.overlay_clips.is_empty() {
            // Append directly: these are a *replay* of already-tracked clips, so
            // they must not be re-tracked by `push`.
            for rect in self.overlay_clips.clone() {
                self.overlay.push(DrawCommand::PushClip(rect));
            }
        }
    }

    /// Clear all commands (reuse the allocation across frames).
    pub fn clear(&mut self) {
        self.commands.clear();
        self.overlay.clear();
        self.overlay_segs.clear();
        self.overlay_clips.clear();
        self.clip_depth_stack.clear();
        self.to_overlay = false;
        self.seg_start = 0;
        self.overlay_depth = 0;
    }

    /// Total number of queued commands (base + overlay).
    pub fn len(&self) -> usize {
        self.commands.len() + self.overlay.len()
    }

    /// Whether the scene has no commands.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty() && self.overlay.is_empty()
    }

    /// Iterate commands in draw order: base layer first, then overlay (on top).
    pub fn iter(&self) -> impl Iterator<Item = &DrawCommand> {
        self.commands.iter().chain(self.overlay.iter())
    }

    /// Whether the overlay layer has any commands.
    pub fn has_overlay(&self) -> bool {
        !self.overlay.is_empty()
    }

    /// A scene containing only the **base** layer's commands. Hosts that draw in
    /// two passes (rects then text) render this first, then [`overlay_layer`](Scene::overlay_layer)
    /// on top — so overlay content occludes base text, not just base rects.
    pub fn base_layer(&self) -> Scene {
        Scene {
            commands: self.commands.clone(),
            ..Default::default()
        }
    }

    /// A scene containing only the **overlay** layer's commands (drawn on top), all
    /// overlays flattened into one pass. Prefer [`overlay_segments`](Scene::overlay_segments)
    /// when overlays can overlap — a single flattened pass draws all overlay rects
    /// then all overlay text, so a lower overlay's text bleeds over a higher one's panel.
    pub fn overlay_layer(&self) -> Scene {
        Scene {
            commands: self.overlay.clone(),
            ..Default::default()
        }
    }

    /// One sub-scene per overlay, in paint (z) order, each holding that overlay's
    /// commands in the base slot so a host renders it as a single rects→text pass.
    /// Segments are yielded **depth-first ascending** (stable within a depth):
    /// top-level overlays in record order, then nested ones — so an overlay opened
    /// *inside* another overlay's paint (a `Select` dropdown inside a `Dialog`)
    /// composites **above** everything its parent draws after it (the parent's
    /// later action buttons), not just above what came before. Within one depth,
    /// flushing in record order makes each overlay occlude the ones below it — a
    /// later overlay's panel paints over an earlier overlay's text — which fixes
    /// the overlapping-overlay text bleed a single flattened
    /// [`overlay_layer`](Scene::overlay_layer) pass produces.
    pub fn overlay_segments(&self) -> impl Iterator<Item = Scene> + '_ {
        let mut order: Vec<&(usize, usize, usize)> = self.overlay_segs.iter().collect();
        order.sort_by_key(|&&(_, _, depth)| depth); // stable: record order within a depth
        order.into_iter().map(|&(start, end, _)| Scene {
            commands: self.overlay[start..end].to_vec(),
            ..Default::default()
        })
    }
}

/// One drawable element. Adding a new effect is one variant here plus one branch
/// in the renderer — component code is untouched.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    /// Solid or rounded rectangle, with optional border and additive outer glow.
    Rect(RectCmd),
    /// L-shaped corner brackets framing a rectangle (Tron reticle).
    Brackets(BracketCmd),
    /// A positioned text run.
    Text(TextCmd),
    /// A scanline overlay confined to a region.
    Scanline(ScanlineCmd),
    /// Push a clip rectangle; subsequent commands are clipped to it.
    PushClip(Rectangle),
    /// Pop the most recent clip rectangle.
    PopClip,
}

/// A filled/rounded rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectCmd {
    pub rect: Rectangle,
    pub fill: Color,
    pub border: Option<Border>,
    /// Corner radius in logical pixels (0.0 = sharp).
    pub radius: f32,
    pub glow: Option<Glow>,
    /// A soft drop shadow cast *behind* this rect (darkens the background).
    /// Independent of [`glow`](RectCmd::glow) (which only adds light).
    pub shadow: Option<Shadow>,
}

/// A soft drop shadow: a dark, blurred, offset halo drawn **behind** a shape to
/// lift it off the background. Unlike [`Glow`] (additive light), it composites a
/// dark color *with alpha* so it reads on dark themes where a glow can't. It is
/// independent of the glow + border tokens, so it shows even when both are off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// Shadow color, including its alpha (the umbra strength).
    pub color: Color,
    /// Blur / falloff radius in logical pixels.
    pub radius: f32,
    /// Horizontal offset of the cast shadow (logical px; positive = right).
    pub dx: f32,
    /// Vertical offset of the cast shadow (logical px; positive = down).
    pub dy: f32,
}

/// A rectangle outline.
///
/// Serializable so a description can override it (F003/P017/T7). Its `color` is a
/// [`Color`], whose serde form is a hex string, so a border crosses as
/// `{"color": "#rrggbbaa", "width": 1.0}`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Border {
    pub color: Color,
    pub width: f32,
}

/// An additive neon glow halo around a shape.
///
/// Serializable for the same reason as [`Border`] — see F003/P017/T7.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Glow {
    pub color: Color,
    /// Falloff radius in logical pixels.
    pub radius: f32,
    /// Multiplier on glow strength (driven by theme intensity).
    pub intensity: f32,
}

/// Corner brackets framing a rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BracketCmd {
    pub rect: Rectangle,
    pub color: Color,
    /// Length of each bracket arm in logical pixels.
    pub len: f32,
    pub thickness: f32,
    pub glow: Option<Glow>,
}

/// Horizontal text alignment within the target rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Which embedded font family a text run is shaped with — **three faces, three jobs**. A widget
/// names the role; the host maps it to a family, so nothing here knows a font's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontRole {
    /// The theme's monospace text family (default).
    #[default]
    Text,
    /// The embedded icon font (Phosphor); the codepoint is a glyph.
    Icon,
    /// The embedded **Nerd Font** ([`NfIcon`](crate::widgets::NfIcon)); the codepoint is one of its
    /// glyphs.
    ///
    /// A third role rather than a variant of `Icon` because it is a different font with a different
    /// job: Phosphor is the app's pictogram set, the Nerd Font supplies the glyphs Phosphor has none
    /// of — keyboard keys above all (`⇧`, `⌘`, `⎋`), which is why a keycap uses it. The alternative
    /// was drawing those as plain text in the UI face, and the UI face has no `⌃ ⌥ ⌘ ⎋`: they render
    /// as tofu. Verified against the embedded font's cmap, not assumed.
    NerdFont,
}

/// The **font style** of a text run: what the shaper does to the glyphs.
///
/// Decorations (underline, strikethrough) are deliberately **not** here — a line is not a glyph
/// attribute, it is a rect. The widget draws them itself, from the theme, like any other chrome (see
/// [`Label`](crate::widgets::Label)). Keeping the two apart is what lets the renderer stay a pure
/// text shaper.
///
/// It is a value rather than a pile of `bool` parameters so that the *next* attribute doesn't break
/// [`PaintCx::text`](crate::component::PaintCx::text)'s signature a second time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextStyle {
    /// The bold weight (a real face — the embedded family ships one).
    pub bold: bool,
    /// Slanted. Rendered as a **synthesized oblique** (a glyph shear), because the embedded family
    /// has no italic face — see the bridge in `heca-renderer`.
    pub italic: bool,
}

impl TextStyle {
    /// Upright, regular weight — the default.
    pub const REGULAR: Self = Self {
        bold: false,
        italic: false,
    };
    /// Bold, upright.
    pub const BOLD: Self = Self {
        bold: true,
        italic: false,
    };
    /// Regular weight, slanted.
    pub const ITALIC: Self = Self {
        bold: false,
        italic: true,
    };

    /// Set the weight (chainable, so a state-derived flag reads straight through:
    /// `TextStyle::REGULAR.bold(is_active)`).
    pub const fn bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    /// Set the slant.
    pub const fn italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }
}

/// A run of text positioned within a rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct TextCmd {
    pub rect: Rectangle,
    pub text: String,
    pub color: Color,
    pub size: f32,
    pub align: TextAlign,
    /// Weight + slant (decorations are drawn by the widget, not shaped — see [`TextStyle`]).
    pub style: TextStyle,
    /// Which font family shapes this run (text vs. icon glyph font).
    pub font: FontRole,
    /// Additive halo behind the glyphs, or `None` for flat text (the default, and
    /// what every terminal run uses).
    ///
    /// This is **declarative, exactly like [`RectCmd::glow`]**: the scene says *this
    /// run glows, this colour, this falloff* and the renderer decides how to realize
    /// it. `heca-renderer` currently does so by blurring the glyph's coverage mask
    /// into its own atlas entry and drawing that behind the sharp glyph — but that is
    /// a renderer detail, and replacing it changes no scene code and no widget.
    pub glow: Option<Glow>,
}

/// A scanline overlay confined to a region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScanlineCmd {
    pub rect: Rectangle,
    pub color: Color,
    /// Vertical spacing between lines in logical pixels.
    pub spacing: f32,
    /// Per-line opacity multiplier (0.0..=1.0).
    pub opacity: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(segs.len(), 3, "parent segment must survive the nested child");
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
        assert_eq!(segs[0].iter().cloned().collect::<Vec<_>>(), vec![marker(1.0)]);
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
}
