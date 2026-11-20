//! Filtering, selection and query history through SearchModel.

use super::*;

impl CommandPalette {
    /// The current filtered + ranked results — **entirely the model's answer**, through the pure
    /// [`filter::rank`]. Only the active mode's commands are candidates, and each comes back under
    /// its index in the **whole** list: the rows, their signals and the live titles are all keyed by
    /// position in `commands`, so filtering must not renumber them.
    pub(super) fn results(&self) -> Vec<Ranked> {
        let (_, scope, query) = self.active();
        // Read each row's **live** text, not the label it was built with: a host may be following a
        // pane's process or a rename, and a list that shows one name while matching another is
        // worse than a stale one. The labels are short and there are a few hundred at most, so
        // materialising them per keystroke costs nothing measurable.
        let of_mode: Vec<usize> = (0..self.commands.len())
            .filter(|&i| self.in_mode(&self.commands[i], &scope))
            .collect();
        let titles: Vec<String> = of_mode.iter().map(|&i| self.title_text[i].get_untracked()).collect();
        // The description is matched too, but only in a mode that said so
        // ([`match_description_in`](Self::match_description_in)).
        let describes = self.describe_scopes.contains(&scope);
        let candidates: Vec<filter::Candidate<'_>> = of_mode
            .iter()
            .zip(&titles)
            .map(|(&index, title)| {
                let command = &self.commands[index];
                filter::Candidate {
                    index,
                    id: command.id.as_deref(),
                    text: command.completion.as_deref().unwrap_or(title),
                    secondary: command.description.as_deref().filter(|_| describes),
                    group: command.group,
                    fallback: command.fallback,
                    marks_title: command.completion.as_ref().is_none_or(|text| text == title),
                }
            })
            .collect();
        filter::rank(&self.search.with_scope(scope), &candidates, &query)
    }

    /// Move the selection down (Ctrl+J / ↓), keeping it on-screen.
    pub fn select_next(&mut self) {
        self.sync_query();
        let n = self.results().len();
        if n > 0 {
            self.move_to((self.selected + 1).min(n - 1), true);
        }
    }

    /// Move the selection up (Ctrl+K / ↑).
    pub fn select_prev(&mut self) {
        self.sync_query();
        self.move_to(self.selected.saturating_sub(1), true);
    }

    /// Move the selection by one **page**: as many rows as the list shows whole right now (at least
    /// one). The count is read from the laid-out rows, so a list of tall rows pages by fewer.
    pub fn select_page(&mut self, down: bool) {
        self.sync_query();
        let n = self.results().len();
        if n == 0 {
            return;
        }
        let list = self.part_mut(super::build::LIST).base().bounds;
        let page = self
            .list_mut()
            .children
            .iter()
            .map(|row| row.base().bounds)
            .filter(|row| {
                row.size.h > 0.0 && row.loc.y >= list.loc.y - 0.5 && row.loc.y + row.size.h <= list.loc.y + list.size.h + 0.5
            })
            .count()
            .max(1);
        let at = if down { (self.selected + page).min(n - 1) } else { self.selected.saturating_sub(page) };
        self.move_to(at, true);
    }

    /// Move the selection to the first (`last = false`) or last row.
    pub fn select_edge(&mut self, last: bool) {
        self.sync_query();
        let n = self.results().len();
        if n > 0 {
            self.move_to(if last { n - 1 } else { 0 }, true);
        }
    }

    /// Put the cursor on result `at`. `by_keyboard` says who moved it, because only the keyboard's
    /// cursor is scrolled into view — see [`keyboard_moved`](CommandPalette::keyboard_moved).
    pub(super) fn move_to(&mut self, at: usize, by_keyboard: bool) {
        self.selected = at;
        self.keyboard_moved.set(by_keyboard);
        self.sync_rows();
        // The selected description reflows, which changes the layout.
        self.base.mark_needs_layout();
    }

    /// Act on what the rows' handlers noticed since the last event: a hover selects that row, a
    /// press chooses it. Run at the start of the next event and every tick.
    pub(super) fn take_pointer(&mut self) {
        let position = |palette: &Self, command: usize| {
            palette.results().iter().position(|ranked| ranked.index == command)
        };
        if let Some(command) = self.pointer.hover.get_untracked() {
            self.pointer.hover.set(None);
            if let Some(at) = position(self, command)
                && at != self.selected
            {
                self.move_to(at, false);
            }
        }
        if let Some(command) = self.pointer.press.get_untracked() {
            self.pointer.press.set(None);
            if let Some(at) = position(self, command) {
                self.selected = at;
                self.run_selected();
            }
        }
    }

    /// Run the selected command (fires its callback) and close.
    pub fn run_selected(&mut self) {
        self.sync_query();
        let results = self.results();
        if let Some(m) = results.get(self.selected) {
            let cmd = &self.commands[m.index];
            // The query as typed and the command's identity — remembered only on a **run**. An
            // abandoned search is not a search anyone wants back, so `close()` records nothing.
            // **Without the sigil, into the active mode's scope**: the sigil selected which memory
            // this is, so storing it inside that memory would put it back in every recalled query.
            let (_, scope, query) = self.active();
            let id = cmd.id.clone();
            (cmd.on_run)();
            self.search
                .with_scope(scope)
                .record_run(&query, id.as_deref());
        }
        self.close();
    }

    /// Bring every row in line with the query and the cursor: the rows the query left are shown, in
    /// rank order, and the rest are out of the layout; the matches are marked; the cursor row is lit
    /// and its description reflows while every other description is cut to one line.
    ///
    /// All of it is signals and layout properties on the rows — nothing is measured or placed here.
    /// Cheap and idempotent, so it runs wherever the answer could have changed.
    pub(super) fn sync_rows(&mut self) {
        let results = self.results();
        let mut place: Vec<Option<usize>> = vec![None; self.commands.len()];
        for (at, ranked) in results.iter().enumerate() {
            place[ranked.index] = Some(at);
        }
        for (row, at) in self.list_mut().children.iter_mut().zip(&place) {
            let base = row.base_mut();
            base.set_hidden(at.is_none());
            base.style.layout.order = at.map(|at| (at as i64).into());
        }
        let cursor = results.get(self.selected).map(|ranked| ranked.index);
        for (index, row) in self.rows.iter().enumerate() {
            row.look(Some(index) == cursor);
            if let Some(wrap) = row.desc_wrap {
                wrap.set(Some(index) == cursor);
            }
            row.title_marks.set(Vec::new());
            if let Some(marks) = row.desc_marks {
                marks.set(Vec::new());
            }
        }
        for ranked in &results {
            self.rows[ranked.index].title_marks.set(ranked.hits.clone());
            if let Some(marks) = self.rows[ranked.index].desc_marks {
                marks.set(ranked.secondary_hits.clone());
            }
        }
    }

    /// Put the selection on the row that asked for it — **only while nothing is typed**.
    ///
    /// A query is better context than any default, so the first keystroke hands the lead back to
    /// the best match (`on_query_changed` resets to 0 and never calls this). Which makes it an
    /// open-time rule, the same shape as [`Command::group`]'s ordering.
    ///
    /// It runs after the results are known rather than over `commands`, because the row's *position*
    /// is what a selection is, and that position comes from the ranking.
    pub(super) fn sync_initial_selection(&mut self) {
        let (_, _, query) = self.active();
        if !query.is_empty() {
            return;
        }
        let results = self.results();
        let Some(at) = results
            .iter()
            .position(|m| self.commands[m.index].preselect)
        else {
            return;
        };
        self.selected = at;
        // The preselected row can rank anywhere, so the list opens scrolled to reach it.
        self.keyboard_moved.set(true);
    }

    pub(super) fn close(&mut self) {
        *self.entered.borrow_mut() = None;
        self.mode_enter.iter().for_each(|enter| enter.announced.set(false));
        self.named_prefill = None;
        self.open.set(false);
        self.query.set(String::new());
        self.selected = 0;
        self.sync_rows();
    }

    /// The user typed: reset the filter/selection, move the marks with it, and **leave the history
    /// walk** — the field is theirs again.
    ///
    /// It ends the walk in **every** mode, not just the active one: there is a single field, and
    /// typing a sigil switches mode with a keystroke — leaving the mode just left parked mid-walk
    /// would make its next `Ctrl+p` resume from the middle of its history.
    pub(super) fn on_query_changed(&mut self) {
        for scope in self.scopes() {
            self.search.with_scope(scope).query_changed();
        }
        self.on_query_replaced();
    }

    /// The query changed without the user typing it — a history recall. Everything
    /// [`on_query_changed`](Self::on_query_changed) does **except** ending the walk: resetting the
    /// cursor here would make a second step back start again from the newest entry, so walking the
    /// history would be impossible past its first step.
    pub(super) fn on_query_replaced(&mut self) {
        self.refresh_source();
        self.named_prefill = None;
        self.selected = 0;
        self.keyboard_moved.set(true);
        self.sync_rows();
        self.announce_mode();
    }

}
