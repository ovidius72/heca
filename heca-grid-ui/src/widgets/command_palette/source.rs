//! Query-driven rows reuse the palette's input, matcher, layout and activation.
use super::*;
pub(super) type CandidateSource = Box<dyn Fn(&str) -> Vec<Command>>;

impl CommandPalette {
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
}
