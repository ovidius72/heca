//! A shared flash request, painted by the framework rather than by individual widgets.

use std::time::{Duration, Instant};

use crate::{Base, PaintCx, Tone, effects::Flash, reactive::{Signal, SignalGet, SignalUpdate, signal}, scene::{Border, Glow}};

#[derive(Clone, Copy, Debug)]
struct Request {
    started: Instant,
    tone: Tone,
    repeat: bool,
}

/// A cheap, copyable handle to semantic light pulses on one or more widgets.
/// Bind it with [`FlashExt::flash`](crate::FlashExt::flash), then trigger it from an event,
/// a reactive effect or an existing declared action. The framework owns timing and redraws.
///
/// ```no_run
/// use heca_grid_ui::prelude::*;
/// let flash = FlashHandle::new();
/// let row = Row::new().flash(flash).child(Label::new("Operation"));
/// flash.trigger(Tone::Success);
/// flash.repeat(Tone::Warning); // Until the operation is acknowledged.
/// flash.clear();
/// ```
#[derive(Clone, Copy, Debug)]
pub struct FlashHandle {
    request: Signal<Option<Request>>,
    next_frame: Signal<Option<Instant>>,
}

impl FlashHandle {
    /// An idle handle. Binding it never flashes the target by itself.
    pub fn new() -> Self { Self { request: signal(None), next_frame: signal(None) } }

    /// Start a finite burst using the theme's colour, timing and pulse count.
    /// A later trigger restarts it; a shared handle flashes all its bound widgets.
    pub fn trigger(self, tone: Tone) {
        self.start(tone, false);
    }

    /// Pulse until [`clear`](Self::clear), for example when a background operation
    /// needs attention. Acknowledgement belongs to the operation's existing event handler.
    pub fn repeat(self, tone: Tone) {
        self.start(tone, true);
    }

    fn start(self, tone: Tone, repeat: bool) {
        self.request.set(Some(Request { started: Instant::now(), tone, repeat }));
        self.next_frame.set(Some(Instant::now()));
        crate::request_frame();
    }

    /// Stop the flash on every widget bound to this handle.
    pub fn clear(self) {
        self.request.set(None);
        self.next_frame.set(None);
        crate::request_frame();
    }
}

/// Use the existing widget deadline walk, including when a custom widget owns `tick`.
pub(crate) fn wake(base: &Base) -> Option<f32> {
    if base.style.layout.hidden || !base.visible.get_untracked() { return None }
    base.flash?.next_frame.get_untracked().map(|at|
        at.saturating_duration_since(Instant::now()).as_secs_f32())
}

impl Default for FlashHandle {
    fn default() -> Self { Self::new() }
}

fn amount(request: Request, style: heca_theme::FlashStyle, now: Instant) -> Option<f32> {
    if !style.enabled || !style.duration_s.is_finite() || style.duration_s <= 0.0
        || !style.pause_s.is_finite() || style.pause_s < 0.0
        || !style.border_width_em.is_finite() || style.border_width_em <= 0.0 || style.border_alpha == 0
    { return None }
    let elapsed = now.saturating_duration_since(request.started).as_secs_f64();
    let cycle = f64::from(style.duration_s) + f64::from(style.pause_s);
    if !request.repeat && elapsed >= cycle * f64::from(style.pulses.max(1)) { return None }
    let phase = elapsed % cycle;
    let mut effect = Flash::with_duration(style.duration_s);
    effect.trigger();
    effect.tick(phase as f32);
    Some(effect.amount())
}

/// Paint inside the target's own bounds, so ordinary damage/clipping remains sufficient.
/// Real elapsed time also supports custom widgets that override `tick` without forwarding it.
pub(crate) fn paint(base: &Base, cx: &mut PaintCx) {
    let Some(request) = base.flash.and_then(|handle| handle.request.get()) else { return };
    let style = cx.theme().colors.flash;
    let now = Instant::now();
    let Some(strength) = amount(request, style, now) else {
        if let Some(handle) = base.flash { handle.next_frame.set(None); }
        return
    };
    if strength > 0.0 {
        let color = request.tone.resolve(cx.theme());
        let border = color.with_alpha((f32::from(style.border_alpha) * strength).round() as u8);
        let fill = color.with_alpha((f32::from(style.fill_alpha) * strength).round() as u8);
        let glow = (style.glow_radius_em.is_finite() && style.glow_radius_em > 0.0
            && style.glow_strength.is_finite() && style.glow_strength > 0.0).then_some(Glow {
                color: border, radius: base.font * style.glow_radius_em,
                intensity: style.glow_strength * strength,
            });
        cx.with_rounded_clip(base.bounds, base.style.visual.radius, |cx| {
            cx.rect(base.bounds, fill, Some(Border {
                color: border, width: base.font * style.border_width_em,
            }), base.style.visual.radius, glow);
        });
    }
    // Schedule through the same deadline path as caret/tooltip updates. Asking
    // for an immediate frame from paint would create a busy redraw loop.
    let cycle = f64::from(style.duration_s) + f64::from(style.pause_s);
    let phase = now.saturating_duration_since(request.started).as_secs_f64() % cycle;
    let delay = if phase < f64::from(style.duration_s) {
        (1.0 / f64::from(style.frames_per_second.max(1))).min(f64::from(style.duration_s) - phase)
    } else { cycle - phase };
    if let Some(handle) = base.flash {
        handle.next_frame.set(Duration::try_from_secs_f64(delay).ok().and_then(|delay| now.checked_add(delay)));
    }
    base.dirty_next_paint();
}

#[cfg(test)]
mod tests;
