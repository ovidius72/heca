//! **Which surface acted** — the origin every intent a widget dispatches is stamped with.
//!
//! A widget says *what* it wants (`ev.dispatch(Intent)`) and says nothing about *where from*, so the
//! host sink used to stamp every intent `MouseContent`. That is the wrong answer for a dialog's
//! button (a layer acting on itself, which the policy judges differently from the page behind it)
//! and for the sidebar. The host knows where the event went, so it **records the origin around each
//! delivery** and the sink reads it back.
//!
//! One rule for it: [`source_at`] is what the `prefix+/` picker already used to judge a letter
//! ([`pick_source`](crate::chrome::hint)), so a letter is shown for the source its act will be
//! dispatched with — the two cannot disagree about the domain.

use crate::app::interaction::{InteractionIntent, InteractionSource as Source, SurfaceKey};
use crate::chrome::panes::workspace::WORKSPACE_KEY;
use crate::chrome::{CHROME_KEY, Intent, LayerRegistry};
use heca_grid_ui::{Component, Event};
use std::cell::Cell;

thread_local! {
    /// The origin of the delivery in progress, if one is. Thread-local because the UI delivers
    /// events on one thread and the sink is called from inside that delivery.
    static ORIGIN: Cell<Option<Source>> = const { Cell::new(None) };
}

/// Run `deliver` with `origin` recorded, and put back whatever was recorded before — a delivery can
/// start another (a tree that rebuilds a surface in a handler).
pub(crate) fn within<R>(origin: Option<Source>, deliver: impl FnOnce() -> R) -> R {
    let before = ORIGIN.with(|o| o.replace(origin));
    let out = deliver();
    ORIGIN.with(|o| o.set(before));
    out
}

/// The origin of the delivery in progress. `None` outside one.
pub(crate) fn current() -> Option<Source> {
    ORIGIN.with(Cell::get)
}

/// **The sink the host installs for a widget's `ev.dispatch(Intent)`**: every intent is stamped with
/// the origin recorded around the delivery that produced it, and with `MouseContent` — what the
/// sink always said — when nothing was recorded (a timer, a handle).
///
/// `post` is how the host carries it on (a message to its event loop); a test passes a recorder.
pub(crate) fn sink(
    post: impl Fn(Source, InteractionIntent) + 'static,
) -> impl Fn(Intent) + 'static {
    move |intent| {
        post(
            current().unwrap_or(Source::MouseContent),
            InteractionIntent::View(intent),
        )
    }
}

/// The parts of the window, besides its tree, that answer *"which surface is this path in, and what
/// source do its acts carry?"* — gathered once, so the picker and the delivery read the same thing.
pub(crate) struct Seats<'a> {
    pub(crate) layers: &'a LayerRegistry,
    /// The source the chrome tree's own emitter stamps (`RetainedChrome::intent_source`).
    pub(crate) chrome: Option<Source>,
}

impl<'a> Seats<'a> {
    /// From the two fields of the app state it is read from. Fields, not the state, so a caller can
    /// hold these and still deliver into `state.window_root`.
    pub(crate) fn new(
        layers: &'a LayerRegistry,
        chrome_tree: Option<&crate::chrome::RetainedChrome>,
    ) -> Self {
        Self {
            layers,
            chrome: chrome_tree.map(|t| t.intent_source),
        }
    }
}

/// **The source acts from the node of `window` at `path` are dispatched with** — `None` when the
/// path names no seat (the root itself, or a child that has gone).
///
/// **Which seat a path is in is its first step**: the window root's children are the workspace, the
/// chrome and the surfaces placed beside them, and each says which it is — the chrome and the
/// workspace by their key, a surface by the slot it was placed in. Each answers as the emitter that
/// built it does: the chrome tree carries its emitter's source, the workspace is the content area,
/// and a surface is its own key (a layer's own tree acting on itself, which `domain_for` tells apart
/// from the user typing at the app behind it).
pub(crate) fn source_at(window: &dyn Component, seats: &Seats<'_>, path: &[usize]) -> Option<Source> {
    let seat = window.base().children.get(*path.first()?)?.base();
    // A surface is named by its slot; the chrome and the workspace by the key they declare.
    let slot = seat.surface_slot.as_deref().or(seat.key.as_deref())?;
    if slot == CHROME_KEY {
        return Some(seats.chrome.unwrap_or(Source::Keyboard));
    }
    if slot == WORKSPACE_KEY {
        return Some(Source::MouseContent);
    }
    // A registered surface answers by its addressable **name** (`heca.expose`) and one nobody named
    // by `surface:<n>`; one that registered nothing is named by the slot it was placed in (the
    // toast stack's `heca.notifications`). Both arrive as the same `SurfaceKey`, which is what
    // stops a surface having two identities.
    Some(Source::Surface(
        seats
            .layers
            .declaration_at(slot)
            .map(|layer| seats.layers.surface_key(layer.id))
            .unwrap_or_else(|| SurfaceKey::of(slot)),
    ))
}

/// **Where the keyboard is, as a source** — what a key, typed text or a resolved intent is
/// delivered to.
pub(crate) fn keyboard_origin(window: &dyn Component, seats: &Seats<'_>) -> Option<Source> {
    source_at(window, seats, &heca_grid_ui::keyboard_owner(window)?.path)
}

/// **The origin of delivering `ev` to the window tree.**
///
/// A pointer event goes to what is under it and a keyboard event to what holds the keyboard — the
/// two rules the router itself uses to pick a target. `None` for an event that goes to no one place
/// (a modifier change).
pub(crate) fn origin_of(window: &dyn Component, seats: &Seats<'_>, ev: &Event) -> Option<Source> {
    match ev {
        Event::Key { .. } | Event::TextInput(_) | Event::Widget(_) => keyboard_origin(window, seats),
        _ => source_at(window, seats, &heca_grid_ui::hit_test(window, ev.position()?)?),
    }
}

/// **Deliver `ev` to the window tree, recording where it is going** — so an intent a widget
/// dispatches from inside it is stamped with that surface. The one door every event the host gives
/// the window goes through.
pub(crate) fn deliver_window(
    window: &mut dyn Component,
    seats: &Seats<'_>,
    ev: &Event,
) -> heca_grid_ui::Handled {
    let origin = origin_of(window, seats, ev);
    within(origin, || heca_grid_ui::dispatch(window, ev))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::{CHROME_KEY, LayerKind, new_window_root, place_surface, seat_chrome};
    use heca_grid_ui::builders::{EventExt, IdentityExt, LayoutExt, Parent};
    use heca_grid_ui::style::Length;
    use heca_grid_ui::widgets::{Button, Flex};
    use heca_grid_ui::{GridKey, LayoutEngine, Point, PointerButton, Size};
    use std::cell::RefCell;
    use std::rc::Rc;

    type Log = Rc<RefCell<Vec<(Source, Intent)>>>;

    /// Install the host sink over a recorder, so what a click does is what the app would have been
    /// told: the intent, and the source it was stamped with.
    fn host() -> Log {
        let log: Log = Default::default();
        let out = log.clone();
        heca_grid_ui::intent::install_intent_sink(sink(move |source, intent| {
            let InteractionIntent::View(intent) = intent else {
                unreachable!("the sink posts view intents")
            };
            out.borrow_mut().push((source, intent));
        }));
        log
    }

    /// A small tree with one button that says `act` — what any surface is built from.
    fn panel(act: &'static str) -> heca_grid_ui::WidgetModel {
        heca_grid_ui::WidgetModel::new(
            Flex::column()
                .width(Length::Px(120.0))
                .height(Length::Px(60.0))
                .child(Button::new("Go").on_press(Intent::new(act))),
        )
    }

    fn lay(window: &mut Flex) {
        LayoutEngine::new().compute(window, Size::new(400.0, 300.0));
    }

    /// The centre of the first button under the window root's child `at`.
    fn button_in(window: &Flex, at: usize) -> Point {
        fn find(node: &dyn Component) -> Option<&dyn Component> {
            if node.base().one_click_target {
                return Some(node);
            }
            node.base().children.iter().find_map(|c| find(c.as_ref()))
        }
        let b = find(window.base().children[at].as_ref())
            .expect("a button in that seat")
            .base()
            .bounds;
        Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0)
    }

    fn click(window: &mut Flex, seats: &Seats<'_>, at: Point) {
        lay(window);
        let _ = deliver_window(window, seats, &Event::pointer_moved(at));
        let _ = deliver_window(window, seats, &Event::pointer_pressed(at, PointerButton::Left));
        let _ = deliver_window(window, seats, &Event::pointer_released(at, PointerButton::Left));
    }

    fn only(log: &Log) -> (Source, String) {
        let log = log.borrow();
        assert_eq!(log.len(), 1, "exactly one intent: {log:?}");
        (log[0].0, log[0].1.action.clone())
    }

    /// **A click in the sidebar is the sidebar.** The chrome tree carries its emitter's source, and
    /// the sink says so — it used to say `MouseContent` for everything.
    #[test]
    fn a_click_in_the_chrome_arrives_with_the_chromes_source() {
        let log = host();
        let layers = LayerRegistry::default();
        let mut window = new_window_root();
        seat_chrome(&mut window, Flex::column().child(Button::new("Go").on_press(Intent::new("side"))));
        let seats = Seats { layers: &layers, chrome: Some(Source::MouseLeftSidebar) };
        lay(&mut window);
        let at = button_in(&window, 0);
        click(&mut window, &seats, at);
        assert_eq!(only(&log), (Source::MouseLeftSidebar, "side".into()));
    }

    /// **A click in a named surface (the exposé) is that surface acting on itself** — the source
    /// the policy needs to let the map act while it holds the screen.
    #[test]
    fn a_click_in_the_expose_arrives_as_that_surface() {
        let log = host();
        let mut layers = LayerRegistry::default();
        let mut window = new_window_root();
        let id = layers.reserve_id();
        layers.add_named(id, "heca.expose".into(), None, LayerKind::OnDemand, panel("expose_act"), &mut window);
        let seats = Seats { layers: &layers, chrome: Some(Source::MouseLeftSidebar) };
        lay(&mut window);
        let at = button_in(&window, 0);
        click(&mut window, &seats, at);
        assert_eq!(
            only(&log),
            (Source::Surface(SurfaceKey::of("heca.expose")), "expose_act".into())
        );
    }

    /// **A click in a dialog nobody named is that dialog** — `surface:<n>` — which is what lets its
    /// button submit *its own* overlay rather than whichever is in front.
    #[test]
    fn a_click_in_an_unnamed_dialog_arrives_as_that_dialog_and_names_its_layer() {
        let log = host();
        let mut layers = LayerRegistry::default();
        let mut window = new_window_root();
        let id = layers.reserve_id();
        layers.insert(id, None, LayerKind::OnDemand, panel("submit"), &mut window);
        let seats = Seats { layers: &layers, chrome: None };
        lay(&mut window);
        let at = button_in(&window, 0);
        click(&mut window, &seats, at);
        let (source, action) = only(&log);
        assert_eq!(action, "submit");
        assert_eq!(source, Source::Surface(layers.surface_key(id)));
        let Source::Surface(key) = source else { unreachable!() };
        assert_eq!(layers.id_of_surface(key), Some(id), "the stamp names the layer it came from");
    }

    /// A surface that registered nothing (the toast stack) is named by the slot it was placed in.
    #[test]
    fn a_click_in_an_unregistered_surface_is_named_by_its_slot() {
        let log = host();
        let layers = LayerRegistry::default();
        let mut window = new_window_root();
        place_surface(&mut window, "heca.notifications", panel("toast"));
        let seats = Seats { layers: &layers, chrome: None };
        lay(&mut window);
        let at = button_in(&window, 0);
        click(&mut window, &seats, at);
        assert_eq!(
            only(&log),
            (Source::Surface(SurfaceKey::of("heca.notifications")), "toast".into())
        );
    }

    /// The panes are the content area.
    #[test]
    fn a_click_in_the_workspace_is_content() {
        let log = host();
        let layers = LayerRegistry::default();
        let mut window = new_window_root();
        window.base_mut().children.push(heca_grid_ui::WidgetModel::new(
            Flex::column()
                .width(Length::Px(120.0))
                .height(Length::Px(60.0))
                .child(Button::new("Go").on_press(Intent::new("pane")))
                .key(crate::chrome::panes::workspace::WORKSPACE_KEY),
        ));
        let seats = Seats { layers: &layers, chrome: Some(Source::MouseLeftSidebar) };
        lay(&mut window);
        let at = button_in(&window, 0);
        click(&mut window, &seats, at);
        assert_eq!(only(&log), (Source::MouseContent, "pane".into()));
    }

    /// **Enter is delivered to where the keyboard is**, and stamped as that surface.
    #[test]
    fn a_key_is_stamped_as_the_surface_that_holds_the_keyboard() {
        let log = host();
        let mut layers = LayerRegistry::default();
        let mut window = new_window_root();
        let id = layers.reserve_id();
        layers.add_named(id, "heca.expose".into(), None, LayerKind::OnDemand, panel("expose_act"), &mut window);
        let seats = Seats { layers: &layers, chrome: None };
        lay(&mut window);
        let at = button_in(&window, 0);
        // Hand the button the keyboard, as a press on it would.
        let _ = deliver_window(&mut window, &seats, &Event::pointer_moved(at));
        let _ = deliver_window(&mut window, &seats, &Event::pointer_pressed(at, PointerButton::Left));
        let _ = deliver_window(&mut window, &seats, &Event::pointer_released(at, PointerButton::Left));
        log.borrow_mut().clear();
        let enter = Event::Key { key: GridKey::Enter, pressed: true };
        let _ = deliver_window(&mut window, &seats, &enter);
        assert_eq!(
            only(&log),
            (Source::Surface(SurfaceKey::of("heca.expose")), "expose_act".into())
        );
    }

    /// With nothing recorded — a timer, a handle — the sink says what it always said.
    #[test]
    fn an_intent_with_no_recorded_origin_is_mouse_content() {
        let log = host();
        heca_grid_ui::EventCx::new(&Event::Focus).dispatch(Intent::new("x"));
        assert_eq!(only(&log), (Source::MouseContent, "x".into()));
    }

    /// A delivery inside a delivery puts the outer origin back when it ends.
    #[test]
    fn a_nested_delivery_restores_the_outer_origin() {
        assert_eq!(current(), None);
        within(Some(Source::Keyboard), || {
            within(Some(Source::MouseLeftSidebar), || {
                assert_eq!(current(), Some(Source::MouseLeftSidebar));
            });
            assert_eq!(current(), Some(Source::Keyboard));
        });
        assert_eq!(current(), None);
    }

    #[test]
    fn the_chrome_key_names_the_chrome_seat() {
        // The mapping reads this key; renaming it must fail here, not silently re-source the sidebar.
        assert_eq!(CHROME_KEY, "heca.chrome");
    }
}
