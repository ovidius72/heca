//! Which commands a query leaves, and in what order — a pure function over plain items.
//!
//! No widget, no tree: a list of [`Candidate`]s, a query and a [`SearchModel`] go in and the ranked
//! result comes out, so the whole rule can be tested with nothing built. The palette gathers its
//! candidates (the live title, the mode's commands) and applies the answer to its rows; it owns
//! neither the matching (the `SearchModel`'s) nor this ordering.

use crate::search::{Ranked, SearchModel};

/// One command, reduced to what filtering needs.
pub(super) struct Candidate<'a> {
    /// Position in the palette's whole command list. Copied into [`Ranked::index`] on the way out,
    /// so a row keeps its identity however the list is ranked.
    pub(super) index: usize,
    /// Stable identity for ranking by past use, when the command has one.
    pub(super) id: Option<&'a str>,
    /// What the query is matched against: the completion text when there is one, else the title.
    pub(super) text: &'a str,
    /// A second text the query also matches — only in a mode that asked for it.
    pub(super) secondary: Option<&'a str>,
    /// The block this command leads in while nothing is typed — lower first.
    pub(super) group: u16,
    /// Offered after the matching suggestions.
    pub(super) fallback: bool,
    /// Whether the match positions are positions in the **title that is shown**. A command matched
    /// on completion text that differs from its title has nothing to mark.
    pub(super) marks_title: bool,
}

/// The candidates `query` leaves, best first.
///
/// The model decides the match and its order (score plus past use). Two things are decided here and
/// nowhere else: while the query is **empty** the caller's groups lead, because with nothing typed
/// every match scores the same and the groups are the only context there is — a typed query is
/// better context and dissolves them — and **fallback** rows always come last, whatever matched.
pub(super) fn rank(model: &SearchModel, candidates: &[Candidate<'_>], query: &str) -> Vec<Ranked> {
    let mut out: Vec<(Ranked, &Candidate<'_>)> = model
        .rank_with_secondary(candidates, query, |c| (c.id, c.text, c.secondary))
        .into_iter()
        .map(|mut ranked| {
            let candidate = &candidates[ranked.index];
            if !candidate.marks_title {
                ranked.hits.clear();
            }
            ranked.index = candidate.index;
            (ranked, candidate)
        })
        .collect();
    if query.is_empty() {
        // Stable, so the ranking inside each block survives.
        out.sort_by_key(|(_, candidate)| candidate.group);
    }
    out.sort_by_key(|(_, candidate)| candidate.fallback);
    out.into_iter().map(|(ranked, _)| ranked).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(index: usize, text: &str) -> Candidate<'_> {
        Candidate {
            index,
            id: None,
            text,
            secondary: None,
            group: 0,
            fallback: false,
            marks_title: true,
        }
    }

    fn order(ranked: &[Ranked]) -> Vec<usize> {
        ranked.iter().map(|r| r.index).collect()
    }

    #[test]
    fn a_query_keeps_only_what_matches_and_reports_where() {
        let candidates = [
            plain(0, "Split pane"),
            plain(1, "Close pane"),
            plain(2, "Toggle sidebar"),
        ];
        let ranked = rank(&SearchModel::detached("t"), &candidates, "clo");
        assert_eq!(order(&ranked), [1]);
        assert_eq!(
            ranked[0].hits,
            [0, 1, 2],
            "the characters of `Close` that matched"
        );
    }

    #[test]
    fn rows_come_back_under_their_own_index_not_their_place_in_the_candidate_list() {
        // The palette ranks only the active mode's commands, so the candidate list is a subset: the
        // answer must name commands by their place in the WHOLE list.
        let candidates = [plain(4, "alpha"), plain(9, "alpine")];
        let ranked = rank(&SearchModel::detached("t"), &candidates, "alp");
        assert!(
            order(&ranked).iter().all(|i| [4, 9].contains(i)),
            "{ranked:?}"
        );
        assert_eq!(ranked.len(), 2);
    }

    #[test]
    fn groups_lead_only_while_nothing_is_typed() {
        let mut late = plain(0, "build");
        late.group = 2;
        let mut early = plain(1, "bundle");
        early.group = 1;
        let candidates = [late, early];
        let empty = rank(&SearchModel::detached("t"), &candidates, "");
        assert_eq!(order(&empty), [1, 0], "browsing: the lower group leads");
        let typed = rank(&SearchModel::detached("t"), &candidates, "bu");
        let ungrouped = rank(
            &SearchModel::detached("t"),
            &[plain(0, "build"), plain(1, "bundle")],
            "bu",
        );
        assert_eq!(
            order(&typed),
            order(&ungrouped),
            "typing: the match decides, the groups dissolve"
        );
    }

    #[test]
    fn a_fallback_row_is_last_even_when_it_matches_best() {
        let mut create = plain(0, "create folder");
        create.fallback = true;
        let candidates = [create, plain(1, "create"), plain(2, "recreate")];
        for query in ["", "create"] {
            let ranked = rank(&SearchModel::detached("t"), &candidates, query);
            assert_eq!(
                ranked.last().map(|r| r.index),
                Some(0),
                "query {query:?}: {ranked:?}"
            );
        }
    }

    #[test]
    fn a_command_matched_on_its_completion_marks_nothing_in_a_different_title() {
        let mut path = plain(0, "Create folder: project");
        path.text = "project/";
        path.marks_title = false;
        let ranked = rank(&SearchModel::detached("t"), &[path], "proj");
        assert_eq!(order(&ranked), [0]);
        assert!(
            ranked[0].hits.is_empty(),
            "the offsets belong to `project/`, not to the title"
        );
    }

    #[test]
    fn the_description_matches_only_when_it_is_supplied() {
        let mut api = plain(0, "api");
        api.secondary = Some("~/work/billing");
        let with = rank(&SearchModel::detached("t"), &[api], "bill");
        assert_eq!(order(&with), [0]);
        assert_eq!(with[0].secondary_hits, [7, 8, 9, 10]);
        let without = rank(&SearchModel::detached("t"), &[plain(0, "api")], "bill");
        assert!(without.is_empty());
    }
}
