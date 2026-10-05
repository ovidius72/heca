use crate::builders::ComponentExt;
use crate::component::Component;
use crate::widgets::Flex;
use std::cell::RefCell;
use std::rc::Rc;

/// The right model is taken and reaches the widget's own handler; a value of another type is
/// refused, and so is a widget that takes none.
#[test]
fn a_widget_takes_the_props_it_declared_and_refuses_the_rest() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut widget = Flex::column().on_props({
        let seen = seen.clone();
        move |n: &u32, _| seen.borrow_mut().push(*n)
    });
    assert!(widget.set_props(&7_u32), "the declared type is taken");
    assert!(!widget.set_props(&"seven"), "another type is refused");
    assert_eq!(*seen.borrow(), vec![7], "only what was taken reached the handler");
    assert!(!Flex::column().set_props(&7_u32), "a widget with no props refuses");
}

/// **A widget places its own children from its props**: the handler is given the widget's base, so
/// it adds a child per number it is handed — and is still there for the next props.
#[test]
fn a_widget_builds_its_own_children_from_the_props_it_is_handed() {
    let mut widget = Flex::column().on_props(|count: &usize, base| {
        base.children.clear();
        for _ in 0..*count {
            base.children.push(Box::new(Flex::row()));
        }
    });
    assert!(widget.set_props(&3_usize));
    assert_eq!(widget.base().children.len(), 3);
    assert!(widget.set_props(&1_usize), "the handler is kept for the next props");
    assert_eq!(widget.base().children.len(), 1);
}
