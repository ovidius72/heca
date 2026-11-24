use super::*;

#[test]
fn a_path_stays_on_one_line_and_details_have_their_own_line_at_every_width() {
    let path = "/Users/example/projects/a-very-long-worktree/target/native/projects/A";
    for width in [360.0, 900.0, 1600.0] {
        let viewport = Size::new(width, 800.0);
        let mut palette = CommandPalette::new()
            .command(
                Command::new("Project A", || {})
                    .path(path)
                    .detail("2 windows / 1 agent"),
            )
            .default_open(true);
        LayoutEngine::new().compute(&mut palette, viewport);
        let scene = common::paint_in(&palette, &Theme::default(), viewport);
        let text = scene
            .iter()
            .filter_map(|command| match command {
                DrawCommand::Text(text) => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>();
        let title = text.iter().find(|text| text.text == "Project A").unwrap();
        let paths = text
            .iter()
            .filter(|text| text.text.ends_with("projects/A"))
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 1, "path must not wrap at {width}px: {text:?}");
        let details = text
            .iter()
            .find(|text| text.text == "2 windows / 1 agent")
            .unwrap();
        assert!(paths[0].rect.loc.y >= title.rect.loc.y + title.rect.size.h - 1.0);
        assert!(details.rect.loc.y >= paths[0].rect.loc.y + paths[0].rect.size.h - 1.0);
        assert!(paths[0].rect.loc.x + paths[0].rect.size.w <= width);
    }
}

/// A description is a **second line under its label**, and it makes every row two lines high — the
/// measured thing, because a row-height rule that only the paint knows would misplace every click.
#[test]
fn command_palette_describes_a_command_on_a_second_line() {
    use heca_grid_ui::{Command, CommandPalette};
    let theme = Theme::default();
    let mut p = CommandPalette::new()
        .command(
            Command::new("Split pane right", || {})
                .description("New column to the right of the active pane."),
        )
        .command(Command::new("Close pane", || {}).description("Close the focused pane."))
        .default_open(true);

    LayoutEngine::new().compute(&mut p, Size::new(1200.0, 800.0));
    let scene = common::paint_in(&p, &theme, Size::new(1200.0, 800.0));
    let runs: Vec<(String, Rectangle)> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some((t.text.clone(), t.rect)),
            _ => None,
        })
        .collect();
    let at = |text: &str| {
        runs.iter()
            .find(|(t, _)| t == text)
            .unwrap_or_else(|| panic!("{text:?} is painted, got {runs:?}"))
            .1
    };
    let label = at("Split pane right");
    let description = at("New column to the right of the active pane.");
    assert!(
        description.loc.y > label.loc.y,
        "the description sits under its label, not beside it",
    );
    assert_eq!(
        description.loc.x, label.loc.x,
        "…and is indented to it, so the two read as one block",
    );
    // The next row starts a full two lines down: one row = label line + description line + padding.
    let next_label = at("Close pane");
    let line = description.loc.y - label.loc.y;
    assert!(
        next_label.loc.y - label.loc.y > 2.0 * line,
        "a described list gives every row two lines: rows are {}px apart, one line is {line}px",
        next_label.loc.y - label.loc.y,
    );
}

/// **An action bound three times draws its bindings side by side**, at the row's right edge, and the
/// text beside them stops before the first cap — so text and chips never overlap.
#[test]
fn command_palette_sets_a_rows_bindings_side_by_side_at_its_right_edge() {
    use heca_grid_ui::widgets::{KeyCap, NfGlyph};
    use heca_grid_ui::{Command, CommandPalette};
    let theme = Theme::default();
    let cap = |s: &str| KeyCap::Text(s.to_string());
    let mut p = CommandPalette::new()
        .command(
            Command::new("Focus Dock", || {})
                .description("Give chrome keyboard focus to a dock.")
                .keys([cap("λ"), KeyCap::Nf(NfGlyph::Shift), cap("e")])
                .keys([cap("λ"), cap("e")])
                .keys([KeyCap::Nf(NfGlyph::Control), cap("e")]),
        )
        .command(Command::new("Reload Config", || {}).description("Re-read config.toml."))
        .default_open(true);

    LayoutEngine::new().compute(&mut p, Size::new(1200.0, 800.0));
    let scene = common::paint_in(&p, &theme, Size::new(1200.0, 800.0));
    // A cap is a small bordered chip. The size bound is what tells it apart from the panel's own
    // border, the query line and the selected row's outline, which are all bordered and all span
    // the panel.
    let chips: Vec<Rectangle> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r)
                if r.border.is_some() && r.rect.size.w < 60.0 && r.rect.size.h < 30.0 =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .collect();
    // Three chords of 3 + 2 + 2 caps = 7 chips (plus the panel/query/selection chrome, which is why
    // this asserts on the ROWS the chips occupy rather than an exact count).
    let mut rows: Vec<f64> = chips
        .iter()
        .map(|r| (r.loc.y * 10.0).round() / 10.0)
        .collect();
    rows.sort_by(|a, b| a.partial_cmp(b).unwrap());
    rows.dedup();
    assert_eq!(
        rows.len(),
        1,
        "three short bindings fit side by side on one line, got rows at {rows:?}",
    );
    // The row's own title stops before its caps, or text and chips would overlap on a narrow row. A
    // row with no bindings gives its text the whole width, so only the row's own caps count.
    let leftmost_chip = chips.iter().map(|r| r.loc.x).fold(f64::MAX, f64::min);
    let label = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text(t) if t.text == "Focus Dock" => Some(t.rect),
            _ => None,
        })
        .expect("the bound command is painted");
    assert!(
        label.loc.x + label.size.w <= leftmost_chip,
        "the title box ({}) runs into its own shortcuts ({leftmost_chip})",
        label.loc.x + label.size.w,
    );
}

/// **A row bound to many keys stays a line or two tall.** Its bindings wrap onto another line only
/// when they do not fit, instead of taking a line each — ten one-key bindings (`1` … `0`) once made
/// a row ten caps tall.
#[test]
fn command_palette_rows_are_only_as_tall_as_their_own_bindings() {
    use heca_grid_ui::widgets::KeyCap;
    use heca_grid_ui::{Command, CommandPalette};
    let theme = Theme::default();
    let cap = |s: &str| KeyCap::Text(s.to_string());
    let mut p = CommandPalette::new()
        .command(
            ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"]
                .into_iter()
                .fold(
                    Command::new("Paste Clipboard", || {}).description("Paste the clipboard."),
                    |c, k| c.keys([cap(k)]),
                ),
        )
        .command(Command::new("Close Pane", || {}).description("Close the focused pane."))
        .command(Command::new("Reload Config", || {}).description("Re-read config.toml."))
        .default_open(true);

    LayoutEngine::new().compute(&mut p, Size::new(1200.0, 800.0));
    let scene = common::paint_in(&p, &theme, Size::new(1200.0, 800.0));
    let label_y = |text: &str| {
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Text(t) if t.text == text => Some(t.rect.loc.y),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{text} is painted"))
    };
    // The row of ten bindings is at most twice as tall as a plain one: its caps took a line or two.
    let tall_gap = label_y("Close Pane") - label_y("Paste Clipboard");
    let plain_gap = label_y("Reload Config") - label_y("Close Pane");
    assert!(
        tall_gap <= 2.0 * plain_gap,
        "ten bindings made the row {tall_gap}px tall against {plain_gap}px for a plain one",
    );
}

/// A long description **reflows while its row is selected, and is cut when it is not**.
///
/// Both halves matter. Cutting every description hides what the row does; wrapping every one turns a
/// ten-row list into a wall of text and pushes the rest off the panel. So the selected row reflows —
/// and because its height is *measured*, the rows below it move down, which is the whole reason the
/// label's height had to become a function of its width.
#[test]
fn command_palette_reflows_the_selected_description_and_cuts_the_rest() {
    use heca_grid_ui::widgets::KeyCap;
    use heca_grid_ui::{Command, CommandPalette};
    let theme = Theme::default();
    let long = "Give chrome keyboard focus to a dock — press a letter to pick one, or name it.";
    // Longer than any row is wide, so its cut is forced even in a row with no shortcuts.
    let other = "Re-read config.toml and apply every change without restarting the compositor, \
                 the panes, the docks, the themes, the fonts or anything else that is running.";
    let mut p = CommandPalette::new()
        .command(
            Command::new("Focus Dock", || {})
                .description(long)
                .keys([KeyCap::Text("λ".into()), KeyCap::Text("e".into())]),
        )
        .command(Command::new("Reload Config", || {}).description(other))
        .default_open(true);

    LayoutEngine::new().compute(&mut p, Size::new(1200.0, 800.0));
    let scene = common::paint_in(&p, &theme, Size::new(1200.0, 800.0));
    let drawn: Vec<(String, Rectangle)> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some((t.text.clone(), t.rect)),
            _ => None,
        })
        .collect();

    // The selected row (the first) reflowed: several lines, none of them ellipsised, and the words
    // all survive in order.
    let lines: Vec<&(String, Rectangle)> = drawn
        .iter()
        // Longer than a keycap: a single-cap chip like "e" is also a substring of the description.
        .filter(|(t, _)| t.trim().chars().count() > 2 && long.contains(t.trim()))
        .collect();
    assert!(
        lines.len() > 1,
        "the selected description reflows, got {drawn:?}"
    );
    assert!(
        lines.iter().all(|(t, _)| !t.ends_with('…')),
        "a reflowed description is not also cut: {lines:?}",
    );
    let rejoined: Vec<&str> = lines.iter().flat_map(|(t, _)| t.split(' ')).collect();
    assert_eq!(rejoined.join(" "), long, "reflowing must not lose text");
    assert!(
        lines.windows(2).all(|p| p[1].1.loc.y > p[0].1.loc.y),
        "its lines stack downward",
    );

    // The unselected row is cut to one line — and sits **below** the reflowed block, which is the
    // reflow pushing it down.
    let (cut, cut_rect) = drawn
        .iter()
        .find(|(t, _)| t.starts_with("Re-read config"))
        .expect("the second description is drawn");
    assert!(
        cut.ends_with('…'),
        "an unselected description is cut: {cut:?}"
    );
    let lowest_selected_line = lines.iter().map(|(_, r)| r.loc.y).fold(0.0_f64, f64::max);
    assert!(
        cut_rect.loc.y > lowest_selected_line,
        "the row below starts under the whole reflowed block, not under its first line",
    );
}

/// **A size is a maximum, not a demand.** `large` asks for an 850px panel; a small window still
/// wins, because a panel wider than the screen is worse than a narrow one.
#[test]
fn the_palette_size_is_capped_by_the_window() {
    use heca_grid_ui::{Command, CommandPalette, WidgetSize};
    let theme = Theme::default();
    let panel_w = |size: WidgetSize, viewport: Size| {
        let mut p = CommandPalette::new()
            .panel_size(size)
            .command(Command::new("Close Pane", || {}))
            .default_open(true);
        LayoutEngine::new().compute(&mut p, viewport);
        let scene = common::paint_in(&p, &theme, viewport);
        // The panel is the widest painted rect that is NOT the full-viewport scrim.
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Rect(r) if r.rect.size.w < viewport.w => Some(r.rect.size.w),
                _ => None,
            })
            .fold(0.0f64, f64::max)
    };

    // An ORDINARY window, not a huge one: the sizes used to be capped by a viewport fraction, so
    // they were identical below ~1550px and the setting did nothing on a normal screen. Testing at
    // 1800px hid exactly that.
    let roomy = Size::new(1280.0, 900.0);
    let small = panel_w(WidgetSize::Small, roomy);
    let normal = panel_w(WidgetSize::Normal, roomy);
    let large = panel_w(WidgetSize::Large, roomy);
    assert!(
        small < normal && normal < large,
        "each size is roomier than the last: {small} / {normal} / {large}",
    );
    assert!(large <= 1000.5, "large caps at 1000px, got {large}");
    assert!(
        (small - 560.0).abs() < 0.5 && (normal - 700.0).abs() < 0.5,
        "each size is its own width when the window has room: {small} / {normal}",
    );

    // On a narrow window every size collapses to what fits, and none touches the edges.
    let narrow = Size::new(480.0, 700.0);
    for size in [WidgetSize::Small, WidgetSize::Normal, WidgetSize::Large] {
        let w = panel_w(size, narrow);
        assert!(
            w <= narrow.w * 0.92 + 0.5,
            "{size:?} took {w}px of a 480px window — a panel must never reach the edges",
        );
    }
}

/// **The panel fits the window, with room left under it.** The row count is capped by what the
/// window can actually hold, and a row is two lines when described and taller again with stacked
/// bindings — a nominal one-line estimate over-counted and the list ran off the bottom.
///
/// It asserts a **gap**, not `bottom <= viewport.h`: a panel resting exactly on the screen edge
/// passes the second and that is what shipped. And it **sweeps** the height rather than sampling
/// it — the flush case only appears where the room divides evenly into rows, so 1280x577 left
/// 0.76px under the panel while the three sampled viewports left 23px to 69px and looked fine.
#[test]
fn the_palette_never_runs_off_a_short_window() {
    use heca_grid_ui::widgets::KeyCap;
    use heca_grid_ui::{Command, CommandPalette, WidgetSize};
    let theme = Theme::default();
    let cap = |s: &str| KeyCap::Text(s.to_string());
    let mut p = CommandPalette::new().panel_size(WidgetSize::Large);
    for i in 0..40 {
        p = p.command(
            Command::new(format!("Command {i}"), || {})
                .description("What this command does, at some length.")
                .keys([cap("λ"), cap("a")])
                .keys([cap("λ"), cap("b")]),
        );
    }
    let mut p = p.default_open(true);

    // The overlay keeps this much air from every edge of the window, the bottom included.
    let margin = f64::from(heca_grid_ui::widgets::overlay::VIEWPORT_MARGIN);
    for w in [1280.0, 900.0] {
        for h in 260..=1000 {
            let viewport = Size::new(w, f64::from(h));
            // Layout, a paint (which tells the overlay the window's size), and layout again.
            LayoutEngine::new().compute(&mut p, viewport);
            let _ = common::paint_in(&p, &theme, viewport);
            LayoutEngine::new().compute(&mut p, viewport);
            let panel = p.base().children[0].base().children[0].base().bounds;
            let gap = viewport.h - (panel.loc.y + panel.size.h);
            assert!(
                gap >= margin - 0.5 && panel.loc.y >= margin - 0.5,
                "at {}x{} the panel spans {}..{}px, closer than the {margin}px margin to an edge",
                viewport.w,
                viewport.h,
                panel.loc.y,
                panel.loc.y + panel.size.h,
            );
        }
    }
}

/// **A palette opens where it stays** — on the first frame, before anything is painted, its panel
/// is already in the upper part of the window. A panel placed only after a first paint was drawn
/// centred for a frame and then jumped up, which read as the palette bouncing in from below.
#[test]
fn a_palette_opens_at_the_top_on_its_first_frame() {
    use heca_grid_ui::widgets::overlay::TOP_POSITION_FRAC;
    use heca_grid_ui::{Command, CommandPalette};
    let viewport = Size::new(1280.0, 800.0);
    let mut p = CommandPalette::new().command(Command::new("Close pane", || {}));
    heca_grid_ui::reactive::SignalUpdate::set(&p.open_signal(), true);
    LayoutEngine::new().compute(&mut p, viewport);
    let panel = p.base().children[0].base().children[0].base().bounds;
    assert_eq!(
        panel.loc.y,
        viewport.h * TOP_POSITION_FRAC,
        "the first layout already puts the panel at the top"
    );
}

/// **A long palette ends at about 60% of the window's height**, at any window height — past that its
/// list scrolls.
#[test]
fn a_long_palette_ends_at_about_sixty_percent_of_the_window() {
    use heca_grid_ui::{Command, CommandPalette};
    let mut p = CommandPalette::new();
    for i in 0..300 {
        p = p.command(Command::new(format!("Command {i}"), || {}).description("does things"));
    }
    let mut p = p.default_open(true);
    for h in [600.0, 900.0, 1400.0] {
        let viewport = Size::new(1400.0, h);
        for _ in 0..4 {
            p.tick(0.016);
            LayoutEngine::new().compute(&mut p, viewport);
            let _ = common::paint_in(&p, &Theme::default(), viewport);
        }
        let panel = p.base().children[0].base().children[0].base().bounds;
        let bottom = (panel.loc.y + panel.size.h) / h;
        assert!(
            (0.58..0.63).contains(&bottom),
            "in a window {h} tall the panel ends at {:.0}%",
            bottom * 100.0
        );
    }
}
