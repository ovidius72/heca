//! **Suggestions under the caret** — what [`Input::suggest`](super::Input::suggest) adds to a field.
//!
//! Typing the trigger (`@`) at the start of a word opens a list of the matching items under the
//! caret, narrowed by fuzzy search as the word grows (`@bo` keeps `@bob`). While it is open, the
//! list intents drive it, from whatever keys they are bound to: up / down (the arrows, Ctrl+k /
//! Ctrl+j) move through it, `activate` / `complete` (Enter, Tab) put the chosen item's text in
//! place of the word, and `dismiss` (Escape) closes it until the word changes; a click on an item
//! does the same as Enter. The field
//! owns all of it — the trigger, the list, the keys and the replacement — so a caller writes one
//! line and nothing about the caret.

use std::rc::Rc;

use heca_core::layout::Rectangle;

use crate::builders::{EventExt, LayoutExt, Parent, PlaceExt};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::style::{Count, Length, Spacing};
use crate::widgets::{Flex, Overlay, Row};

/// The list's name among the field's parts.
pub(super) const LIST_AREA: &str = "suggestions";

/// One item as the list holds it: the text it puts in the field, and how its row is drawn.
#[derive(Clone)]
pub(super) struct Candidate {
    pub insert: String,
    pub row: Rc<dyn Fn() -> crate::WidgetModel>,
}

impl PartialEq for Candidate {
    /// Two candidates are the same row when they insert the same text — the row keeps its widget
    /// while the list narrows around it.
    fn eq(&self, other: &Self) -> bool {
        self.insert == other.insert
    }
}

/// **The suggestions a field carries**: its trigger, its items, what is being typed after the
/// trigger, and which item is chosen.
pub(super) struct Suggest {
    trigger: char,
    source: Rc<dyn Fn() -> Vec<Candidate>>,
    /// The word after the trigger while the list is open; `None` closes it.
    pub query: Signal<Option<String>>,
    /// Which shown item Enter takes.
    pub selected: Signal<usize>,
    /// Where the open word starts (the trigger's index).
    start: usize,
    /// A word the user closed with Escape: it stays closed until a different word is typed.
    dismissed: Option<usize>,
    /// Where the caret is on screen — what the list hangs from.
    pub caret: Signal<Rectangle>,
    /// The item a click chose, by the text it inserts, waiting for the field to put it in.
    pub picked: Signal<Option<String>>,
}

impl Suggest {
    pub fn new(trigger: char, source: Rc<dyn Fn() -> Vec<Candidate>>) -> Self {
        Self {
            trigger,
            source,
            query: signal(None),
            selected: signal(0),
            start: 0,
            dismissed: None,
            caret: signal(Rectangle::from_size(heca_core::layout::Size::new(0.0, 0.0))),
            picked: signal(None),
        }
    }

    /// **The items shown for `query`**: those whose text matches it, best first, at most `limit`.
    pub fn shown(&self, query: &str) -> Vec<Candidate> {
        matching(&(self.source)(), self.trigger, query, shown_at_most())
    }

    /// **The list part**: a light panel hung from the caret — it leaves the keyboard in the field —
    /// up while something matches the open word, holding the shown items as rows, the chosen one
    /// lit. A click on a row picks it.
    pub fn list(&self) -> Overlay {
        let (source, trigger, query, selected) =
            (self.source.clone(), self.trigger, self.query, self.selected);
        let shown: Rc<dyn Fn() -> Vec<Candidate>> = Rc::new(move || {
            query
                .get()
                .map(|query| matching(&source(), trigger, &query, shown_at_most()))
                .unwrap_or_default()
        });
        let picked = self.picked;
        let up = {
            let shown = shown.clone();
            move || !shown().is_empty()
        };
        let read = {
            let shown = shown.clone();
            move || shown()
        };
        let rows = crate::builders::BoundChildren::new(read, move |candidate: &Candidate| {
            // Lit while it is the chosen one among those shown now.
            let (shown, insert) = (shown.clone(), candidate.insert.clone());
            let chosen = move || {
                shown()
                    .get(selected.get())
                    .is_some_and(|at| at.insert == insert)
            };
            // The chosen row is lit by a fill alone. A click on it names it by the text it inserts.
            let pick = candidate.insert.clone();
            Row::new()
                .nav_selected(chosen)
                .cursor_fill(true)
                .padding_x(Spacing::Sm)
                .on_click(move |_| picked.set(Some(pick.clone())))
                .child((candidate.row)())
        });
        Overlay::new()
            .blocking(false)
            .keyboard(false)
            .anchored_to(self.caret)
            .open(up)
            .panel(
                Flex::column()
                    .padding(Spacing::Xs)
                    // The narrowest it is, said by the theme (`[input] suggestions_width`).
                    .min_width(Length::theme("input.suggestions_width"))
                    .child(rows),
            )
            .area(LIST_AREA)
    }

    /// **Bring the list in line with the caret**: open on a word that starts with the trigger at
    /// the caret, follow what is typed after it, close everywhere else. `chars` is the field's
    /// text, `caret` where the caret is, `selecting` whether a selection is up.
    pub fn follow(&mut self, chars: &[char], caret: usize, selecting: bool) {
        let word = (!selecting)
            .then(|| word_at(chars, caret, self.trigger))
            .flatten();
        match word {
            Some((start, query)) if self.dismissed != Some(start) => {
                if self.query.get_untracked().as_deref() != Some(query.as_str()) {
                    self.selected.set(0);
                    self.query.set(Some(query));
                }
                self.start = start;
            }
            other => {
                if other.is_none() {
                    self.dismissed = None;
                }
                if self.query.get_untracked().is_some() {
                    self.query.set(None);
                }
            }
        }
    }

    /// Whether the list is open with something in it.
    pub fn is_open(&self) -> bool {
        self.query
            .get_untracked()
            .is_some_and(|query| !self.shown(&query).is_empty())
    }

    /// Move the choice one up or down, going round at either end.
    pub fn step(&self, down: bool) {
        let Some(query) = self.query.get_untracked() else {
            return;
        };
        let count = self.shown(&query).len();
        if count == 0 {
            return;
        }
        let at = self.selected.get_untracked().min(count - 1);
        let next = if down {
            (at + 1) % count
        } else {
            (at + count - 1) % count
        };
        self.selected.set(next);
    }

    /// Close the list on this word until a different one is typed.
    pub fn dismiss(&mut self) {
        self.dismissed = Some(self.start);
        self.query.set(None);
    }

    /// **The replacement for item `index`** among those shown: the range the open word covers (the
    /// trigger to the caret) and the text to put there. `None` when nothing is open at that index.
    pub fn replacement(
        &self,
        index: usize,
        caret: usize,
    ) -> Option<(std::ops::Range<usize>, String)> {
        let query = self.query.get_untracked()?;
        let chosen = self.shown(&query).into_iter().nth(index)?;
        Some((self.start..caret, chosen.insert))
    }

    /// Where the item that inserts `insert` is among those shown — what a click on its row picks.
    pub fn index_of(&self, insert: &str) -> Option<usize> {
        let query = self.query.get_untracked()?;
        self.shown(&query)
            .iter()
            .position(|candidate| candidate.insert == insert)
    }
}

/// **The word at the caret, when it opens the list** — it starts with `trigger`, right at the
/// start of the text or after whitespace, and runs to the caret with no whitespace in it. Gives
/// where the trigger is and what follows it.
pub(super) fn word_at(chars: &[char], caret: usize, trigger: char) -> Option<(usize, String)> {
    let caret = caret.min(chars.len());
    let start = chars[..caret]
        .iter()
        .rposition(|c| c.is_whitespace())
        .map_or(0, |space| space + 1);
    // The trigger must be before the caret: a caret just ahead of it has no word yet.
    (start < caret && chars.get(start) == Some(&trigger))
        .then(|| (start, chars[start + 1..caret].iter().collect()))
}

/// **How many items the list shows at most** — the theme's (`[input] suggestions`), at least one.
fn shown_at_most() -> usize {
    Count::theme("input.suggestions").get().unwrap_or(1).max(1)
}

/// **The items matching `query`**, best first, at most `limit`: an item matches when the query is
/// a fuzzy subsequence of its text (the trigger left off), with case only counting when the query
/// has a capital letter.
pub(super) fn matching(
    items: &[Candidate],
    trigger: char,
    query: &str,
    limit: usize,
) -> Vec<Candidate> {
    let case = crate::search::smart_case(query);
    let mut scored: Vec<(i32, usize, &Candidate)> = items
        .iter()
        .enumerate()
        .filter_map(|(at, item)| {
            let text = item.insert.trim().trim_start_matches(trigger);
            crate::search::fuzzy(query, text, case).map(|found| (found.score, at, item))
        })
        .collect();
    // Best score first; equal scores keep the order the items were given in.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, _, item)| item.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    fn candidates(names: &[&str]) -> Vec<Candidate> {
        names
            .iter()
            .map(|name| Candidate {
                insert: format!("@{name} "),
                row: Rc::new(|| crate::WidgetModel::new(Flex::row())),
            })
            .collect()
    }

    #[test]
    fn a_word_opens_the_list_only_when_it_starts_with_the_trigger() {
        assert_eq!(word_at(&chars("hi @bo"), 6, '@'), Some((3, "bo".into())));
        assert_eq!(word_at(&chars("@"), 1, '@'), Some((0, String::new())));
        assert_eq!(
            word_at(&chars("mail@bo"), 7, '@'),
            None,
            "inside a word: an address"
        );
        assert_eq!(
            word_at(&chars("@bob done"), 9, '@'),
            None,
            "the caret left the word"
        );
        assert_eq!(
            word_at(&chars("hi @bob"), 3, '@'),
            None,
            "the caret before the trigger"
        );
    }

    #[test]
    fn items_match_by_fuzzy_search_best_first_and_up_to_the_limit() {
        let items = candidates(&["bob", "carol", "bobby", "all"]);
        let names = |found: Vec<Candidate>| -> Vec<String> {
            found.into_iter().map(|c| c.insert).collect()
        };
        assert_eq!(names(matching(&items, '@', "bob", 8)), ["@bob ", "@bobby "]);
        assert_eq!(
            names(matching(&items, '@', "", 2)),
            ["@bob ", "@carol "],
            "empty: all, given order, capped"
        );
        assert!(matching(&items, '@', "zz", 8).is_empty());
    }
}
