//! Naming a widget — whether or not anyone named it — and the lookup twin that
//! finds the node answering to a given name.

use super::*;

/// **What to call the widget at `path`, whether or not anyone named it**.
///
/// Something has to recognise a widget between one frame and the next — a hint letter that stays
/// with its target, a cursor that survives a rebuild. A [`key`](crate::builders::IdentityExt::key)
/// is the good answer and the caller writes it on items in a collection. This is the answer for
/// everything else, so a plain button is still recognisable with nothing written on it.
///
/// The identity is the **keys of every keyed ancestor**, then the widget's own name:
///
/// ```text
/// ws:0/pane:7        a keyed row inside a keyed workspace
/// ws:0/pane:7/×      an unkeyed close button inside that row
/// ×[1]               the second unkeyed × in a scope nobody keyed
/// ```
///
/// Three levels for the last part, each used only when the one above is ambiguous:
///
/// 1. its own `key`, when it has one;
/// 2. its **name** — [`Component::text_summary`], the accessible-name algorithm the library already
///    has, so an `Icon` + `Label("HIGH")` is `HIGH` with nothing wired;
/// 3. that name plus an **index among identically-named widgets in the same scope**.
///
/// ⚠️ **Derived from content, never from position.** A path like `Flex/Row[2]/Button[0]` looks
/// automatic and moves on every tree change — which is the bug this exists for: expanding a pane
/// moved a button's hint letter from `k` to `j`. The level-3 index counts only identically-named
/// widgets *within one keyed scope*, so it shifts when a second `×` appears beside the first and
/// never because something changed elsewhere on screen.
///
/// **Known limit:** a derived identity changes if the widget's text changes. Fine for a remembered
/// letter; anything durable should carry a `key`.
pub fn identity_of(root: &dyn Component, path: &[usize]) -> Option<String> {
    // Down the path, collecting the keys of keyed ancestors — the scope this widget is identified
    // within. The last keyed node is also where a level-3 index is counted from.
    let mut node = root;
    let mut scope: Vec<String> = Vec::new();
    let mut scope_root = root;
    let mut scope_depth = 0usize;
    for (depth, step) in path.iter().enumerate() {
        // **Whatever the ancestor declared itself as** — `key` for a row, `scope_key` for a dock or
        // panel that names a region. Reading only `key` here is what let two docks holding the same
        // rows produce two sets of identical names.
        if let Some(k) = node.base().identity() {
            scope.push(k.to_string());
            scope_root = node;
            scope_depth = depth;
        }
        node = node.base().children.get(*step)?.as_ref();
    }

    // **A wrapper answers with the identity of what it wraps.** The picker addresses the node that
    // declared the hint, which is commonly a `KeyHint` around the control that has the key — so
    // resolving here is what lets a caller key the *control*, once, and every wrapper above it
    // agree. Keying the wrapper instead was a call-site fix for a missing rule, and it would have
    // had to be repeated at every wrapped control in the app.
    let node = through_wrappers(node);
    let own = match node.base().identity() {
        Some(k) => k.to_string(),
        None => {
            let name = node.text_summary()?;
            match nth_named(scope_root, &path[scope_depth..], &name) {
                0 => name,
                n => format!("{name}[{n}]"),
            }
        }
    };

    scope.push(own);
    Some(scope.join("/"))
}

/// **The node inside this tree that answers to `key`**, or `None`.
///
/// The lookup twin of [`identity_of`]: that one asks what a widget is called, this one goes and
/// finds it. It exists because a tree can hold several named things that each own a surface of
/// their own — a column holds its panes — and whoever collects from one of them needs to start at
/// that node, not at the tree that contains it.
pub fn node_with_key<'a>(root: &'a dyn Component, key: &str) -> Option<&'a dyn Component> {
    if root.base().answers_to(key) {
        return Some(root);
    }
    root.base()
        .children
        .iter()
        .find_map(|c| node_with_key(c.as_ref(), key))
}

/// The path to the node that answers to `key`, for descending to it mutably.
fn path_to_key(root: &dyn Component, key: &str, here: &mut Vec<usize>) -> Option<Vec<usize>> {
    if root.base().answers_to(key) {
        return Some(here.clone());
    }
    for (i, child) in root.base().children.iter().enumerate() {
        here.push(i);
        if let Some(found) = path_to_key(child.as_ref(), key, here) {
            return Some(found);
        }
        here.pop();
    }
    None
}

/// [`node_with_key`], **mutably** — what running a pick needs, since a pick acts on the widget
/// through its own handlers and handlers are `FnMut`.
pub fn node_with_key_mut<'a>(
    root: &'a mut dyn Component,
    key: &str,
) -> Option<&'a mut dyn Component> {
    let path = path_to_key(root, key, &mut Vec::new())?;
    let mut node = root;
    for step in path {
        node = node.base_mut().children.get_mut(step)?.as_mut();
    }
    Some(node)
}

/// **What one child answers to inside its parent** — its own name, without the scope its ancestors
/// put in front of it.
///
/// [`identity_of`] gives the full path name (`col:3/pane:7`), which is what a letter is filed under
/// because it has to be unique across the whole screen. A parent matching up its own children wants
/// the last part of that (`pane:7`): the scope is the same for all of them, and the caller knows
/// only the names it asked for.
///
/// Same rule either way — a declared `key` or `scope_key`, read through any wrapper, and otherwise
/// derived from content with an index among identically-named siblings.
pub fn child_name(parent: &dyn Component, index: usize) -> Option<String> {
    let node = through_wrappers(parent.base().children.get(index)?.as_ref());
    match node.base().identity() {
        Some(k) => Some(k.to_string()),
        None => {
            let name = node.text_summary()?;
            Some(match nth_named(parent, &[index], &name) {
                0 => name,
                n => format!("{name}[{n}]"),
            })
        }
    }
}

/// How many widgets named `name` come before `path` within this scope, in document order — the
/// index that disambiguates a repeated anonymous control. `0` for the first, which wears the bare
/// name.
fn nth_named(scope_root: &dyn Component, path: &[usize], name: &str) -> usize {
    fn walk(
        node: &dyn Component,
        here: &mut Vec<usize>,
        target: &[usize],
        name: &str,
        seen: &mut usize,
        done: &mut bool,
    ) {
        if *done || skip(node) {
            return;
        }
        if here.as_slice() == target {
            *done = true;
            return;
        }
        // A node that declares a name opens its own scope, so nothing inside it counts towards this
        // one. Read through `identity` so this agrees with the scope `identity_of` builds — a dock
        // naming itself with `scope_key` opens a scope there, and must open one here too.
        if !here.is_empty() && node.base().identity().is_some() {
            return;
        }
        // **An ancestor of the target never counts towards its index.** `text_summary` is the
        // accessible-name algorithm, so a container inherits its first named child's name — a
        // `Flex` wrapping a `Label("×")` is itself called `×`. Counting it would make every wrapped
        // control `×[1]`, and add a wrapper and it becomes `×[2]`, which is exactly the drift this
        // whole thing exists to avoid.
        let ancestor_of_target = target.starts_with(here.as_slice());
        if !here.is_empty() && !ancestor_of_target && node.text_summary().as_deref() == Some(name) {
            *seen += 1;
            // **What is inside it is not a second one.** `text_summary` reads a container's name
            // *from* its content, so a row called `×` holds a label also called `×` — counting both
            // made every row after the first jump two indices, and made an untouched row's name
            // move when a SIBLING gained a child. The index is over the named things in this scope,
            // and this node is the named thing; its content is where its name came from.
            return;
        }
        for (i, child) in node.base().children.iter().enumerate() {
            here.push(i);
            walk(child.as_ref(), here, target, name, seen, done);
            here.pop();
            if *done {
                return;
            }
        }
    }
    let (mut seen, mut done) = (0usize, false);
    walk(
        scope_root,
        &mut Vec::new(),
        path,
        name,
        &mut seen,
        &mut done,
    );
    seen
}

#[cfg(test)]
mod derived_name_tests {
    use super::*;
    use crate::builders::Parent;
    use crate::widgets::{Flex, Label};

    fn row(n: usize) -> Flex {
        let mut f = Flex::column();
        for _ in 0..n {
            f = f.child(Label::new("same"));
        }
        f
    }

    fn names(parent: &dyn Component, n: usize) -> Vec<String> {
        (0..n)
            .map(|i| identity_of(parent, &[i]).expect("a widget always has a name"))
            .collect()
    }

    /// **Identical siblings are numbered from one, not from however many labels they contain.**
    #[test]
    fn identical_siblings_are_numbered_in_order() {
        let parent = Flex::column().child(row(1)).child(row(1)).child(row(1));
        assert_eq!(names(&parent, 3), ["same", "same[1]", "same[2]"]);
    }

    /// **And a name never moves because something changed ELSEWHERE** — the promise
    /// `docs/widgets.md` makes for derived identity, and what a remembered hint letter and a
    /// reconciled child both depend on.
    ///
    /// The first row gains a second label. Nothing about the other two changed, so nothing about
    /// their names may change either.
    #[test]
    fn a_sibling_growing_inside_does_not_rename_the_others() {
        let before = Flex::column().child(row(1)).child(row(1)).child(row(1));
        let after = Flex::column().child(row(2)).child(row(1)).child(row(1));
        assert_eq!(
            names(&before, 3)[1..],
            names(&after, 3)[1..],
            "an untouched row was renamed because a sibling grew",
        );
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use crate::builders::{IdentityExt, Parent};
    use crate::widgets::{Flex, Label};

    /// A keyed widget is named by its own key, under the keys of everything keyed above it — so two
    /// lists may each hold a `key("1")` without colliding.
    #[test]
    fn a_keyed_widget_is_its_key_under_its_keyed_ancestors() {
        let tree = Flex::column().key("ws:0").child(
            Flex::column()
                .key("col:1")
                .child(Label::new("zsh").key("pane:7")),
        );

        assert_eq!(
            identity_of(&tree, &[0, 0]).as_deref(),
            Some("ws:0/col:1/pane:7")
        );
        assert_eq!(identity_of(&tree, &[0]).as_deref(), Some("ws:0/col:1"));
    }

    /// **Nothing written, still recognisable.** An unkeyed widget is named by its own text, under
    /// the nearest keyed ancestor — which is what lets a plain button keep its hint letter.
    #[test]
    fn an_unkeyed_widget_is_named_by_its_text_within_its_scope() {
        let tree = Flex::column()
            .key("pane:7")
            .child(Label::new("×"))
            .child(Label::new("edit"));

        assert_eq!(identity_of(&tree, &[0]).as_deref(), Some("pane:7/×"));
        assert_eq!(identity_of(&tree, &[1]).as_deref(), Some("pane:7/edit"));
    }

    /// Repeated anonymous controls in one scope are told apart by an index — the last resort, and
    /// only among widgets sharing a name.
    #[test]
    fn identically_named_widgets_in_one_scope_are_numbered() {
        let tree = Flex::column()
            .key("topbar")
            .child(Label::new("×"))
            .child(Label::new("edit"))
            .child(Label::new("×"));

        assert_eq!(identity_of(&tree, &[0]).as_deref(), Some("topbar/×"));
        assert_eq!(
            identity_of(&tree, &[1]).as_deref(),
            Some("topbar/edit"),
            "a different name is not numbered"
        );
        assert_eq!(identity_of(&tree, &[2]).as_deref(), Some("topbar/×[1]"));
    }

    /// ⭐ **The whole point.** Adding something elsewhere must not rename anything — a position-based
    /// identity would, and that is the bug this exists for: expanding a pane moved a button's hint
    /// letter from `k` to `j`.
    #[test]
    fn adding_a_widget_elsewhere_renames_nothing() {
        let before = Flex::column()
            .key("pane:7")
            .child(Flex::column().child(Label::new("×")))
            .child(Label::new("edit"));

        let after = Flex::column()
            .key("pane:7")
            // A whole new subtree in front of everything…
            .child(Flex::column().child(Label::new("status")))
            .child(Flex::column().child(Label::new("×")))
            .child(Label::new("edit"));

        assert_eq!(identity_of(&before, &[0, 0]).as_deref(), Some("pane:7/×"));
        assert_eq!(
            identity_of(&after, &[1, 0]).as_deref(),
            Some("pane:7/×"),
            "…and the × is still called the same thing, one index further along the tree"
        );
    }

    /// A keyed node opens a scope of its own, so an index never counts across one: two rows may
    /// each hold a bare `×`.
    #[test]
    fn a_keyed_node_starts_a_fresh_scope() {
        let tree = Flex::column()
            .key("col:1")
            .child(Flex::column().key("pane:7").child(Label::new("×")))
            .child(Flex::column().key("pane:9").child(Label::new("×")));

        assert_eq!(
            identity_of(&tree, &[0, 0]).as_deref(),
            Some("col:1/pane:7/×")
        );
        assert_eq!(
            identity_of(&tree, &[1, 0]).as_deref(),
            Some("col:1/pane:9/×"),
            "not ×[1] — the keyed row above it is a new scope"
        );
    }

    /// **A container that names itself with `scope_key` scopes its rows too**.
    ///
    /// A dock declares itself with `scope_key` rather than `key` — it names a region holding rows,
    /// not a row. Naming read only `key`, so the dock contributed nothing and two docks showing the
    /// same workspace produced two sets of identical names. Everything keyed on identity then
    /// addressed the wrong copy: a remembered hint letter bounced between the two on every opening.
    #[test]
    fn a_container_that_declares_a_scope_key_names_the_rows_inside_it() {
        let dock = |mount: &str| {
            Flex::column()
                .scope_key(mount)
                .child(Flex::column().key("pane:7").child(Label::new("×")))
        };
        let tree = Flex::column().child(dock("left")).child(dock("right"));

        assert_eq!(
            identity_of(&tree, &[0, 0, 0]).as_deref(),
            Some("left/pane:7/×")
        );
        assert_eq!(
            identity_of(&tree, &[1, 0, 0]).as_deref(),
            Some("right/pane:7/×"),
            "the same row in a second dock is a different thing, and must be named differently",
        );
    }

    /// …and the container itself is called what it declared, not what it happens to contain.
    ///
    /// With no name of its own a dock fell back to its text, which is its decorative drag grip — so
    /// every dock was called `⠿`, and the index disambiguating them shifted whenever a row was
    /// added or removed.
    #[test]
    fn a_container_is_named_by_its_scope_key_not_by_its_decoration() {
        let tree = Flex::column().child(
            Flex::column()
                .scope_key("workspaces")
                .child(Label::new("⠿")),
        );

        assert_eq!(identity_of(&tree, &[0]).as_deref(), Some("workspaces"));
    }

    /// Nothing to go on: no key anywhere above, and no text of its own.
    #[test]
    fn a_widget_with_no_key_and_no_text_has_no_identity() {
        let tree = Flex::column().child(Flex::column());
        assert_eq!(identity_of(&tree, &[0]), None);
    }
}
