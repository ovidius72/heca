//! Filtering, selection and query history through SearchModel.

use super::*;

impl CommandPalette {
    /// The current filtered + ranked results — **entirely the model's answer**.
    ///
    /// Matching, smart-case and the ranking by past use all live in
    /// [`search`](crate::search); this reads the order back. The one thing decided here is the
    /// block ordering, and only while nothing is typed: with an empty query every match scores the
    /// same and the caller's groups are the only context there is, but a typed query is better
    /// context than a group and dissolves it.
    pub(super) fn results(&self) -> Vec<Ranked> {
        let (_, scope, query) = self.active();
        // Only the active mode's commands are ranked, and the ids that come back are mapped to
        // indices into the **whole** list — the children, the marks and the measured heights are all
        // keyed by position in `commands`, so the filter must not renumber them.
        let of_mode: Vec<usize> = self
            .commands
            .iter()
            .enumerate()
            .filter(|(_, c)| self.in_mode(c, &scope))
            .map(|(i, _)| i)
            .collect();
        // Read each row's **live** text, not the label it was built with: a host may be following a
        // pane's process or a rename, and a list that shows one name while matching another is
        // worse than a stale one. The labels are short and there are a few hundred at most, so
        // materialising them per keystroke costs nothing measurable.
        let items: Vec<(Option<&str>, String)> = of_mode
            .iter()
            .map(|&i| {
                (
                    self.commands[i].id.as_deref(),
                    self.title_text[i].get_untracked(),
                )
            })
            .collect();
        let mut out = self
            .search
            .with_scope(scope)
            .rank(&items, &query, |(id, text)| (*id, text.as_str()));
        for r in &mut out {
            r.index = of_mode[r.index];
        }
        if query.is_empty() {
            // Stable, so the ranking inside each block survives.
            out.sort_by_key(|r| self.commands[r.index].group);
        }
        out
    }

    /// Move the selection down (Ctrl+J / ↓), keeping it on-screen.
    pub fn select_next(&mut self) {
        let n = self.results().len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + 1).min(n - 1);
        self.follow_selection();
        self.sync_wrap();
    }

    /// Move the selection up (Ctrl+K / ↑).
    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.follow_selection();
        self.sync_wrap();
    }

    /// Run the selected command (fires its callback) and close.
    pub fn run_selected(&mut self) {
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

    /// Reflow the selected description and cut every other row's description.
    pub(super) fn sync_wrap(&mut self) {
        let selected = self.results().get(self.selected).map(|m| m.index);
        for (i, wrap) in self.desc_wrap.iter().enumerate() {
            if let Some(wrap) = wrap {
                wrap.set(Some(i) == selected);
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
        // The preselected row can rank anywhere, so the list may have to open scrolled to reach it.
        self.follow_selection();
    }

    /// Move the match marks onto the title labels that matched.
    pub(super) fn sync_marks(&mut self) {
        for signal in &self.title_marks {
            signal.set(Vec::new());
        }
        for m in self.results() {
            self.title_marks[m.index].set(m.hits.clone());
        }
    }

    pub(super) fn close(&mut self) {
        self.named_prefill = None;
        self.open.set(false);
        self.query.borrow_mut().set_value("");
        self.selected = 0;
        self.scroll = 0;
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
        self.named_prefill = None;
        self.selected = 0;
        self.scroll = 0;
        self.sync_marks();
        self.sync_wrap();
    }

}
