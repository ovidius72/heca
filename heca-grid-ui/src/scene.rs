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

    /// **The base layer cut at every terminal surface**, so a host can draw each surface in scene
    /// order: flush a run, put the surface where the scene put it, flush the next run over it.
    ///
    /// Each run is **self-contained**: the clips open at a cut are closed at the end of the run and
    /// re-opened at the start of the next, so a run flushed on its own is clipped exactly as it was
    /// in place. The outline band stays last, so a frame drawn over its children lands over the
    /// surface too. A scene with no surface is one run, equal to [`base_layer`](Scene::base_layer).
    ///
    /// Only [`HostDraw::Surface`] cuts. A backdrop is not a cut: it is performed between the base
    /// flush and the overlay flush, as before.
    pub fn base_runs(&self) -> Vec<BaseRun> {
        let mut runs = Vec::new();
        let mut open: Vec<Rectangle> = Vec::new();
        let mut draws = Scene::new();
        for cmd in self.commands.iter().chain(self.outline.iter()) {
            match cmd {
                DrawCommand::Host(HostCmd {
                    draw: HostDraw::Surface { id },
                    rect,
                    alpha,
                }) => {
                    for _ in &open {
                        draws.commands.push(DrawCommand::PopClip);
                    }
                    runs.push(BaseRun {
                        draws: std::mem::take(&mut draws),
                        then: Some(SurfaceAt {
                            id: *id,
                            rect: *rect,
                            alpha: *alpha,
                            clip: open.iter().copied().reduce(|a, b| {
                                a.intersection(b).unwrap_or(Rectangle::new(
                                    a.loc,
                                    heca_core::layout::Size::new(0.0, 0.0),
                                ))
                            }),
                        }),
                    });
                    for rect in &open {
                        draws.commands.push(DrawCommand::PushClip(*rect));
                    }
                }
                DrawCommand::PushClip(r) => {
                    open.push(*r);
                    draws.commands.push(cmd.clone());
                }
                DrawCommand::PopClip => {
                    open.pop();
                    draws.commands.push(cmd.clone());
                }
                _ => draws.commands.push(cmd.clone()),
            }
        }
        runs.push(BaseRun { draws, then: None });
        runs
    }

    /// **What this scene draws outside `within`** — the framework's own answer to "does anything
    /// paint past the box it was given?", so a sweep asks it instead of re-implementing the walk.
    ///
    /// Two crates asked it, each with its own copy of the clip arithmetic, and both copies carried
    /// the same defect: a clip that lies **entirely outside** the clip already open has an empty
    /// intersection, and treating that as "no clip at all" reports every draw inside it as an
    /// escape. It is the opposite — nothing inside such a clip reaches the screen at all. That
    /// alone accounted for five phantom escapes in the catalog sweep, which is why it was
    /// committed `#[ignore]`d (F003/P082/T481).
    ///
    /// **Base layer only, by design.** The overlay band exists precisely for what must leave its
    /// box — a dropdown opened inside a scroll region, a hint keycap on a half-visible row — so
    /// asking this of it would forbid the feature.
    ///
    /// Rects and text runs only: those carry the widget's picture. A zero-sized draw is skipped —
    /// a box squeezed to nothing paints nothing, wherever its origin ended up.
    pub fn draws_outside(&self, within: Rectangle) -> Vec<Escape> {
        let mut out = Vec::new();
        // `None` = an empty clip: everything inside it is scissored away.
        let mut clips: Vec<Option<Rectangle>> = Vec::new();
        for cmd in self.commands.iter().chain(self.outline.iter()) {
            let (what, rect) = match cmd {
                DrawCommand::PushClip(r) => {
                    let inner = match clips.last() {
                        Some(Some(open)) => open.intersection(*r),
                        Some(None) => None,
                        None => Some(*r),
                    };
                    clips.push(inner);
                    continue;
                }
                DrawCommand::PopClip => {
                    clips.pop();
                    continue;
                }
                DrawCommand::Text(t) if t.text.is_empty() => continue,
                DrawCommand::Text(t) => (format!("text {:?}", t.text), t.rect),
                DrawCommand::Rect(r) => ("rect".to_string(), r.rect),
                _ => continue,
            };
            if rect.size.w <= 0.0 || rect.size.h <= 0.0 {
                continue;
            }
            let visible = match clips.last() {
                Some(Some(clip)) => clip.intersection(rect),
                Some(None) => None,
                None => Some(rect),
            };
            let Some(rect) = visible else { continue };
            if rect.loc.x < within.loc.x - 0.5
                || rect.loc.x + rect.size.w > within.loc.x + within.size.w + 0.5
            {
                out.push(Escape { what, rect });
            }
        }
        out
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

/// One stretch of a scene's base layer, and the terminal surface the scene puts after it — see
/// [`Scene::base_runs`].
#[derive(Debug, Clone)]
pub struct BaseRun {
    /// What to flush first: ordinary rects and text, clipped as they were in the scene.
    pub draws: Scene,
    /// The surface that goes over `draws`, or `None` for the last run.
    pub then: Option<SurfaceAt>,
}

/// Where the scene put a surface: the opaque id the widget gave it, its box, its opacity and what
/// clips it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceAt {
    pub id: u64,
    pub rect: Rectangle,
    pub alpha: f32,
    /// Every clip open where the surface was recorded, intersected — `None` when nothing clips it.
    /// An empty intersection is a zero-sized rect: nothing of it shows.
    pub clip: Option<Rectangle>,
}

/// One draw that fell outside the box it was measured against — see
/// [`Scene::draws_outside`](Scene::draws_outside). Carries **what** was drawn, not just where, so a
/// failing sweep names the content that escaped instead of an anonymous rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct Escape {
    /// The draw, described the way a reader can find it: `text "item 04"`, or `rect`.
    pub what: String,
    /// Where it landed, already intersected with the clips in force.
    pub rect: Rectangle,
}

impl std::fmt::Display for Escape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} spans {:.0}..{:.0}",
            self.what,
            self.rect.loc.x,
            self.rect.loc.x + self.rect.size.w,
        )
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
    /// **Work only the host can do, recorded in scene order.** See [`HostDraw`].
    Host(HostCmd),
}

/// **A request the drawing pass records and the host performs.**
///
/// Some content cannot be expressed as rectangles and text, and must not be forced into them:
///
/// - a **terminal** is rasterised into a texture, because its cell glyphs are the hottest path in
///   the app — drawing them as ordinary commands is rejected;
/// - a **frosted backdrop** is the frame so far, blurred, which is a pass over what is already
///   drawn rather than a shape.
///
/// Neither is a special case in this crate. A widget says *what it wants* and where; the host owns
/// the GPU and does it. That keeps this library free of graphics types — the same bargain
/// [`Text`](DrawCommand::Text) already makes, where the scene names a role and the renderer owns the
/// atlas.
///
/// **Recorded in scene order, with the clip stack resolved**, so a surface inside a scroll region
/// clips like anything else and a backdrop blurs exactly what was drawn before it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HostDraw {
    /// **Put the surface `id` here.** The host holds the texture and knows nothing else is needed;
    /// this crate never sees a texture, a format or a device.
    ///
    /// Who allocates the id is the host's business. A plugin rendering its own content places it
    /// with one builder on its own widget — no host-private type, no registry.
    Surface {
        /// Opaque to this crate: the host maps it to whatever it rasterised.
        id: u64,
    },
    /// **Blur whatever has been drawn behind me, here.** `radius` is in logical pixels.
    Backdrop { radius: f32 },
}

/// A [`HostDraw`] and the box it applies to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostCmd {
    pub draw: HostDraw,
    /// Where it goes, in logical pixels, already placed by the paint context.
    pub rect: Rectangle,
    /// How strongly it composites, `0.0..=1.0`. Carries the paint context's
    /// [opacity](crate::PaintCx::with_opacity) like every other command, so a surface inside a
    /// fading overlay fades with it — and a frost fades in with the surface that asked for it,
    /// rather than holding the session out of focus and snapping sharp in one frame.
    pub alpha: f32,
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
mod tests;
