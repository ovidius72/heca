//! **A pane whose header changed shape is built again** — which panes those are, and dropping
//! them so the next reconcile builds them afresh.
//!
//! A column keeps each pane's widget between frames, so a gesture in flight and a picker letter
//! survive. That is right for everything about a pane except what its header is *made of*: which
//! chips have data at all, which buttons there are. A pane starts before its shell has reported a
//! working directory, so it has no location chip; when the directory arrives the header has a
//! different shape and nothing but a rebuild can add the chip, because the words are written by
//! name onto chips that exist (`refresh_pane_header_text`). The floating panes already rebuild on
//! this; the columns did not, so a new pane showed no path until a reload.

use std::collections::HashMap;

use heca_core::layout::PaneId;
use heca_grid_ui::Component;
use heca_grid_ui::widgets::Flex;

/// The panes built with a header of one shape whose header is now another. A pane with nothing
/// recorded was never built, and one with no header now is not asked about.
pub(super) fn stale_panes(
    built: &HashMap<PaneId, String>,
    now: &HashMap<PaneId, String>,
) -> Vec<PaneId> {
    now.iter()
        .filter(|(id, key)| built.get(id).is_some_and(|was| was != *key))
        .map(|(id, _)| *id)
        .collect()
}

/// Take the children standing for `panes` out of the column, so reconciling builds them again.
pub(super) fn drop_panes(root: &mut Flex, panes: &[PaneId]) {
    let doomed: Vec<String> = panes
        .iter()
        .map(|id| crate::chrome::pane_key(*id))
        .collect();
    root.base_mut().children.retain(|child| {
        !child
            .base()
            .key
            .as_ref()
            .is_some_and(|k| doomed.contains(k))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::builders::{ComponentExt, Parent};
    use heca_grid_ui::widgets::Label;

    fn keys(entries: &[(u64, &str)]) -> HashMap<PaneId, String> {
        entries
            .iter()
            .map(|(id, key)| (PaneId(*id), (*key).to_string()))
            .collect()
    }

    /// Only a pane whose header key CHANGED is stale: unchanged, new and gone panes are not.
    #[test]
    fn only_a_pane_whose_header_changed_shape_is_stale() {
        let built = keys(&[(1, "no-path"), (2, "same"), (4, "gone")]);
        let now = keys(&[(1, "with-path"), (2, "same"), (3, "new")]);
        assert_eq!(stale_panes(&built, &now), vec![PaneId(1)]);
    }

    /// A stale pane is built again by the next reconcile, and the others are kept — the same
    /// widget, still holding what it held.
    #[test]
    fn a_stale_pane_is_rebuilt_and_the_others_are_kept() {
        let mut column = Flex::column()
            .child(Label::new("one").key(crate::chrome::pane_key(PaneId(1))))
            .child(Label::new("two").key(crate::chrome::pane_key(PaneId(2))));
        drop_panes(&mut column, &[PaneId(1)]);

        let wanted = vec![
            crate::chrome::pane_key(PaneId(1)),
            crate::chrome::pane_key(PaneId(2)),
        ];
        let mut built = Vec::new();
        heca_grid_ui::reconcile::reconcile_children(&mut column, &wanted, |key| {
            built.push(key.to_string());
            Box::new(Label::new("rebuilt").key(key.to_string()))
        });
        assert_eq!(built, vec![crate::chrome::pane_key(PaneId(1))]);
    }
}
