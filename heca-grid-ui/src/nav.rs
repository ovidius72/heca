//! **Navigation keys** — a row's own identity, declared once and read by everything that has to
//! name a row.
//!
//! A list-shaped component labels its rows with [`ComponentExt::key`]; the host then derives the
//! keyboard cursor, the right-click target, and (later) the drag identity from that **one**
//! declaration. Three readers, one thing said — instead of a closed enum of row kinds that only the
//! app can extend, which is what made a plugin row impossible to point at.
//!
//! # Why a string, when [`DragItemId`](crate::drag::DragItemId) is an opaque integer
//!
//! That one is a **registry slot**: the widget takes a token and the app keeps the map, valid for
//! as long as the tree that registered it. A nav key is the opposite — it must **survive a tree
//! rebuild**, because a chrome tree is rebuilt for reasons that have nothing to do with navigation
//! (a pane's git status changing is enough), and a cursor that resets every time is not a cursor.
//! An index into a tree cannot do that; an identity the row asserts about itself can. This is also
//! why a scoped [`FocusManager`](crate::focus::FocusManager) — a visit *index* — cannot be the
//! cursor.
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

/// The **innermost scope** under `point` — which enclosing region a press landed in.
///
/// The twin of [`key_at`] one level up: that answers *which row*, this answers *which region
/// containing rows*. A host commonly needs both from one press — heca focuses the chrome container
/// and moves its cursor to the clicked row.
///
/// Same walk, for the same reason: children last-added-first, descended into before the parent is
/// tested, so a scope nested inside another resolves to the inner one. `None` means the point is
/// outside every scope, which is a real answer a host acts on.
///
/// Deliberately independent of whether a widget *consumed* the press: a click on a scrollbar thumb
/// is still a click inside the region that holds it.
pub fn scope_at(root: &dyn Component, point: Point) -> Option<String> {
    if skip(root) {
        return None;
    }
    for child in root.base().children.iter().rev() {
        if let Some(id) = scope_at(child.as_ref(), point) {
            return Some(id);
        }
    }
    root.base()
        .scope_key
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
        if let Some(k) = node.base().key.as_ref() {
            scope.push(k.clone());
            scope_root = node;
            scope_depth = depth;
        }
        node = node.base().children.get(*step)?.as_ref();
    }

    let own = match node.base().key.as_ref() {
        Some(k) => k.clone(),
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
        // A keyed node opens its own scope, so nothing inside it counts towards this one.
        if !here.is_empty() && node.base().key.is_some() {
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
    walk(scope_root, &mut Vec::new(), path, name, &mut seen, &mut done);
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::{ComponentExt, Parent};
    use crate::widgets::{Flex, Surface};
    use crate::reactive::SignalUpdate;
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
        root.base_mut().children.push(at(Surface::new(), 40.0, 20.0));
        root
    }

    /// A container wrapping rows: the press resolves to the **innermost** container, and to nothing
    /// at all outside every one — which is what releases chrome focus (F003/P086/T365).
    #[test]
    fn a_press_resolves_to_the_innermost_container_it_landed_in() {
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));

        let mut dock = Flex::column().scope_key("workspaces");
        dock.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 40.0));
        dock.base_mut()
            .children
            .push(at(Surface::new().key("pane:7"), 0.0, 20.0));
        // A container seated inside another — the inner one owns the point.
        let mut inner = Flex::column().scope_key("notes");
        inner.base_mut().bounds = Rectangle::new(Point::new(0.0, 20.0), Size::new(100.0, 20.0));
        dock.base_mut().children.push(Box::new(inner));
        root.base_mut().children.push(Box::new(dock));

        assert_eq!(
            scope_at(&root, Point::new(50.0, 10.0)),
            Some("workspaces".to_string()),
        );
        assert_eq!(
            scope_at(&root, Point::new(50.0, 30.0)),
            Some("notes".to_string()),
            "the innermost container wins, as the deepest row does",
        );
        assert_eq!(
            scope_at(&root, Point::new(50.0, 80.0)),
            None,
            "outside every container — this is what releases focus",
        );
    }

    /// A press on a widget that would consume it is still a press *in* that container.
    #[test]
    fn a_consumed_press_still_names_its_container() {
        let mut dock = Flex::column().scope_key("workspaces");
        dock.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 40.0));
        // Stand-in for a scrollbar thumb: a child with no container of its own.
        dock.base_mut()
            .children
            .push(at(Surface::new(), 0.0, 40.0));
        assert_eq!(
            scope_at(&dock, Point::new(50.0, 20.0)),
            Some("workspaces".to_string()),
        );
    }

    #[test]
    fn collects_declared_rows_in_document_order() {
        let keys: Vec<String> = collect_keys(&tree())
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        assert_eq!(keys, ["ws:0", "pane:7"], "and nothing for the undeclared row");
    }

    #[test]
    fn a_hidden_subtree_has_no_navigable_rows() {
        let root = tree();
        root.base().children[0].base().visible.set(false);
        let keys: Vec<String> = collect_keys(&root).into_iter().map(|(k, _)| k).collect();
        assert_eq!(
            keys, ["pane:7"],
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
        group = group.child_boxed(at(Surface::new().key("pane:7"), 10.0, 20.0));
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

        assert_eq!(identity_of(&tree, &[0, 0]).as_deref(), Some("ws:0/col:1/pane:7"));
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
        assert_eq!(identity_of(&tree, &[1]).as_deref(), Some("topbar/edit"), "a different name is not numbered");
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

        assert_eq!(identity_of(&tree, &[0, 0]).as_deref(), Some("col:1/pane:7/×"));
        assert_eq!(
            identity_of(&tree, &[1, 0]).as_deref(),
            Some("col:1/pane:9/×"),
            "not ×[1] — the keyed row above it is a new scope"
        );
    }

    /// Nothing to go on: no key anywhere above, and no text of its own.
    #[test]
    fn a_widget_with_no_key_and_no_text_has_no_identity() {
        let tree = Flex::column().child(Flex::column());
        assert_eq!(identity_of(&tree, &[0]), None);
    }
}
