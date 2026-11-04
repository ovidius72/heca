//! The declared default when the shown tree has no keyboard owner.

use crate::component::Component;
use crate::reactive::SignalGet;

pub(super) fn target(node: &dyn Component) -> Option<Vec<usize>> {
    if !node.is_shown() || node.base().disabled.get_untracked() { return None; }
    if node.base().focus_fallback {
        if let Some(path) = super::handed_to(node) { return Some(path); }
        if node.focusable() { return Some(Vec::new()); }
    }
    node.base().children.iter().enumerate().rev().find_map(|(index, child)| {
        let mut path = target(child.as_ref())?;
        path.insert(0, index);
        Some(path)
    })
}

#[cfg(test)]
mod tests {
    use crate::{Component, Flex, Input, CommandPalette, WidgetModel, keyboard_holder, settle_focus};
    use crate::builders::{Parent, PlaceExt, PresenceExt, LayoutExt};

    #[test]
    fn a_fallback_delegates_to_content_and_never_steals_an_existing_owner() {
        let pane = Flex::column().keyboard_to("content").focus_fallback(true)
            .child(Input::new().area("content"));
        let mut root = Flex::column().child(pane).child(Input::new());
        settle_focus(&mut root);
        let content = root.base().children[0].base().children[0].base().focus_id();
        assert_eq!(keyboard_holder(&root), Some(content));
        root.base().children[1].base().focus(false);
        settle_focus(&mut root);
        let other = root.base().children[1].base().focus_id();
        assert_eq!(keyboard_holder(&root), Some(other));
        root.base_mut().children.push(WidgetModel::new(CommandPalette::new().default_open(true)));
        settle_focus(&mut root);
        assert_ne!(keyboard_holder(&root), Some(content));
        let _ = crate::unmount_child(&mut root, 2);
        assert_eq!(keyboard_holder(&root), Some(other), "closing an overlay returns to its actual opener");
    }

    #[test]
    fn hidden_disabled_or_missing_content_never_becomes_the_fallback() {
        for node in [
            Flex::column().focus_fallback(true).keyboard_to("missing"),
            Flex::column().focus_fallback(true).keyboard_to("body").child(Input::new().area("body")).hidden(true),
            Flex::column().focus_fallback(true).keyboard_to("body").child(Input::new().area("body")).disabled(true),
        ] {
            let mut root = Flex::column().child(node);
            settle_focus(&mut root);
            assert_eq!(keyboard_holder(&root), None);
        }
    }
}
