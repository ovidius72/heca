//! **Reading the screen for drawing** — snapshots, rows by stable index, what changed since a
//! sequence number, and search through the history.

use super::*;

impl TerminalEngine {
    pub(in crate::backend::terminal) fn snapshot(&self, cell_size: (f32, f32)) -> TerminalSnapshot {
        self.snapshot_at(cell_size, self.viewport_offset)
    }

    pub(in crate::backend::terminal) fn snapshot_at(&self, cell_size: (f32, f32), offset: usize) -> TerminalSnapshot {
        let (cell_w, cell_h) = cell_size;
        let palette = self.terminal.palette();
        let blank = blank_cell(&palette);
        let size = self.terminal.get_size();
        let cols = size.cols.max(1);
        let rows = size.rows.max(1);
        let blank_line = TerminalLine {
            cells: vec![blank; cols],
        };
        let (lines, scrollback_rows, viewport_offset, mut hyperlinks, graphics, images) =
            self.visible_lines(cols, rows, &palette, &blank_line, offset);

        // Auto-detect plain-text URLs (echo, logs, …) and add them as link spans
        // alongside the explicit OSC 8 ones. OSC 8 wins: a cell already inside an
        // explicit link is never re-linked.
        if self.link_detection {
            detect_plain_links(&lines, &mut hyperlinks);
        }

        let cursor = self.terminal.cursor_pos();
        // The cursor sits on the live screen. Scrolled back N rows, that screen is drawn N rows
        // lower, so the cursor moves down with its prompt — and is out of view once the prompt is.
        let cursor_row = (cursor.y.max(0) as usize) + viewport_offset;
        let snapshot = TerminalSnapshot {
            cols,
            rows,
            cell_w,
            cell_h,
            default_fg: to_rgba(palette.resolve_fg(ColorAttribute::Default)),
            default_bg: to_rgba(palette.resolve_bg(ColorAttribute::Default)),
            cursor_color: to_rgba(palette.cursor_bg),
            cursor: TerminalCursor {
                col: cursor.x.min(cols.saturating_sub(1)),
                row: cursor_row.min(rows.saturating_sub(1)),
                visible: cols > 0
                    && rows > 0
                    && cursor_row < rows
                    && cursor.visibility == CursorVisibility::Visible,
                shape: map_cursor_shape(cursor.shape),
            },
            lines,
            viewport_offset,
            at_bottom: viewport_offset == 0,
            scrollback_rows,
            viewport_top_stable_row: self.terminal.screen().visible_row_to_stable_row(0) - viewport_offset as isize,
            hyperlinks,
            graphics,
            images,
        };
        snapshot.debug_assert_valid();
        snapshot
    }

    /// Fetch the inclusive stable-row range `[start, end]` as renderer-ready
    /// [`TerminalLine`]s, padded/truncated to `cols`.
    ///
    /// Host-grid selections live in stable-row coordinates; copying a selection
    /// that spans history requires fetching content by stable row rather than
    /// indexing the visible snapshot. Each stable row maps 1:1 to a physical
    /// (ring-buffer) index via wezterm's `stable_row_to_phys`; phys indices
    /// ascend monotonically with stable rows, so the clamped range maps to an
    /// ascending phys range consumed by `lines_in_phys_range`.
    ///
    /// Rows outside the current retained range yield `None` and are emitted as
    /// blank lines so the returned count always matches `end - start + 1`.
    pub(in crate::backend::terminal) fn lines_in_stable_range(
        &self,
        start: isize,
        end: isize,
        cols: usize,
    ) -> Vec<TerminalLine> {
        let screen = self.terminal.screen();
        let palette = self.terminal.palette();
        let blank = blank_cell(&palette);
        let blank_line = TerminalLine {
            cells: vec![blank.clone(); cols],
        };
        if start > end || cols == 0 {
            return Vec::new();
        }
        let mut out: Vec<TerminalLine> = Vec::with_capacity((end - start + 1) as usize);
        for stable in start..=end {
            match screen.stable_row_to_phys(stable) {
                Some(phys) => {
                    let phys_lines = screen.lines_in_phys_range(phys..phys + 1);
                    if let Some(mut line) = phys_lines.into_iter().next() {
                        out.push(snapshot_line(&mut line, cols, &palette, &blank_line));
                    } else {
                        out.push(blank_line.clone());
                    }
                }
                None => out.push(blank_line.clone()),
            }
        }
        out
    }

    /// Case-insensitive substring search over the **entire** scrollback (history +
    /// visible), returning every matching run as a [`SearchMatch`] in ascending
    /// stable-row / column order. Reads cell text directly (no color resolution),
    /// fills inter-cell gaps with spaces so internal spaces match and columns stay
    /// aligned, and reports non-overlapping matches.
    pub(in crate::backend::terminal) fn search_scrollback(
        &self,
        query: &str,
        cols: usize,
    ) -> Vec<SearchMatch> {
        let screen = self.terminal.screen();
        let rows = screen.physical_rows.max(1);
        let visible_count = self.rows.min(rows);
        let max_offset = screen.scrollback_rows().saturating_sub(visible_count);
        let top_stable = screen.visible_row_to_stable_row(0) - max_offset as isize;
        let bottom_stable = screen.visible_row_to_stable_row(rows as i64 - 1);
        self.search_stable_range(query, cols, top_stable, bottom_stable)
    }

    pub(in crate::backend::terminal) fn search_stable_range(&self, query: &str, cols: usize, start: isize, end: isize) -> Vec<SearchMatch> {
        if query.is_empty() || cols == 0 { return Vec::new(); }
        let needle: Vec<char> = query.to_lowercase().chars().collect();
        let screen = self.terminal.screen();
        let mut out = Vec::new();
        for stable in start..=end {
            let Some(phys) = screen.stable_row_to_phys(stable) else {
                continue;
            };
            let lines = screen.lines_in_phys_range(phys..phys + 1);
            let Some(line) = lines.into_iter().next() else {
                continue;
            };
            // Lowercased char vector + the column each char sits in.
            let mut chars: Vec<char> = Vec::new();
            let mut col_of: Vec<usize> = Vec::new();
            let mut next_col = 0usize;
            for cell in line.visible_cells() {
                let idx = cell.cell_index();
                if idx >= cols {
                    break;
                }
                // Fill the gap of blank cells before this one with spaces.
                while next_col < idx {
                    chars.push(' ');
                    col_of.push(next_col);
                    next_col += 1;
                }
                for ch in cell.str().chars().flat_map(char::to_lowercase) {
                    chars.push(ch);
                    col_of.push(idx);
                }
                next_col = idx + cell.width().max(1);
            }
            if needle.len() > chars.len() {
                continue;
            }
            let mut i = 0;
            while i + needle.len() <= chars.len() {
                if chars[i..i + needle.len()] == needle[..] {
                    out.push(SearchMatch {
                        stable_row: stable,
                        start_col: col_of[i],
                        end_col: col_of[i + needle.len() - 1] + 1,
                    });
                    i += needle.len();
                } else {
                    i += 1;
                }
            }
        }
        out
    }

    pub(in crate::backend::terminal) fn current_seqno(&self) -> usize {
        self.terminal.current_seqno()
    }

    pub(in crate::backend::terminal) fn visible_top_stable_row(&self) -> isize {
        // wezterm's `visible_row_to_stable_row(0)` reports the LIVE viewport top
        // (offset 0). heca scrolls the host viewport by `viewport_offset` rows
        // ABOVE the live bottom, so the displayed top is that many rows older
        // (a smaller stable row index). Subtracting the offset makes
        // `viewport_top_stable_row` track the scrolled content instead of staying
        // pinned to the live screen — which selection overlays and caret
        // auto-scroll depend on (terminal-task-01g).
        self.terminal.screen().visible_row_to_stable_row(0) - self.viewport_offset as isize
    }

    pub(in crate::backend::terminal) fn changed_visible_rows_since(
        &self,
        seqno: usize,
    ) -> Vec<usize> {
        let screen = self.terminal.screen();
        let rows = screen.physical_rows.max(1);
        let top = screen.visible_row_to_stable_row(0);
        let bottom = screen.visible_row_to_stable_row(rows.saturating_sub(1) as i64);
        screen
            .get_changed_stable_rows(top..bottom.saturating_add(1), seqno)
            .into_iter()
            .filter_map(|stable_row| {
                let visible_row = stable_row - top;
                usize::try_from(visible_row).ok().filter(|row| *row < rows)
            })
            .collect()
    }

    #[allow(clippy::type_complexity)]
    pub(super) fn visible_lines(
        &self,
        cols: usize,
        rows: usize,
        palette: &ColorPalette,
        blank_line: &TerminalLine,
        requested_offset: usize,
    ) -> (
        Vec<TerminalLine>,
        usize,
        usize,
        Vec<HyperlinkSpan>,
        Vec<GraphicsPlacement>,
        Vec<TerminalImage>,
    ) {
        let screen = self.terminal.screen();
        let visible_count = rows.min(screen.physical_rows.max(1));
        let total = screen.scrollback_rows();
        let max_offset = total.saturating_sub(visible_count);
        // Defensive read-clamp: `reconcile_viewport_offset` (called at the end of
        // `update`) keeps the stored offset within bounds, so this normally does
        // nothing. It stays as a belt-and-suspenders guarantee that a snapshot is
        // always visually correct even if a read happens outside the update path.
        let offset = requested_offset.min(max_offset);
        // `total` is the bottom of the live viewport; subtract `offset` to walk
        // `offset` rows up into history, then take `visible_count` rows below.
        let visible_end = total.saturating_sub(offset);
        let visible_start = visible_end.saturating_sub(visible_count);
        let mut lines = Vec::with_capacity(rows);
        let mut hyperlinks = Vec::new();
        let mut graphics = GraphicsCollector::default();

        for (row, mut line) in screen
            .lines_in_phys_range(visible_start..visible_end)
            .into_iter()
            .take(rows)
            .enumerate()
        {
            collect_row_hyperlinks(&line, row, cols, &mut hyperlinks);
            if self.image_capture {
                self.collect_row_graphics(&line, row, cols, &mut graphics);
            }
            lines.push(snapshot_line(&mut line, cols, palette, blank_line));
        }

        while lines.len() < rows {
            lines.push(blank_line.clone());
        }

        let (placements, images) = graphics.finish();
        (lines, total, offset, hyperlinks, placements, images)
    }

    /// Collect inline-image placements for one visible row into `out`.
    ///
    /// wezterm attaches an `ImageCell` to each grid cell an image covers
    /// (identically for Sixel / iTerm2 / Kitty). We group those per-cell slices
    /// by `(content hash, placement id, z-index)` into rectangular blocks — see
    /// [`GraphicsCollector`] — and decode each unique source image once.
    pub(super) fn collect_row_graphics(
        &self,
        line: &wezterm_term::Line,
        row: usize,
        cols: usize,
        out: &mut GraphicsCollector,
    ) {
        for cell in line.visible_cells() {
            let col = cell.cell_index();
            if col >= cols {
                break;
            }
            let Some(images) = cell.attrs().images() else {
                continue;
            };
            for image in images {
                let data = image.image_data();
                let hash = data.hash();
                // Decode/register the source image once; skip undecodable ones.
                let Some(decoded) = self.decode_image(data) else {
                    continue;
                };
                out.add_cell(
                    hash,
                    image.placement_id(),
                    image.z_index(),
                    row,
                    col,
                    texcoord(image.top_left()),
                    texcoord(image.bottom_right()),
                    &decoded,
                );
            }
        }
    }

    /// Decode (or fetch from cache) the RGBA pixels for one source image. Returns
    /// `None` for an undecodable payload (already-logged, cached so it is not
    /// retried every frame).
    pub(super) fn decode_image(
        &self,
        data: &Arc<wezterm_term::image::ImageData>,
    ) -> Option<Arc<TerminalImage>> {
        let hash = data.hash();
        if let Some(cached) = self.decoded_images.borrow().get(&hash) {
            return cached.clone();
        }
        let decoded = decode_terminal_image(hash, &data.data()).map(Arc::new);
        self.decoded_images
            .borrow_mut()
            .insert(hash, decoded.clone());
        decoded
    }
}

pub(super) fn snapshot_line(
    line: &mut wezterm_term::Line,
    cols: usize,
    palette: &ColorPalette,
    blank_line: &TerminalLine,
) -> TerminalLine {
    let mut cells = blank_line.cells.clone();
    for (idx, cell) in line.cells_mut().iter().enumerate().take(cols) {
        cells[idx] = terminal_cell(" ", 1, cell.attrs(), palette);
    }
    for cell in line.visible_cells() {
        if cell.cell_index() >= cols {
            continue;
        }

        let rendered = terminal_cell(cell.str(), cell.width(), cell.attrs(), palette);
        let span_end = (cell.cell_index() + rendered.width).min(cols);
        for slot in cells.iter_mut().take(span_end).skip(cell.cell_index()) {
            *slot = blank_with_attrs(&rendered);
        }
        cells[cell.cell_index()] = rendered;
    }

    TerminalLine { cells }
}

/// Collect OSC 8 hyperlink spans for one visible row. Consecutive cells that
/// carry the same link URI are merged into a single span (in cell columns), so a
/// caller can underline / open contiguous links without re-scanning.
pub(super) fn collect_row_hyperlinks(
    line: &wezterm_term::Line,
    row: usize,
    cols: usize,
    out: &mut Vec<HyperlinkSpan>,
) {
    let mut open: Option<HyperlinkSpan> = None;
    for cell in line.visible_cells() {
        let idx = cell.cell_index();
        if idx >= cols {
            break;
        }
        let end = (idx + cell.width().max(1)).min(cols);
        match cell.attrs().hyperlink().map(|link| link.uri()) {
            Some(uri) => match open.as_mut() {
                // Extend the run only if it is the same link and contiguous.
                Some(span) if span.uri == uri && span.end_col == idx => span.end_col = end,
                _ => {
                    if let Some(span) = open.take() {
                        out.push(span);
                    }
                    open = Some(HyperlinkSpan {
                        row,
                        start_col: idx,
                        end_col: end,
                        uri: uri.to_string(),
                    });
                }
            },
            None => {
                if let Some(span) = open.take() {
                    out.push(span);
                }
            }
        }
    }
    if let Some(span) = open.take() {
        out.push(span);
    }
}
