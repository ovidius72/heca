//! **Navigation keys** — a row's own identity, declared once and read by everything that has to
//! name a row.
//!
//! A list-shaped component labels its rows with [`NavExt::nav_key`]; the host then derives the
//! keyboard cursor, the right-click target, and (later) the drag identity from that **one**
//! declaration. Three readers, one thing said — instead of a closed enum of row kinds that only the
//! app can extend, which is what made a plugin row impossible to point at.
//!
//! # Why a string, when [`DragItemId`](crate::drag::DragItemId) and
//! [`HintTargetId`](crate::hint::HintTargetId) are opaque integers
//!
//! Those two are **registry slots**: the widget takes a token and the app keeps the map, valid for
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
//! Row::new().nav_key(format!("pane:{}", pane.id)).child(Label::new(&pane.name))
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
pub fn collect_nav_keys(root: &dyn Component) -> Vec<(String, Rectangle)> {
    let mut out = Vec::new();
    collect_into(root, &mut out);
    out
}

fn collect_into(node: &dyn Component, out: &mut Vec<(String, Rectangle)>) {
    if skip(node) {
        return;
    }
    if let Some(key) = node.base().nav_key.as_ref() {
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
pub fn nav_key_at(root: &dyn Component, point: Point) -> Option<String> {
    if skip(root) {
        return None;
    }
    for child in root.base().children.iter().rev() {
        if let Some(key) = nav_key_at(child.as_ref(), point) {
            return Some(key);
        }
    }
    root.base()
        .nav_key
        .clone()
        .filter(|_| root.base().bounds.contains(point))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::{NavExt, Parent};
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
            .push(at(Surface::new().nav_key("ws:0"), 0.0, 20.0));
        root.base_mut()
            .children
            .push(at(Surface::new().nav_key("pane:7"), 20.0, 20.0));
        // Undeclared rows are simply not navigable.
        root.base_mut().children.push(at(Surface::new(), 40.0, 20.0));
        root
    }

    #[test]
    fn collects_declared_rows_in_document_order() {
        let keys: Vec<String> = collect_nav_keys(&tree())
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        assert_eq!(keys, ["ws:0", "pane:7"], "and nothing for the undeclared row");
    }

    #[test]
    fn a_hidden_subtree_has_no_navigable_rows() {
        let root = tree();
        root.base().children[0].base().visible.set(false);
        let keys: Vec<String> = collect_nav_keys(&root).into_iter().map(|(k, _)| k).collect();
        assert_eq!(
            keys, ["pane:7"],
            "a collapsed group's rows are not steppable — which is what collapsing is for",
        );
    }

    #[test]
    fn a_point_resolves_to_the_row_under_it() {
        let root = tree();
        assert_eq!(nav_key_at(&root, Point::new(5.0, 5.0)), Some("ws:0".into()));
        assert_eq!(nav_key_at(&root, Point::new(5.0, 25.0)), Some("pane:7".into()));
        assert_eq!(
            nav_key_at(&root, Point::new(5.0, 45.0)),
            None,
            "over a row that declared nothing",
        );
    }

    /// The **innermost** row wins, so a pane inside a column group resolves to the pane.
    #[test]
    fn the_deepest_row_under_the_point_wins() {
        let mut group = Surface::new().nav_key("col:0:1");
        group.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 60.0));
        group = group.child_boxed(at(Surface::new().nav_key("pane:7"), 10.0, 20.0));
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        root.base_mut().children.push(Box::new(group));

        assert_eq!(
            nav_key_at(&root, Point::new(5.0, 15.0)),
            Some("pane:7".into()),
            "inside the nested row",
        );
        assert_eq!(
            nav_key_at(&root, Point::new(5.0, 45.0)),
            Some("col:0:1".into()),
            "on the group's own chrome, below its child",
        );
    }
}
