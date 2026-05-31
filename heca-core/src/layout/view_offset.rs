use super::animation::{Animation, AnimationConfig, SwipeTracker};
use std::time::Instant;

/// The horizontal scroll offset of the view.
///
/// This is the core mechanism that enables NIRI-style horizontal scrolling:
/// - `Static`: view is stationary
/// - `Animation`: view is animating to a new position (e.g., focus change)
/// - `Gesture`: view is being dragged by a touchpad/mouse gesture
#[derive(Debug, Clone)]
pub enum ViewOffset {
    Static(f64),
    Animation(Animation),
    Gesture(ViewGesture),
}

/// A gesture-controlled view offset.
#[derive(Debug, Clone)]
pub struct ViewGesture {
    pub current_view_offset: f64,
    /// Animation for extra offset (e.g., DnD scroll activation).
    pub animation: Option<Animation>,
    pub tracker: SwipeTracker,
    /// Delta to convert tracker position to view offset.
    pub delta_from_tracker: f64,
    /// The view offset to restore if gesture is cancelled.
    pub stationary_view_offset: f64,
    /// Whether the gesture is touchpad-driven.
    pub is_touchpad: bool,
}

impl ViewOffset {
    /// Current view offset value.
    pub fn current(&self) -> f64 {
        match self {
            Self::Static(offset) => *offset,
            Self::Animation(anim) => anim.value(),
            Self::Gesture(gesture) => {
                gesture.current_view_offset
                    + gesture.animation.as_ref().map_or(0.0, |a| a.value())
            }
        }
    }

    /// Target view offset (for computing new positions).
    pub fn target(&self) -> f64 {
        match self {
            Self::Static(offset) => *offset,
            Self::Animation(anim) => anim.target(),
            Self::Gesture(gesture) => gesture.current_view_offset,
        }
    }

    /// A stable value suitable for saving/restoring.
    pub fn stationary(&self) -> f64 {
        match self {
            Self::Static(offset) => *offset,
            Self::Animation(anim) => anim.target(),
            Self::Gesture(gesture) => gesture.stationary_view_offset,
        }
    }

    pub fn is_static(&self) -> bool {
        matches!(self, Self::Static(_))
    }

    pub fn is_gesture(&self) -> bool {
        matches!(self, Self::Gesture(_))
    }

    pub fn is_animation_ongoing(&self) -> bool {
        match self {
            Self::Static(_) => false,
            Self::Animation(_) => true,
            Self::Gesture(gesture) => gesture.animation.is_some(),
        }
    }

    /// Shift the view offset by a delta.
    pub fn offset(&mut self, delta: f64) {
        match self {
            Self::Static(offset) => *offset += delta,
            Self::Animation(anim) => anim.offset(delta),
            Self::Gesture(gesture) => {
                gesture.stationary_view_offset += delta;
                gesture.delta_from_tracker += delta;
                gesture.current_view_offset += delta;
            }
        }
    }

    /// Cancel any ongoing gesture, snapping to current position.
    pub fn cancel_gesture(&mut self) {
        if let Self::Gesture(gesture) = self {
            *self = Self::Static(gesture.current_view_offset);
        }
    }

    /// Stop all animation/gesture and snap to current value.
    pub fn stop_anim_and_gesture(&mut self) {
        *self = Self::Static(self.current());
    }
}

impl ViewGesture {
    pub fn new(is_touchpad: bool, current_offset: f64) -> Self {
        Self {
            current_view_offset: current_offset,
            animation: None,
            tracker: SwipeTracker::new(),
            delta_from_tracker: current_offset,
            stationary_view_offset: current_offset,
            is_touchpad,
        }
    }

    pub fn animate_from(&mut self, from: f64, config: AnimationConfig) {
        let current = self.animation.as_ref().map_or(0.0, Animation::value);
        self.animation = Some(Animation::new(
            from + current,
            0.0,
            config,
        ));
    }
}

/// Compute the new view offset to ensure a column at `col_x` with `col_width`
/// is fully visible within the view.
///
/// `view_width` is the visible width, `padding` is the minimum gap to edges.
pub fn compute_new_view_offset(
    cur_view_x: f64,
    view_width: f64,
    col_x: f64,
    col_width: f64,
    padding: f64,
) -> f64 {
    // If the column is wider than the view, left-align it.
    if view_width <= col_width {
        return -col_x;
    }

    // Clamp padding if column is very wide.
    let pad = ((view_width - col_width) / 2.0).clamp(0.0, padding);

    let new_left = col_x - pad;
    let new_right = col_x + col_width + pad;

    // If fully visible, keep current view.
    if cur_view_x <= new_left && new_right <= cur_view_x + view_width {
        return -(col_x - cur_view_x);
    }

    // Prefer the alignment with less motion.
    let dist_to_left = (cur_view_x - new_left).abs();
    let dist_to_right = ((cur_view_x + view_width) - new_right).abs();

    if dist_to_left <= dist_to_right {
        -pad
    } else {
        -(view_width - pad - col_width)
    }
}
