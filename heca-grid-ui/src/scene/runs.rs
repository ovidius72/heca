//! **Reading a finished scene**: the stretches the host flushes one at a time, the draws that
//! left a box, and the overlays in the order they stack.

use super::{DrawCommand, HostCmd, HostDraw, Scene};
use heca_core::layout::Rectangle;

impl Scene {
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
