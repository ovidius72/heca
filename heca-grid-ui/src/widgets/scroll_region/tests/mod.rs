
    use super::*;
    use crate::event::PointerButton;
    use std::cell::RefCell;
    use std::rc::Rc;


    type Log = Rc<RefCell<Vec<(&'static str, bool)>>>;
    /// A resolved wheel event over the region under test. The router puts a real one here only
    /// when the pointer is over the region; these tests call the widget directly, so they say so.
    fn wheel(delta_x: f32, delta_y: f32) -> Event {
        Event::Scroll(crate::event::PointerEvent {
            delta_x,
            delta_y,
            ..crate::event::PointerEvent::at(Point::new(10.0, 10.0))
        })
    }
    /// A **raw** wheel at a point — for the tests that go through the router, which is what
    /// decides whose wheel it is.
    fn wheel_at(pos: Point, delta_x: f32, delta_y: f32) -> Event {
        Event::wheel(pos, delta_x, delta_y)
    }
    fn region_with_children(child_heights: &[f64]) -> ScrollRegion {
        // Children are real components only for layout; here we just need bounds
        // set on them. We build a ScrollRegion and manually stamp child bounds
        // (as the layout engine would) so the geometry helpers are testable in
        // isolation. `applied_offset` starts at 0, so bounds are "natural".
        let mut r = ScrollRegion::new();
        r.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 100.0));
        r.base.children.clear();
        let mut y = 0.0;
        for &h in child_heights {
            let mut child = crate::widgets::Flex::column();
            child.base_mut().bounds = Rectangle::new(Point::new(0.0, y), Size::new(200.0, h));
            r.base.children.push(crate::WidgetModel::new(child));
            y += h;
        }
        r
    }
    /// A region with the given `axes` and child WIDTHS laid left→right in a 100×100
    /// viewport (mirrors `region_with_children`, which stamps heights).
    fn h_region_with_widths(axes: ScrollAxes, child_widths: &[f64]) -> ScrollRegion {
        let mut r = ScrollRegion::new().axes(axes);
        r.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        r.base.children.clear();
        let mut x = 0.0;
        for &w in child_widths {
            let mut child = crate::widgets::Flex::column();
            child.base_mut().bounds = Rectangle::new(Point::new(x, 0.0), Size::new(w, 100.0));
            r.base.children.push(crate::WidgetModel::new(child));
            x += w;
        }
        r
    }
    /// A region wired to all three callbacks, plus the log they write to.
    fn recorder() -> (Log, ScrollRegion) {
        let log: Log = Rc::new(RefCell::new(Vec::new()));
        let (a, b, c) = (log.clone(), log.clone(), log.clone());
        let r = region_with_children(&[400.0])
            .on_scroll_start(move |s| a.borrow_mut().push(("start", s.event.is_some())))
            .on_scroll(move |s| b.borrow_mut().push(("scroll", s.event.is_some())))
            .on_scroll_end(move |s| c.borrow_mut().push(("end", s.event.is_some())));
        (log, r)
    }

mod part_0;
mod part_1;
mod part_2;

mod follow_bottom;
