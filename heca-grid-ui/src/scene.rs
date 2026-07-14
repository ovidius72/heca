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
    /// `(start, end)` ranges into `overlay`, one per [`begin_overlay`](Scene::begin_overlay)/
    /// [`end_overlay`](Scene::end_overlay) pair that produced commands. Each is a
    /// distinct overlay (a dropdown, a toast stack, …); [`overlay_segments`](Scene::overlay_segments)
    /// hands them back so a host can flush each as its own rects→text pass and have
    /// later overlays occlude earlier ones (no overlapping-overlay text bleed).
    overlay_segs: Vec<(usize, usize)>,
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
        // Descending into a nested overlay: bank the parent's segment so far.
        if self.to_overlay && self.overlay.len() > self.seg_start {
            self.overlay_segs.push((self.seg_start, self.overlay.len()));
        }
        self.to_overlay = true;
        self.seg_start = self.overlay.len();
        self.overlay_depth += 1;
    }

    /// Stop routing to the overlay layer for this level, closing the current segment (recorded
    /// only if it produced any commands). While still nested inside a parent overlay, routing
    /// stays on the overlay layer and a fresh segment opens for the parent's remaining draws;
    /// overlay mode ends only when the outermost pair closes.
    pub fn end_overlay(&mut self) {
        if self.overlay.len() > self.seg_start {
            self.overlay_segs.push((self.seg_start, self.overlay.len()));
        }
        self.overlay_depth = self.overlay_depth.saturating_sub(1);
        // A new segment begins here for whatever the parent overlay draws next.
        self.seg_start = self.overlay.len();
        if self.overlay_depth == 0 {
            self.to_overlay = false;
        }
    }

    /// Clear all commands (reuse the allocation across frames).
    pub fn clear(&mut self) {
        self.commands.clear();
        self.overlay.clear();
        self.overlay_segs.clear();
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
    /// Flushing them in order makes each overlay occlude the ones below it — a later
    /// overlay's panel paints over an earlier overlay's text — which fixes the
    /// overlapping-overlay text bleed a single flattened [`overlay_layer`](Scene::overlay_layer)
    /// pass produces.
    pub fn overlay_segments(&self) -> impl Iterator<Item = Scene> + '_ {
        self.overlay_segs.iter().map(|&(start, end)| Scene {
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    pub color: Color,
    pub width: f32,
}

/// An additive neon glow halo around a shape.
#[derive(Debug, Clone, Copy, PartialEq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Which embedded font family a text run is shaped with: the default monospace
/// text face, or the icon glyph font ([`Icon`](crate::widgets::Icon)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontRole {
    /// The theme's monospace text family (default).
    #[default]
    Text,
    /// The embedded icon font (Phosphor); the codepoint is a glyph.
    Icon,
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

    #[test]
    fn overlay_segments_yields_one_per_nonempty_begin_end_pair() {
        let mut s = Scene::new();
        s.begin_overlay(); // overlay A: two commands
        s.push(clip(1.0));
        s.push(clip(1.5));
        s.end_overlay();
        s.begin_overlay(); // overlay B: one command
        s.push(clip(2.0));
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
            vec![clip(1.0), clip(1.5)],
            "first segment should hold overlay A's commands"
        );
        assert_eq!(
            segs[1].iter().cloned().collect::<Vec<_>>(),
            vec![clip(2.0)],
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
        s.push(clip(0.0)); // base
        s.begin_overlay();
        s.push(clip(1.0)); // overlay
        s.end_overlay();
        assert_eq!(
            s.base_layer().iter().cloned().collect::<Vec<_>>(),
            vec![clip(0.0)],
            "base layer should hold only base commands, not overlay ones"
        );
    }

    #[test]
    fn nested_overlay_keeps_parent_segment() {
        // A `Dialog` paints its panel inside `with_overlay`; a child `Tooltip` paints inside its
        // own `with_overlay` — nested. The bug: the inner `end_overlay` dropped the parent's
        // segment, so the modal's scrim/panel vanished when the tooltip showed. Nesting must
        // yield three ordered segments: parent-before, the nested child, parent-after.
        let mut s = Scene::new();
        s.begin_overlay(); // outer (Dialog panel)
        s.push(clip(1.0)); // panel, before the nested overlay
        s.begin_overlay(); // inner (Tooltip)
        s.push(clip(2.0)); // tooltip
        s.end_overlay(); // close inner — must NOT drop the outer
        s.push(clip(3.0)); // outer continues (a sibling drawn after the tooltip)
        s.end_overlay(); // close outer

        let segs: Vec<Scene> = s.overlay_segments().collect();
        assert_eq!(segs.len(), 3, "parent segment must survive the nested child");
        assert_eq!(
            segs[0].iter().cloned().collect::<Vec<_>>(),
            vec![clip(1.0)],
            "segment 0 = parent content before the nest"
        );
        assert_eq!(
            segs[1].iter().cloned().collect::<Vec<_>>(),
            vec![clip(2.0)],
            "segment 1 = the nested (tooltip) overlay, on top"
        );
        assert_eq!(
            segs[2].iter().cloned().collect::<Vec<_>>(),
            vec![clip(3.0)],
            "segment 2 = parent content after the nest"
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
        s.push(clip(9.0)); // still overlay-targeted
        s.end_overlay(); // outer closed → back to base
        s.push(clip(8.0)); // base
        assert_eq!(
            s.base_layer().iter().cloned().collect::<Vec<_>>(),
            vec![clip(8.0)],
            "the mid-nest push must not leak to base; only post-outer-close pushes are base"
        );
    }
}
