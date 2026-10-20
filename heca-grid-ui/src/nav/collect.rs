//! Gathering the navigable rows of a tree in document order, and hit-testing a
//! point to the row under it.

use super::*;
use heca_core::layout::{Point, Rectangle};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::{IdentityExt, Parent};
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
