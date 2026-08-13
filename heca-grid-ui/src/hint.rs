//! Universal leader / vimium **peek targets** over a laid-out component tree.
//!
//! The host's global picker (a leader like `prefix+/`) lights a letter over **every region that
//! said what a pick does to it** and runs that region's declaration on the keypress. This module is
//! the framework's half: [`collect_peeks`] walks the **retained** widget tree (whose `Base.bounds`
//! are filled in by layout each frame) and enumerates every declaration with the rect its letter
//! goes over, and [`fire_peek`] runs one.
//!
//! **Nothing is registered and no id exists.** A widget declares the behaviour itself with
//! [`KeyHint::on_peek`](crate::widgets::KeyHint::on_peek); a target is addressed by its **path**
//! from the root, which lives exactly as long as the frame it was collected in. That is what makes
//! the picker something a plugin can join: there is no host registry to reach and no host-private
//! intent type to name. It replaced an opaque `HintTargetId` the host mapped back to a
//! `pub(crate)` enum — a shipped feature a plugin could only have a second-class version of.

use crate::component::Component;
use crate::reactive::SignalGet;
use heca_core::layout::Rectangle;

/// Should this subtree be enumerated? Hidden widgets have stale bounds and never
/// receive input, so they're skipped (matching paint / event / drag resolution).
fn skip(c: &dyn Component) -> bool {
    !c.base().visible.get_untracked() || c.base().style.layout.hidden
}

/// **Every widget in this tree that says what a pick does to it**, with the rect the letter goes
/// over, in document order. The path addresses the widget so [`fire_peek`] can reach it again.
///
/// The framework's half of [`on_peek`](crate::builders::ComponentExt::on_peek): a host walks its trees,
/// lays the letters out and draws them, and hands the pick back here. Nothing is registered, and no
/// id outlives the frame it was collected in — a retained tree rebuilt between the letters
/// appearing and one being picked simply offers a fresh set.
pub fn collect_peeks(root: &dyn Component) -> Vec<(Vec<usize>, Rectangle)> {
    let mut out = Vec::new();
    peeks_into(root, &mut Vec::new(), &mut out);
    out
}

fn peeks_into(node: &dyn Component, path: &mut Vec<usize>, out: &mut Vec<(Vec<usize>, Rectangle)>) {
    if skip(node) {
        return;
    }
    if node.base().peek.is_some() {
        out.push((path.clone(), node.base().bounds));
    }
    for (i, child) in node.base().children.iter().enumerate() {
        path.push(i);
        peeks_into(child.as_ref(), path, out);
        path.pop();
    }
}

/// **Run what the widget at `path` said a pick does.** `false` when the path no longer leads to a
/// widget that declared one — a tree rebuilt under the letters, which is not an error.
pub fn fire_peek(root: &dyn Component, path: &[usize]) -> bool {
    let mut node = root;
    for step in path {
        match node.base().children.get(*step) {
            Some(child) => node = child.as_ref(),
            None => return false,
        }
    }
    match &node.base().peek {
        Some(f) => {
            f();
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactive::SignalUpdate;
    use crate::widgets::{Flex, KeyHint, Surface};
    use heca_core::layout::{Point, Size};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A peeking wrapper with laid-out bounds, appending `tag` to `log` when it is picked.
    fn peek_at(
        tag: &'static str,
        log: &Rc<RefCell<Vec<&'static str>>>,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) -> Box<dyn Component> {
        let log = log.clone();
        let mut wrapper = KeyHint::new(Surface::new()).on_peek(move || log.borrow_mut().push(tag));
        wrapper.base_mut().bounds = Rectangle::new(Point::new(x, y), Size::new(w, h));
        Box::new(wrapper)
    }

    #[test]
    fn collects_declarations_with_bounds_in_document_order() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        root.base_mut()
            .children
            .push(peek_at("first", &log, 0.0, 0.0, 100.0, 40.0));
        root.base_mut()
            .children
            .push(peek_at("second", &log, 0.0, 50.0, 100.0, 40.0));

        let targets = collect_peeks(&root);
        assert_eq!(
            targets.iter().map(|(path, _)| path.clone()).collect::<Vec<_>>(),
            vec![vec![0], vec![1]],
            "document order, addressed by path"
        );
        assert_eq!(targets[1].1.loc.y, 50.0, "…each with the rect its letter goes over");

        assert!(fire_peek(&root, &targets[1].0));
        assert_eq!(*log.borrow(), vec!["second"]);
    }

    #[test]
    fn skips_hidden_subtrees_and_silent_nodes() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        // A silent parent holding a declaring child, then a hidden declaring node.
        let mut silent_parent = Flex::column();
        silent_parent
            .base_mut()
            .children
            .push(peek_at("nested", &log, 0.0, 0.0, 10.0, 10.0));
        root.base_mut().children.push(Box::new(silent_parent));

        let mut hidden = KeyHint::new(Surface::new()).on_peek(|| unreachable!("hidden"));
        hidden.base_mut().visible.set(false);
        root.base_mut().children.push(Box::new(hidden));

        let targets = collect_peeks(&root);
        assert_eq!(
            targets.iter().map(|(path, _)| path.clone()).collect::<Vec<_>>(),
            vec![vec![0, 0]],
            "a silent parent is transparent; a hidden declaration is skipped"
        );
    }
}
