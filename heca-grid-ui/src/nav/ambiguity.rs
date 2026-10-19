//! The [`key`](crate::builders::IdentityExt::key) rule's warning half: every place a container
//! holds two or more unkeyed, actionable children that derive the **same** name — a collection
//! whose items were never keyed.

use super::*;

/// Two or more unkeyed siblings that answer to the **same derived name** — the one place a derived
/// identity cannot tell them apart.
///
/// Reported by [`ambiguous_identities`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ambiguity {
    /// The scope they sit in — the `/`-joined keys of their keyed ancestors, empty at the root.
    /// This is the identity prefix every one of them shares, so it is what locates them.
    pub scope: String,
    /// The name all of them derive from their content.
    pub name: String,
    /// How many of them there are.
    pub count: usize,
}

/// Every place in `root` where a container holds **two or more unkeyed children deriving the same
/// name** — a collection whose items were never keyed.
///
/// This is the [`key`](crate::builders::IdentityExt::key) rule's warning half, and the condition is
/// **ambiguity, not iteration**: nothing here asks whether the author was writing a loop. A
/// `Choice` of `[Icon, Label("HIGH")]` never trips it — an `Icon` deliberately supplies no
/// [`text_summary`](Component::text_summary), so exactly one child is named. Three rows built from
/// three panes always do, because all three answer to the same thing.
///
/// **Direct children of one container, not every widget in a scope.** That is what "a collection"
/// means when you are looking at a tree: the items of one list share a parent. Grouping by scope
/// instead would pair a row in a header with a row in a body and report a clash the user could not
/// see, and it would double-count wrappers — [`Component::text_summary`] is the accessible-name
/// algorithm, so a `Flex` wrapping a `Label("×")` is itself called `×`, and a parent and its child
/// are never two items of a collection.
///
/// A **keyed** child is skipped: it said who it is, which is the whole point of saying it. So the
/// fix a report asks for is always the same one line — `.key(item.id)` on the item.
///
/// **Only an actionable child is reported** — [`Base::activatable`](crate::component::Base), the
/// same thing that decides whether the picker offers it a letter. Identity is what a cursor, a
/// right-click, a drag and a remembered hint letter are kept *on*, and all four need something to
/// act on: two labels inside one row are that row's content, not two items, and the row above them
/// is what carries the key. Without this clause the walk reports every transparent wrapper in the
/// tree — a `KeyHint` around a keyed row inherits the row's name through
/// [`Component::text_summary`] while carrying no key of its own, so heca's own sidebar came back
/// with seven findings and not one of them was real. It is the same clause the declarative half
/// applies by asking for a `press`.
///
/// Hidden subtrees are skipped, like everywhere else in this module: a collapsed group holds no
/// rows the user can reach, so it has nothing to disambiguate.
///
/// The result is in **document order**, and it is pure data — this library prints nothing. A host
/// reports it however it reports anything (heca does so once per distinct finding, in debug builds).
///
/// ```ignore
/// for a in nav::ambiguous_identities(&tree) {
///     eprintln!("[heca] {} unkeyed children of {} are all called {:?}", a.count, a.scope, a.name);
/// }
/// ```
pub fn ambiguous_identities(root: &dyn Component) -> Vec<Ambiguity> {
    let mut out = Vec::new();
    walk_ambiguities(root, "", &mut out);
    out
}

/// The **item** a container's child stands for — the thing whose identity would be kept — or `None`
/// when that child is not an item at all.
///
/// [`through_wrappers`] finds the control; this decides whether it is an item worth reporting. A
/// **keyed** node is not: it said who it is, which is the whole point of saying it. Neither is an
/// inert one: identity is what a cursor, a right-click, a drag and a remembered letter are kept
/// *on*, and all four need something to act on.
fn item_of(child: &dyn Component) -> Option<&dyn Component> {
    if skip(child) {
        return None;
    }
    let at = through_wrappers(child);
    (at.base().identity().is_none() && at.base().activatable).then_some(at)
}

fn walk_ambiguities(node: &dyn Component, scope: &str, out: &mut Vec<Ambiguity>) {
    if skip(node) {
        return;
    }
    // This node's children live in this node's scope, so its own declared name joins the prefix
    // first — the same scope `identity_of` builds, so a reported ambiguity names what the picker
    // would name.
    let scope = match node.base().identity() {
        Some(k) if scope.is_empty() => k.to_string(),
        Some(k) => format!("{scope}/{k}"),
        None => scope.to_string(),
    };

    // Count the names its unkeyed children derive. A Vec rather than a map: these are a handful of
    // siblings, and document order is the order the user sees, which is the order to report in.
    let mut names: Vec<(String, usize)> = Vec::new();
    for child in node.base().children.iter() {
        let Some(child) = item_of(child.as_ref()) else {
            continue;
        };
        let Some(name) = child.text_summary() else {
            continue;
        };
        match names.iter_mut().find(|(n, _)| *n == name) {
            Some((_, count)) => *count += 1,
            None => names.push((name, 1)),
        }
    }
    for (name, count) in names {
        if count >= 2 {
            out.push(Ambiguity {
                scope: scope.clone(),
                name,
                count,
            });
        }
    }

    for child in node.base().children.iter() {
        walk_ambiguities(child.as_ref(), &scope, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::{IdentityExt, Parent};
    use crate::reactive::SignalUpdate;
    use crate::widgets::{Flex, Glyph, Icon, KeyHint, Label, Row};

    /// A row you can act on — the thing a cursor stops on and a hint letter lands on.
    fn row(name: &str) -> Row {
        Row::new().on_activate(|| {}).child(Label::new(name))
    }

    /// The case the warning exists for: rows built from a collection, none of them keyed.
    #[test]
    fn a_collection_of_unkeyed_rows_is_reported() {
        let tree = Flex::column()
            .key("workspaces")
            .child(row("pane"))
            .child(row("pane"))
            .child(row("pane"));

        assert_eq!(
            ambiguous_identities(&tree),
            vec![Ambiguity {
                scope: "workspaces".into(),
                name: "pane".into(),
                count: 3,
            }],
        );
    }

    /// **Only what you can act on is reported.** Identity is what a cursor, a right-click, a drag
    /// and a remembered letter are kept on; three inert labels have none of those to lose, and
    /// asking someone to key them would be asking for `"btn1"`.
    #[test]
    fn inert_siblings_are_not_reported() {
        let tree = Flex::column()
            .child(Label::new("zsh"))
            .child(Label::new("zsh"));

        assert_eq!(ambiguous_identities(&tree), vec![]);
    }

    /// **A wrapped control is still an item.** A chrome button is `Tooltip(KeyHint(IconButton))`,
    /// so a walk that only looks at direct children finds two `Tooltip`s, neither actionable, and
    /// reports nothing — which is exactly how the top bar's two sidebar toggles collided in the
    /// running app with this check live and silent.
    #[test]
    fn a_control_behind_wrappers_is_still_an_item() {
        let wrapped = || KeyHint::new(row("×"));
        let tree = Flex::row().child(wrapped()).child(wrapped());

        assert_eq!(
            ambiguous_identities(&tree),
            vec![Ambiguity {
                scope: String::new(),
                name: "×".into(),
                count: 2
            }],
            "the wrapper is transparent; the control inside it is the item",
        );
    }

    /// **A transparent wrapper is not an item.** `KeyHint` wraps a row without keying itself, and
    /// `text_summary` is the accessible-name algorithm, so the wrapper answers to the row's name.
    /// It carries no action of its own — the row inside does — which is what keeps it out.
    #[test]
    fn a_transparent_wrapper_around_a_keyed_row_is_not_an_item() {
        let tree = Flex::column()
            .child(KeyHint::new(row("zsh").key("pane:1")))
            .child(KeyHint::new(row("zsh").key("pane:2")));

        assert_eq!(ambiguous_identities(&tree), vec![]);
    }

    /// The condition is **ambiguity, not iteration**. An icon beside a label is a composed control,
    /// and only one of the two is named — an `Icon` supplies no `text_summary` on purpose.
    #[test]
    fn a_composed_control_is_not_a_collection() {
        let tree = Flex::row()
            .child(Icon::new(Glyph::Warning))
            .child(Label::new("HIGH"));

        assert_eq!(ambiguous_identities(&tree), vec![]);
    }

    /// Keying the items is the fix, and it is the only thing the report ever asks for.
    #[test]
    fn keyed_items_are_never_reported() {
        let tree = Flex::column()
            .key("workspaces")
            .child(row("pane").key("pane:7"))
            .child(row("pane").key("pane:9"));

        assert_eq!(ambiguous_identities(&tree), vec![]);
    }

    /// Siblings that read differently are told apart by their names alone — nothing to warn about.
    #[test]
    fn differently_named_siblings_are_not_ambiguous() {
        let tree = Flex::row().child(row("×")).child(row("edit"));

        assert_eq!(ambiguous_identities(&tree), vec![]);
    }

    /// **A parent and its child are never two items of a collection.** `text_summary` is the
    /// accessible-name algorithm, so a `Flex` wrapping a `Label("×")` is itself called `×`; counting
    /// the pair would report every wrapped control in the tree.
    #[test]
    fn a_wrapper_and_the_child_it_is_named_after_are_not_a_collection() {
        let tree = Flex::row().child(row("×"));

        assert_eq!(ambiguous_identities(&tree), vec![]);
    }

    /// Two lists in one tree are two findings, each named by the scope that locates it — not one
    /// clash pooled across both.
    #[test]
    fn each_container_is_reported_in_its_own_scope() {
        let tree = Flex::column()
            .child(Flex::column().key("header").child(row("×")).child(row("×")))
            .child(
                Flex::column()
                    .key("body")
                    .child(row("row"))
                    .child(row("row")),
            );

        assert_eq!(
            ambiguous_identities(&tree),
            vec![
                Ambiguity {
                    scope: "header".into(),
                    name: "×".into(),
                    count: 2
                },
                Ambiguity {
                    scope: "body".into(),
                    name: "row".into(),
                    count: 2
                },
            ],
        );
    }

    /// A collapsed group holds no rows the user can reach, so there is nothing to disambiguate —
    /// the same rule every other walk in this module follows.
    #[test]
    fn a_hidden_collection_is_not_reported() {
        let tree = Flex::column().child(row("row")).child(row("row"));
        tree.base().children[1].base().visible.set(false);

        assert_eq!(ambiguous_identities(&tree), vec![]);
    }
}
