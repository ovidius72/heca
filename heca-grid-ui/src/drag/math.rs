//! Drag math utilities.
//!
//! Threshold constants and rubberband functions for drag-and-drop.
//! These are pure functions with no dependency on app state.

/// Default drag threshold in squared pixels.
/// 10px of movement before a press becomes a drag.
pub const DEFAULT_DRAG_THRESHOLD_SQ: f32 = 100.0;

/// NIRI rubberband formula.
///
/// Maps absolute pixel distance `x` to a dampened offset via:
/// `(1.0 - (1.0 / (x * c / d + 1.0))) * d`
///
/// With `c = 1.0` and `d = 0.5`, this produces a smooth deceleration
/// curve for interactive pane movement.
pub fn rubberband(x: f32) -> f32 {
    let c = 1.0;
    let d = 0.5;
    (1.0 - (1.0 / (x * c / d + 1.0))) * d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rubberband_zero() {
        assert_eq!(rubberband(0.0), 0.0);
    }

    #[test]
    fn test_rubberband_positive() {
        let result = rubberband(10.0);
        assert!(result > 0.0 && result < 10.0, "rubberband should dampen");
    }

    #[test]
    fn test_rubberband_monotonic() {
        let r1 = rubberband(5.0);
        let r2 = rubberband(10.0);
        let r3 = rubberband(50.0);
        assert!(
            r1 < r2 && r2 < r3,
            "rubberband should be monotonically increasing"
        );
    }

    #[test]
    fn test_default_threshold() {
        assert_eq!(DEFAULT_DRAG_THRESHOLD_SQ, 100.0);
    }
}
