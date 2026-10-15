//! Panel and row geometry, shared by drawing and input.

use super::*;

impl CommandPalette {
    /// The text column of row `i` — the child holding its title and description.
    pub(super) fn row_child(&self, i: usize) -> &dyn Component {
        self.base.children[i].as_ref()
    }

    /// Keep the selection within the visible scroll window.
    pub(super) fn follow_selection(&mut self) {
        let visible = self.visible_rows(&self.results());
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + visible {
            self.scroll = self.selected + 1 - visible;
        }
    }

    /// How many rows the panel shows: the size variant's count, **and never more than the window
    /// can hold with [`BOTTOM_FRAC`] still clear beneath it**.
    ///
    /// It measures the rows it would actually draw, from `scroll` onward, instead of dividing the
    /// room by a nominal row height. A row is two lines when the list is described and taller again
    /// when an action carries several bindings, so a one-line estimate over-counted and the panel ran
    /// off the bottom of a short window — which is what this looked like in the app.
    pub(super) fn visible_rows(&self, results: &[Ranked]) -> usize {
        let (_, max_rows) = panel_metrics(self.panel_size);
        let vp = self.viewport.get();
        if !vp.h.is_finite() {
            return results.len().clamp(1, max_rows);
        }
        // Everything the panel spends before the first row: its own padding above and below the
        // query line, the query line itself, and the padding under the list.
        let chrome = 3.0 * PAD + self.line_h() + 2.0 * QUERY_PAD_Y;
        // The panel starts at `vh * TOP_FRAC`, so filling the remaining `1 - TOP_FRAC` put its
        // bottom exactly on the window's edge. `BOTTOM_FRAC` is the room kept clear under it.
        let mut room = (vp.h * (1.0 - TOP_FRAC - BOTTOM_FRAC) - chrome).max(0.0);
        let mut fits = 0usize;
        for m in results.iter().skip(self.scroll).take(max_rows) {
            let h = self.row_h(m.index);
            if h > room && fits > 0 {
                break;
            }
            room -= h;
            fits += 1;
        }
        // Always at least one row: a window too short for even that is better showing a clipped row
        // than an empty panel.
        results.len().clamp(1, fits.max(1))
    }

    pub(super) fn line_h(&self) -> f64 {
        (self.base.font * MONO_LINE_RATIO) as f64
    }

    /// Does any command carry a description? Then **every** row is two lines high.
    ///
    /// One height for the whole list, not per row: `row_rect` is the same rectangle the paint, the
    /// hover-select and the click hit-test all compute, and rows of differing heights would make
    /// that an index-dependent running sum in three places. A described list simply leaves the
    /// second line of an undescribed row empty.
    ///
    /// Asked of the **active mode only**: pane rows carry no description, so charging them a second
    /// line because the action list has one would leave every row in `@` mode half empty.
    pub(super) fn two_line_rows(&self) -> bool {
        let (_, scope, _) = self.active();
        self.commands
            .iter()
            .filter(|c| self.in_mode(c, &scope))
            .any(|c| c.description.is_some())
    }

    /// How many lines **this** row needs: its text (label, plus a description if the list has any)
    /// or its stack of bindings, whichever is taller.
    ///
    /// Per row, **not** one height for the list. The uniform version was simpler — `row_rect` is
    /// shared by the paint, the hover-select and the click hit-test, and one height makes it
    /// arithmetic instead of a running sum — but it charges every row for the worst row: one action
    /// bound three times (`paste_clipboard` is) made all 100 rows three lines tall, which is exactly
    /// the vertical sprawl this fixes. The running sum lives in [`row_rects`](Self::row_rects) and
    /// all three callers read it there, so it is still written once.
    pub(super) fn row_lines(&self, cmd: &Command) -> usize {
        let text = if self.two_line_rows() { 2 } else { 1 };
        text.max(cmd.chords.len()).max(1)
    }

    /// Height of one result row — **measured**, not computed.
    ///
    /// The text column is a real child, so its height already accounts for a description that
    /// wrapped onto three lines; reading it here is the whole of the reflow. Before the first layout
    /// pass — the palette opens and paints in the same frame — it falls back to the font-derived
    /// estimate, exactly as [`Select`](crate::widgets::Select) does with its own rows.
    pub(super) fn row_h(&self, cmd: usize) -> f64 {
        let measured = self.natural_h.get(cmd).copied().unwrap_or(0.0);
        let text_h = if measured > 0.0 {
            measured
        } else {
            self.line_h() * self.row_lines(&self.commands[cmd]) as f64
        };
        // A row bound three times is taller than its text: the chips stack one per line.
        let chips_h = self.line_h() * self.commands[cmd].chords.len().max(1) as f64;
        text_h.max(chips_h) + 2.0 * ROW_PAD_Y
    }

    /// The panel's width for the current viewport — extracted from [`layout`](Self::layout) because
    /// [`remeasure`](Component::remeasure) needs it too, and the two must never disagree about how
    /// wide the text is allowed to be.
    pub(super) fn panel_w(&self) -> f64 {
        let (want_w, _) = panel_metrics(self.panel_size);
        let vp = self.viewport.get();
        if !vp.w.is_finite() {
            return want_w;
        }
        let room = vp.w * PANEL_VIEWPORT_FRAC;
        want_w.min(room).max(PANEL_MIN_W.min(room))
    }

    /// The icon column's width — reserved whether or not a row has an icon.
    pub(super) fn icon_col(&self) -> f64 {
        (self.base.font * ICON_FONT_MUL) as f64 + ICON_GAP
    }

    /// The width the row text is laid out at: the panel's inside, less the icon column and the
    /// reserved shortcut column. **The labels cut and wrap against exactly this**, because it is the
    /// width the engine gives the text children in `remeasure`.
    pub(super) fn text_w(&self) -> f64 {
        (self.panel_w()
            - 2.0 * PAD
            - 2.0 * ROW_PAD_X
            - self.icon_col()
            - self.shortcut_col_w()
            - SHORTCUT_GAP)
            .max(0.0)
    }

    /// Move each visible row's text column into its slot in the panel, and collapse the rest.
    ///
    /// The engine lays the children out in the palette's own flow and measures them there; it cannot
    /// place them in an overlay panel, because a taffy node sits where its parent puts it. So this
    /// bakes an **absolute** target into each child's bounds — the same thing
    /// [`Select::place_options`](crate::widgets::Select) does, and idempotent for the same reason: re-running
    /// it never compounds an offset.
    ///
    /// Rows outside the visible window collapse to zero size, so no stale rect is left behind.
    pub(super) fn place_rows(&mut self) {
        let results = self.results();
        let (panel, _query, list_top) = self.layout(&results);
        let dx = panel.loc.x + PAD + ROW_PAD_X + self.icon_col();
        let mut targets: Vec<Option<Rectangle>> = vec![None; self.base.children.len()];
        for (ri, row) in self.row_rects(&results, panel, list_top) {
            let cmd = results[ri].index;
            let h = self.base.children[cmd].base().bounds.size.h;
            targets[cmd] = Some(Rectangle::new(
                Point::new(dx, row.loc.y + ROW_PAD_Y),
                Size::new(self.text_w(), h),
            ));
        }
        for (i, target) in targets.into_iter().enumerate() {
            let Some(target) = target else {
                self.base.children[i].base_mut().bounds.size = Size::new(0.0, 0.0);
                continue;
            };
            let child = self.base.children[i].as_mut();
            let d = (
                target.loc.x - child.base().bounds.loc.x,
                target.loc.y - child.base().bounds.loc.y,
            );
            if d.0 != 0.0 || d.1 != 0.0 {
                crate::component::shift_subtree(child, d.0, d.1);
            }
            child.base_mut().bounds = target;
        }
    }

    /// The rectangle of every **visible** row, paired with its index in `results`.
    ///
    /// The one place row geometry is computed. Paint draws these, hover-select and the click
    /// hit-test read them — so a row can never be drawn in one place and clicked in another.
    pub(super) fn row_rects(
        &self,
        results: &[Ranked],
        panel: Rectangle,
        list_top: f64,
    ) -> Vec<(usize, Rectangle)> {
        let mut out = Vec::new();
        let mut y = list_top;
        let window = results
            .iter()
            .enumerate()
            .skip(self.scroll)
            .take(self.visible_rows(results));
        for (ri, m) in window {
            let h = self.row_h(m.index);
            out.push((
                ri,
                Rectangle::new(
                    Point::new(panel.loc.x + PAD, y),
                    Size::new(panel.size.w - 2.0 * PAD, h),
                ),
            ));
            y += h;
        }
        out
    }

    /// Width of one chord's chips, laid end to end.
    pub(super) fn chord_w(&self, chord: &[KeyCap]) -> f64 {
        let font = self.keycap_font();
        let caps: f64 = chord.iter().map(|c| c.size(font).w).sum();
        caps + CAP_GAP * chord.len().saturating_sub(1) as f64
    }

    /// The width **reserved on the right of every row** for bindings — the widest chord in the whole
    /// list, so the labels of all rows end at the same place and the chips line up in a column
    /// instead of tracking each label's length.
    ///
    /// The **active mode's** widest, for the same reason the row height is the active mode's: a
    /// pane list binds nothing, and reserving the action list's keycap column across it would spend
    /// that width on empty space.
    pub(super) fn shortcut_col_w(&self) -> f64 {
        let (_, scope, _) = self.active();
        self.commands
            .iter()
            .filter(|c| self.in_mode(c, &scope))
            .flat_map(|c| c.chords.iter())
            .map(|chord| self.chord_w(chord))
            .fold(0.0, f64::max)
    }

    /// Keycap chips are drawn a touch smaller than the row text, like the context menu's quick-pick.
    pub(super) fn keycap_font(&self) -> f32 {
        self.base.font * KEYCAP_FONT_MUL
    }

    /// Panel + query + first-row geometry for the current viewport + result count.
    pub(super) fn layout(&self, results: &[Ranked]) -> (Rectangle, Rectangle, f64) {
        let vp = self.viewport.get();
        let line = self.line_h();
        let query_h = line + 2.0 * QUERY_PAD_Y;
        let n_results = results.len();
        let visible = self.visible_rows(results);
        // The panel is as tall as the rows it will actually show — summed, since a row with two
        // bindings is taller than one with none.
        let empty_row = line + 2.0 * ROW_PAD_Y;
        let list_h = match n_results {
            0 => empty_row,
            _ => results
                .iter()
                .skip(self.scroll)
                .take(visible)
                .map(|m| self.row_h(m.index))
                .sum(),
        };

        // **The size decides the width; the window only takes it away** — see `panel_w`, which
        // `remeasure` shares so the text is laid out at the width it will be drawn at.
        let panel_w = self.panel_w();
        let panel_h = PAD + query_h + PAD + list_h + PAD;
        let (vw, vh) = if vp.w.is_finite() {
            (vp.w, vp.h)
        } else {
            (panel_w, panel_h)
        };
        let px = (vw - panel_w) / 2.0;
        let py = vh * TOP_FRAC;
        let panel = Rectangle::new(Point::new(px, py), Size::new(panel_w, panel_h));
        let query = Rectangle::new(
            Point::new(px + PAD, py + PAD),
            Size::new(panel_w - 2.0 * PAD, query_h),
        );
        let list_top = query.loc.y + query_h + PAD;
        (panel, query, list_top)
    }
}
