//! A focused dock's own keys reach it: the tree does not take them first.

use super::*;

/// **Enter and Space in the workspaces dock are the dock's** (`activate_selected`,
/// `peek_selected` in `[[keys.surface]]`). heca offers a key to the tree first and consults the
/// dock's own keys only for one the tree did not take, so the tree must leave them: here, the real
/// dock as heca seats it, heca's own widget keymap from the default config, and the key delivered
/// the way `deliver_press_to_tree` delivers it.
#[test]
fn enter_and_space_in_the_focused_workspaces_dock_are_left_to_the_dock() {
    use heca_grid_ui::{GridKey, Handled, KeyPress, LayoutEngine, Modifiers, Size};
    let theme = GuiTheme::default();
    let chrome = SharedChromeState::new(280.0, 260.0);
    let tree = one_pane_tree(1);
    let body = region_body(
        &tree,
        &theme,
        &chrome,
        &mut super::ChromeSignals::default(),
        &mut super::DragItemRegistry::default(),
    )
    .expect("the workspaces dock is seated");
    let mut region: heca_grid_ui::WidgetModel =
        heca_grid_ui::WidgetModel::new(Flex::column().width(280.0).height(400.0).child(body));
    LayoutEngine::new().compute(region.as_mut(), Size::new(280.0, 400.0));
    heca_grid_ui::node_with_key_mut(region.as_mut(), "workspaces")
        .expect("the dock's seat answers to its name")
        .base()
        .focus(false);
    let keymap = crate::app::registry::build_widget_keymap(&heca_config::theme::Config::default());
    for key in [GridKey::Enter, GridKey::Space] {
        let press = KeyPress {
            key,
            text: None,
            mods: Modifiers::default(),
        };
        let handled =
            keymap.deliver_press(&press, |ev| heca_grid_ui::dispatch(region.as_mut(), ev));
        assert_eq!(
            handled,
            Handled::No,
            "{key:?} is left to the dock's own keys"
        );
    }
}
