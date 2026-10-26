use super::*;

    /// The alignment travels as a property, so a described surface reaches it — the capability
    /// `axes` and `placeholder` both lacked for months.
    #[test]
    fn the_reveal_alignment_is_a_property_and_defaults_to_minimal() {
        use crate::prop::{PropInput, SetProp};
        assert_eq!(
            ScrollRegion::new().clone_reveal_align(),
            RevealAlign::Minimal
        );
        let centred =
            ScrollRegion::new().set_prop("reveal_align", &PropInput::Text("center".into()));
        assert_eq!(centred.clone_reveal_align(), RevealAlign::Center);
    }
    #[test]
    fn ensure_visible_is_noop_when_item_already_visible() {
        let mut r = region_with_children(&[60.0, 60.0]);
        r.scroll_to(0.0);
        // child[0] natural 0..60 → visual 0..60, fully inside vp 0..100.
        r.ensure_visible(Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 60.0)));
        assert!(
            r.scroll_offset.get_untracked().abs() < 1e-6,
            "no scroll when already visible"
        );
    }
    #[test]
    fn scroll_to_child_brings_a_direct_child_into_view() {
        let mut r = region_with_children(&[60.0, 60.0]); // max 20
        r.scroll_to_child(1); // child[1] below → scroll to 20
        assert!((r.scroll_offset.get_untracked() - 20.0_f32).abs() < 1e-6);
        // Out-of-range index is a no-op (no panic, no scroll change).
        let before = r.scroll_offset.get_untracked();
        r.scroll_to_child(99);
        assert!((r.scroll_offset.get_untracked() - before).abs() < f32::EPSILON);
    }
    /// A relayout that GROWS the viewport (resize / zoom-out) must re-clamp a
    /// stale offset — otherwise content stays shifted past the edge with no
    /// scrollbar left to bring it back (the "scrollbar disappears after zoom"
    /// regression).
    #[test]
    fn on_layout_reclamps_a_stale_offset_when_the_viewport_grows() {
        let mut r = region_with_children(&[60.0, 60.0]); // vp 100, content 120
        r.scroll_to(20.0); // at max
        // The viewport grows past the content; layout restamps natural bounds.
        r.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 200.0));
        let mut y = 0.0;
        for c in r.base.children.iter_mut() {
            c.base_mut().bounds = Rectangle::new(Point::new(0.0, y), Size::new(200.0, 60.0));
            y += 60.0;
        }
        r.on_layout();
        // Offset re-clamped to the new max (0): content back at the top, not left
        // shifted off-screen.
        assert!(r.scroll_offset.get_untracked().abs() < f32::EPSILON);
        assert!(r.base.children[0].base().bounds.loc.y.abs() < f64::EPSILON);
    }
    /// A held press in the scrollbar track pages once immediately, then repeats
    /// after the initial delay at the repeat interval, and stops on release.
    #[test]
    fn held_track_press_repeats_paging_until_released() {
        // vp 100 (h), content 600 → max 500; one page = 100.
        let mut r = region_with_children(&[300.0, 300.0]);
        // Press in the vertical track lane, below the thumb.
        let pos = Point::new(195.0, 90.0);
        assert_eq!(
            crate::component::dispatch(&mut r, &Event::pointer_pressed(pos, PointerButton::Left)),
            Handled::Yes
        );
        assert!(
            (r.scroll_offset.get_untracked() - 100.0).abs() < 1e-3,
            "first page fires on press"
        );
        // Held but before the initial delay: animating, no extra page.
        assert!(r.tick(0.2), "held press keeps the host ticking");
        assert!((r.scroll_offset.get_untracked() - 100.0).abs() < 1e-3);
        // Past the delay: a repeat fires.
        r.tick(0.2);
        assert!(
            (r.scroll_offset.get_untracked() - 200.0).abs() < 1e-3,
            "repeat after the delay"
        );
        // Steady repeat at the interval.
        r.tick(0.1);
        assert!(
            (r.scroll_offset.get_untracked() - 300.0).abs() < 1e-3,
            "repeat at the interval"
        );
        // Release disarms it: no further paging however long we tick.
        crate::component::dispatch(&mut r, &Event::pointer_released(pos, PointerButton::Left));
        assert!(!r.tick(1.0));
        assert!(
            (r.scroll_offset.get_untracked() - 300.0).abs() < 1e-3,
            "stopped on release"
        );
    }
    #[test]
    fn horizontal_extent_and_max_offset() {
        let r = h_region_with_widths(ScrollAxes::Both, &[80.0, 80.0]); // content 160, vp 100
        assert!((r.content_extent_x() - 160.0).abs() < f64::EPSILON);
        assert!((r.max_offset_x() - 60.0).abs() < f64::EPSILON);
    }
    #[test]
    fn horizontal_axis_off_means_no_horizontal_scroll() {
        // A vertical-only region ignores horizontal overflow entirely.
        let r = h_region_with_widths(ScrollAxes::Vertical, &[80.0, 80.0]);
        assert_eq!(r.max_offset_x(), 0.0);
        assert!(r.h_thumb_rect().is_none());
    }
    #[test]
    fn horizontal_thumb_appears_only_when_content_overflows() {
        let fits = h_region_with_widths(ScrollAxes::Horizontal, &[40.0, 40.0]); // 80 ≤ 100
        assert!(fits.h_thumb_rect().is_none());
        let over = h_region_with_widths(ScrollAxes::Horizontal, &[80.0, 80.0]); // 160 > 100
        assert!(over.h_thumb_rect().is_some());
    }
    #[test]
    fn scroll_to_x_clamps_to_bounds() {
        let mut r = h_region_with_widths(ScrollAxes::Both, &[80.0, 80.0]); // max_x 60
        assert!(
            (r.scroll_to_x(1000.0) - 60.0).abs() < f32::EPSILON,
            "clamped to max"
        );
        assert!((r.scroll_to_x(-5.0)).abs() < f32::EPSILON, "clamped to 0");
    }
    /// Nested regions: the wheel goes to children FIRST, so an inner hovered
    /// scrollable consumes it and the outer region stays put (innermost wins).
    #[test]
    fn nested_region_consumes_the_wheel_before_the_outer_one() {
        // Outer: 200×100 viewport, vertical content 300 (scrollable).
        let mut outer = ScrollRegion::new();
        outer.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 100.0));
        // Inner region at the top of the outer content: 100×50 viewport, content 200.
        let mut inner = ScrollRegion::new();
        inner.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 50.0));
        let mut tall = crate::widgets::Flex::column();
        tall.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 200.0));
        inner.base.children.push(Box::new(tall));
        let inner_offset = inner.scroll_offset();
        outer.base.children.push(Box::new(inner));
        // Filler that makes the OUTER content overflow too.
        let mut filler = crate::widgets::Flex::column();
        filler.base_mut().bounds = Rectangle::new(Point::new(0.0, 50.0), Size::new(200.0, 250.0));
        outer.base.children.push(Box::new(filler));

        // The wheel is routed by position now: over the inner region, it is the inner region's.
        let pos = Point::new(10.0, 10.0);
        let handled = crate::component::dispatch(&mut outer, &wheel_at(pos, 0.0, 1.0));
        assert_eq!(handled, Handled::Yes);
        assert!(
            inner_offset.get_untracked() > 0.0,
            "inner (hovered) region scrolled"
        );
        assert!(
            outer.scroll_offset.get_untracked().abs() < f32::EPSILON,
            "outer region did not scroll while the inner one consumed the wheel"
        );

        // Over the outer region but OFF the inner one: now the outer scrolls.
        let pos = Point::new(150.0, 80.0);
        let handled = crate::component::dispatch(&mut outer, &wheel_at(pos, 0.0, 1.0));
        assert_eq!(handled, Handled::Yes);
        assert!(
            outer.scroll_offset.get_untracked() > 0.0,
            "outer region scrolls when no child consumed the wheel"
        );
    }
    /// A horizontal-only region shows NO vertical thumb even with tall content, and a
    /// horizontal thumb when it overflows — the axes gate both scrollbars.
    #[test]
    fn horizontal_only_region_gates_both_scrollbars() {
        let mut r = ScrollRegion::new().horizontal();
        r.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        let mut child = crate::widgets::Flex::column();
        child.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(300.0, 500.0));
        r.base.children.push(Box::new(child));
        assert!(
            r.thumb_rect().is_none(),
            "vertical axis disabled → no vertical thumb"
        );
        assert_eq!(r.max_offset(), 0.0);
        assert!(
            r.h_thumb_rect().is_some(),
            "horizontal overflow → horizontal thumb"
        );
    }
    /// The host's own `scroll_to` reports too — with **no event**, which is exactly how a listener
    /// avoids reacting to its own call and fighting the user.
    #[test]
    fn a_programmatic_scroll_reports_with_no_event() {
        let (log, mut r) = recorder();
        r.scroll_to(40.0);
        assert_eq!(
            log.borrow().as_slice(),
            &[("start", false), ("scroll", false)],
            "start then scroll, neither caused by input",
        );
    }
    /// Nothing is reported when nothing moved — a clamped no-op at either end is not a scroll.
    #[test]
    fn a_clamped_no_op_reports_nothing() {
        let (log, mut r) = recorder();
        r.scroll_to(0.0);
        assert!(log.borrow().is_empty(), "already at the top; nothing moved");
        r.scroll_to(1e9);
        let n = log.borrow().len();
        r.scroll_to(1e9);
        assert_eq!(
            log.borrow().len(),
            n,
            "already at the bottom the second time"
        );
    }
    /// The wheel carries its event through, and `on_scroll` fires per movement.
    #[test]
    fn the_wheel_reports_with_the_event_that_caused_it() {
        let (log, mut r) = recorder();
        crate::component::dispatch(&mut r, &Event::pointer_moved(Point::new(50.0, 50.0)));
        crate::component::dispatch(&mut r, &wheel(0.0, 1.0));
        assert_eq!(
            log.borrow().as_slice(),
            &[("start", true), ("scroll", true)]
        );
    }
    /// A wheel gesture has no release, so its end is a **pause**: `tick` must reach the idle
    /// timeout before `scroll_end` fires, and must keep asking for frames until it does.
    #[test]
    fn a_wheel_gesture_ends_after_the_idle_timeout_and_not_before() {
        let (log, mut r) = recorder();
        crate::component::dispatch(&mut r, &Event::pointer_moved(Point::new(50.0, 50.0)));
        crate::component::dispatch(&mut r, &wheel(0.0, 1.0));

        assert!(
            r.tick(WHEEL_IDLE_END / 2.0),
            "still pending: keep the frames coming"
        );
        assert!(
            !log.borrow().iter().any(|(k, _)| *k == "end"),
            "half the idle time is not the end of a gesture",
        );
        r.tick(WHEEL_IDLE_END);
        assert_eq!(log.borrow().last(), Some(&("end", false)), "settled");
        assert!(
            !r.tick(1.0),
            "and it stops asking for frames once it has settled"
        );
    }
    /// A thumb drag ends on its release — no timer involved, and the release is the event reported.
    #[test]
    fn a_thumb_drag_ends_on_the_release() {
        let (log, mut r) = recorder();
        let lane_x = r.base.bounds.loc.x + r.base.bounds.size.w - 2.0;
        crate::component::dispatch(
            &mut r,
            &Event::pointer_pressed(Point::new(lane_x, 5.0), PointerButton::Left),
        );
        crate::component::dispatch(&mut r, &Event::pointer_moved(Point::new(lane_x, 40.0)));
        assert!(
            log.borrow().iter().any(|(k, _)| *k == "scroll"),
            "the drag scrolled it"
        );
        assert!(
            !log.borrow().iter().any(|(k, _)| *k == "end"),
            "not while the button is down"
        );

        crate::component::dispatch(
            &mut r,
            &Event::pointer_released(Point::new(900.0, 900.0), PointerButton::Left),
        );
        assert_eq!(
            log.borrow().last(),
            Some(&("end", true)),
            "the release ended it"
        );
    }
    /// The payload answers the questions a listener would otherwise have to ask the region.
    #[test]
    fn the_payload_carries_the_whole_picture() {
        let seen = Rc::new(RefCell::new(None));
        let sink = seen.clone();
        let mut r = region_with_children(&[400.0]).on_scroll(move |s| {
            sink.replace(Some((s.offset_y, s.max_y, s.content.h, s.viewport.h)));
        });
        r.scroll_to(1e9);
        let (offset_y, max_y, content_h, viewport_h) = seen.borrow().expect("reported");
        assert_eq!(
            offset_y, max_y,
            "scrolled to the bottom, and it says so without asking"
        );
        assert_eq!(content_h, 400.0);
        assert_eq!(viewport_h, 100.0);
        assert_eq!(max_y, (content_h - viewport_h) as f32);
    }
    #[test]
    fn releasing_one_request_does_not_reveal_an_unchanged_fallback() {
        let mut r = region_with_children(&[40.0, 400.0, 40.0]);
        let mut first = crate::widgets::Row::new().nav_selected(true);
        let mut last = crate::widgets::Row::new().nav_selected(true);
        let first_nav = first.nav_state();
        first.base_mut().bounds = r.base.children[0].base().bounds;
        last.base_mut().bounds = r.base.children[2].base().bounds;
        r.base.children[0] = Box::new(first);
        r.base.children[2] = Box::new(last);
        r.follow_revealed();
        r.scroll_to(100.0);

        first_nav.set(false);
        r.follow_revealed();

        assert_eq!(r.scroll_offset.get_untracked(), 100.0,
            "the fallback was already requesting: releasing another request is not navigation");
    }

    #[test]
    fn a_fallback_that_requests_again_is_revealed() {
        let mut r = region_with_children(&[40.0, 400.0, 40.0]);
        let mut first = crate::widgets::Row::new().nav_selected(true);
        let mut last = crate::widgets::Row::new().nav_selected(true);
        let (first_nav, last_nav) = (first.nav_state(), last.nav_state());
        first.base_mut().bounds = r.base.children[0].base().bounds;
        last.base_mut().bounds = r.base.children[2].base().bounds;
        r.base.children[0] = Box::new(first);
        r.base.children[2] = Box::new(last);
        r.follow_revealed();
        r.scroll_to(100.0);
        first_nav.set(false);
        last_nav.set(false);
        r.follow_revealed();

        last_nav.set(true);
        r.follow_revealed();

        assert_eq!(r.scroll_offset.get_untracked(), 380.0,
            "a renewed navigation request still brings its row into view");
    }

    #[test]
    fn manual_scrolling_pauses_reflow_following_until_typing_moves_the_target() {
        let mut r = region_with_children(&[40.0, 400.0, 40.0]);
        let mut row = crate::widgets::Row::new().nav_selected(true);
        row.base_mut().bounds = r.base.children[2].base().bounds;
        r.base.children[2] = Box::new(row);
        r.follow_revealed();
        r.scroll_to(100.0);

        r.base.children[2].base_mut().bounds.loc.y += 20.0;
        r.follow_revealed();
        assert_eq!(r.scroll_offset.get_untracked(), 100.0,
            "layout reflow must not undo the reader's scroll");

        r.on_event_capture(&Event::TextInput("x".into()));
        r.base.children[2].base_mut().bounds.loc.y += 20.0;
        r.follow_revealed();
        assert_eq!(r.scroll_offset.get_untracked(), 420.0,
            "typing resumes following a moving cursor");
    }
