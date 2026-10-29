//! Component lifecycle and input handling.

use super::*;

impl Component for CommandPalette {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn focusable(&self) -> bool { self.is_open() }

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
    fn overlay_occludes(&self, _pos: Point) -> bool {
        self.is_open()
    }

    fn paint(&self, cx: &mut PaintCx) {
        self.paint_palette(cx);
    }

    /// **A layer is not scrolled into view.** `Base::focused` says this widget holds the keyboard
    /// while it is open, and `wants_visible` defaults to exactly that — so an enclosing
    /// `ScrollRegion` would scroll the page to wherever this widget's layout node happens to sit,
    /// every frame it is open. A layer draws over the page; the page does not come to it.
    fn wants_visible(&self) -> bool {
        false
    }

    /// Lay the row text out at **the width it will be drawn at**, in a column.
    ///
    /// This is what makes a description wrap correctly: the engine measures each text child against
    /// this width, so the line count it reports is the line count that will be painted. The viewport
    /// is the one cached at the last paint, so the very first frame falls back to the size variant's
    /// nominal width and settles on the next — the same one-frame settle `Select` documents for its
    /// measured rows.
    fn remeasure(&mut self) {
        // Reflow the selected row **before** the children are measured. `build` calls this on a node
        // and only then descends into its children, so a wrap flipped here is the wrap they measure
        // with — flipping it after layout would show the previous selection's shape for a frame.
        self.sync_query();
        self.sync_wrap();
        let viewport = self.base.viewport;
        if viewport.w > 0.0 && viewport.h > 0.0 && viewport.w.is_finite() && viewport.h.is_finite() { self.viewport.set(viewport); }
        // The palette **fills the viewport**, like every other layer root, so `on_layout` can read
        // its own size and learn the viewport from the layout pass rather than waiting for a paint.
        self.base.style.layout.direction = Direction::Column;
        self.base.style.layout.width = Length::Percent(1.0);
        self.base.style.layout.height = Length::Percent(1.0);
        // The text children carry the width instead: the engine measures each against the width it
        // will be drawn at, so the line count it reports is the line count that gets painted.
        let w = self.text_w() as f32;
        for child in self.base.children.iter_mut().take(self.commands.len()) {
            child.base_mut().style.layout.width = Length::Px(w);
        }
        let query_width = (self.panel_w() - 2.0 * PAD).max(0.0) as f32;
        let query_height = (self.line_h() + 2.0 * self.query_padding()) as f32;
        let query = self.base.children.last_mut().expect("query child").base_mut();
        query.style.layout.width = Length::Px(query_width);
        query.style.layout.height = Length::Px(query_height);
    }

    fn taffy_style(&self) -> taffy::Style {
        self.base.style.layout.to_taffy(self.base.font)
    }

    /// Layout just reset every row to its natural position and size: re-read the measured heights
    /// from it, then place the rows into the panel again.
    fn on_layout(&mut self) {
        // **The viewport the engine was told to compute against**, which it records on every node
        // (`Base::viewport`) — known one whole frame earlier than the paint that used to be the
        // only source. Without it the first frame after the palette opens places its rows against a
        // fallback panel and the text lands off the panel.
        //
        // It used to read its OWN bounds, on the assumption "the root fills the viewport". That
        // holds when the palette is mounted as a full-size layer, as the app mounts it, and not
        // when a host puts it in a column of overlays, as the showcase does — there its bounds are
        // whatever the flex gave it, the panel is computed against the wrong box, and every row's
        // text lands 76px above the row it belongs to while the icons and keycaps (painted from the
        // panel, not placed) stay put. A widget must not infer the viewport from where it happens
        // to be mounted; the framework already records it.
        let size = self.base.viewport;
        if size.w > 0.0 && size.h > 0.0 && size.w.is_finite() && size.h.is_finite() {
            self.viewport.set(size);
        }
        self.natural_h = self
            .base
            .children
            .iter()
            .map(|c| c.base().bounds.size.h)
            .collect();
        self.place_rows();
        let (_, target, _) = self.layout(&self.results());
        let query = self.base.children.last_mut().expect("query child").as_mut();
        let current = query.base().bounds;
        crate::component::shift_subtree(query, target.loc.x - current.loc.x, target.loc.y - current.loc.y);
    }

    /// The palette paints its panel on the overlay layer, not at its layout `bounds`.
    fn damage_bounds(&self) -> Rectangle {
        if self.is_open() {
            self.panel.get()
        } else {
            // Closed, it damages like any other widget — its own box plus its bubble, which draws
            // on the overlay layer outside that box.
            crate::widgets::tooltip::damage(&self.base)
        }
    }

    /// Its **input** surface is the panel while open, and nothing while closed.
    fn hit_bounds(&self) -> Option<Rectangle> {
        if self.is_open() {
            Some(self.panel.get())
        } else {
            None
        }
    }

    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if !self.is_open() { return Handled::No; }
        self.sync_query();
        match ev {
            Event::Widget(WidgetIntent::Dismiss) => { self.dismiss(); Handled::Yes }
            Event::Widget(WidgetIntent::Activate) => { self.run_selected(); Handled::Yes }
            Event::Widget(WidgetIntent::MenuDown) => { self.select_next(); Handled::Yes }
            Event::Widget(WidgetIntent::MenuUp) => { self.select_prev(); Handled::Yes }
            Event::Widget(intent @ (WidgetIntent::MenuHistoryUp | WidgetIntent::MenuHistoryDown)) => {
                let (sigil, scope, query) = self.active();
                let recalled = self.search.with_scope(scope).handle(*intent, &query);
                if let SearchAction::SetQuery(text) = recalled && text != query {
                    self.query.set(Self::with_sigil(sigil, &text));
                    self.on_query_replaced();
                    self.base.mark_needs_layout();
                }
                Handled::Yes
            }
            Event::PointerDownOutside(_) => { self.dismiss(); Handled::No }
            _ => Handled::No,
        }
    }

    // The query consumes its own presses and edits. Only unhandled row input bubbles here.
    fn on_event(&mut self, ev: &Event) -> Handled {
        if !self.is_open() { return Handled::No; }
        match ev {
            Event::PointerMove(p) => {
                let results = self.results();
                let (panel, _, list_top) = self.layout(&results);
                let selected = self.row_rects(&results, panel, list_top).into_iter()
                    .find_map(|(index, row)| row.contains(p.pos).then_some(index));
                if let Some(selected) = selected && selected != self.selected {
                    self.selected = selected;
                    self.sync_wrap();
                }
                Handled::Yes
            }
            Event::PointerDown(p) => {
                let results = self.results();
                let (panel, _, list_top) = self.layout(&results);
                for (index, row) in self.row_rects(&results, panel, list_top) {
                    if row.contains(p.pos) { self.selected = index; self.run_selected(); break; }
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }
}
