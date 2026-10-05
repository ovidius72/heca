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
        move |n: &u32| seen.borrow_mut().push(*n)
    });
    assert!(widget.set_props(&7_u32), "the declared type is taken");
    assert!(!widget.set_props(&"seven"), "another type is refused");
    assert_eq!(*seen.borrow(), vec![7], "only what was taken reached the handler");
    assert!(!Flex::column().set_props(&7_u32), "a widget with no props refuses");
}
