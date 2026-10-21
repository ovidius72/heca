//! Draw the palette and its existing row children.

use super::*;

impl CommandPalette {
    pub(super) fn paint_palette(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() || !self.is_open() {
            return;
        }
        self.viewport.set(cx.viewport());
        // NB: the panel's own surface fill + corner radius are read by the shared
        // `paint_panel_chrome`, so they are deliberately not pulled out here.
        let (background, accent, glow_c, foreground, muted, ctrl_radius) = {
            let t = cx.theme();
            (
                t.colors.background,
                cx.accent(),
                t.colors.glow,
                t.colors.foreground,
                t.colors.muted,
                t.colors.control_radius(),
            )
        };
        let font = self.base.font;
        let line = self.line_h();
        let results = self.results();
        let (panel, _query, list_top) = self.layout(&results);
        // Remember the panel so the idle caret blink can damage just this rect.
        self.panel.set(panel);

        cx.with_overlay(|cx| {
            // Scrim + panel.
            let vp = self.viewport.get();
            let scrim = if vp.w.is_finite() {
                Rectangle::new(Point::new(0.0, 0.0), vp)
            } else {
                panel
            };
            cx.rect(
                scrim,
                background.with_alpha(cx.theme().colors.interaction.scrim),
                None,
                0.0,
                None,
            );
            // The SHARED overlay panel chrome (drop shadow + theme surface fill +
            // bracket reticle) so the palette reads as the same surface as every
            // other overlay panel, plus its own accent edge and glow. The scrim
            // above is the blocking LAYER's, not part of the panel chrome.
            let panel_border =
                cx.border(accent.with_alpha(cx.theme().colors.interaction.panel_border));
            paint_panel_chrome(
                cx,
                panel,
                PanelChrome {
                    border: panel_border,
                    glow: Some(Glow {
                        color: glow_c,
                        radius: 12.0,
                        intensity: 0.3,
                    }),
                    elevation: PanelElevation::Panel,
                },
            );

            // The query is laid out with the rest of the tree, and receives input through it.
            if let Some(field) = self.base.children.last() {
                crate::component::paint_child(field.as_ref(), cx);
            }
            // Result rows (the visible scroll window), from the one geometry helper.
            for (ri, row) in self.row_rects(&results, panel, list_top) {
                let m = &results[ri];
                let cmd = &self.commands[m.index];
                let is_sel = ri == self.selected;
                if is_sel {
                    let row_border = cx
                        .border(accent.with_alpha(cx.theme().colors.interaction.panel_row_border));
                    cx.rect(
                        row,
                        accent.with_alpha(cx.theme().colors.interaction.panel_row_fill),
                        row_border,
                        ctrl_radius,
                        None,
                    );
                    // Left accent bar.
                    cx.rect(
                        Rectangle::new(
                            Point::new(row.loc.x, row.loc.y + row.size.h * 0.2),
                            Size::new(2.5, row.size.h * 0.6),
                        ),
                        accent,
                        None,
                        1.0,
                        None,
                    );
                }
                // The line the label, its match highlights, the icon and the shortcut all share.
                // In a one-line list that is the whole row (unchanged); in a described list it is
                // the top line, with the description under it — so every element of the row keeps
                // one vertical rule instead of each re-deriving it.
                let first_line = Rectangle::new(
                    Point::new(row.loc.x, row.loc.y + ROW_PAD_Y),
                    Size::new(row.size.w, line),
                );
                // Icon. The column is reserved **whether or not this row has one**, so a list where
                // some commands carry a glyph and some do not still reads as one column of text —
                // indenting only the iconed rows left the others starting at the panel edge.
                let icon_x = row.loc.x + ROW_PAD_X;
                let isz = font * ICON_FONT_MUL;
                if let Some(ch) = cmd.icon.and_then(|g| g.primary_char()) {
                    let irect = Rectangle::new(
                        Point::new(icon_x, first_line.loc.y),
                        Size::new(isz as f64, first_line.size.h),
                    );
                    // The accent says "here": the selected row, and the row you are already on.
                    cx.icon(
                        irect,
                        &ch.to_string(),
                        if is_sel || cmd.current { accent } else { muted },
                        isz,
                    );
                }
                // **The row's text is a child.** A title `Label` (cut to its box, its match
                // marks a property) over an optional description `Label` (wrapped) — placed by
                // `place_rows` and painted here under the row's content colour, so both track the
                // selection without either being told what a selection is. Truncation, wrapping and
                // the marks are the label's; this widget draws no text.
                let content = if is_sel {
                    foreground
                } else if cmd.current {
                    // Where you already are, when the keyboard is somewhere else. Selection still
                    // wins on the row it is on: two accents on one row would say nothing.
                    accent
                } else {
                    muted.lerp(foreground, 0.7)
                };
                cx.with_content_color(content, |cx| {
                    crate::component::paint_child(self.row_child(m.index), cx)
                });
                // Bindings: one row of keycap chips per binding, stacked down the row, each
                // right-aligned to the same reserved column so every command's chips line up.
                let cap_font = self.keycap_font();
                let right_edge = row.loc.x + row.size.w - ROW_PAD_X;
                for (ci, chord) in cmd.chords.iter().enumerate() {
                    let line_top = first_line.loc.y + ci as f64 * line;
                    let mut x = right_edge - self.chord_w(chord);
                    for cap in chord {
                        let cs = cap.size(cap_font);
                        let chip =
                            Rectangle::new(Point::new(x, line_top + (line - cs.h) / 2.0), cs);
                        // The bordered chip — the same primitive the context-menu quick-pick draws,
                        // never hand-rolled here.
                        cap.paint(
                            cx,
                            chip,
                            cap_font,
                            Some(if is_sel { accent } else { muted }),
                            KeycapVariant::Bordered,
                        );
                        x += cs.w + CAP_GAP;
                    }
                }
            }
        });
    }

}
