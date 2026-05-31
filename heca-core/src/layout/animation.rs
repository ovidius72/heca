use std::time::{Duration, Instant};

/// Easing function for animations.
pub type EasingFn = fn(f64) -> f64;

/// Animation configuration (duration + easing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationConfig {
    pub duration_ms: u64,
    pub easing: EasingFn,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            duration_ms: 250,
            easing: ease_out_cubic,
        }
    }
}

/// An animated value that interpolates from a start to an end over time.
#[derive(Debug, Clone)]
pub struct Animation {
    start: f64,
    end: f64,
    /// Initial velocity for spring-like animations.
    velocity: f64,
    start_time: Instant,
    config: AnimationConfig,
}

impl Animation {
    pub fn new(
        start: f64,
        end: f64,
        velocity: f64,
        config: AnimationConfig,
    ) -> Self {
        Self {
            start,
            end,
            velocity,
            start_time: Instant::now(),
            config,
        }
    }

    /// Current interpolated value.
    pub fn value(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_millis() as f64;
        let duration = self.config.duration_ms as f64;

        if elapsed >= duration {
            return self.end;
        }

        let t = (elapsed / duration).clamp(0.0, 1.0);
        let eased = (self.config.easing)(t);
        self.start + (self.end - self.start) * eased
    }

    /// Target value.
    pub fn target(&self) -> f64 {
        self.end
    }

    /// Whether the animation is complete.
    pub fn is_done(&self) -> bool {
        self.start_time.elapsed().as_millis() as u64 >= self.config.duration_ms
    }

    /// Shift both start and end by a delta.
    pub fn offset(&mut self, delta: f64) {
        self.start += delta;
        self.end += delta;
    }
}

/// An animated value of any type that can be interpolated.
/// For simplicity, we start with f64 and Point variants.
#[derive(Debug, Clone)]
pub enum Animated<T> {
    Static(T),
    Animating { animation: Animation, from: T },
}

impl Animated<f64> {
    pub fn current(&self) -> f64 {
        match self {
            Self::Static(v) => *v,
            Self::Animating { animation, .. } => animation.value(),
        }
    }

    pub fn target(&self) -> f64 {
        match self {
            Self::Static(v) => *v,
            Self::Animating { animation, .. } => animation.target(),
        }
    }

    pub fn is_done(&self) -> bool {
        match self {
            Self::Static(_) => true,
            Self::Animating { animation, .. } => animation.is_done(),
        }
    }

    pub fn to_static(&mut self) {
        if let Self::Animating { animation, .. } = self {
            *self = Self::Static(animation.value());
        }
    }
}

impl Animated<super::types::Point> {
    pub fn current(&self) -> super::types::Point {
        use super::types::Point;
        match self {
            Self::Static(v) => *v,
            Self::Animating { animation, from } => {
                let t = if animation.is_done() {
                    1.0
                } else {
                    let elapsed = animation.start_time.elapsed().as_millis() as f64;
                    let duration = animation.config.duration_ms as f64;
                    ((elapsed / duration).clamp(0.0, 1.0))
                };
                let eased = (animation.config.easing)(t);
                Point::new(
                    from.x + (animation.target() - animation.start) * eased,
                    from.y + (animation.target() - animation.start) * eased,
                )
            }
        }
    }
}

// Easing functions

pub fn linear(t: f64) -> f64 {
    t
}

pub fn ease_out_quad(t: f64) -> f64 {
    1.0 - (1.0 - t) * (1.0 - t)
}

pub fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

pub fn ease_in_out_cubic(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Swipe tracker for gesture-driven scrolling.
#[derive(Debug, Clone)]
pub struct SwipeTracker {
    samples: Vec<(f64, Instant)>,
    pos: f64,
}

impl SwipeTracker {
    pub fn new() -> Self {
        Self {
            samples: Vec::with_capacity(16),
            pos: 0.0,
        }
    }

    pub fn push(&mut self, delta: f64, _timestamp: Instant) {
        self.pos += delta;
        self.samples.push((delta, Instant::now()));
        // Keep only last 8 samples for velocity calculation.
        if self.samples.len() > 8 {
            self.samples.remove(0);
        }
    }

    pub fn pos(&self) -> f64 {
        self.pos
    }

    pub fn velocity(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }
        let first = self.samples.first().unwrap();
        let last = self.samples.last().unwrap();
        let dt = last.1.duration_since(first.1).as_secs_f64();
        if dt < 0.001 {
            return 0.0;
        }
        let dx: f64 = self.samples.iter().map(|(d, _)| d).sum();
        dx / dt
    }

    /// Project where the swipe would end after deceleration.
    pub fn projected_end_pos(&self) -> f64 {
        let velocity = self.velocity();
        self.pos + velocity * 0.3 // rough deceleration projection
    }
}
