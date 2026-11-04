//! Display-name filtering on the retained sidebar tree.

use super::*;
use heca_core::runtime::{PaneRuntime, ProcessStatus};
use heca_grid_ui::{DrawCommand, Flex};
use heca_grid_ui::builders::Parent;

#[test]
fn a_retained_sidebar_filters_short_names_and_publishes_a_quiet_program_without_rebuilding() {
    let tree = testing::tree();
    let mut fx = testing::Fixture::default();
    let pane = PaneId(1);
    let mut runtime = PaneRuntime { program: Some("zsh".into()), ..Default::default() };
    fx.store.panes.set_runtime(pane, &runtime, None);
    let root = {
        let (seams, mut reg) = fx.split("left");
        DockView { tree: &tree, scroll: heca_grid_ui::reactive::signal(0.0) }.build(&seams, &mut reg)
    };
    let mut root = Flex::column().child(root);
    let words = |root: &mut Flex| {
        crate::chrome::paint_chrome_root(root, 340.0, 800.0, &fx.theme).iter()
            .filter_map(|command| match command {
                DrawCommand::Text(text) => Some(text.text.clone()), _ => None,
            }).collect::<Vec<_>>()
    };
    assert!(words(&mut root).iter().any(|text| text == "zsh"));
    for command in ["ls", "cd"] {
        runtime.program = Some(command.into());
        runtime.status = ProcessStatus::Running;
        fx.store.panes.set_runtime(pane, &runtime, None);
        let text = words(&mut root);
        assert!(text.iter().any(|text| text == "zsh"));
        assert!(!text.iter().any(|text| text == command));
        runtime.status = ProcessStatus::Success;
        fx.store.panes.set_runtime(pane, &runtime, None);
        runtime.program = Some("zsh".into());
        fx.store.panes.set_runtime(pane, &runtime, None);
        assert_eq!(fx.store.panes.next_name_update(), None);
    }
    runtime.program = Some("nvim".into());
    runtime.status = ProcessStatus::Running;
    fx.store.panes.set_runtime(pane, &runtime, None);
    let due = fx.store.panes.next_name_update().unwrap();
    fx.store.panes.advance_names(due);
    assert!(words(&mut root).iter().any(|text| text == "Neovim"));
    runtime.program = Some("zsh".into());
    runtime.status = ProcessStatus::Success;
    fx.store.panes.set_runtime(pane, &runtime, None);
    assert!(words(&mut root).iter().any(|text| text == "zsh"));
}
