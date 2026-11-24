//! **Which items are built** — the pure arithmetic of a virtual list, kept apart from the widget so
//! it can be checked without a tree.
//!
//! Every item is assumed to be as tall as the average of the items built so far. That estimate
//! decides which items fall in the visible window and how much room the items not built take
//! above and below them; the items that are built are measured for real, and the average follows
//! them.

/// Items built beyond each edge of the visible window, so a scroll of a few rows never shows a gap
/// before the next frame builds what came into view.
pub(super) const OVERSCAN: usize = 4;

/// The window of items to build, `start..end`, for a list of `len` items each about `item_h`
/// tall, scrolled by `offset` in a viewport `viewport_h` tall.
pub(super) fn window(len: usize, item_h: f64, offset: f64, viewport_h: f64) -> (usize, usize) {
    if len == 0 || item_h <= 0.0 {
        return (0, len.min(1));
    }
    let first = (offset.max(0.0) / item_h).floor() as usize;
    let last = ((offset.max(0.0) + viewport_h.max(item_h)) / item_h).ceil() as usize;
    let start = first.saturating_sub(OVERSCAN).min(len);
    let end = (last + OVERSCAN).min(len).max(start);
    (start, end)
}

/// The scroll offset that brings item `index` into view, or `None` when it already is.
pub(super) fn reveal(index: usize, item_h: f64, offset: f64, viewport_h: f64) -> Option<f64> {
    let top = index as f64 * item_h;
    let bottom = top + item_h;
    if top < offset {
        Some(top)
    } else if bottom > offset + viewport_h {
        Some((bottom - viewport_h).max(0.0))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A long list builds the rows on screen and a few beyond each edge — never all of them.
    #[test]
    fn only_the_visible_items_and_a_margin_are_built() {
        assert_eq!(window(300, 40.0, 0.0, 400.0), (0, 10 + OVERSCAN));
        let (start, end) = window(300, 40.0, 4000.0, 400.0);
        assert_eq!((start, end), (100 - OVERSCAN, 110 + OVERSCAN));
        assert_eq!(
            window(300, 40.0, 99_999.0, 400.0).1,
            300,
            "never past the end"
        );
        assert_eq!(window(0, 40.0, 0.0, 400.0), (0, 0));
    }

    /// An item above or below the viewport is brought to its nearest edge; one in view stays put.
    #[test]
    fn revealing_scrolls_only_as_far_as_needed() {
        assert_eq!(reveal(2, 40.0, 0.0, 400.0), None);
        assert_eq!(reveal(20, 40.0, 0.0, 400.0), Some(440.0));
        assert_eq!(reveal(1, 40.0, 400.0, 400.0), Some(40.0));
    }
}
