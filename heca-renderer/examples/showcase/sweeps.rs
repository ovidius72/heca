
    use super::*;

    #[test]
    fn clicking_a_visible_control_does_not_move_the_page() {
        fn visible_controls(node: &dyn Component, viewport: Size, points: &mut Vec<Point>) {
            let b = node.base();
            if !b.visible.get_untracked() {
                return;
            }
            let rect = b.bounds;
            if node.focusable() && rect.loc.y >= 0.0 && rect.loc.y + rect.size.h <= viewport.h {
                points.push(Point::new(rect.loc.x + rect.size.w / 2.0, rect.loc.y + rect.size.h / 2.0));
            }
            for child in &b.children { visible_controls(child.as_ref(), viewport, points); }
        }
        let theme = Theme::default();
        let ctl = ThemeCtl {
            glow: signal(theme.colors.glow_size), radius: signal(theme.colors.border_radius),
            border: signal(theme.colors.border_width), font: signal(theme.font_size),
            intensity: signal(theme.colors.intensity), overlay_frame: signal(theme.colors.overlay_frame),
            size: signal(WidgetSize::Normal), zoom: signal(ZOOM_DEFAULT), theme_idx: signal(0),
        };
        let mut built = build_ui(&theme, ctl);
        let viewport = Size::new(1400.0, 900.0);
        built.ui.base_mut().style.layout.width = Length::Px(viewport.w as f32);
        built.ui.base_mut().style.layout.height = Length::Px(viewport.h as f32);
        LayoutEngine::new().theme(&theme).compute(&mut built.ui, viewport);
        let maximum = built.ui.scroll_to(f32::MAX);
        let mut focus = FocusManager::new();
        for offset in (0..=maximum as usize).step_by(400) {
            built.ui.scroll_to(offset as f32);
            let mut points = Vec::new();
            visible_controls(&built.ui, viewport, &mut points);
            for point in points {
                let before = built.ui.scroll_offset().get_untracked();
                focus.dispatch(&mut built.ui, &Event::pointer_pressed(point, heca_grid_ui::PointerButton::Left));
                LayoutEngine::new().theme(&theme).compute(&mut built.ui, viewport);
                assert_eq!(built.ui.scroll_offset().get_untracked(), before,
                    "a pointer press at {point:?} at offset {offset} must not reveal another demo");
                focus.dispatch(&mut built.ui, &Event::pointer_released(point, heca_grid_ui::PointerButton::Left));
                LayoutEngine::new().theme(&theme).compute(&mut built.ui, viewport);
                assert_eq!(built.ui.scroll_offset().get_untracked(), before,
                    "a pointer release at {point:?} at offset {offset} must not reveal another demo");
            }
        }
    }

    #[test]
    fn the_message_demo_wraps_to_the_window_despite_wide_catalog_recipes() {
        for width in [900.0, 600.0, 320.0] {
            let theme = Theme::default();
            let ctl = ThemeCtl {
                glow: signal(theme.colors.glow_size), radius: signal(theme.colors.border_radius),
                border: signal(theme.colors.border_width), font: signal(theme.font_size),
                intensity: signal(theme.colors.intensity), overlay_frame: signal(theme.colors.overlay_frame),
                size: signal(WidgetSize::Normal), zoom: signal(ZOOM_DEFAULT), theme_idx: signal(0),
            };
            let mut built = build_ui(&theme, ctl);
            built.ui.base_mut().style.layout.width = Length::Px(width);
            built.ui.base_mut().style.layout.height = Length::Px(900.0);
            LayoutEngine::new().theme(&theme).compute(&mut built.ui, Size::new(width as f64, 900.0));
            // The page contains the image demo, the text demo, then the catalog.
            let [_, text_demo, catalog] = built.ui.base().children.as_slice() else {
                panic!("showcase page structure changed");
            };
            let demo = text_demo.base();
            assert!(catalog.base().bounds.size.h > demo.bounds.size.h,
                "the full catalog keeps its natural height in the vertical page");
            let list = demo.children[3].base().children[1].base();
            let viewport = list.children[0].base();
            let message = viewport.children[0].base();
            assert!(message.bounds.size.h > (message.font * heca_grid_ui::font::MONO_LINE_RATIO) as f64);
            assert!(viewport.bounds.loc.x + viewport.bounds.size.w <= width as f64);
            assert!(message.bounds.size.w <= viewport.bounds.size.w);
        }
    }

    /// **The catalog stays inside its window, at every width**.
    ///
    /// The same invariant `heca-view-realize` asks of every `WidgetId`, asked here of the real
    /// thing: the showcase is every widget in a **real composition**, with real text, which is
    /// where the failures actually live — a synthetic sample of a `Card` says `TITLE`, while the
    /// catalog's says what a card's title really looks like.
    ///
    /// **Base layer only**: the overlay band exists for what must escape its box (a dropdown, a
    /// hint keycap), so asserting there would forbid the feature. Both that and the clip
    /// arithmetic come from `Scene::draws_outside` — this sweep and the per-kind one each had
    /// their own copy, and both read a clip lying outside the clip already open as "no clip",
    /// which is what produced the five phantom escapes it was ignored for.
    #[test]
    fn the_catalog_paints_nothing_outside_the_window_it_is_given() {
        let mut escapes: Vec<String> = Vec::new();
        for w in [1400.0f64, 900.0, 600.0, 320.0] {
            let theme = Theme::default();
            let ctl = ThemeCtl {
                glow: signal(GlowLevel::default()),
                radius: signal(6.0),
                border: signal(1.0),
                font: signal(theme.font_size),
                intensity: signal(Intensity::default()),
                overlay_frame: signal(FrameStyle::default()),
                size: signal(WidgetSize::Normal),
                zoom: signal(1.0),
                theme_idx: signal(0),
            };
            let mut built = build_ui(&theme, ctl);
            // **Give the page the window**, as the event loop does (see `resize`): a root left at
            // `Auto` sizes to its own content, and then its clip is the content rather than the
            // viewport — the page clips nothing and every measurement below is meaningless.
            built.ui.base_mut().style.layout.width = Length::Px(w as f32);
            built.ui.base_mut().style.layout.height = Length::Px(900.0);
            LayoutEngine::new().compute(&mut built.ui, Size::new(w, 900.0));

            let mut scene = Scene::new();
            {
                let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(Size::new(w, 900.0));
                built.ui.paint(&mut cx);
            }
            let window = heca_core::layout::Rectangle::new(
                heca_core::layout::Point::default(),
                Size::new(w, 900.0),
            );
            escapes.extend(
                scene
                    .draws_outside(window)
                    .iter()
                    .map(|e| format!("{w}px: {e}")),
            );
        }
        escapes.sort();
        escapes.dedup();
        assert!(
            escapes.is_empty(),
            "the catalog draws outside its window ({} places):\n{}",
            escapes.len(),
            escapes.join("\n"),
        );
    }

    #[test]
    fn resizing_the_catalog_preserves_the_readers_scroll_position() {
        let theme = Theme::default();
        let ctl = ThemeCtl {
            glow: signal(theme.colors.glow_size), radius: signal(theme.colors.border_radius),
            border: signal(theme.colors.border_width), font: signal(theme.font_size),
            intensity: signal(theme.colors.intensity), overlay_frame: signal(theme.colors.overlay_frame),
            size: signal(WidgetSize::Normal), zoom: signal(ZOOM_DEFAULT), theme_idx: signal(0),
        };
        for offset in [0.0, 900.0, 2400.0] {
            let mut built = build_ui(&theme, ctl);
            for (step, (width, height)) in [(1400.0, 900.0), (1300.0, 500.0),
                (1400.0, 700.0), (600.0, 400.0), (1400.0, 900.0)].into_iter().enumerate() {
                built.ui.base_mut().style.layout.width = Length::Px(width as f32);
                built.ui.base_mut().style.layout.height = Length::Px(height as f32);
                LayoutEngine::new().theme(&theme).compute(&mut built.ui, Size::new(width, height));
                if step == 0 {
                    built.ui.scroll_to(offset);
                }
                assert_eq!(built.ui.scroll_offset().get_untracked(), offset,
                    "resizing to {width}x{height} must not reveal a different catalog demo");
            }
        }
    }

    #[test]
    fn the_dialog_body_remains_visible_and_scrollable_in_a_short_window() {
        let theme = Theme::default();
        let ctl = ThemeCtl {
            glow: signal(theme.colors.glow_size), radius: signal(theme.colors.border_radius),
            border: signal(theme.colors.border_width), font: signal(theme.font_size),
            intensity: signal(theme.colors.intensity), overlay_frame: signal(theme.colors.overlay_frame),
            size: signal(WidgetSize::Normal), zoom: signal(ZOOM_DEFAULT), theme_idx: signal(0),
        };
        let mut built = build_ui(&theme, ctl);
        built.overlays.base().children[0].base().open.set(true);
        built.overlays.tick(0.016);
        for height in [700.0, 400.0, 260.0, 400.0, 700.0] {
            let viewport = Size::new(900.0, height);
            built.overlays.base_mut().style.layout.width = Length::Px(900.0);
            built.overlays.base_mut().style.layout.height = Length::Px(height as f32);
            LayoutEngine::new().theme(&theme).compute(&mut built.overlays, viewport);
            let dialog = built.overlays.base().children[0].as_ref();
            let body = heca_grid_ui::area(dialog, "dialog-body").unwrap();
            let actions = heca_grid_ui::area(dialog, "dialog-actions").unwrap();
            let title = heca_grid_ui::area(dialog, "dialog-title").unwrap();
            let region = &body.base().children[0];
            let rect = region.base().bounds;
            assert!(rect.size.h >= theme.font_size as f64,
                "the body must retain a readable scroll viewport at height {height}: {rect:?}");
            assert!(rect.loc.y >= title.base().bounds.loc.y + title.base().bounds.size.h);
            assert!(rect.loc.y + rect.size.h <= actions.base().bounds.loc.y);
            assert!(actions.base().bounds.loc.y + actions.base().bounds.size.h <= height);
            let before = region.base().children[0].base().bounds.loc.y;
            let point = Point::new(rect.loc.x + rect.size.w / 2.0, rect.loc.y + rect.size.h / 2.0);
            heca_grid_ui::dispatch(&mut built.overlays, &Event::wheel(point, 0.0, 1.0));
            let body = heca_grid_ui::area(built.overlays.base().children[0].as_ref(), "dialog-body").unwrap();
            assert!(body.base().children[0].base().children[0].base().bounds.loc.y < before,
                "wheel over the body must scroll it at height {height}: body={rect:?}, title={:?}, actions={:?}",
                heca_grid_ui::area(built.overlays.base().children[0].as_ref(), "dialog-title").unwrap().base().bounds,
                heca_grid_ui::area(built.overlays.base().children[0].as_ref(), "dialog-actions").unwrap().base().bounds);
        }
    }
