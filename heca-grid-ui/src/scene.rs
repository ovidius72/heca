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
    /// What widgets asked to have painted **over their children** (a pane's border and glow): drawn
    /// after every base command and before the overlay band. See
    /// [`begin_outline`](Scene::begin_outline).
    outline: Vec<DrawCommand>,
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
    /// When set, [`push`](Scene::push) targets the outline layer (unless the overlay layer is open).
    to_outline: bool,
    /// How many [`begin_outline`](Scene::begin_outline) calls are open **inside** an outline or an
    /// overlay, where there is nothing to lift the draws over: they paint in place.
    outline_nested: usize,
    /// Clip rects currently open in the **base** layer, so an outline can open the same ones.
    base_clips: Vec<Rectangle>,
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
        if !self.to_overlay && self.to_outline {
            self.outline.push(cmd);
            return;
        }
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
            // Tracked so an outline begun inside these clips stays inside them.
            match &cmd {
                DrawCommand::PushClip(r) => self.base_clips.push(*r),
                DrawCommand::PopClip => {
                    self.base_clips.pop();
                }
                _ => {}
            }
            self.commands.push(cmd);
        }
    }

    /// **Paint what follows over everything drawn so far in this layer** — over the widget's own
    /// children, which are painted after it. The border and glow of a pane belong here: a pane that
    /// frames a terminal must not be covered by it.
    ///
    /// The outline is its own list, drawn after every base command and before the overlay band, and
    /// each begin opens the clips that were open around it, so what is drawn there is clipped
    /// exactly as it would have been in place — and the clips are closed again at
    /// [`end_outline`](Scene::end_outline), so a piece never leaks into the next.
    ///
    /// **Inside an overlay, or another outline, it paints in place**: the overlay band is already
    /// above, and the outer outline is already last.
    pub fn begin_outline(&mut self) {
        if self.to_overlay || self.to_outline {
            self.outline_nested += 1;
            return;
        }
        self.to_outline = true;
        // A replay of already-tracked clips: appended directly so they are not tracked twice.
        for rect in self.base_clips.clone() {
            self.outline.push(DrawCommand::PushClip(rect));
        }
    }

    /// Close the piece [`begin_outline`](Scene::begin_outline) opened.
    pub fn end_outline(&mut self) {
        if self.outline_nested > 0 {
            self.outline_nested -= 1;
            return;
        }
        if !self.to_outline {
            return;
        }
        self.to_outline = false;
        for _ in 0..self.base_clips.len() {
            self.outline.push(DrawCommand::PopClip);
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
        // must be able to extend beyond it — and so must a hint keycap, which is
        // drawn whole for a row only half in view (F003/P082/T438). The cost of
        // that freedom is that geometry alone cannot stop a cap for a row nobody
        // can see: the picker's candidacy walk does, before a letter is handed out.
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
        self.outline.clear();
        self.overlay.clear();
        self.overlay_segs.clear();
        self.overlay_clips.clear();
        self.clip_depth_stack.clear();
        self.to_overlay = false;
        self.to_outline = false;
        self.outline_nested = 0;
        self.base_clips.clear();
        self.seg_start = 0;
        self.overlay_depth = 0;
    }

    /// Total number of queued commands (base + overlay).
    pub fn len(&self) -> usize {
        self.commands.len() + self.outline.len() + self.overlay.len()
    }

    /// Whether the scene has no commands.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty() && self.outline.is_empty() && self.overlay.is_empty()
    }

    /// Iterate commands in draw order: base layer, then the outline, then overlay (on top).
    pub fn iter(&self) -> impl Iterator<Item = &DrawCommand> {
        self.commands
            .iter()
            .chain(self.outline.iter())
            .chain(self.overlay.iter())
    }

    /// Whether the overlay layer has any commands.
    pub fn has_overlay(&self) -> bool {
        !self.overlay.is_empty()
    }

    /// A scene containing only the **base** layer's commands, then the outline drawn over them.
    /// Hosts that draw in two passes (rects then text) render this first, then
    /// [`overlay_layer`](Scene::overlay_layer) on top — so overlay content occludes base text, not
    /// just base rects.
    pub fn base_layer(&self) -> Scene {
        Scene {
            commands: self
                .commands
                .iter()
                .chain(self.outline.iter())
                .cloned()
                .collect(),
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
}

mod commands;
mod runs;

pub use commands::{
    Border, BracketCmd, DrawCommand, FontRole, Glow, HostCmd, HostDraw, RectCmd, ScanlineCmd,
    Shadow, TextAlign, TextCmd, TextStyle,
};
pub use runs::{BaseRun, Escape, SurfaceAt};

#[cfg(test)]
mod tests;
