//! **A description binds an intent to any event the native API has, by the native name.**
//!
//! One table (`heca_grid_ui::EVENT_NAMES`) names every event; `ViewEvent` is how an SDK spells
//! them. These tests hold the two lists equal and drive each name end to end: describe a node with
//! `.on("<name>", intent)`, realize it, make the event happen, and see the intent arrive.

use heca_grid_ui::event::{DragEvent, EVENT_NAMES, EventName, Modifiers, PointerEvent};
use heca_grid_ui::{Component, DropSide, Event, GridKey, LayoutEngine, Point, Size, Theme, WidgetIntent};
use heca_view::{Intent, ViewEvent, ViewNode, WidgetId};
use heca_view_realize::{FormBindings, IntentEmitter, realize};
use std::cell::RefCell;
use std::rc::Rc;

fn emitter() -> (IntentEmitter, Rc<RefCell<Vec<Intent>>>) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let out = seen.clone();
    (Rc::new(move |intent| out.borrow_mut().push(intent)), seen)
}

/// **The native names and the described names are one list.** Every native event can be described
/// and every described name is either a native event or an event a widget raises — so neither side
/// can grow an event the other cannot say.
#[test]
fn the_native_and_the_described_event_names_are_one_list() {
    let mut native: Vec<&str> = EVENT_NAMES.iter().map(|e| e.name).collect();
    let mut described: Vec<&str> = ViewEvent::ALL
        .iter()
        .filter(|event| !ViewEvent::WIDGET.contains(event))
        .map(|event| event.name())
        .collect();
    native.sort_unstable();
    described.sort_unstable();
    assert_eq!(
        native, described,
        "an EventKind with no ViewEvent, or a ViewEvent that is no native event, drifted"
    );
    for widget in ViewEvent::WIDGET {
        assert!(
            EventName::of(widget.name()).is_none(),
            "{} is a widget's own event — it must not shadow a native name",
            widget.name()
        );
    }
}

/// A sample of the event a name stands for.
fn sample(entry: &EventName) -> Event {
    let at = Point::new(1.0, 1.0);
    let p = PointerEvent::at(at);
    let drag = || DragEvent {
        item: "x".into(),
        pos: at,
        modifiers: Modifiers::default(),
        side: DropSide::Onto,
    };
    match entry.name {
        "key_down" => Event::Key { key: GridKey::Char('x'), pressed: true },
        "key_up" => Event::Key { key: GridKey::Char('x'), pressed: false },
        "text_input" => Event::TextInput("x".into()),
        "modifiers_changed" => Event::ModifiersChanged(Modifiers::default()),
        "widget" => Event::Widget(WidgetIntent::Dismiss),
        "pointer_down" => Event::PointerDown(p),
        "pointer_up" => Event::PointerUp(p),
        "pointer_move" => Event::PointerMove(p),
        "pointer_recheck" => Event::pointer_rechecked(at),
        "pointer_enter" => Event::PointerEnter(p),
        "pointer_leave" => Event::PointerLeave(p),
        "click" => Event::Click(p),
        "double_click" => Event::DoubleClick(p),
        "triple_click" => Event::TripleClick(p),
        "right_click" => Event::RightClick(p),
        "middle_click" => Event::MiddleClick(p),
        "pointer_down_outside" => Event::PointerDownOutside(p),
        "scroll" => Event::Scroll(p),
        "drag_start" => Event::DragStart(drag()),
        "drag" => Event::Drag(drag()),
        "drag_end" => Event::DragEnd(drag()),
        "drag_enter" => Event::DragEnter(drag()),
        "drag_over" => Event::DragOver(drag()),
        "drag_leave" => Event::DragLeave(drag()),
        "drop" => Event::Drop(drag()),
        "drag_in_flight" => Event::DragInFlight(drag()),
        "drag_settled" => Event::DragSettled(drag()),
        "focus_gained" => Event::Focus,
        "focus_lost" => Event::Blur,
        "mount" => Event::Mount,
        other => unreachable!("no sample for {other}: add the event it stands for"),
    }
}

/// **Every native event, by its name, reaches the intent bound to it** — on a plain `Surface`,
/// which has no click of its own: any widget can be asked about any event.
#[test]
fn every_native_event_reaches_the_intent_bound_to_its_name() {
    for entry in EVENT_NAMES {
        // `hint` is what a pick does (read with the node's other declarations, below); `unmount`
        // cannot be delivered — it fires as the widget is dropped, so it has its own test.
        if entry.name == "hint" || entry.name == "unmount" {
            continue;
        }
        let (emit, seen) = emitter();
        let node: ViewNode = ViewNode::new(WidgetId::Surface).on(entry.name, Intent::new(entry.name));
        let mut widget = realize(&node, &Theme::default(), &emit, &mut FormBindings::default())
            .expect("a surface realizes");
        LayoutEngine::new().compute(widget.as_mut(), Size::new(100.0, 100.0));
        seen.borrow_mut().clear(); // `mount` also fired once on that layout pass
        // A key is delivered to whoever holds the keyboard.
        widget.base().focus(false);
        seen.borrow_mut().clear();
        heca_grid_ui::deliver(widget.as_mut(), &sample(entry));
        assert!(
            seen.borrow().iter().any(|i| i.action == entry.name),
            "`.on({:?}, intent)` never fired when the event happened (got {:?})",
            entry.name,
            seen.borrow()
        );
    }
}

/// `unmount` fires as the widget is dropped, which is how a rebuilt tree throws one away.
#[test]
fn unmount_fires_when_the_described_widget_is_dropped() {
    let (emit, seen) = emitter();
    let node: ViewNode = ViewNode::new(WidgetId::Surface).on("unmount", Intent::new("unmount"));
    let widget = realize(&node, &Theme::default(), &emit, &mut FormBindings::default()).unwrap();
    assert!(seen.borrow().is_empty());
    drop(widget);
    assert_eq!(seen.borrow().len(), 1);
}

/// A name the table does not have is ignored, never a panic — a plugin ships JSON.
#[test]
fn an_unknown_event_name_is_ignored() {
    let (emit, seen) = emitter();
    let node: ViewNode = ViewNode::new(WidgetId::Surface).on("not_an_event", Intent::new("x"));
    let mut widget = realize(&node, &Theme::default(), &emit, &mut FormBindings::default()).unwrap();
    LayoutEngine::new().compute(widget.as_mut(), Size::new(100.0, 100.0));
    heca_grid_ui::deliver(widget.as_mut(), &Event::Focus);
    assert!(seen.borrow().is_empty());
}

/// **A node's `press` is the native `on_press`**: a click emits the intent, and a pick of the node
/// carries the same one as data — so a host can drop a letter whose action it would refuse.
#[test]
fn a_nodes_press_is_a_click_and_a_pick_carrying_one_intent() {
    let (emit, seen) = emitter();
    let press = Intent::new("close").arg("pane_id", 7_i64.into());
    let node: ViewNode = ViewNode::new(WidgetId::Button).text("Close").on_press(press.clone());
    let mut widget = realize(&node, &Theme::default(), &emit, &mut FormBindings::default()).unwrap();
    LayoutEngine::new().compute(widget.as_mut(), Size::new(200.0, 60.0));

    heca_grid_ui::click(widget.as_mut());
    assert_eq!(*seen.borrow(), vec![press.clone()], "the click emitted it");

    let targets = heca_grid_ui::collect_hints(widget.as_ref());
    assert_eq!(heca_grid_ui::hint_intent(widget.as_ref(), &targets[0].0), Some(press));
}
