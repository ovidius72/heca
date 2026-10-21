use super::*;

    /// **Ctrl or Cmd with the wheel is a zoom, not a scroll.** The region lets it carry on outward
    /// to whatever zooms, and still scrolls for the plain wheel.
    #[test]
    fn a_zoom_wheel_is_not_a_scroll() {
        let scroll = |modifiers: crate::event::Modifiers| {
            let mut r = region_with_children(&[60.0, 60.0]);
            let ev = Event::Scroll(crate::event::PointerEvent {
                delta_y: 3.0,
                modifiers,
                ..crate::event::PointerEvent::at(Point::new(10.0, 10.0))
            });
            (r.on_event(&ev), r.scroll_offset.get_untracked())
        };
        let (plain, moved) = scroll(Default::default());
        assert_eq!(plain, Handled::Yes);
        assert!(moved > 0.0);
        for held in [
            crate::event::Modifiers {
                ctrl: true,
                ..Default::default()
            },
            crate::event::Modifiers {
                meta: true,
                ..Default::default()
            },
        ] {
            let (handled, offset) = scroll(held);
            assert_eq!(handled, Handled::No, "{held:?}");
            assert_eq!(offset, 0.0, "{held:?}");
        }
    }
    #[test]
    fn wheel_step_is_viewport_fraction_not_fixed_lines() {
        // Regression guard: the step must scale with the viewport, so a small
        // viewport doesn't jump ~75% per notch (the original overshoot bug).
        let r = region_with_children(&[60.0, 60.0]); // vp 100
        let step = WHEEL_STEP_FRAC * r.base.bounds.size.h;
        // 10% of viewport per line of delta — gentle, proportional.
        assert!((step - 10.0).abs() < f64::EPSILON);
    }
    #[test]
    fn ensure_visible_scrolls_down_when_item_is_below_viewport() {
        let mut r = region_with_children(&[60.0, 60.0]); // vp 100, max 20
        // child[1] natural 60..120, off 0 → visual 60..120, below vp (0..100).
        r.ensure_visible(Rectangle::new(
            Point::new(0.0, 60.0),
            Size::new(200.0, 60.0),
        ));
        // Scrolled so the item's bottom (120) aligns with the viewport bottom
        // (100): offset = 120 - 100 = 20 (= max_offset).
        assert!((r.scroll_offset.get_untracked() - 20.0_f32).abs() < 1e-6);
    }
    #[test]
    fn ensure_visible_scrolls_up_when_item_is_above_viewport() {
        let mut r = region_with_children(&[60.0, 60.0]);
        r.scroll_to(20.0); // applied_offset=20; child[0] visual y = 0 - 20 = -20
        // child[0] visual rect {y:-20, h:60} — top is above the viewport (y=0).
        r.ensure_visible(Rectangle::new(
            Point::new(0.0, -20.0),
            Size::new(200.0, 60.0),
        ));
        // Scrolled back so the item's top (natural 0) aligns with viewport top.
        assert!(r.scroll_offset.get_untracked().abs() < 1e-6);
    }
    /// **Centring is unconditional**, which is the difference from `ensure_visible`: a target
    /// already fully on screen but sitting off to one side is still brought back to the middle.
    /// That is what makes an overview read as a map — the current thing stays put and the
    /// neighbours move past it.
    #[test]
    fn center_on_puts_the_target_in_the_middle_even_when_it_is_already_visible() {
        let mut r = region_with_children(&[60.0, 60.0, 60.0, 60.0]); // vp 100, content 240
        // child[1] natural 60..120: its middle is 90, the viewport's is 50.
        r.center_on(Rectangle::new(
            Point::new(0.0, 60.0),
            Size::new(200.0, 60.0),
        ));
        assert!(
            (r.scroll_offset.get_untracked() - 40.0_f32).abs() < 1e-6,
            "90 (target middle) - 50 (viewport middle) = 40, not the 0 a minimal reveal would leave",
        );
    }
    /// The clamp is the whole end-of-content story: near the top the target stops short of the
    /// centre rather than the view scrolling into empty space. A surface that wants it centred
    /// there too pads its own content — the widget does not invent space its caller did not ask for.
    #[test]
    fn center_on_clamps_at_the_ends_instead_of_scrolling_into_nothing() {
        let mut r = region_with_children(&[60.0, 60.0, 60.0, 60.0]);
        r.center_on(Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 60.0)));
        assert!(
            r.scroll_offset.get_untracked().abs() < 1e-6,
            "the first child cannot go lower"
        );
        r.center_on(Rectangle::new(
            Point::new(0.0, 180.0),
            Size::new(200.0, 60.0),
        ));
        assert!(
            (r.scroll_offset.get_untracked() - 140.0_f32).abs() < 1e-6,
            "the last child stops at max_offset (240 content - 100 viewport)",
        );
    }
    /// **Content that fits is centred in place.** This is the half of "centred" that has nothing to
    /// do with scrolling, and the half that was missing: with three columns on a wide screen there
    /// is no offset that moves them anywhere, so the map opened hard against the top-left corner —
    /// which is the one thing an overview must never look like.
    #[test]
    fn content_smaller_than_the_region_is_centred_in_place() {
        let mut r = region_with_children(&[40.0]); // viewport 200 × 100, content 200 × 40
        r.axes = ScrollAxes::Both;
        r.reveal_align = RevealAlign::Center;
        r.base.children[0].base_mut().bounds =
            Rectangle::new(Point::new(0.0, 0.0), Size::new(80.0, 40.0));
        r.sync_shift();

        let b = r.base.children[0].base().bounds;
        assert!(
            (b.loc.x - 60.0).abs() < 1e-9,
            "(200 - 80) / 2 = 60 from the left, got {b:?}"
        );
        assert!(
            (b.loc.y - 30.0).abs() < 1e-9,
            "(100 - 40) / 2 = 30 from the top, got {b:?}"
        );
        assert!(
            r.scroll_offset.get_untracked().abs() < 1e-6
                && r.scroll_offset_x.get_untracked().abs() < 1e-6,
            "and it is not a scroll — there is nothing to scroll",
        );
    }
    /// The pad is **only** the slack, so it can never make the start of the content unreachable —
    /// the failure mode of a plain `justify_content: center`, which overflows both ways.
    #[test]
    fn overflowing_content_gets_no_centring_pad() {
        let mut r = region_with_children(&[60.0, 60.0, 60.0]); // content 180, viewport 100
        r.reveal_align = RevealAlign::Center;
        r.sync_shift();
        assert!(
            r.base.children[0].base().bounds.loc.y.abs() < 1e-9,
            "the first child stays at the top; centring here is the scroll's job",
        );
    }
    /// **A horizontal region centres horizontally.** A strip of cards is the case that needs it,
    /// and `ensure_visible` — vertical by construction — could never serve one.
    #[test]
    fn center_on_works_on_whichever_axis_the_region_scrolls() {
        let mut r = region_with_children(&[60.0]);
        r.axes = ScrollAxes::Horizontal;
        // One wide child: viewport 200 wide, content 600.
        r.base.children[0].base_mut().bounds =
            Rectangle::new(Point::new(0.0, 0.0), Size::new(600.0, 60.0));
        // A card at x 300..400 — its middle is 350, the viewport's is 100.
        r.center_on(Rectangle::new(
            Point::new(300.0, 0.0),
            Size::new(100.0, 60.0),
        ));
        assert!(
            (r.scroll_offset_x.get_untracked() - 250.0_f32).abs() < 1e-6,
            "350 - 100 = 250 horizontally",
        );
        assert!(
            r.scroll_offset.get_untracked().abs() < 1e-6,
            "and nothing vertically — that axis does not scroll here",
        );
    }
    /// **An eased region walks to its target over several frames**, and keeps asking for them
    /// until it arrives — a view that cut between two positions of the same picture would read as
    /// a different picture.
    #[test]
    fn a_smooth_region_glides_to_its_target_instead_of_jumping() {
        let mut r = region_with_children(&[60.0, 60.0, 60.0]).smooth_scroll(0.09); // content 180, vp 100
        r.sync_shift(); // the first sync snaps: a surface opens where it belongs
        assert!(r.applied_offset.abs() < 1e-9, "starts settled at the top");

        r.scroll_to(80.0); // programmatic — the eased path
        assert!(
            r.applied_offset < 80.0,
            "not there yet on the frame the move was asked for: {}",
            r.applied_offset,
        );
        let mut frames = 0;
        while r.tick(1.0 / 60.0) && frames < 120 {
            frames += 1;
        }
        assert!(
            frames > 1,
            "it took more than one frame — that is the animation"
        );
        assert!(frames < 120, "and it finished rather than easing forever");
        assert!(
            (r.applied_offset - 80.0).abs() < 0.5,
            "and it arrived exactly: {}",
            r.applied_offset,
        );
    }
    /// **A drag lands at once; a wheel glides.**
    ///
    /// The thing under the finger has to track it one to one — a thumb that lags reads as broken.
    /// A wheel notch has nothing under it: it is a discrete tenth-of-a-viewport step, so applying
    /// it instantly is a jump and a run of them is a run of jumps. Eased, the same notches read as
    /// one movement. Snapping the wheel was what made the map jerky under the mouse.
    #[test]
    fn a_drag_lands_at_once_while_a_wheel_glides() {
        let mut r = region_with_children(&[60.0, 60.0, 60.0]).smooth_scroll(0.09);
        r.sync_shift();

        let wheel = wheel(0.0, 3.0);
        r.set_offset_y(50.0, Some(&wheel));
        assert!(
            r.applied_offset < 50.0,
            "the wheel has somewhere to travel: {}",
            r.applied_offset,
        );
        let mut frames = 0;
        while r.tick(1.0 / 60.0) && frames < 240 {
            frames += 1;
        }
        assert!(frames > 1, "over several frames — that is the glide");

        let drag = Event::PointerMove(crate::event::PointerEvent::at(Point::new(0.0, 0.0)));
        r.set_offset_y(10.0, Some(&drag));
        assert!(
            (r.applied_offset - 10.0).abs() < 1e-9,
            "a pointer drag is there on the same frame: {}",
            r.applied_offset,
        );
    }
    /// **REVERSED.** This used to assert the opposite — that an overscrolling surface
    /// can be wheeled even when its content fits — on the reasoning that its position is carried by
    /// the centring pad, so `max_offset` is `0` whenever it fits, and clamping the range to
    /// `[0, 0]` left it "frozen under the mouse".
    ///
    /// That is reversed: the wheel must not scroll content that already fits. Freedom to scroll what is entirely visible is not responsiveness,
    /// it is a way to lose the picture — and in the exposé it did exactly that, wheeling the whole
    /// map out of the window with no way back but the keyboard. Overscroll is a **cushion past the
    /// ends**; content that fits has no ends to cushion.
    #[test]
    fn a_wheel_does_not_move_an_overscrolling_region_whose_content_fits() {
        let mut r = region_with_children(&[40.0]).overscroll(true);
        r.reveal_align = RevealAlign::Center;
        assert_eq!(
            r.max_offset(),
            0.0,
            "the content fits — there is nothing to scroll *into*"
        );
        r.on_event(&wheel(0.0, 3.0));
        assert_eq!(
            r.scroll_offset.get_untracked(),
            0.0,
            "so the wheel leaves it exactly where it is",
        );
    }
    /// **Shift+wheel reaches the horizontal axis** — the host maps the modifier to `delta_x`, and a
    /// horizontal region consumes it on the same terms.
    #[test]
    fn a_horizontal_delta_scrolls_a_horizontal_overscrolling_region() {
        // Wide enough to actually overflow: a region whose content fits has nowhere to go
        // (see `a_wheel_does_not_move_an_overscrolling_region_whose_content_fits`).
        let mut r = region_with_children(&[40.0]);
        r.axes = ScrollAxes::Horizontal;
        r.base.children[0].base_mut().bounds =
            Rectangle::new(Point::new(0.0, 0.0), Size::new(600.0, 40.0));
        r = r.overscroll(true);
        r.on_event(&wheel(3.0, 0.0));
        assert!(
            r.scroll_offset_x.get_untracked() > 0.0,
            "shift+wheel moves it sideways: {}",
            r.scroll_offset_x.get_untracked(),
        );
    }
    /// Moving the cursor supersedes wherever the user had wheeled to — otherwise the map would
    /// centre every later selection at the same offset from the middle, for good.
    #[test]
    fn moving_the_cursor_cancels_a_wheel_on_an_overscrolling_region() {
        // Tall enough to actually overflow, so there is a wheel position to cancel.
        let mut r = region_with_children(&[160.0]).overscroll(true);
        r.reveal_align = RevealAlign::Center;
        r.on_event(&wheel(0.0, 3.0));
        assert!(r.scroll_offset.get_untracked() > 0.0);

        let mut row = crate::widgets::Row::new();
        row.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(40.0, 40.0));
        row.nav_state().set(true);
        r.base.children.push(Box::new(row));
        r.follow_revealed();
        assert!(
            r.scroll_offset.get_untracked().abs() < 1e-6,
            "back to zero, so the pad centres the new target exactly",
        );
    }
    /// **No combination of pad and wheel can empty the view.** The position of an overscrolling
    /// surface is two numbers added together, and bounding each separately does not bound the sum —
    /// which is how the map twice ended up as an empty window with everything scrolled out of it.
    #[test]
    fn the_content_can_never_be_pushed_off_screen() {
        let mut r = region_with_children(&[40.0]).overscroll(true);
        r.reveal_align = RevealAlign::Center;
        // Wheel as hard as the range allows, repeatedly.
        for _ in 0..50 {
            r.on_event(&wheel(0.0, 20.0));
        }
        for _ in 0..200 {
            r.tick(1.0 / 60.0);
        }
        let child = r.base.children[0].base().bounds;
        let vp = r.base.bounds;
        assert!(
            child.loc.y < vp.loc.y + vp.size.h && child.loc.y + child.size.h > vp.loc.y,
            "the content still overlaps the viewport: {child:?} in {vp:?}",
        );
    }
    /// **`scrollbars(false)` draws no bar at all.** Gating only the reserved gutter left the thumb
    /// painted — visible on screen, grabbable, and taking up no space.
    #[test]
    fn a_region_with_scrollbars_off_paints_no_thumb() {
        let with = region_with_children(&[60.0, 60.0, 60.0]);
        assert!(
            with.thumb_rect().is_some(),
            "overflowing, so it would normally show one"
        );
        let without = region_with_children(&[60.0, 60.0, 60.0]).scrollbars(false);
        assert!(
            without.thumb_rect().is_none(),
            "and none at all when they are off"
        );
    }
