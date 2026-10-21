use super::*;

    /// **A wheel over content that fits does nothing.** `overscroll` exists so a map larger than
    /// the window can be pushed past its ends; applied to content with no ends it gave a full
    /// viewport of travel in each direction, and the wheel carried the whole exposé off screen
    /// with no way back but the keyboard.
    #[test]
    fn overscroll_gives_no_travel_to_content_that_fits() {
        let mut r = region_with_children(&[40.0, 40.0]); // content 80, viewport 100
        r.overscroll = true;
        assert_eq!(r.max_offset(), 0.0, "the fixture must actually fit");
        assert_eq!(
            r.offset_bounds_y(),
            (0.0, 0.0),
            "so there is nowhere to scroll"
        );

        r.scroll_by(500.0, None);
        assert_eq!(
            r.scroll_offset.get_untracked(),
            0.0,
            "and the wheel moves nothing"
        );
    }
    /// The counterpart: real overflow still gets its cushion, so the map can be pushed past its
    /// last row rather than stopping dead against it.
    #[test]
    fn overscroll_still_cushions_content_that_overflows() {
        let mut r = region_with_children(&[80.0, 80.0]); // content 160, viewport 100
        r.overscroll = true;
        assert!(r.max_offset() > 0.0, "the fixture must actually overflow");
        let (min, max) = r.offset_bounds_y();
        assert!(
            min < 0.0 && max > r.max_offset(),
            "a viewport of cushion each way: {min}..{max}"
        );
    }
    /// **The content is cut short of the scrollbar's lane and tight on the scrolling axis**, by
    /// the framework, from the shape the region declares: what it holds is drawn inside that clip.
    #[test]
    fn content_is_cut_short_of_the_lane_and_tight_where_it_scrolls() {
        use crate::builders::{LayoutExt, Parent, StyleExt};
        use crate::scene::{DrawCommand, Scene};
        let child = || {
            crate::widgets::Surface::row()
                .height(80.0)
                .background(crate::color::Color::rgb(9, 9, 9))
        };
        let mut root = crate::widgets::Flex::column().child(
            ScrollRegion::new()
                .width(200.0)
                .height(100.0)
                .child(child())
                .child(child()),
        );
        crate::LayoutEngine::new().compute(&mut root, Size::new(400.0, 400.0));
        let theme = crate::theme::Theme::default();
        let mut scene = Scene::new();
        paint_child(&root, &mut PaintCx::new(&mut scene, &theme));
        let want = Size::new(200.0 - SCROLLBAR_GUTTER + 2.0 * GLOW_BLEED, 100.0);
        let clip = scene
            .iter()
            .position(|c| matches!(c, DrawCommand::PushClip(r) if r.size == want));
        let row = scene.iter().position(
            |c| matches!(c, DrawCommand::Rect(r) if r.fill == crate::color::Color::rgb(9, 9, 9)),
        );
        assert!(
            matches!((clip, row), (Some(c), Some(r)) if c < r),
            "the rows are drawn inside a clip of {want:?}: clip at {clip:?}, row at {row:?}"
        );
    }
    /// **The grab lane stays inside the space the bar reserved**.
    ///
    /// The reserved strip is what content is laid out and clipped short of; the lane is where a
    /// press counts as grabbing the thumb. While the lane was wider than the strip, 9px of it sat on
    /// the row beside it and one pixel belonged to two widgets — so a press there was ambiguous, and
    /// the host could only pick a winner by knowing this widget's internals. Nothing outside can fix
    /// that; keeping the hit area inside the reserved box is the widget's own job.
    #[test]
    fn the_grab_lane_does_not_reach_over_the_content() {
        // Three 60px children in a 100px viewport ⇒ it overflows, so the bar shows.
        let r = region_with_children(&[60.0, 60.0, 60.0]);
        assert!(
            r.v_overflow(),
            "the fixture must overflow for a bar to exist"
        );

        let lane = r
            .thumb_hit_rect()
            .expect("a scrollable region has a grab lane");
        let content_right = r.base.bounds.loc.x + r.base.bounds.size.w - SCROLLBAR_GUTTER;
        assert!(
            lane.loc.x >= content_right,
            "the lane starts at {} but content runs to {} — {}px belong to both",
            lane.loc.x,
            content_right,
            content_right - lane.loc.x,
        );
        // The painted bar is inside its own lane too, hard against the outer edge.
        let thumb = r.thumb_rect().expect("scrollable ⇒ a thumb");
        assert!(thumb.loc.x >= lane.loc.x);
        assert!(thumb.loc.x + thumb.size.w <= lane.loc.x + lane.size.w);
    }
    /// A keyboard page moves by most of a viewport, and the region clamps its own ends.
    ///
    /// The intents are semantic on purpose: the host says "one page on" and the
    /// widget decides what that means, because it is the only thing that knows its viewport and
    /// content. `ScrollToBottom` asks for `f64::MAX` and lands on the last scrollable pixel without
    /// the caller knowing where that is.
    #[test]
    fn a_keyboard_page_scrolls_by_most_of_a_viewport_and_clamps() {
        let mut r = region_with_children(&[100.0, 100.0, 100.0]); // 300 of content in a 100 viewport
        let page = PAGE_STEP_FRAC * 100.0; // 90

        assert_eq!(
            r.scroll_intent(
                WidgetIntent::ScrollPageDown,
                &Event::Widget(WidgetIntent::ScrollPageDown)
            ),
            Handled::Yes
        );
        assert!(
            (r.scroll_offset.get_untracked() as f64 - page).abs() < 0.01,
            "one page down"
        );

        r.scroll_intent(
            WidgetIntent::ScrollPageUp,
            &Event::Widget(WidgetIntent::ScrollPageUp),
        );
        assert_eq!(
            r.scroll_offset.get_untracked(),
            0.0,
            "back to the top, not past it"
        );

        r.scroll_intent(
            WidgetIntent::ScrollToBottom,
            &Event::Widget(WidgetIntent::ScrollToBottom),
        );
        assert_eq!(
            r.scroll_offset.get_untracked() as f64,
            r.max_offset(),
            "the bottom is the last scrollable pixel, clamped by the widget",
        );

        r.scroll_intent(
            WidgetIntent::ScrollToTop,
            &Event::Widget(WidgetIntent::ScrollToTop),
        );
        assert_eq!(r.scroll_offset.get_untracked(), 0.0);
    }
    /// A region declines an axis it cannot scroll, so the event carries on.
    ///
    /// This is what makes nesting and the host fallback work: a vertical-only region must not
    /// swallow a horizontal page, and a region whose content fits must not swallow anything. If it
    /// consumed them, an outer region would never see the key and the user would press it to no
    /// effect — silently.
    #[test]
    fn a_region_declines_an_axis_it_cannot_scroll() {
        // Vertical-only (the default), so horizontal intents are not ours.
        let mut r = region_with_children(&[100.0, 100.0, 100.0]);
        assert_eq!(
            r.scroll_intent(
                WidgetIntent::ScrollPageRight,
                &Event::Widget(WidgetIntent::ScrollPageRight)
            ),
            Handled::No,
            "a vertical-only region declines a horizontal page",
        );
        assert_eq!(r.scroll_offset_x.get_untracked(), 0.0, "and moves nothing");

        // Content that fits: nothing to scroll on either axis.
        let mut fits = region_with_children(&[40.0]);
        assert_eq!(
            fits.scroll_intent(
                WidgetIntent::ScrollPageDown,
                &Event::Widget(WidgetIntent::ScrollPageDown)
            ),
            Handled::No,
            "content that fits declines, so an outer region gets its turn",
        );

        // A non-scroll intent is never ours.
        assert_eq!(
            r.scroll_intent(
                WidgetIntent::Activate,
                &Event::Widget(WidgetIntent::Activate)
            ),
            Handled::No,
        );
    }
    /// The intents reach the region through normal dispatch, after the children.
    ///
    /// The unit tests above call the handler directly; this one goes through `dispatch` to prove the
    /// wiring, since `on_event` (not `on_event_capture`) is what gives the innermost region the
    /// first refusal — the same order the wheel uses.
    ///
    /// It puts the keyboard here first, because that is the whole of how a keyboard event
    /// finds anything: an intent is delivered to the focus owner and the region it encloses, so a
    /// tree with nothing focused has nowhere to deliver one.
    #[test]
    fn a_scroll_intent_arrives_through_dispatch() {
        let mut r = region_with_children(&[100.0, 100.0, 100.0]);
        r.base().focus(true);
        let handled =
            crate::component::dispatch(&mut r, &Event::Widget(WidgetIntent::ScrollPageDown));
        assert_eq!(handled, Handled::Yes);
        assert!(r.scroll_offset.get_untracked() > 0.0, "it scrolled");
    }
    #[test]
    fn content_extent_sums_children_and_clamps_to_viewport() {
        let r = region_with_children(&[60.0, 60.0]);
        assert!((r.content_extent() - 120.0).abs() < f64::EPSILON);
        assert!((r.max_offset() - 20.0).abs() < f64::EPSILON);
    }
    #[test]
    fn no_thumb_when_content_fits() {
        let r = region_with_children(&[40.0, 40.0]);
        assert!(r.thumb_rect().is_none());
        assert!(r.max_offset().abs() < f64::EPSILON);
    }
    #[test]
    fn thumb_appears_and_scales_with_visible_fraction() {
        let r = region_with_children(&[60.0, 60.0]);
        let t = r.thumb_rect().expect("scrollable → thumb");
        assert!((t.size.h - (100.0 / 120.0 * 100.0)).abs() < 1e-6);
        assert!(t.loc.y.abs() < 1e-6);
    }
    #[test]
    fn thumb_hit_lane_is_wider_than_the_visible_thumb_and_contains_it() {
        // Regression guard for "thumb misses clicks": the grab lane must be wider
        // than the thin visible thumb and must contain it, so a click near the
        // thumb still grabs.
        let r = region_with_children(&[60.0, 60.0]);
        let t = r.thumb_rect().expect("scrollable → thumb");
        let hit = r.thumb_hit_rect().expect("scrollable → hit lane");
        assert!(
            hit.size.w > t.size.w,
            "hit lane is wider than the visible thumb"
        );
        assert_eq!(hit.size.w as i32, THUMB_HIT_W as i32);
        // The visible thumb sits inside the lane horizontally.
        assert!(t.loc.x >= hit.loc.x - 0.001);
        assert!(t.loc.x + t.size.w <= hit.loc.x + hit.size.w + 0.001);
        // Same vertical span.
        assert!((hit.loc.y - t.loc.y).abs() < 1e-6);
        assert!((hit.size.h - t.size.h).abs() < 1e-6);
    }
    #[test]
    fn pressing_in_the_hit_lane_but_off_the_visible_thumb_still_grabs() {
        // A click in the grab padding (left of the thin thumb, inside the wider
        // lane) must still start a drag — the original 8px thumb missed these.
        let mut r = region_with_children(&[60.0, 60.0]);
        let t = r.thumb_rect().expect("scrollable → thumb");
        // A point just left of the visible thumb, inside the hit lane.
        let pos = Point::new(t.loc.x - 4.0, t.loc.y + 4.0);
        assert!(
            r.thumb_hit_rect().unwrap().contains(pos),
            "pos is in the hit lane"
        );
        assert!(!t.contains(pos), "pos is NOT on the thin visible thumb");
        let handled =
            crate::component::dispatch(&mut r, &Event::pointer_pressed(pos, PointerButton::Left));
        assert_eq!(handled, Handled::Yes, "press in the lane grabs the thumb");
        assert!(r.thumb_grab.is_some(), "a drag started");
    }
    #[test]
    fn scroll_to_clamps_to_max_offset() {
        let mut r = region_with_children(&[60.0, 60.0]); // max_offset 20
        assert!((r.scroll_to(50.0) - 20.0_f32).abs() < f32::EPSILON);
        assert!((r.scroll_to(-5.0) - 0.0_f32).abs() < f32::EPSILON);
        assert!((r.scroll_to(10.0) - 10.0_f32).abs() < f32::EPSILON);
    }
    #[test]
    fn sync_shift_bakes_offset_into_children_and_recovers_natural() {
        let mut r = region_with_children(&[60.0, 60.0]); // content 120, vp 100
        r.scroll_offset.set(20.0); // max_offset
        r.sync_shift();
        // applied_offset now equals scroll_offset.
        assert!((r.applied_offset - 20.0).abs() < f64::EPSILON);
        // Children shifted up by 20 (visual position): first child now at y=-20.
        assert!((r.base.children[0].base().bounds.loc.y - (-20.0)).abs() < f64::EPSILON);
        // Geometry helpers still report the *natural* extent (120) via the
        // applied_offset recovery — scrolling must not change content_extent.
        assert!((r.content_extent() - 120.0).abs() < f64::EPSILON);
        assert!((r.max_offset() - 20.0).abs() < f64::EPSILON);
    }
    #[test]
    fn on_layout_resets_applied_offset_so_shift_does_not_compound() {
        let mut r = region_with_children(&[60.0, 60.0]);
        r.scroll_offset.set(20.0);
        r.sync_shift();
        assert!(r.applied_offset.abs() > 0.0); // applied = 20
        // Simulate a fresh layout pass re-stamping natural bounds, then on_layout.
        let mut y = 0.0;
        for c in r.base.children.iter_mut() {
            c.base_mut().bounds = Rectangle::new(Point::new(0.0, y), Size::new(200.0, 60.0));
            y += 60.0;
        }
        r.on_layout();
        // on_layout re-applies the shift immediately (paint is \u0026self and can't
        // sync, so we must leave the bounds already shifted) — no snap to top, no
        // compounding: applied is back to the scroll offset and child 0 is at -20.
        assert!((r.applied_offset - 20.0).abs() < f64::EPSILON);
        assert!((r.base.children[0].base().bounds.loc.y - (-20.0)).abs() < f64::EPSILON);
        // A follow-up sync_shift is a no-op (already in sync) — no double shift.
        r.sync_shift();
        assert!((r.base.children[0].base().bounds.loc.y - (-20.0)).abs() < f64::EPSILON);
    }
