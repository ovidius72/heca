use super::*;

/// **A modal layer covers whatever it declared** — the property no call site can get wrong.
/// It demands a decision, so acts on the panes behind it are refused by construction; the
/// alternative is trusting a `bool` at every `insert`, and the one that passed `false` let
/// `prefix+x` raise the close-pane confirm with a context menu open.
#[test]
fn coverage_is_what_the_surface_declared_not_what_its_place_implies() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    // Takes the keyboard, declares it covers nothing — and is believed.
    reg.insert(
        LayerId(7),
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, false),
        &mut window,
    );
    assert!(
        !reg.content_covered(&window),
        "a modal that says it covers nothing covers nothing — the band used to overrule this",
    );

    // The decision-demanding surfaces declare it themselves, which is where it belongs:
    // `open_modal` and `insert_menu_layer` both pass `true`.
    reg.insert(
        LayerId(8),
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, true),
        &mut window,
    );
    assert!(
        reg.content_covered(&window),
        "and a surface that says it covers, does"
    );
}

/// **Below the `Modal` band, coverage is what the layer declared — `modal` says nothing about
/// it.** The two answer different questions: `modal` is "does this take the keyboard",
/// `lock` is "may actions still touch the panes". Conflating them made one surface
/// impossible to describe — the exposé takes the keyboard *and* is a map of the panes, so while
/// `modal` implied coverage it refused every act on the pane it exists to let you choose.
#[test]
fn an_overlay_that_takes_the_keyboard_can_still_declare_it_covers_nothing() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let map = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, false),
        &mut window,
    );
    reg.show(&mut window, map);
    assert!(
        !reg.content_covered(&window),
        "a map of the panes does not cover them"
    );
    assert_eq!(
        reg.top_modal_id(&window),
        Some(map),
        "and it still owns the keyboard"
    );
}

/// The one input `Domain::Overlay` reads: a *visible* covering layer, and only that.
/// A layer that covers but is hidden is not covering anything.
#[test]
fn coverage_is_reported_only_while_the_layer_is_visible() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let corner = reg.add(
        None,
        LayerKind::Persistent,
        declaring(empty_root(), false, false),
        &mut window,
    );
    assert!(
        !reg.content_covered(&window),
        "a non-covering layer covers nothing"
    );

    let over = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, true),
        &mut window,
    );
    assert!(!reg.content_covered(&window), "on-demand starts hidden");
    reg.show(&mut window, over);
    assert!(
        reg.content_covered(&window),
        "shown, and it covers the panes"
    );
    reg.hide(&mut window, over);
    assert!(!reg.content_covered(&window));
    reg.remove(&mut window, corner);
}

/// **A menu is its own panel, so seating it as a surface must not resize it**.
///
/// `place_surface` gives every surface the whole viewport, which is right for an `Overlay` — a
/// layer fills the window and positions a panel *inside* itself. A `ContextMenu` is not shaped
/// that way: the widget **is** the panel (`panel_base` sets its own direction, padding and width
/// bounds, and its rows are its children), so handing it a full-viewport box stretches the panel
/// down the whole window.
///
/// Two things go wrong at once, and only one of them is visible: the panel is drawn floor to
/// ceiling, **and** its hit area becomes the whole window — so with a menu open, every press
/// anywhere lands on the menu instead of what is under the cursor, and a right-click elsewhere
/// opens nothing.
#[test]
fn a_menu_seated_as_a_surface_keeps_its_own_height() {
    use heca_core::layout::Size;
    use heca_grid_ui::LayoutEngine;
    use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};

    let viewport = Size::new(1400.0, 900.0);
    let menu = ContextMenu::new("pane")
        .child(
            Menu::new("Pane", "")
                .child(MenuItem::new().label("New column"))
                .child(MenuItem::new().label("New pane"))
                .child(MenuItem::new().label("Close pane")),
        )
        .default_open(true);

    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let id = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(heca_grid_ui::WidgetModel::new(menu), true, true),
        &mut window,
    );
    reg.show(&mut window, id);
    LayoutEngine::new().compute(&mut window, viewport);

    let h = crate::chrome::surface_node(&window, id)
        .expect("the menu is seated")
        .base()
        .bounds
        .size
        .h;
    assert!(
        h < viewport.h / 2.0,
        "the menu was stretched to the seat instead of keeping its own height: {h} of {}",
        viewport.h,
    );
}

/// The other half of the seat, and the silent one: **a layer-shaped surface still fills the
/// window**.
///
/// The seat leaves both sizes to the surface, so this is what would break if a layer ever stopped
/// declaring its own — and it would break invisibly, as a surface that paints and hit-tests in a
/// box the size of its content while still believing it owns the screen.
#[test]
fn a_layer_seated_as_a_surface_still_fills_the_window() {
    use heca_core::layout::Size;
    use heca_grid_ui::LayoutEngine;

    let viewport = Size::new(1400.0, 900.0);
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    let id = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(heca_grid_ui::WidgetModel::new(fading_root_open()), true, true),
        &mut window,
    );
    reg.show(&mut window, id);
    LayoutEngine::new().compute(&mut window, viewport);

    assert_eq!(
        crate::chrome::surface_node(&window, id)
            .expect("the layer is seated")
            .base()
            .bounds
            .size,
        viewport,
        "a layer declares its own full-viewport size and the seat must not shrink it",
    );
}

/// **A seated surface must not stand between the pointer and the page where it draws nothing**.
///
/// The notification stack's box is the whole window on purpose — that is how a corner means the
/// *screen's* corner — and it sits above the chrome, because child order is z-order. Take the
/// default input surface and the consequence is total and silent: every press anywhere in the app
/// hit-tests to the stack, so the chrome under it stops answering the mouse and a sidebar row can
/// never be right-clicked.
///
/// The guard is here, on the seating, rather than only in the widget: this is the arrangement that
/// makes it dangerous — a window-sized node in front of everything — and any future surface seated
/// the same way is subject to the same rule.
#[test]
fn a_surface_that_draws_nothing_does_not_take_the_pointer_from_the_chrome() {
    use heca_core::layout::{Point, Size};
    use heca_grid_ui::builders::{EventExt, Parent};
    use heca_grid_ui::reactive::signal;
    use heca_grid_ui::style::Length;
    use heca_grid_ui::widgets::{KeyHintGroup, ToastStack};
    use heca_grid_ui::{LayoutEngine, LayoutExt};

    let viewport = Size::new(1280.0, 800.0);
    let mut window = crate::chrome::new_window_root();
    // The chrome: a plain child of the window root, and the thing the pointer must reach. The
    // sidebar it holds answers a click; the chrome's own box answers nothing.
    crate::chrome::seat_chrome(
        &mut window,
        Flex::row().width(Length::FULL).height(Length::FULL).child(
            Flex::row()
                .width(Length::FULL)
                .height(Length::FULL)
                .on_click(|_| {}),
        ),
    );
    // The stack, seated **exactly as the app seats it** — wrapped in its picker group, and empty,
    // as it is nearly always. The wrapper is the point: a first version of this guard placed a bare
    // stack, passed, and the app was still broken, because what the pointer actually meets is the
    // decorator hugging it. A guard for a seating must build the arrangement the app seats.
    crate::chrome::place_surface(
        &mut window,
        "heca.notifications",
        heca_grid_ui::WidgetModel::new(KeyHintGroup::new(ToastStack::new(signal(Vec::new())))),
    );
    LayoutEngine::new().compute(&mut window, viewport);

    let path = heca_grid_ui::hit_test(&window, Point::new(146.0, 171.0))
        .expect("the chrome is under the pointer");
    assert_eq!(
        path.first(),
        Some(&0),
        "the pointer reached child {:?} — the chrome is child 0 and an empty surface must not \
         claim the point in front of it",
        path.first(),
    );
}

/// **A dismissed surface declares nothing.**
///
/// Hiding a surface calls `node.close()` and leaves it **seated** in the window root —
/// `remove_surface` runs only when it is destroyed — so presence in the tree says nothing about
/// being in charge. The exposé and the command palette both declare `lock` and `modal`,
/// so a reader that takes a seated surface's declaration at face value applies a hidden exposé's
/// occluders and treats a hidden modal as the active context.
///
/// That is exactly what happened: the `prefix+/` stack stopped asking the registry for its visible
/// layers and started walking the tree's children, and lost the `visible` filter it had been
/// getting for free — **every letter in the app disappeared**, the universal picker and `prefix+q`
/// alike, in every state.
#[test]
fn a_hidden_surfaces_declaration_is_not_live() {
    let mut window = Flex::column();
    let mut reg = LayerRegistry::default();

    // An on-demand surface that declares BOTH things a dismissed one must stop declaring — said
    // on the widget, which is the only place they live now.
    let id = reg.add(
        None,
        LayerKind::OnDemand,
        declaring(empty_root(), true, true),
        &mut window,
    );
    let slot = crate::chrome::surface_slot(id);

    // On-demand starts hidden, and its node is already seated.
    assert!(
        reg.declaration_at(&slot).is_some(),
        "the surface is seated and registered whether or not it is up",
    );
    assert!(
        !reg.slot_is_live(&window, &slot),
        "a surface that is not up declares nothing — reading it anyway is what blanked \
         every letter in the app",
    );

    reg.show(&mut window, id);
    assert!(
        reg.slot_is_live(&window, &slot),
        "a surface that is up declares what it declared",
    );
    let node = crate::chrome::surface_node(&window, id).expect("seated");
    assert!(
        node.base().lock,
        "and coverage is the widget's own declaration, not a record the registry kept",
    );

    reg.hide(&mut window, id);
    assert!(
        !reg.slot_is_live(&window, &slot),
        "dismissed again: seated, still registered, and declaring nothing",
    );
}
