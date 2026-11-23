//! What a widget inside a seated dock names goes out as that dock's own.

use super::*;

/// **A row names its action and it arrives addressed** — seated on the dock that holds it and
/// judged as a gesture there, exactly as `fires` would send it. The row is built with nothing from
/// the host: no mount, no emitter. That is the line a plugin writes.
#[test]
fn a_named_action_inside_a_dock_goes_out_seated_on_that_dock() {
    use crate::app::interaction::{InteractionIntent, InteractionSource};
    use crate::chrome::{BuildCx, ContainerContribution, Contribution, RegionSet};
    use crate::providers::{ChromeCtx, Provider};
    use heca_grid_ui::builders::{EventExt, HintExt, IdentityExt};
    use heca_grid_ui::widgets::Row;
    use heca_grid_ui::{LayoutEngine, Size};
    use heca_view::Intent;

    struct Named;
    impl Provider for Named {
        fn id(&self) -> &str {
            "named"
        }
        fn title(&self) -> &str {
            "named"
        }
        fn supported_regions(&self) -> RegionSet {
            RegionSet::sidebars()
        }
        fn build_contribution(&self, _ctx: &ChromeCtx<'_>) -> Contribution {
            Contribution::Container(ContainerContribution::new(
                self,
                |_: &ChromeCtx<'_>, _: &mut BuildCx<'_>| {
                    Flex::column()
                        .child(
                            Row::new()
                                .key("activated")
                                .on_press(Intent::new("activated")),
                        )
                        .child(Row::new().key("picked").on_hint(Intent::new("picked")))
                        .child(
                            Row::new()
                                .key("clicked")
                                .on_click(|ev| ev.dispatch(Intent::new("clicked"))),
                        )
                },
            ))
        }
    }

    let chrome = SharedChromeState::new(280.0, 260.0);
    let mut host = super::ChromeHost::new(chrome.events());
    host.register(super::RegionId::LeftSidebar, Box::new(Named));
    let theme = GuiTheme::default();
    let sent = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = sent.clone();
    let emit = ChromeIntentEmitter::of(
        InteractionSource::MouseLeftSidebar,
        move |source, intent| {
            seen.borrow_mut().push((source, intent));
        },
    );
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let ctx = ChromeCtx::for_build(&chrome, &theme, &emit, &catalog);
    let body = super::build_region_content(
        &host,
        super::RegionId::LeftSidebar,
        &ctx,
        &mut super::ChromeSignals::default(),
        &mut super::DragItemRegistry::default(),
    )
    .expect("one dock is seated");
    let mut region: heca_grid_ui::WidgetModel =
        heca_grid_ui::WidgetModel::new(Flex::column().width(200.0).height(400.0).child(body));
    LayoutEngine::new().compute(region.as_mut(), Size::new(200.0, 400.0));

    for name in ["activated", "picked", "clicked"] {
        let path =
            heca_grid_ui::path_with_key(region.as_ref(), name).expect("the row is in the dock");
        assert!(
            heca_grid_ui::fire_hint(region.as_mut(), &path),
            "{name} is a target"
        );
    }
    let sent = sent.borrow();
    let names: Vec<_> = sent
        .iter()
        .map(|(source, intent)| match intent {
            InteractionIntent::View(intent) => {
                assert_eq!(*source, InteractionSource::MouseLeftSidebar);
                assert_eq!(
                    intent.args.get(crate::providers::SEAT_ARG),
                    Some(&heca_view::PropValue::Text("named".into())),
                    "seated on the dock that holds the row",
                );
                intent.action.clone()
            }
            other => panic!("only the rows' own actions are sent: {other:?}"),
        })
        .collect();
    assert_eq!(names, ["activated", "picked", "clicked"]);
}

/// **A sidebar toggle names its action and goes out as a chrome gesture** — through the chrome
/// root's listener and its emitter's source, the one `pick_source` reads for the chrome. The
/// button carries no emitter.
#[test]
fn a_sidebar_toggle_goes_out_as_a_chrome_gesture() {
    use crate::app::interaction::{InteractionIntent, InteractionSource};
    use heca_grid_ui::builders::EventExt;
    use heca_grid_ui::{LayoutEngine, Size};
    let sent = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = sent.clone();
    let emit = ChromeIntentEmitter::of(
        InteractionSource::MouseLeftSidebar,
        move |source, intent| {
            seen.borrow_mut().push((source, intent));
        },
    );
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let button = super::sidebar_toggle_button(
        heca_grid_ui::Glyph::ArrowLineLeft,
        "sidebar_left",
        &crate::chrome::ActionShortcuts::default(),
        &catalog,
    );
    let mut chrome: heca_grid_ui::WidgetModel = heca_grid_ui::WidgetModel::new(
        Flex::row()
            .child(button)
            .on_dispatch(move |intent| emit.fire_intent(intent)),
    );
    LayoutEngine::new().compute(chrome.as_mut(), Size::new(300.0, 40.0));
    let path =
        heca_grid_ui::path_with_key(chrome.as_ref(), "sidebar_left").expect("keyed by its action");
    let _ = heca_grid_ui::click(chrome.as_mut(), &path);
    let sent = sent.borrow();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0].0, InteractionSource::MouseLeftSidebar);
    match &sent[0].1 {
        InteractionIntent::View(intent) => assert_eq!(intent.action, "sidebar_left"),
        other => panic!("the button's action went out: {other:?}"),
    }
}

/// **A click on a pane row in the workspaces dock** sends, seated on the dock, what each part of
/// the row declares, in the order the click rises: the row's own click first — focus the dock,
/// then put its cursor on this row — then the card around it, which focuses the pane. No widget
/// carries the dock's emitter.
#[test]
fn a_click_on_a_pane_row_goes_out_seated_on_the_workspaces_dock() {
    use crate::app::interaction::{InteractionIntent, InteractionSource};
    use heca_grid_ui::{LayoutEngine, Size};
    let chrome = SharedChromeState::new(280.0, 260.0);
    *chrome.workspaces.tree_mut() = super::one_pane_tree(1);
    let mut host = super::ChromeHost::new(chrome.events());
    host.register(
        super::RegionId::LeftSidebar,
        Box::new(crate::providers::WorkspacesContainerProvider::new(
            "workspaces",
        )),
    );
    let theme = GuiTheme::default();
    let sent = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = sent.clone();
    let emit = ChromeIntentEmitter::of(
        InteractionSource::MouseLeftSidebar,
        move |source, intent| {
            seen.borrow_mut().push((source, intent));
        },
    );
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let ctx = crate::providers::ChromeCtx::for_build(&chrome, &theme, &emit, &catalog);
    let body = super::build_region_content(
        &host,
        super::RegionId::LeftSidebar,
        &ctx,
        &mut super::ChromeSignals::default(),
        &mut super::DragItemRegistry::default(),
    )
    .expect("the workspaces dock is seated");
    let mut region: heca_grid_ui::WidgetModel =
        heca_grid_ui::WidgetModel::new(Flex::column().width(280.0).height(400.0).child(body));
    LayoutEngine::new().compute(region.as_mut(), Size::new(280.0, 400.0));

    let row = crate::chrome::pane_key(heca_core::layout::PaneId(1));
    let b = heca_grid_ui::node_with_key(region.as_ref(), &row)
        .expect("the pane row is shown")
        .base()
        .bounds;
    let at = heca_grid_ui::Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    let _ = heca_grid_ui::dispatch(
        region.as_mut(),
        &heca_grid_ui::Event::pointer_pressed(at, heca_grid_ui::PointerButton::Left),
    );
    let _ = heca_grid_ui::dispatch(
        region.as_mut(),
        &heca_grid_ui::Event::pointer_released(at, heca_grid_ui::PointerButton::Left),
    );
    let named: Vec<(String, Option<String>)> = sent
        .borrow()
        .iter()
        .filter_map(|(source, intent)| match intent {
            InteractionIntent::View(intent) => {
                assert_eq!(*source, InteractionSource::MouseLeftSidebar);
                let seat = intent
                    .args
                    .get(crate::providers::SEAT_ARG)
                    .and_then(heca_view::PropValue::as_text)
                    .map(str::to_string);
                Some((intent.action.clone(), seat))
            }
            _ => None,
        })
        .collect();
    let seat = Some("workspaces".to_string());
    assert_eq!(
        named,
        [
            ("focus_dock".to_string(), seat.clone()),
            ("cursor_to".to_string(), seat.clone()),
            ("focus_pane".to_string(), seat),
        ],
        "the row's two acts, then the card's press, all seated on the dock",
    );
}

/// **Folding a workspace names its action**: the fold control sends `toggle_workspace_collapsed`
/// for that workspace, seated on the workspaces dock — from a callback handed only the new state,
/// through `heca_grid_ui::intent::send`, with no emitter in the row.
#[test]
fn folding_a_workspace_sends_its_named_action_seated_on_the_dock() {
    use crate::app::interaction::{InteractionIntent, InteractionSource};
    use heca_grid_ui::{LayoutEngine, Size};
    let chrome = SharedChromeState::new(280.0, 260.0);
    *chrome.workspaces.tree_mut() = super::one_pane_tree(1);
    let mut host = super::ChromeHost::new(chrome.events());
    host.register(
        super::RegionId::LeftSidebar,
        Box::new(crate::providers::WorkspacesContainerProvider::new(
            "workspaces",
        )),
    );
    let theme = GuiTheme::default();
    let sent = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = sent.clone();
    let emit = ChromeIntentEmitter::of(
        InteractionSource::MouseLeftSidebar,
        move |source, intent| {
            seen.borrow_mut().push((source, intent));
        },
    );
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let ctx = crate::providers::ChromeCtx::for_build(&chrome, &theme, &emit, &catalog);
    let body = super::build_region_content(
        &host,
        super::RegionId::LeftSidebar,
        &ctx,
        &mut super::ChromeSignals::default(),
        &mut super::DragItemRegistry::default(),
    )
    .expect("the workspaces dock is seated");
    let mut region: heca_grid_ui::WidgetModel =
        heca_grid_ui::WidgetModel::new(Flex::column().width(280.0).height(400.0).child(body));
    LayoutEngine::new().compute(region.as_mut(), Size::new(280.0, 400.0));

    // The fold control is the workspace frame's part named for it, found by name, not position.
    fn path_to_area(node: &dyn heca_grid_ui::Component, area: &str, at: &mut Vec<usize>) -> bool {
        if node.base().grid_area.as_deref() == Some(area) {
            return true;
        }
        for (i, child) in node.base().children.iter().enumerate() {
            at.push(i);
            if path_to_area(child.as_ref(), area, at) {
                return true;
            }
            at.pop();
        }
        false
    }
    let frame_key = crate::providers::workspaces::workspace_key(heca_core::layout::WorkspaceId(0));
    let mut path =
        heca_grid_ui::path_with_key(region.as_ref(), &frame_key).expect("the workspace row");
    let frame = heca_grid_ui::node_with_key(region.as_ref(), &frame_key).unwrap();
    let mut inner = Vec::new();
    assert!(
        path_to_area(frame, "toggle", &mut inner),
        "its fold control"
    );
    path.extend(inner);
    let _ = heca_grid_ui::click(region.as_mut(), &path);

    let folds: Vec<_> = sent
        .borrow()
        .iter()
        .filter_map(|(source, intent)| match intent {
            InteractionIntent::View(intent) if intent.action == "toggle_workspace_collapsed" => {
                Some((*source, intent.args.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(folds.len(), 1, "{folds:?}");
    let (source, args) = &folds[0];
    assert_eq!(*source, InteractionSource::MouseLeftSidebar);
    assert_eq!(args.get("ws_idx"), Some(&heca_view::PropValue::Int(0)));
    assert_eq!(
        args.get(crate::providers::SEAT_ARG),
        Some(&heca_view::PropValue::Text("workspaces".into()))
    );
}
