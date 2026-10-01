//! **The terminal's scrollback controls** — the "N lines above" chip and the scrollbar.
//!
//! They are children of the [`Terminal`](super::Terminal), so they are placed by the layout, drawn
//! by the same walk that draws the terminal, and hear the pointer from the same tree. Nothing
//! outside positions, paints or routes them; the host only says what the viewport looks like
//! ([`Viewport`]) and what a click means ([`ScrollIntents`]).

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use heca_config::appearance::ScrollbarVisibility;
use heca_grid_ui::builders::LayoutExt;
use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate};
use heca_grid_ui::style::Length;
use heca_grid_ui::widgets::{BadgeButton, ScrollBar};
use heca_grid_ui::{Component, SignalData};

/// **What a terminal's viewport looks like right now**, as plain data: how many rows it shows, how
/// many the backend keeps, how far up it is scrolled — and which of the two controls the user
/// asked to see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Viewport {
    /// Rows on screen.
    pub rows: usize,
    /// Rows the backend retains, history and screen together.
    pub scrollback_rows: usize,
    /// Rows scrolled above the live bottom; `0` is the live bottom.
    pub offset: usize,
    pub scrollbar: ScrollbarVisibility,
    /// Whether the "N lines above" chip is wanted at all.
    pub badge: bool,
}

impl Viewport {
    fn at_bottom(&self) -> bool {
        self.offset == 0
    }

    fn scrollable(&self) -> bool {
        self.scrollback_rows > self.rows
    }

    /// How far the thumb is from the top, in rows: the bar counts down from the oldest line, the
    /// backend counts up from the live bottom.
    fn offset_from_top(&self) -> f32 {
        let max = self.scrollback_rows.saturating_sub(self.rows);
        max.saturating_sub(self.offset) as f32
    }

    fn scrollbar_shown(&self) -> bool {
        match self.scrollbar {
            ScrollbarVisibility::Always => self.scrollable(),
            ScrollbarVisibility::WhenNeeded => self.scrollable() && !self.at_bottom(),
            ScrollbarVisibility::Never => false,
        }
    }

    fn badge_shown(&self) -> bool {
        self.badge && !self.at_bottom()
    }
}

/// What a click on the controls means, said by whoever owns the terminal. Two callbacks, as one
/// group — the seams a component binds travel together.
pub(crate) struct ScrollIntents {
    /// The chip was clicked: back to the live bottom.
    pub to_bottom: Box<dyn Fn()>,
    /// The thumb moved: scroll to this many rows above the live bottom.
    pub to_offset: Box<dyn Fn(usize)>,
}

/// Where a terminal keeps what a click means, shared by every node placed for it.
pub(super) type IntentSlot = Rc<RefCell<Option<ScrollIntents>>>;

/// "1 line above" / "N lines above".
pub(super) fn lines_above(lines: usize) -> String {
    if lines == 1 {
        "1 line above".to_string()
    } else {
        format!("{lines} lines above")
    }
}

/// The thumb's distance from the top, back to rows above the live bottom.
pub(super) fn rows_above(content: f32, viewport: f32, offset_from_top: f64) -> usize {
    let max = (content - viewport).max(0.0) as f64;
    (max - offset_from_top).round().clamp(0.0, max) as usize
}

/// **The handles a built set of controls is driven through** — the signals the two widgets read.
/// They are `Copy` handles, so keeping them costs nothing and the widgets stay in the tree.
pub(super) struct Controls {
    content: Signal<f32>,
    viewport: Signal<f32>,
    offset: Signal<f32>,
    bar_visible: Signal<bool>,
    badge_visible: Signal<bool>,
    badge_label: Signal<String>,
    /// Whether the pointer is over the bar or the chip.
    bar_hovered: Signal<bool>,
    badge_hovered: Signal<bool>,
}

impl Controls {
    /// Build the two widgets and the handles that drive them.
    ///
    /// Returns the widgets to put under the terminal, which lays them out: the bar flows to its
    /// right edge at its full height, and the chip is taken out of the flow and left at the
    /// container's own top-right corner — a layout, not a measurement.
    pub(super) fn build(intents: &IntentSlot) -> (Self, [Box<dyn Component>; 2]) {
        let to_bottom = intents.clone();
        let mut badge = BadgeButton::accent("0 lines above").on_click(move || {
            if let Some(i) = to_bottom.borrow().as_ref() {
                (i.to_bottom)();
            }
        });
        badge.base_mut().visible.set(false);

        let to_offset = intents.clone();
        let mut bar = ScrollBar::new();
        let content = bar.content_extent_signal();
        let viewport = bar.viewport_extent_signal();
        bar = bar.on_change(move |action| {
            if let SignalData::Float(top) = action.data
                && let Some(i) = to_offset.borrow().as_ref()
            {
                (i.to_offset)(rows_above(
                    content.get_untracked(),
                    viewport.get_untracked(),
                    top,
                ));
            }
        });
        bar.base_mut().visible.set(false);

        let controls = Self {
            content,
            viewport,
            offset: bar.offset_signal(),
            bar_visible: bar.base().visible,
            badge_visible: badge.base().visible,
            badge_label: badge.label_signal(),
            bar_hovered: bar.base().pointer.hovered,
            badge_hovered: badge.base().pointer.hovered,
        };
        let bar = bar.height(Length::Percent(1.0));
        // Out of the flow, with every edge left to the container: it lands where a lone child
        // would, which for a row that ends at the right is the right edge, at the top.
        let badge = badge.at_rect(Length::Auto, Length::Auto, Length::Auto, Length::Auto);
        let layers: [Box<dyn Component>; 2] = [Box::new(bar), Box::new(badge)];
        (controls, layers)
    }

    /// Show `viewport`. Returns whether anything the user can see changed.
    pub(super) fn show(&self, viewport: &Viewport) -> bool {
        let before = (
            self.content.get_untracked(),
            self.viewport.get_untracked(),
            self.offset.get_untracked(),
            self.bar_visible.get_untracked(),
            self.badge_visible.get_untracked(),
            self.badge_label.get_untracked(),
        );
        self.content.set(viewport.scrollback_rows as f32);
        self.viewport.set(viewport.rows as f32);
        self.offset.set(viewport.offset_from_top());
        self.bar_visible.set(viewport.scrollbar_shown());
        self.badge_visible.set(viewport.badge_shown());
        if viewport.badge_shown() {
            self.badge_label.set(lines_above(viewport.offset));
        }
        before.0 != self.content.get_untracked()
            || before.1 != self.viewport.get_untracked()
            || before.2 != self.offset.get_untracked()
            || before.3 != self.bar_visible.get_untracked()
            || before.4 != self.badge_visible.get_untracked()
            || before.5 != self.badge_label.get_untracked()
    }

    /// Is the pointer on the bar or the chip? The terminal under them must not hear it.
    pub(super) fn hovered(&self) -> bool {
        self.bar_hovered.get_untracked() || self.badge_hovered.get_untracked()
    }
}

/// Every set of controls built for one terminal — one per node placed in a tree. The terminal
/// shows a viewport on all that are still alive, forgets the rest, and gives a node placed later
/// the viewport it last showed — so a rebuilt tree agrees with the one it replaced.
#[derive(Default)]
pub(super) struct Placed {
    nodes: Vec<Weak<Controls>>,
    last: Option<Viewport>,
}

impl Placed {
    pub(super) fn add(&mut self, controls: &Rc<Controls>) {
        self.nodes.retain(|c| c.strong_count() > 0);
        if let Some(viewport) = &self.last {
            controls.show(viewport);
        }
        self.nodes.push(Rc::downgrade(controls));
    }

    pub(super) fn show(&mut self, viewport: &Viewport) -> bool {
        self.last = Some(*viewport);
        self.nodes
            .iter()
            .filter_map(Weak::upgrade)
            .fold(false, |changed, c| c.show(viewport) | changed)
    }

    pub(super) fn hovered(&self) -> bool {
        self.nodes
            .iter()
            .filter_map(Weak::upgrade)
            .any(|c| c.hovered())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(offset: usize, mode: ScrollbarVisibility) -> Viewport {
        Viewport {
            rows: 24,
            scrollback_rows: 124,
            offset,
            scrollbar: mode,
            badge: true,
        }
    }

    #[test]
    fn the_chip_counts_lines_the_way_it_reads() {
        assert_eq!(lines_above(1), "1 line above");
        assert_eq!(lines_above(40), "40 lines above");
    }

    #[test]
    fn the_thumb_counts_down_from_the_oldest_line() {
        // At the live bottom the thumb is at the bottom of the track: all 100 rows of history up.
        assert_eq!(at(0, ScrollbarVisibility::Always).offset_from_top(), 100.0);
        assert_eq!(at(100, ScrollbarVisibility::Always).offset_from_top(), 0.0);
    }

    #[test]
    fn the_thumb_position_comes_back_as_rows_above_the_bottom() {
        for offset in [0, 1, 37, 100] {
            let v = at(offset, ScrollbarVisibility::Always);
            assert_eq!(rows_above(124.0, 24.0, v.offset_from_top() as f64), offset);
        }
        assert_eq!(rows_above(124.0, 24.0, -5.0), 100, "past the top clamps");
        assert_eq!(rows_above(124.0, 24.0, 500.0), 0, "past the bottom clamps");
    }

    #[test]
    fn each_mode_shows_the_bar_when_it_says() {
        use ScrollbarVisibility::*;
        assert!(at(0, Always).scrollbar_shown());
        assert!(!at(0, WhenNeeded).scrollbar_shown());
        assert!(at(5, WhenNeeded).scrollbar_shown());
        assert!(!at(5, Never).scrollbar_shown());
        let mut nothing_to_scroll = at(0, Always);
        nothing_to_scroll.scrollback_rows = 24;
        assert!(!nothing_to_scroll.scrollbar_shown());
    }

    #[test]
    fn the_chip_shows_only_while_scrolled_up_and_wanted() {
        assert!(!at(0, ScrollbarVisibility::Always).badge_shown());
        assert!(at(3, ScrollbarVisibility::Always).badge_shown());
        let mut off = at(3, ScrollbarVisibility::Always);
        off.badge = false;
        assert!(!off.badge_shown());
    }
}
