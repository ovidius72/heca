
    use super::*;

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
    /// The same invariant `heca-view-realize` asks of every `WidgetKind`, asked here of the real
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
