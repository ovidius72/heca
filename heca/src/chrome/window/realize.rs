//! The app's described-tree entry point uses the shared bridge and author diagnostics.

use heca_grid_ui::Theme;
use heca_view::ViewNode;
use heca_view_realize::{FormBindings, IntentEmitter, RealizeError};

pub(crate) fn realize(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    crate::entry::widgets::realize(None, node, theme, emit, forms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn malformed_props_are_reported_once_while_the_content_still_renders() {
        let node = ViewNode::new("scroll")
            .prop("axes", "invalid_axes_t398_diagnostic".into())
            .child(ViewNode::new("label").text("Still here"));
        let emit: IntentEmitter = Rc::new(|_| {});
        for _ in 0..2 {
            let root = realize(&node, &Theme::default(), &emit, &mut FormBindings::default())
                .expect("native defaults preserve usable content");
            assert_eq!(root.base().children[0].text_summary().as_deref(), Some("Still here"));
        }
        let diagnostics = crate::chrome::input::identity::said();
        assert_eq!(diagnostics.iter().filter(|message| message.contains("invalid 'axes' on 'scroll'")).count(), 1);
    }
}
