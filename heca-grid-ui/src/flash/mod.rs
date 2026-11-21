//! A shared flash request, painted by the framework rather than by individual widgets.

use std::time::{Duration, Instant};

use crate::{
    Base, PaintCx, Tone,
    effects::Flash,
    reactive::{Signal, SignalGet, SignalUpdate, signal},
    scene::{Border, Glow},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Playback {
    Burst,
    Once,
    Repeat { duration: Option<Duration> },
}

#[derive(Clone, Copy, Debug)]
struct Request {
    started: Instant,
    tone: Tone,
    playback: Playback,
}

/// **The next time a flash needs a frame, and how long that stays worth waking for.**
///
/// Only a paint writes one, because only a paint knows the flash is on screen and what the theme
/// says its timing is. `until` is the end of a whole cycle past `at`: a flash nobody paints any more
/// (its row scrolled away, its pane in another workspace, the sidebar hidden) is not on screen, and a
/// wake that is only ever overdue would wake the host again the moment it went back to sleep.
#[derive(Clone, Copy, Debug)]
struct Booked {
    at: Instant,
    until: Instant,
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
    next_frame: Signal<Option<Booked>>,
}

impl FlashHandle {
    /// An idle handle. Binding it never flashes the target by itself.
    pub fn new() -> Self {
        Self {
            request: signal(None),
            next_frame: signal(None),
        }
    }

    /// Start a finite burst using the theme's colour, timing and pulse count.
    /// A later trigger restarts it; a shared handle flashes all its bound widgets.
    pub fn trigger(self, tone: Tone) {
        self.start(tone, Playback::Burst);
    }

    /// One pulse using the theme's colour and duration, independent of its burst count.
    pub fn pulse(self, tone: Tone) {
        self.start(tone, Playback::Once);
    }

    /// Pulse until [`clear`](Self::clear), for example when a background operation
    /// needs attention. Acknowledgement belongs to the operation's existing event handler.
    pub fn repeat(self, tone: Tone) {
        self.start(tone, Playback::Repeat { duration: None });
    }

    /// Repeat soft pulses for at most `duration`, or until explicitly cleared.
    pub fn repeat_for(self, tone: Tone, duration: Duration) {
        self.start(
            tone,
            Playback::Repeat {
                duration: Some(duration),
            },
        );
    }

    /// Change a repeating request's timeout without restarting its pulses or elapsed time.
    pub fn set_repeat_duration(self, duration: Option<Duration>) {
        let Some(mut request) = self.request.get_untracked() else {
            return;
        };
        let next = Playback::Repeat { duration };
        if matches!(request.playback, Playback::Repeat { .. }) && request.playback != next {
            request.playback = next;
            self.request.set(Some(request));
            crate::request_frame();
        }
    }

    fn start(self, tone: Tone, playback: Playback) {
        // **No wake is booked here.** The frame asked for below draws the flash, and the paint of a
        // widget that is on screen books the next one. Booking "now" from here as well left a wake
        // that only a paint could ever clear, so a flash on a widget nothing paints kept the host
        // awake for ever.
        self.request.set(Some(Request {
            started: crate::clock::now(),
            tone,
            playback,
        }));
        crate::request_frame();
    }

    /// Stop the flash on every widget bound to this handle.
    pub fn clear(self) {
        self.request.set(None);
        self.next_frame.set(None);
        crate::request_frame();
    }
}

/// **Does this widget want attention** — a repeating flash that has not run out, the pulse a row
/// keeps until someone looks. What [`ScrollRegion`](crate::widgets::ScrollRegion) counts for the
/// widgets pinned in it.
pub(crate) fn wants_attention(base: &Base) -> bool {
    let Some(request) = base.flash.and_then(|flash| flash.request.get_untracked()) else {
        return false;
    };
    match request.playback {
        Playback::Repeat { duration: None } => true,
        Playback::Repeat {
            duration: Some(limit),
        } => request.started.elapsed() < limit,
        _ => false,
    }
}

/// Use the existing widget deadline walk, including when a custom widget owns `tick`.
pub(crate) fn wake(base: &Base) -> Option<f32> {
    if base.style.layout.hidden || !base.visible.get_untracked() {
        return None;
    }
    let booked = base.flash?.next_frame.get_untracked()?;
    let now = crate::clock::now();
    // Overdue is still a wake — the host draws at once — but only for one cycle: past that, no paint
    // has reached this widget since it asked, so it is not on screen and has nothing to draw.
    (now <= booked.until).then(|| booked.at.saturating_duration_since(now).as_secs_f32())
}

impl Default for FlashHandle {
    fn default() -> Self {
        Self::new()
    }
}

fn amount(request: Request, style: heca_theme::FlashStyle, now: Instant) -> Option<f32> {
    if !style.enabled
        || !style.duration_s.is_finite()
        || style.duration_s <= 0.0
        || !style.pause_s.is_finite()
        || style.pause_s < 0.0
        || !style.border_width_em.is_finite()
        || style.border_width_em <= 0.0
        || style.border_alpha == 0
    {
        return None;
    }
    let elapsed = now.saturating_duration_since(request.started).as_secs_f64();
    let cycle = f64::from(style.duration_s) + f64::from(style.pause_s);
    if elapsed >= lifetime(request, style) {
        return None;
    }
    let phase = elapsed % cycle;
    if matches!(request.playback, Playback::Repeat { .. }) {
        return Some(if phase >= f64::from(style.duration_s) {
            0.0
        } else {
            ((1.0 - (std::f64::consts::TAU * phase / f64::from(style.duration_s)).cos()) * 0.5)
                as f32
        });
    }
    let mut effect = Flash::with_duration(style.duration_s);
    effect.trigger();
    effect.tick(phase as f32);
    Some(effect.amount())
}

fn lifetime(request: Request, style: heca_theme::FlashStyle) -> f64 {
    match request.playback {
        Playback::Once => f64::from(style.duration_s),
        Playback::Burst => {
            (f64::from(style.duration_s) + f64::from(style.pause_s))
                * f64::from(style.pulses.max(1))
        }
        Playback::Repeat { duration } => {
            duration.map_or(f64::INFINITY, |duration| duration.as_secs_f64())
        }
    }
}

/// Paint the target's configured shape, retaining its parent's clipping for the outer stroke.
/// Real elapsed time also supports custom widgets that override `tick` without forwarding it.
pub(crate) fn paint(base: &Base, cx: &mut PaintCx) {
    let Some(request) = base.flash.and_then(|handle| handle.request.get()) else {
        return;
    };
    let style = cx.theme().colors.flash;
    let now = crate::clock::now();
    let Some(strength) = amount(request, style, now) else {
        if let Some(handle) = base.flash {
            handle.next_frame.set(None);
        }
        return;
    };
    if strength > 0.0 {
        let color = request.tone.resolve(cx.theme());
        let border = color.with_alpha((f32::from(style.border_alpha) * strength).round() as u8);
        let fill = color.with_alpha((f32::from(style.fill_alpha) * strength).round() as u8);
        let glow = (style.glow_radius_em.is_finite()
            && style.glow_radius_em > 0.0
            && style.glow_strength.is_finite()
            && style.glow_strength > 0.0)
            .then_some(Glow {
                color: border,
                radius: base.font * style.glow_radius_em,
                intensity: style.glow_strength * strength,
            });
        // The renderer places strokes outside the shape. Keep the parent's clip,
        // just like the surface itself, rather than cutting the stroke to its own box.
        cx.rect(
            base.bounds,
            fill,
            Some(Border {
                color: border,
                width: base.font * style.border_width_em,
            }),
            base.style.visual.radius,
            glow,
        );
    }
    // Schedule through the same deadline path as caret/tooltip updates. Asking
    // for an immediate frame from paint would create a busy redraw loop.
    let cycle = f64::from(style.duration_s) + f64::from(style.pause_s);
    let phase = now.saturating_duration_since(request.started).as_secs_f64() % cycle;
    let delay = if phase < f64::from(style.duration_s) {
        (1.0 / f64::from(style.frames_per_second.max(1))).min(f64::from(style.duration_s) - phase)
    } else {
        cycle - phase
    };
    let remaining =
        lifetime(request, style) - now.saturating_duration_since(request.started).as_secs_f64();
    if let Some(handle) = base.flash {
        handle.next_frame.set(
            Duration::try_from_secs_f64(delay.min(remaining).max(0.0))
                .ok()
                .and_then(|delay| {
                    let at = now.checked_add(delay)?;
                    Some(Booked {
                        at,
                        until: at.checked_add(Duration::try_from_secs_f64(cycle).ok()?)?,
                    })
                }),
        );
    }
    base.dirty_next_paint();
}

#[cfg(test)]
mod tests;
