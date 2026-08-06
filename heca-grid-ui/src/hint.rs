//! Universal leader / vimium **hint targets** over a laid-out component tree.
//!
//! The host's global KeyHint picker (a leader like `prefix+/`) lights a letter
//! over **every actionable target on screen** and, on the keypress, fires that
//! target's intent. This is the geometry+opt-in layer for it, mirroring the drag
//! framework: a widget opts in with an opaque [`HintTargetId`] (via
//! [`HintExt::hint_target`](crate::builders::HintExt::hint_target)), and
//! [`collect_hint_targets`] walks the **retained** widget tree (whose `Base.bounds`
//! are filled in by layout each frame) to enumerate every target + its bounds.
//!
//! Domain-neutral: the id is opaque and the app maps it back to an intent — the
//! framework never knows what "activating" a target does (same contract as
//! [`DragItemId`](crate::drag::DragItemId)).

use crate::component::Component;
use heca_core::layout::Rectangle;

/// Opaque hint-target identifier. The host maps it back to an intent to dispatch
/// when the target's letter is pressed. Construct with [`HintTargetId::new`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HintTargetId(usize);

impl HintTargetId {
    /// Create a new hint-target id from a flat index.
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// Access the raw flat index (only in host code that knows how to map it).
    pub fn raw(self) -> usize {
        self.0
    }
}

/// Enumerate **every** hint target in the tree with its laid-out bounds, in
/// document order (parents before children). The host assigns letters to the
/// result and stamps a keycap over each `bounds`. Hidden and disabled subtrees
/// are skipped because they cannot receive activation.
pub fn collect_hint_targets(root: &dyn Component) -> Vec<(HintTargetId, Rectangle)> {
    let mut out = Vec::new();
    root.visit_hint_targets(&mut |id, bounds| out.push((id, bounds)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::HintExt;
    use crate::reactive::SignalUpdate;
    use crate::widgets::{Flex, Surface};
    use heca_core::layout::{Point, Size};

    fn at(mut w: impl Component + 'static, x: f64, y: f64, w_: f64, h_: f64) -> Box<dyn Component> {
        w.base_mut().bounds = Rectangle::new(Point::new(x, y), Size::new(w_, h_));
        Box::new(w)
    }

    #[test]
    fn collects_targets_with_bounds_in_document_order() {
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        root.base_mut().children.push(at(
            Surface::new().hint_target(HintTargetId::new(1)),
            0.0,
            0.0,
            100.0,
            40.0,
        ));
        root.base_mut().children.push(at(
            Surface::new().hint_target(HintTargetId::new(2)),
            0.0,
            50.0,
            100.0,
            40.0,
        ));

        let targets = collect_hint_targets(&root);
        assert_eq!(
            targets.iter().map(|(id, _)| id.raw()).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(targets[1].1.loc.y, 50.0);
    }

    #[test]
    fn skips_hidden_disabled_and_untagged_nodes() {
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        // An untagged node with a tagged child, then hidden/disabled targets.
        let mut visible_parent = Flex::column();
        visible_parent
            .base_mut()
            .children
            .push(at(Surface::new().hint_target(HintTargetId::new(7)), 0.0, 0.0, 10.0, 10.0));
        root.base_mut().children.push(Box::new(visible_parent));

        let mut hidden = Surface::new().hint_target(HintTargetId::new(9));
        hidden.base_mut().visible.set(false);
        root.base_mut().children.push(Box::new(hidden));

        let mut disabled = Surface::new().hint_target(HintTargetId::new(10));
        disabled.base_mut().disabled.set(true);
        root.base_mut().children.push(Box::new(disabled));

        let targets = collect_hint_targets(&root);
        assert_eq!(
            targets.iter().map(|(id, _)| id.raw()).collect::<Vec<_>>(),
            vec![7],
            "untagged parent is transparent; hidden and disabled targets are skipped"
        );
    }
}
