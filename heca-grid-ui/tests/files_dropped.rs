//! **Dropped files go to what they were dropped on** — the widget under the point, then up its
//! ancestors, as a click does.

mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::{Event, EventKind, Handled, LayoutEngine, Point, Size};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

/// Which listener heard a drop, and the paths it carried.
type Heard = Rc<RefCell<Vec<(&'static str, Vec<PathBuf>)>>>;

#[test]
fn dropped_files_reach_the_widget_under_the_point_and_its_ancestors() {
    let got: Heard = Rc::default();
    let listener = |name: &'static str| {
        let got = got.clone();
        move |cx: &mut EventCx| {
            if let Event::FilesDropped { paths, .. } = cx.event() {
                got.borrow_mut().push((name, paths.clone()));
            }
        }
    };
    let side = || Flex::empty().width(Length::Px(100.0)).height(Length::Px(50.0));
    let mut root = Flex::row()
        .on(EventKind::FilesDropped, listener("row"))
        .child(side().on(EventKind::FilesDropped, listener("left")))
        .child(side().on(EventKind::FilesDropped, listener("right")));
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 50.0));
    // The keyboard is on the left one: a drop does not follow it.
    common::give_keyboard(root.base_mut().children[0].as_mut());
    let paths = vec![PathBuf::from("/tmp/a b.txt")];
    let drop_at = |root: &mut Flex, x: f64| {
        heca_grid_ui::dispatch(root, &Event::FilesDropped { paths: paths.clone(), pos: Point::new(x, 25.0) })
    };
    assert_eq!(drop_at(&mut root, 150.0), Handled::No, "nobody took it");
    assert_eq!(
        *got.borrow(),
        vec![("right", paths.clone()), ("row", paths.clone())],
        "the widget under the point first, then its ancestors; never the one holding the keyboard",
    );
    got.borrow_mut().clear();
    drop_at(&mut root, 500.0);
    assert!(got.borrow().is_empty(), "outside every widget is nobody's drop");
}
