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
                        .child(Row::new().key("activated").on_press(Intent::new("activated")))
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
        let path = heca_grid_ui::path_with_key(region.as_ref(), name).expect("the row is in the dock");
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
