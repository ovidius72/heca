//! **Navigation keys** — a row's own identity, declared once and read by everything that has to
//! name a row.
//!
//! A list-shaped component labels its rows with [`ComponentExt::key`]; the keyboard cursor, the
//! right-click target and the drag identity are all read off that **one** declaration. Three
//! readers, one thing said — instead of a closed enum of row kinds that only the app can extend,
//! which is what made a plugin row impossible to point at.
//!
//! # Why a string, and not a token from a registry
//!
//! A registry slot — the widget takes an opaque id and the app keeps the map — is valid only for as
//! long as the tree that registered it. A nav key is the opposite: it must **survive a tree
//! rebuild**, because a chrome tree is rebuilt for reasons that have nothing to do with navigation
//! (a pane's git status changing is enough), and a cursor that resets every time is not a cursor.
//! An index into a tree cannot do that; an identity the row asserts about itself can. This is also
//! why a scoped [`FocusManager`](crate::focus::FocusManager) — a visit *index* — cannot be the
//! cursor.
//!
//! A row need not be named at all: one that declares no key still has an identity derived from its
//! **content**, so a capability is never gated on having been named.
//!
//! The key is **opaque to this library**: it is a string the component chose (`"pane:7"`,
//! `"container:abc123"`), and nothing here parses it.
//!
//! ```ignore
//! // The component labels its rows; that is the whole of its side.
//! Row::new().key(format!("pane:{}", pane.id)).child(Label::new(&pane.name))
//! ```

use crate::component::Component;
use crate::reactive::SignalGet;
use heca_core::layout::{Point, Rectangle};

/// Should this subtree be enumerated? Hidden widgets have stale bounds and never receive input, so
/// they're skipped — the same rule paint, event routing, drag and hint resolution use.
fn skip(c: &dyn Component) -> bool {
    !c.base().visible.get_untracked() || c.base().style.layout.hidden
}

/// Every navigable row in the tree with its laid-out bounds, in **document order**.
///
/// Document order is the order the user sees, which is what "next row" and "previous row" mean — so
/// a cursor moves through this list rather than through anything the component has to maintain
/// separately. Hidden subtrees are skipped, so a collapsed group's rows are not steppable, which is
/// the behaviour collapsing is *for*.
pub fn collect_keys(root: &dyn Component) -> Vec<(String, Rectangle)> {
    let mut out = Vec::new();
    collect_into(root, &mut out);
    out
}

fn collect_into(node: &dyn Component, out: &mut Vec<(String, Rectangle)>) {
    if skip(node) {
        return;
    }
    if let Some(key) = node.base().key.as_ref() {
        out.push((key.clone(), node.base().bounds));
    }
    for child in node.base().children.iter() {
        collect_into(child.as_ref(), out);
    }
}

/// The **topmost, deepest** navigable row under `point` — what a right-click at that position is
/// aimed at.
///
/// Children are walked last-added-first and descended into before the parent is tested, so the most
/// specific row under the cursor wins: a pane row inside a column group resolves to the pane, and a
/// click on the group's own chrome resolves to the group. Same walk as
/// [`drag::source_at`](crate::drag::source_at), deliberately — a right-click and a drag must agree
/// about what they are pointing at.
pub fn key_at(root: &dyn Component, point: Point) -> Option<String> {
    if skip(root) {
        return None;
    }
    for child in root.base().children.iter().rev() {
        if let Some(key) = key_at(child.as_ref(), point) {
            return Some(key);
        }
    }
    root.base()
        .key
        .clone()
        .filter(|_| root.base().bounds.contains(point))
}

/// **What to call the widget at `path`, whether or not anyone named it** (F003/P082/T444).
///
/// Something has to recognise a widget between one frame and the next — a hint letter that stays
/// with its target, a cursor that survives a rebuild. A [`key`](crate::builders::ComponentExt::key)
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
        // rows produce two sets of identical names (F003/P082).
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
/// This is the [`key`](crate::builders::ComponentExt::key) rule's warning half, and the condition is
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

/// **Down through transparent wrappers to the node that actually carries the identity.**
///
/// A control is rarely the node you are handed. A chrome button is
/// `Tooltip(KeyHint(IconButton))`, and the picker addresses the node that *declared the hint* —
/// the `KeyHint` — while the thing with a name and a `key` is the `IconButton` two levels down.
/// Without this, every author wrapping a control would have to remember to put the key on the
/// wrapper instead of the control, which is a rule a caller has to remember and therefore one that
/// belongs here.
///
/// A **wrapper** is a node with no key of its own, no action of its own, and exactly one visible
/// child. The descent stops at the first node that is keyed (it said who it is) or actionable (it
/// is the control), and at anything holding several children — that is a real container, not a
/// wrapper, and descending into it would speak for something that is not one thing.
fn through_wrappers(node: &dyn Component) -> &dyn Component {
    let mut at = node;
    // Bounded by the depth walked; a wrapper chain is two or three deep in practice.
    loop {
        // A node that names itself is not a wrapper, whichever way it declared that name.
        if at.base().identity().is_some() || at.base().activatable {
            return at;
        }
        match at.base().children.as_slice() {
            [only] if !skip(only.as_ref()) => at = only.as_ref(),
            _ => return at,
        }
    }
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
    use crate::builders::{ComponentExt, Parent};
    use crate::reactive::SignalUpdate;
    use crate::widgets::{Flex, Surface};
    use heca_core::layout::Size;

    /// Force bounds on a widget (layout doesn't run in these unit tests).
    fn at(mut w: impl Component + 'static, y: f64, h: f64) -> Box<dyn Component> {
        w.base_mut().bounds = Rectangle::new(Point::new(0.0, y), Size::new(100.0, h));
        Box::new(w)
    }

    fn tree() -> Flex {
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        root.base_mut()
            .children
            .push(at(Surface::new().key("ws:0"), 0.0, 20.0));
        root.base_mut()
            .children
            .push(at(Surface::new().key("pane:7"), 20.0, 20.0));
        // Undeclared rows are simply not navigable.
        root.base_mut()
            .children
            .push(at(Surface::new(), 40.0, 20.0));
        root
    }

    #[test]
    fn collects_declared_rows_in_document_order() {
        let keys: Vec<String> = collect_keys(&tree()).into_iter().map(|(k, _)| k).collect();
        assert_eq!(
            keys,
            ["ws:0", "pane:7"],
            "and nothing for the undeclared row"
        );
    }

    #[test]
    fn a_hidden_subtree_has_no_navigable_rows() {
        let root = tree();
        root.base().children[0].base().visible.set(false);
        let keys: Vec<String> = collect_keys(&root).into_iter().map(|(k, _)| k).collect();
        assert_eq!(
            keys,
            ["pane:7"],
            "a collapsed group's rows are not steppable — which is what collapsing is for",
        );
    }

    #[test]
    fn a_point_resolves_to_the_row_under_it() {
        let root = tree();
        assert_eq!(key_at(&root, Point::new(5.0, 5.0)), Some("ws:0".into()));
        assert_eq!(key_at(&root, Point::new(5.0, 25.0)), Some("pane:7".into()));
        assert_eq!(
            key_at(&root, Point::new(5.0, 45.0)),
            None,
            "over a row that declared nothing",
        );
    }

    /// The **innermost** row wins, so a pane inside a column group resolves to the pane.
    #[test]
    fn the_deepest_row_under_the_point_wins() {
        let mut group = Surface::new().key("col:0:1");
        group.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 60.0));
        group = group.child(at(Surface::new().key("pane:7"), 10.0, 20.0));
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        root.base_mut().children.push(Box::new(group));

        assert_eq!(
            key_at(&root, Point::new(5.0, 15.0)),
            Some("pane:7".into()),
            "inside the nested row",
        );
        assert_eq!(
            key_at(&root, Point::new(5.0, 45.0)),
            Some("col:0:1".into()),
            "on the group's own chrome, below its child",
        );
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use crate::builders::{ComponentExt, Parent};
    use crate::reactive::SignalUpdate;
    use crate::widgets::{Flex, Glyph, Icon, KeyHint, Label, Row};

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

    /// **A container that names itself with `scope_key` scopes its rows too** (F003/P082).
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

    // ── The warning: two or more unkeyed siblings deriving the same name ──────────────────────

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
    /// running app with this check live and silent (Antonio, driving, 2026-08-19).
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
