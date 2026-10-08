//! **Keep the children that are still there; build the ones that are new.**
//!
//! A host whose children come from state rebuilds that list every frame: panes in a column,
//! cards in a stack, rows in a list. Rebuilding the widgets with it throws away everything the
//! widgets were holding — a hint letter written a keystroke ago, an animation mid-play, a
//! scroll offset, focus — and a tree rebuilt every frame is also the app pinned at 100% CPU.
//!
//! So the list is data and the children are matched against it **by the name each one answers to**
//! inside this parent — [`nav::child_name`](crate::nav::child_name): what it declared about itself,
//! or, when it declared nothing, a name derived from what it holds. Nothing new to learn and
//! nothing new to write: a widget that already has a name here keeps it.
//!
//! ⚠️ **The name is the child's own, not its full path.** `identity_of` prefixes the scope its
//! ancestors declared (`col:3/pane:7`), because a letter has to be unique across the screen. A
//! parent matching up its own children knows only the names it asked for, so comparing against the
//! path name matched nothing and rebuilt every child on every frame.
//!
//! ⚠️ **Never gate on `key`.** It is optional by design, and matching on it alone would destroy and
//! rebuild every widget that did not happen to declare one — which is the defect this exists to
//! prevent, arriving through the door it came in by.
//!
//! Written here rather than in each host because it was already written once by hand in
//! `ToastStack`, and wanted again the moment a column had to hold its panes.

use crate::component::{Base, Component};
use crate::nav::child_name;

/// Reconcile `parent`'s children against `wanted`, in `wanted`'s order.
///
/// A child whose identity is still in `wanted` is **kept as it is** — the same widget, still
/// holding its signals, its letter and whatever it was animating. An identity with no child yet is
/// built with `build`. A child nobody asked for is dropped. Afterwards the children are in
/// `wanted`'s order.
///
/// The name is [`nav::child_name`](crate::nav::child_name) — whatever that child answers to inside
/// this parent: a declared `key` or `scope_key`, read through any wrapper around it, and otherwise
/// derived from its content. A caller does not have to key anything for this to work.
///
/// Returns `true` when anything was added, removed or moved, so a caller can ask for the layout
/// pass those need and skip it when the frame changed nothing.
pub fn reconcile_children(
    parent: &mut dyn Component,
    wanted: &[String],
    build: impl FnMut(&str) -> Box<dyn Component>,
) -> bool {
    // **Ask who each child is before moving any of them.** A derived identity is counted among the
    // siblings that share its name, so it is only meaningful while the tree still stands as it was.
    let names: Vec<Option<String>> = (0..parent.base().children.len())
        .map(|i| child_name(parent, i))
        .collect();
    let wanted: Vec<(String, Option<String>)> =
        wanted.iter().map(|name| (name.clone(), None)).collect();
    reconcile_named(parent.base_mut(), names, &wanted, build)
}

/// [`reconcile_children`] for a widget that has only its own [`Base`] — a props handler places its
/// own children from what it is handed (see [`props`](crate::props)) — and for children that are
/// rebuilt when **what they were built from** changes.
///
/// Each wanted child is a `(name, version)`: a child that answers to the name and was built from
/// the same version is kept as it is; one built from another version is dropped and built afresh.
/// So a widget whose children come from a summary (a column from its model's key) keeps no table of
/// what each was built from — the child carries it. Children are matched by the `key` they
/// declared, so every one must declare it: it is what a widget that decides its own children writes
/// anyway, and one that did not is rebuilt on every pass.
pub fn reconcile_keyed(
    base: &mut Base,
    wanted: &[(String, String)],
    mut build: impl FnMut(&str) -> Box<dyn Component>,
) -> bool {
    let names = base
        .children
        .iter()
        .map(|c| c.base().identity().map(str::to_string))
        .collect();
    let wanted: Vec<(String, Option<String>)> = wanted
        .iter()
        .map(|(name, version)| (name.clone(), Some(version.clone())))
        .collect();
    reconcile_named(base, names, &wanted, |name| {
        let mut child = build(name);
        child.base_mut().built_from = wanted
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, version)| version.clone());
        child
    })
}

/// The matching itself, given the name each current child answers to. A wanted entry that carries a
/// version only matches a child built from that version.
fn reconcile_named(
    parent: &mut Base,
    names: Vec<Option<String>>,
    wanted: &[(String, Option<String>)],
    mut build: impl FnMut(&str) -> Box<dyn Component>,
) -> bool {
    let versions: Vec<Option<String>> = parent
        .children
        .iter()
        .map(|c| c.base().built_from.clone())
        .collect();
    // Take the children out so each can be put back at most once — `Option` makes that true by
    // construction rather than by care.
    let mut held: Vec<Option<Box<dyn Component>>> = parent.children.drain(..).map(Some).collect();
    let mut changed = held.len() != wanted.len();

    for (at, (name, version)) in wanted.iter().enumerate() {
        let found = held.iter().enumerate().position(|(i, c)| {
            c.is_some()
                && names[i].as_deref() == Some(name.as_str())
                && version
                    .as_ref()
                    .is_none_or(|v| versions[i].as_ref() == Some(v))
        });
        match found {
            Some(i) => {
                changed |= i != at;
                // `position` only yields a live child, so there is always one to put back.
                if let Some(child) = held[i].take() {
                    parent.children.push(child);
                }
            }
            None => {
                parent.children.push(build(name));
                changed = true;
            }
        }
    }
    // Whatever is left was not asked for. Dropping it here, after the wanted ones are back, means a
    // child that merely MOVED is never destroyed on the way.
    changed |= held.iter().any(|c| c.is_some());
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::{ComponentExt, Parent};
    use crate::widgets::Flex;

    fn build(key: &str) -> Box<dyn Component> {
        Box::new(Flex::column().key(key))
    }

    fn keys(parent: &dyn Component) -> Vec<String> {
        parent
            .base()
            .children
            .iter()
            .map(|c| c.base().key.clone().unwrap_or_default())
            .collect()
    }

    fn with(wanted: &[&str]) -> Flex {
        let mut parent = Flex::column();
        let w: Vec<String> = wanted.iter().map(|s| s.to_string()).collect();
        reconcile_children(&mut parent, &w, build);
        parent
    }

    /// **A widget with only its base reconciles its own keyed children**, keeping the ones still
    /// wanted exactly as they are.
    #[test]
    fn a_widget_reconciles_its_own_keyed_children_from_its_base_alone() {
        let wanted = |names: &[&str]| -> Vec<(String, String)> {
            names
                .iter()
                .map(|s| (s.to_string(), "v1".to_string()))
                .collect()
        };
        let mut parent = Flex::column();
        reconcile_keyed(parent.base_mut(), &wanted(&["a", "b"]), build);
        parent.base_mut().children[0].base_mut().set_hidden(true);
        let changed = reconcile_keyed(parent.base_mut(), &wanted(&["a", "c"]), build);
        assert!(changed);
        assert_eq!(keys(&parent), ["a", "c"]);
        assert!(
            parent.base().children[0].base().style.layout.hidden,
            "a was kept, not rebuilt"
        );
    }

    /// **A child built from another version is rebuilt; one built from the same version is kept** —
    /// the child carries what it was built from, so its parent keeps no table of it.
    #[test]
    fn a_keyed_child_is_rebuilt_only_when_its_version_changed() {
        let want = |a: &str, b: &str| -> Vec<(String, String)> {
            vec![("a".into(), a.into()), ("b".into(), b.into())]
        };
        let mut parent = Flex::column();
        reconcile_keyed(parent.base_mut(), &want("1", "1"), build);
        // Marks only these instances carry.
        for child in parent.base_mut().children.iter_mut() {
            child.base_mut().set_hidden(true);
        }

        let changed = reconcile_keyed(parent.base_mut(), &want("1", "2"), build);
        assert!(changed);
        let hidden = |i: usize| parent.base().children[i].base().style.layout.hidden;
        assert!(hidden(0), "a, same version, is the same widget");
        assert!(!hidden(1), "b, new version, was built afresh");
        assert_eq!(
            parent.base().children[1].base().built_from.as_deref(),
            Some("2")
        );

        assert!(
            !reconcile_keyed(parent.base_mut(), &want("1", "2"), build),
            "nothing changed"
        );
    }

    /// An empty parent builds everything asked for, in order.
    #[test]
    fn the_first_pass_builds_the_whole_list() {
        let parent = with(&["a", "b", "c"]);
        assert_eq!(keys(&parent), ["a", "b", "c"]);
    }

    /// **The point of the whole thing: an unchanged child is the SAME widget.**
    ///
    /// Not merely a widget with the same key — the same one, still holding whatever it was
    /// holding. A pane that is rebuilt loses the letter the picker wrote into it a keystroke ago,
    /// and a tree rebuilt every frame is the app at 100% CPU.
    #[test]
    fn a_child_that_is_still_wanted_is_not_rebuilt() {
        let mut parent = with(&["a", "b"]);
        // Mark the one we expect to survive, in a way only that instance carries.
        parent.base_mut().children[1].base_mut().set_hidden(true);

        let w: Vec<String> = ["b", "a", "c"].iter().map(|s| s.to_string()).collect();
        reconcile_children(&mut parent, &w, build);

        assert_eq!(keys(&parent), ["b", "a", "c"]);
        assert!(
            parent.base().children[0].base().style.layout.hidden,
            "the surviving child was rebuilt: it lost what it was holding",
        );
    }

    /// **A child that declared nothing is still matched**, because identity is derived from what
    /// it holds. `key` is optional in this framework, and matching on it alone would destroy and
    /// rebuild every unkeyed widget on every pass — which is the defect this function exists to
    /// prevent.
    #[test]
    fn a_child_with_no_key_is_matched_by_what_it_holds() {
        use crate::widgets::Label;
        let mut parent = Flex::column()
            .child(Flex::column().child(Label::new("alpha")))
            .child(Flex::column().child(Label::new("beta")));
        // Mark "beta" so a rebuild is detectable.
        parent.base_mut().children[1].base_mut().set_hidden(true);

        let names: Vec<String> = (0..2)
            .map(|i| child_name(&parent, i).expect("a widget always has a name"))
            .collect();
        assert_eq!(
            names,
            ["alpha", "beta"],
            "derived from content, not from a key"
        );

        // Ask for them in the other order, supplying no key anywhere.
        let wanted = vec![names[1].clone(), names[0].clone()];
        reconcile_children(&mut parent, &wanted, |_| Box::new(Flex::column()));

        assert!(
            parent.base().children[0].base().style.layout.hidden,
            "the unkeyed child was rebuilt instead of moved",
        );
    }

    /// **Two children holding the SAME thing are still two children.**
    ///
    /// A derived name would collide, so identity counts the ones that share it: the first wears
    /// the bare name and the rest are numbered. Without that, a list of identical rows would
    /// reconcile every one of them onto the first and throw the others away.
    #[test]
    fn two_children_with_the_same_content_are_told_apart() {
        use crate::widgets::Label;
        let row = || Flex::column().child(Label::new("same"));
        let mut parent = Flex::column().child(row()).child(row()).child(row());

        let names: Vec<String> = (0..3)
            .map(|i| child_name(&parent, i).expect("a widget always has a name"))
            .collect();
        assert_eq!(
            names,
            ["same", "same[1]", "same[2]"],
            "identical content must not collapse to one name",
        );

        // Mark the last one, then ask for them reversed.
        parent.base_mut().children[2].base_mut().set_hidden(true);
        let wanted: Vec<String> = names.iter().rev().cloned().collect();
        reconcile_children(&mut parent, &wanted, |_| Box::new(Flex::column()));

        assert_eq!(
            parent.base().children.len(),
            3,
            "none of the three was lost"
        );
        assert!(
            parent.base().children[0].base().style.layout.hidden,
            "the third row was rebuilt instead of moved: identical rows were confused",
        );
    }

    /// **A parent with a key of its own still matches its children.**
    ///
    /// `identity_of` puts the scope in front of a name — a pane inside a column is `col:3/pane:7`,
    /// not `pane:7` — because a letter has to be unique across the screen. Matching against that
    /// compares a name the caller never asked for, so nothing matched and every child was rebuilt
    /// on every frame: the panes lost their letters and the app sat at 100% CPU.
    #[test]
    fn a_keyed_parent_does_not_rebuild_its_children_every_pass() {
        let mut parent = Flex::column()
            .key("col:3")
            .child(Flex::column().key("pane:7"))
            .child(Flex::column().key("pane:8"));
        parent.base_mut().children[0].base_mut().set_hidden(true);

        let wanted = vec!["pane:7".to_string(), "pane:8".to_string()];
        let changed = reconcile_children(&mut parent, &wanted, build);

        assert!(!changed, "nothing changed, so nothing should be reported");
        assert!(
            parent.base().children[0].base().style.layout.hidden,
            "the child was rebuilt: the parent's own key was counted against it",
        );
    }

    /// A key that has gone takes its child with it.
    #[test]
    fn a_child_nobody_wants_is_dropped() {
        let mut parent = with(&["a", "b", "c"]);
        let w = vec!["a".to_string(), "c".to_string()];
        reconcile_children(&mut parent, &w, build);
        assert_eq!(keys(&parent), ["a", "c"]);
    }

    /// Reordering alone keeps every widget — it is a move, never a rebuild.
    #[test]
    fn reordering_moves_children_rather_than_remaking_them() {
        let mut parent = with(&["a", "b", "c"]);
        for (i, child) in parent.base_mut().children.iter_mut().enumerate() {
            child.base_mut().set_hidden(i == 2);
        }
        let w: Vec<String> = ["c", "b", "a"].iter().map(|s| s.to_string()).collect();
        reconcile_children(&mut parent, &w, build);

        assert_eq!(keys(&parent), ["c", "b", "a"]);
        assert!(
            parent.base().children[0].base().style.layout.hidden,
            "'c' was rebuilt on the way",
        );
    }

    /// Nothing changed means nothing to report, so a caller can skip the work a change needs.
    #[test]
    fn an_unchanged_list_reports_no_change() {
        let mut parent = with(&["a", "b"]);
        let w: Vec<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        assert!(!reconcile_children(&mut parent, &w, build));
    }

    /// …and every kind of change does report one.
    #[test]
    fn adding_removing_and_moving_all_report_a_change() {
        let mut parent = with(&["a", "b"]);
        let added: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        assert!(reconcile_children(&mut parent, &added, build), "added");

        let moved: Vec<String> = ["c", "a", "b"].iter().map(|s| s.to_string()).collect();
        assert!(reconcile_children(&mut parent, &moved, build), "moved");

        let removed: Vec<String> = ["c", "a"].iter().map(|s| s.to_string()).collect();
        assert!(reconcile_children(&mut parent, &removed, build), "removed");
    }
}
