//! How a region divides its height between the containers seated in it.

use super::*;

/// A rule sits between containers, and takes no share of the height.
///
/// It has to be a leaf with its natural height, or it would be handed a share of its own and the
/// containers would each lose height to a 1px line.
#[test]
fn a_rule_separates_containers_without_taking_a_share() {
    use heca_core::layout::Size as CoreSize;
    use heca_grid_ui::LayoutEngine;

    let body = || -> WidgetModel { Box::new(Flex::column().height(40.0)) };
    let mut stack = Flex::column().gap(8.0).grow(1.0);
    {
        let layout = &mut stack.base_mut().style.layout;
        layout.min_height = Some(Length::Px(0.0));
        layout.flex_shrink = Some(1.0);
    }
    // Two containers with a rule between them, as `build_region_content` assembles them.
    stack.base_mut().children.push(with_flex(body(), 1.0));
    stack = stack.child(Separator::horizontal());
    stack.base_mut().children.push(with_flex(body(), 1.0));

    let mut root = Flex::column().height(600.0).child(stack);
    LayoutEngine::new().compute(&mut root, CoreSize::new(300.0, 600.0));

    let kids = &root.base().children[0].base().children;
    assert_eq!(kids.len(), 3, "container, rule, container");
    assert_eq!(
        kids[1].base().style.layout.flex_grow,
        0.0,
        "the rule takes no share"
    );
    assert!(
        kids[1].base().bounds.size.h < 10.0,
        "the rule keeps its own thin height: {:?}",
        kids[1].base().bounds.size.h,
    );
    // Within a pixel: an odd leftover after the rule and the gaps has to land somewhere, so
    // equal shares of an odd number of pixels differ by one. Measured 292 / 1 / 291.
    assert!(
        (kids[0].base().bounds.size.h - kids[2].base().bounds.size.h).abs() <= 1.0,
        "and the containers still share equally around it: {:?}",
        kids.iter()
            .map(|k| k.base().bounds.size.h)
            .collect::<Vec<_>>(),
    );
}

/// Shares divide the region even when the content is taller than it.
///
/// This is the part that looked done and was not. `flex_grow` distributes only *positive* free
/// space, and a container's content is routinely taller than a sidebar — so two containers
/// measured 1214px each inside a 600px body, overflowed the frame, and divided nothing. The fix
/// is a zero base size plus permission to shrink (CSS `flex: 1 1 0`; this vocabulary has no
/// `flex_basis`), which makes the free space the whole region.
///
/// The numbers are asserted rather than the flags, because the flags were "right" while the
/// layout was wrong.
#[test]
fn shares_divide_the_region_even_with_content_taller_than_it() {
    use heca_core::layout::Size as CoreSize;
    use heca_grid_ui::LayoutEngine;

    // A body far shorter than the content it holds, as a sidebar is.
    let tall = || -> WidgetModel {
        let mut inner = Flex::column();
        for _ in 0..20 {
            inner = inner.child(Flex::column().height(60.0));
        }
        Box::new(heca_grid_ui::ScrollRegion::new().child(inner))
    };

    let mut stack = Flex::column().gap(8.0).grow(1.0);
    {
        let layout = &mut stack.base_mut().style.layout;
        layout.min_height = Some(Length::Px(0.0));
        layout.flex_shrink = Some(1.0);
    }
    for _ in 0..2 {
        stack.base_mut().children.push(with_flex(tall(), 1.0));
    }
    let mut body = Flex::column().height(600.0).child(stack);
    LayoutEngine::new().compute(&mut body, CoreSize::new(300.0, 600.0));

    let stack = &body.base().children[0];
    assert!(
        stack.base().bounds.size.h <= 600.0,
        "the stack fits the region instead of overflowing it: {:?}",
        stack.base().bounds.size.h,
    );
    let heights: Vec<f64> = stack
        .base()
        .children
        .iter()
        .map(|c| c.base().bounds.size.h)
        .collect();
    assert_eq!(heights.len(), 2);
    assert!(
        (heights[0] - heights[1]).abs() < 1.0,
        "equal shares are equal: {heights:?}",
    );
    assert!(
        heights[0] > 250.0 && heights[0] < 300.0,
        "each takes about half the 600px region, less the gap: {heights:?}",
    );
}

/// **A dock says its own size, on the body it builds, and the region honours it.**
///
/// Two docks in one sidebar, one built with `.flex(3.0)` and one with `.flex(1.0)` — the line a
/// plugin author writes. Laid out in a real 400px region they split 3:1, whatever each holds. No
/// template on the region and no method on the provider: a region is a list anyone may append
/// to, so sizes written there break when one more dock arrives.
#[test]
fn a_dock_says_its_own_size_and_the_region_honours_it() {
    use crate::chrome::Contribution;
    use crate::chrome::{BuildCx, ContainerContribution, RegionSet, WidgetModel};
    use crate::providers::{ChromeCtx, Provider};
    use heca_grid_ui::{LayoutEngine, Size};

    struct Sized(&'static str, f32, Option<heca_grid_ui::order::Order>);
    impl Provider for Sized {
        fn id(&self) -> &str {
            self.0
        }
        fn title(&self) -> &str {
            self.0
        }
        fn supported_regions(&self) -> RegionSet {
            RegionSet::sidebars()
        }
        fn default_region(&self) -> super::RegionId {
            super::RegionId::LeftSidebar
        }
        fn build_contribution(&self, _ctx: &ChromeCtx<'_>) -> Contribution {
            let (parts, order) = (self.1, self.2);
            Contribution::Container(ContainerContribution {
                id: self.id().to_string(),
                title: self.title().to_string(),
                supported_regions: self.supported_regions(),
                default_region: self.default_region(),
                movable: self.movable(),
                collapsible: self.collapsible(),
                // The dock's size, said by the dock: one builder on the body, nothing else.
                build: Box::new(move |_: &ChromeCtx<'_>, _: &mut BuildCx<'_>| {
                    // A body far taller than any part, so only `.flex` can explain the split.
                    let mut body = Flex::column().flex(parts);
                    if let Some(order) = order {
                        body = body.order(order);
                    }
                    for _ in 0..20 {
                        body = body.child(Flex::column().height(100.0));
                    }
                    Box::new(body) as WidgetModel
                }),
            })
        }
    }

    let chrome = SharedChromeState::new(280.0, 260.0);
    let mut host = super::ChromeHost::new(chrome.events());
    host.register(Box::new(Sized("big", 3.0, None)));
    host.register(Box::new(Sized("small", 1.0, None)));
    let theme = GuiTheme::default();
    let emit = ChromeIntentEmitter::of(
        crate::app::interaction::InteractionSource::Keyboard,
        |_, _| {},
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
    .expect("two docks are seated, so the region has a body");

    let mut region: Box<dyn heca_grid_ui::Component> =
        Box::new(Flex::column().width(200.0).height(400.0).child(body));
    LayoutEngine::new().compute(region.as_mut(), Size::new(200.0, 400.0));

    // The column holds [big, rule, small]; the rule is a sibling and asks for no part.
    let column = &region.base().children[0];
    let heights: Vec<f64> = column
        .base()
        .children
        .iter()
        .map(|c| c.base().bounds.size.h)
        .collect();
    println!("big / rule / small: {heights:?}");
    assert_eq!(
        heights.len(),
        3,
        "two docks and one rule between them: {heights:?}"
    );
    let (big, small) = (heights[0], heights[2]);
    assert!(
        (big / small - 3.0).abs() < 0.05,
        "the docks split 3:1 as each asked, whatever they hold: {heights:?}",
    );
    let bottom = column.base().children[2].base().bounds;
    assert!(
        bottom.loc.y + bottom.size.h <= 401.0,
        "neither dock pushes the other out of the region: {heights:?}",
    );

    // **And where it sits, said the same way**: the dock added second asks for `.order(-1)` and
    // comes first, with the rule still between the two rather than above both.
    let mut host = super::ChromeHost::new(chrome.events());
    host.register(Box::new(Sized("big", 3.0, None)));
    host.register(Box::new(Sized("small", 1.0, Some((-1).into()))));
    let body = super::build_region_content(
        &host,
        super::RegionId::LeftSidebar,
        &ctx,
        &mut super::ChromeSignals::default(),
        &mut super::DragItemRegistry::default(),
    )
    .expect("two docks are seated");
    let mut region: Box<dyn heca_grid_ui::Component> =
        Box::new(Flex::column().width(200.0).height(400.0).child(body));
    LayoutEngine::new().compute(region.as_mut(), Size::new(200.0, 400.0));
    let heights: Vec<f64> = region.base().children[0]
        .base()
        .children
        .iter()
        .map(|c| c.base().bounds.size.h)
        .collect();
    println!("ordered small / rule / big: {heights:?}");
    assert!(
        heights[0] < heights[2] && heights[1] <= 2.0,
        "the small dock comes first and the rule sits between: {heights:?}",
    );

    // **And whoever ADDS a dock can say both, over what its body says** — the line for placing
    // a dock you did not write. `big` asked for 3 parts on its body; its placer says 1 and puts
    // it last; `small` asked for 1 and first, and its placer says 3.
    use crate::chrome::{PlaceDock, regions};
    let mut host = super::ChromeHost::new(chrome.events());
    regions("sidebar.left").append([
        Sized("big", 3.0, None).flex(1.0).order(1),
        Sized("small", 1.0, Some((-1).into())).flex(3.0).order(0),
    ]);
    host.mount_pending(&ctx);
    let body = super::build_region_content(
        &host,
        super::RegionId::LeftSidebar,
        &ctx,
        &mut super::ChromeSignals::default(),
        &mut super::DragItemRegistry::default(),
    )
    .expect("two docks are seated");
    let mut region: Box<dyn heca_grid_ui::Component> =
        Box::new(Flex::column().width(200.0).height(400.0).child(body));
    LayoutEngine::new().compute(region.as_mut(), Size::new(200.0, 400.0));
    let heights: Vec<f64> = region.base().children[0]
        .base()
        .children
        .iter()
        .map(|c| c.base().bounds.size.h)
        .collect();
    println!("placed small(3) / rule / big(1): {heights:?}");
    assert!(
        (heights[0] / heights[2] - 3.0).abs() < 0.05,
        "the placer's parts and order win over the bodies': {heights:?}",
    );
}

/// Wrapping a container in the focus ring + pick keycap must not disturb the shares.
///
/// The wrappers are transparent, so the share has to be applied to the **outermost** node: on the
/// body it would leave the wrapper content-sized, and two containers would divide nothing —
/// exactly the failure once measured (1214px each inside a 600px body). Pixels are
/// asserted, not flags, because the flags were right while the layout was broken.
#[test]
fn the_focus_wrappers_do_not_disturb_the_shares() {
    use heca_core::layout::Size as CoreSize;
    use heca_grid_ui::LayoutEngine;
    use heca_grid_ui::widgets::{FocusScope, KeyHint};

    // A body far taller than the region it is given, as a real dock is.
    let tall = || -> WidgetModel {
        let mut inner = Flex::column();
        for _ in 0..20 {
            inner = inner.child(Flex::column().height(60.0));
        }
        Box::new(heca_grid_ui::ScrollRegion::new().grow(1.0).child(inner))
    };
    // The wrapping `focus_and_pick` applies, without needing a render context for it.
    let wrapped = || -> WidgetModel {
        let mut body = tall();
        pass_box_down(body.as_mut());
        let mut picked = KeyHint::new(body);
        pass_box_down(&mut picked);
        Box::new(FocusScope::new(picked))
    };

    let mut stack = Flex::column().gap(8.0).grow(1.0);
    {
        let layout = &mut stack.base_mut().style.layout;
        layout.min_height = Some(Length::Px(0.0));
        layout.flex_shrink = Some(1.0);
    }
    for _ in 0..2 {
        stack.base_mut().children.push(with_flex(wrapped(), 1.0));
    }
    let mut body = Flex::column().height(600.0).child(stack);
    LayoutEngine::new().compute(&mut body, CoreSize::new(300.0, 600.0));

    let stack = &body.base().children[0];
    let heights: Vec<f64> = stack
        .base()
        .children
        .iter()
        .map(|c| c.base().bounds.size.h)
        .collect();
    assert_eq!(heights.len(), 2);
    assert!(
        (heights[0] - heights[1]).abs() < 1.0,
        "equal shares are still equal through the wrappers: {heights:?}",
    );
    assert!(
        heights[0] > 250.0 && heights[0] < 300.0,
        "each still takes about half the 600px region, less the gap: {heights:?}",
    );
    // And the wrappers pass the height straight down — a ring around a box half the size of the
    // box would be worse than no ring.
    let ring = &stack.base().children[0];
    let hinted = &ring.base().children[0];
    let dock = &hinted.base().children[0];
    assert_eq!(
        (ring.base().bounds.size.h, dock.base().bounds.size.h),
        (heights[0], heights[0]),
        "ring, keycap wrapper and dock all measure the same box",
    );
}

/// A container that asked for **no** share keeps its content height through the wrappers.
///
/// `grow = 0.0` means "size me to my content", and a zero base size inside a content-sized parent
/// would collapse the whole thing to nothing — which is why the pass-down is conditional.
#[test]
fn a_content_sized_container_keeps_its_height_through_the_wrappers() {
    use heca_core::layout::Size as CoreSize;
    use heca_grid_ui::LayoutEngine;
    use heca_grid_ui::widgets::{FocusScope, KeyHint};

    let body: WidgetModel = Box::new(Flex::column().height(120.0));
    // What `focus_and_pick` does with `share = 0.0`: wrap, and touch no layout.
    let wrapped: WidgetModel = Box::new(FocusScope::new(KeyHint::new(body)));
    let mut region = Flex::column().height(600.0).child(with_flex(wrapped, 0.0));
    LayoutEngine::new().compute(&mut region, CoreSize::new(300.0, 600.0));

    let ring = &region.base().children[0];
    assert_eq!(
        ring.base().bounds.size.h,
        120.0,
        "content-sized means the content's height, not zero and not the region's",
    );
}

/// **A placement is lettered by its own mount id** — `workspaces` and `workspaces.2` are two
/// targets, not one. Asserted through the door every target now uses rather than a host-side
/// projection helper, which is what this replaced.
#[test]
fn a_second_placement_of_a_container_is_its_own_letter_target() {
    let mode = crate::app_state::InputMode::DockPick {
        candidates: vec![
            ('a', "workspaces".to_string()),
            ('b', "workspaces.2".to_string()),
        ],
    };
    let wanted = crate::chrome::hint::wanted_for_tests(&mode);
    assert_eq!(
        wanted,
        vec![
            (
                crate::chrome::hint::Offer::ByKey("workspaces".to_string()),
                'a',
            ),
            (
                crate::chrome::hint::Offer::ByKey("workspaces.2".to_string()),
                'b',
            ),
        ],
    );
}
