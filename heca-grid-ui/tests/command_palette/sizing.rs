use super::*;

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

/// **An action bound three times draws three rows of caps**, and every row of the list reserves the
/// same width for them — so the chips form a column instead of tracking each label's length.
#[test]
fn command_palette_stacks_a_binding_per_row_in_a_reserved_column() {
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
    assert!(
        rows.len() >= 3,
        "three bindings ⇒ three stacked rows of caps, got rows at {rows:?}",
    );
    // The label of the *second* command must stop before the column the caps occupy, or text and
    // chips would overlap on a narrow row.
    let leftmost_chip = chips.iter().map(|r| r.loc.x).fold(f64::MAX, f64::min);
    let label = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text(t) if t.text == "Reload Config" => Some(t.rect),
            _ => None,
        })
        .expect("the second command is painted");
    assert!(
        label.loc.x + label.size.w <= leftmost_chip,
        "the label box ({}) runs into the reserved shortcut column ({leftmost_chip})",
        label.loc.x + label.size.w,
    );
}

/// **A row is as tall as its own content.** One action bound three times must not charge every
/// other row for three lines — that is a hundred rows of empty space for one command's sake.
#[test]
fn command_palette_rows_are_only_as_tall_as_their_own_bindings() {
    use heca_grid_ui::widgets::KeyCap;
    use heca_grid_ui::{Command, CommandPalette};
    let theme = Theme::default();
    let cap = |s: &str| KeyCap::Text(s.to_string());
    let mut p = CommandPalette::new()
        .command(
            Command::new("Paste Clipboard", || {})
                .description("Paste the clipboard.")
                .keys([cap("⌘"), cap("v")])
                .keys([cap("^"), cap("⇧"), cap("v")])
                .keys([cap("λ"), cap("⇧"), cap("p")]),
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
    // The three-binding row is tall; the two plain rows after it are not — so the gap between the
    // two plain rows is strictly smaller than the gap the tall one takes.
    let tall_gap = label_y("Close Pane") - label_y("Paste Clipboard");
    let plain_gap = label_y("Reload Config") - label_y("Close Pane");
    assert!(
        plain_gap < tall_gap,
        "a row with no bindings ({plain_gap}px) must be shorter than one with three ({tall_gap}px)",
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
    let other = "Re-read config.toml and apply every change without restarting the compositor.";
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

/// **The icon column is reserved for every row.** A list where some commands carry a glyph and some
/// do not must still read as one column of text — indenting only the iconed rows left the others
/// starting at the panel edge.
#[test]
fn command_palette_reserves_the_icon_column_even_for_a_command_without_one() {
    use heca_grid_ui::{Command, CommandPalette, Glyph};
    let theme = Theme::default();
    let mut p = CommandPalette::new()
        .command(Command::new("With Icon", || {}).icon(Glyph::Search))
        .command(Command::new("Without Icon", || {}))
        .default_open(true);

    LayoutEngine::new().compute(&mut p, Size::new(1200.0, 800.0));
    let scene = common::paint_in(&p, &theme, Size::new(1200.0, 800.0));
    let x_of = |text: &str| {
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Text(t) if t.text == text => Some(t.rect.loc.x),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{text} is painted"))
    };
    assert_eq!(
        x_of("With Icon"),
        x_of("Without Icon"),
        "both labels start in the same column, icon or no icon",
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

    // The room kept clear beneath the panel, as a fraction of the window height — wider than the
    // gap at its sides, since a bottom edge resting on a pane boundary still reads as touching.
    const MIN_GAP_FRAC: f64 = 0.08;
    for w in [1280.0, 900.0] {
        for h in 260..=1000 {
            let viewport = Size::new(w, f64::from(h));
            LayoutEngine::new().compute(&mut p, viewport);
            let scene = common::paint_in(&p, &theme, viewport);
            // The panel is the tallest painted rect that is not the full-viewport scrim.
            let bottoms: Vec<f64> = scene
                .iter()
                .filter_map(|c| match c {
                    DrawCommand::Rect(r) if r.rect.size.w < viewport.w => {
                        Some(r.rect.loc.y + r.rect.size.h)
                    }
                    _ => None,
                })
                .collect();
            assert!(
                !bottoms.is_empty(),
                "at {}x{} nothing painted a panel rect to measure a bottom from",
                viewport.w,
                viewport.h,
            );
            let bottom = bottoms.into_iter().fold(0.0f64, f64::max);
            let gap = viewport.h - bottom;
            let want = viewport.h * MIN_GAP_FRAC;
            assert!(
                gap >= want - 0.5,
                "at {}x{} the panel reached {bottom}px, leaving {gap}px under it — it needs {want}px",
                viewport.w,
                viewport.h,
            );
        }
    }
}
