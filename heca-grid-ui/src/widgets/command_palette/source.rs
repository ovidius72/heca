//! Query-driven rows reuse the palette's input, matcher, layout and activation.
use super::*;
pub(super) type CandidateSource = Box<dyn Fn(&str) -> Vec<Command>>;

impl CommandPalette {
    /// Replace the query with the selected suggestion, keeping the palette open.
    pub fn complete_selected(&mut self) -> Handled {
        self.sync_query();
        let Some(text) = self.results().get(self.selected)
            .and_then(|row| self.commands[row.index].completion.clone()) else { return Handled::No; };
        self.query.set(text);
        self.on_query_changed();
        self.base.mark_needs_layout();
        Handled::Yes
    }

    /// Supply live candidates for the current query, for example directory completions.
    /// Only row children are replaced; the query field keeps its focus and editing state.
    #[heca_grid_ui_macros::host_only("a live candidate callback, not a scalar")]
    pub fn source(mut self, source: impl Fn(&str) -> Vec<Command> + 'static) -> Self {
        self.source = Some(Box::new(source));
        self.refresh_source();
        self
    }

    pub(super) fn refresh_source(&mut self) {
        let Some(source) = &self.source else {
            return;
        };
        let rows = source(&self.query_text());
        let input = self.base.children.pop();
        self.base.children.clear();
        self.commands.clear();
        self.title_marks.clear();
        self.title_text.clear();
        self.desc_wrap.clear();
        self.desc_marks.clear();
        self.natural_h.clear();
        if let Some(input) = input {
            self.base.children.push(input);
        }
        for row in rows {
            self.push_command(row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    /// The query is marked **where it matched**: in the title, in the description, or both — and a
    /// description is marked only in a mode that matches it.
    #[test]
    fn marks_follow_whichever_text_matched() {
        let build = |describe: bool| {
            let p = CommandPalette::new()
                .mode_with_commands('#', "session", [
                    Command::new("billing", || {}).description("~/work/billing"),
                    Command::new("api", || {}).description("~/work/billing"),
                ]);
            let p = if describe { p.match_description_in("session") } else { p };
            p.query("bill").default_open(true)
        };
        let marks = |p: &CommandPalette, row: usize| {
            (
                p.title_marks[row].get_untracked(),
                p.desc_marks[row].map(|m| m.get_untracked()).unwrap_or_default(),
            )
        };

        let p = build(true);
        let (title, description) = marks(&p, 0);
        assert!(!title.is_empty() && !description.is_empty(), "matched on both: both marked");
        let (title, description) = marks(&p, 1);
        assert!(title.is_empty(), "`api` has no `bill`");
        assert_eq!(description, [7, 8, 9, 10], "the folder's own characters, not the title's");

        let p = build(false);
        assert!(marks(&p, 0).1.is_empty(), "a mode that did not opt in marks no description");
        assert_eq!(p.results().len(), 1, "and finds only by title");
    }

    #[test]
    fn live_rows_follow_query_edits_and_activate_the_new_candidate() {
        let chosen = Rc::new(RefCell::new(String::new()));
        let result = chosen.clone();
        let mut palette = CommandPalette::new()
            .source(move |query| {
                let result = result.clone();
                let query = query.to_owned();
                vec![Command::new(format!("path/{query}"), move || {
                    *result.borrow_mut() = query.clone()
                })]
            })
            .default_open(true);
        crate::dispatch(&mut palette, &Event::TextInput("new".into()));
        palette.sync_query();
        assert_eq!(palette.query_text(), "new");
        assert_eq!(palette.label_signals()[0].get_untracked(), "path/new");
        crate::dispatch(&mut palette, &Event::Widget(WidgetIntent::Activate));
        assert_eq!(*chosen.borrow(), "new");
    }

    #[test]
    fn tab_completes_a_decorated_candidate_without_running_or_closing() {
        use crate::{GridKey, Keymap, Modifiers};
        let chosen = Rc::new(RefCell::new(false));
        let result = chosen.clone();
        let mut palette = CommandPalette::new().source(move |_| {
            let result = result.clone();
            vec![Command::new("Create folder: project", move || *result.borrow_mut() = true)
                .completion("project/")]
        }).default_open(true);
        crate::dispatch(&mut palette, &Event::TextInput("pro".into()));
        assert_eq!(Keymap::with_defaults().dispatch(GridKey::Tab, Modifiers::default(),
            |event| crate::dispatch(&mut palette, event)), Handled::Yes);
        assert_eq!(palette.query_text(), "project/");
        assert!(palette.is_open());
        assert!(!*chosen.borrow());
        assert_eq!(palette.results().len(), 1, "the trailing slash must not hide the creation row");
        crate::dispatch(&mut palette, &Event::Widget(WidgetIntent::Activate));
        assert!(*chosen.borrow());
    }

    #[test]
    fn completion_does_not_intercept_an_ordinary_palettes_focus_traversal() {
        let mut palette = CommandPalette::new().command(Command::new("Run", || {})).default_open(true);
        assert_eq!(crate::dispatch(&mut palette, &Event::Widget(WidgetIntent::Complete)), Handled::No);
        assert!(palette.is_open());
    }

    #[test]
    fn generic_component_close_updates_the_palettes_one_open_state() {
        let mut palette = CommandPalette::new().default_open(true);
        Component::close(&mut palette);
        assert!(!palette.open_signal().get_untracked());
        assert!(!palette.overlay_active());
    }
}
