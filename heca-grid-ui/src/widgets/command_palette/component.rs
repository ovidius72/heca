//! Component lifecycle and input handling.

use super::chord::KeyChord;
use super::*;

impl Component for CommandPalette {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn focusable(&self) -> bool {
        self.is_open()
    }

    fn overlay_active(&self) -> bool {
        self.is_open()
    }

    /// On screen only while open: closed, its rows are still children, and gone to every walk.
    fn is_shown(&self) -> bool {
        self.base.visible.get_untracked() && !self.base.style.layout.hidden && self.is_open()
    }

    /// An open palette grabs input for the whole viewport (typing, nav, outside
    /// click = dismiss), so every point is occluded while open — a host must not
    /// synthesize a page-level action (e.g. open a context menu) under it.
    fn overlay_occludes(&self, _pos: heca_core::layout::Point) -> bool {
        self.is_open()
    }

    /// **A layer is not scrolled into view.** `Base::focused` says this widget holds the keyboard
    /// while it is open, and `wants_visible` defaults to exactly that — so an enclosing
    /// `ScrollRegion` would scroll the page to wherever this widget's layout node happens to sit,
    /// every frame it is open. A layer draws over the page; the page does not come to it.
    fn wants_visible(&self) -> bool {
        false
    }

    /// Bring the rows in line with the query and the cursor **before** the layout engine measures
    /// them, so a description that just started to wrap is measured wrapped.
    ///
    /// `build` calls this on a node and only then descends into its children, so everything written
    /// here is what they are measured with: which rows are in the layout, in what order, which
    /// description wraps — and the one width the layout cannot share between rows, the chord
    /// column's.
    fn remeasure(&mut self) {
        self.sync_query();
        self.sync_rows();
        let font = self.base.font;
        let widest = self
            .results()
            .iter()
            .flat_map(|ranked| &self.commands[ranked.index].chords)
            .map(|chord| KeyChord::width_of(chord, font))
            .fold(0.0, f64::max);
        for row in &mut self.list_mut().children {
            row::reserve_chords(row.as_mut(), widest as f32);
        }
    }

    /// Hover and press are noted by the rows' own handlers and acted on here, once a frame.
    fn tick(&mut self, dt: f32) -> bool {
        if self.is_open() {
            self.take_pointer();
        }
        let mut animating = self.base.tick_presence(dt);
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }

    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        self.take_pointer();
        self.sync_query();
        match ev {
            Event::Widget(WidgetIntent::Dismiss) => {
                self.dismiss();
                Handled::Yes
            }
            Event::Widget(WidgetIntent::Activate) => {
                self.run_selected();
                Handled::Yes
            }
            Event::Widget(WidgetIntent::MenuDown) => {
                self.select_next();
                Handled::Yes
            }
            Event::Widget(WidgetIntent::MenuUp) => {
                self.select_prev();
                Handled::Yes
            }
            Event::Widget(WidgetIntent::MenuPageDown) => {
                self.select_page(true);
                Handled::Yes
            }
            Event::Widget(WidgetIntent::MenuPageUp) => {
                self.select_page(false);
                Handled::Yes
            }
            Event::Widget(WidgetIntent::MenuFirst) => {
                self.select_edge(false);
                Handled::Yes
            }
            Event::Widget(WidgetIntent::MenuLast) => {
                self.select_edge(true);
                Handled::Yes
            }
            Event::Widget(WidgetIntent::Complete) => self.complete_selected(),
            Event::Widget(
                intent @ (WidgetIntent::MenuHistoryUp | WidgetIntent::MenuHistoryDown),
            ) => {
                let (sigil, scope, query) = self.active();
                let recalled = self.search.with_scope(scope).handle(*intent, &query);
                if let SearchAction::SetQuery(text) = recalled
                    && text != query
                {
                    self.query.set(Self::with_sigil(sigil, &text));
                    self.on_query_replaced();
                    self.base.mark_needs_layout();
                }
                Handled::Yes
            }
            // A press on the scrim, outside the panel, dismisses. Anything on the panel goes on to
            // the rows, which note it themselves.
            Event::PointerDown(p) if !self.panel_bounds().contains(p.pos) => {
                self.dismiss();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }
}
