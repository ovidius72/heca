mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, LayoutEngine, Point, Rectangle, Size, TextStyle, Theme};

#[test]
fn signal_set_updates_value() {
    let count = signal(0);
    assert_eq!(count.get_untracked(), 0);
    count.set(7);
    assert_eq!(count.get_untracked(), 7);
}

#[test]
fn label_signal_drives_text() {
    let label = Label::new("ONLINE");
    let sig = label.text_signal();
    assert_eq!(sig.get_untracked(), "ONLINE");
    sig.set("OFFLINE".to_string());
    assert_eq!(sig.get_untracked(), "OFFLINE");
}

/// A truncating label is cut **to its box**, at the end it was told to cut — and the untruncating
/// default is untouched, or every layout in the app would shift at once.
#[test]
fn label_truncates_to_its_box_at_the_end_it_was_given() {
    use heca_grid_ui::Ellipsis;
    let theme = Theme::default();
    // Paint a label into a box `cells` characters wide and report what was drawn.
    let drawn = |label: Label, cells: f64| {
        let mut label = label;
        // Lay out FIRST: the engine resolves the inherited font, and the cell the label cuts on is
        // derived from that font — measuring the box with the pre-layout one is off by a character.
        LayoutEngine::new().compute(&mut label, Size::new(400.0, 40.0));
        let font = label.base().font as f64;
        let w = cells * font * heca_grid_ui::font::MONO_ADVANCE_RATIO as f64; // the same cell the label measures with
        label.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(w, 40.0));
        let scene = common::paint(&label, &theme);
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .expect("a label paints its text")
    };

    let path = "projects/heca/src/widgets";
    // Wide enough ⇒ untouched, whichever end it would cut.
    assert_eq!(drawn(Label::new(path).truncate(Ellipsis::End), 40.0), path);
    // Narrow ⇒ cut, and the ellipsis marks which end went.
    let end = drawn(Label::new(path).truncate(Ellipsis::End), 10.0);
    assert!(end.starts_with("projects") && end.ends_with('…'), "{end:?}");
    assert!(end.chars().count() <= 10, "cut to the box: {end:?}");
    let start = drawn(Label::new(path).truncate(Ellipsis::Start), 10.0);
    assert!(
        start.starts_with('…') && start.ends_with("widgets"),
        "a path keeps its tail — the only part anyone reads: {start:?}",
    );
    // Degenerate boxes still say something rather than panicking.
    assert_eq!(drawn(Label::new(path).truncate(Ellipsis::End), 1.0), "…");

    // **Cutting is the default** (F003/P082/T438): a label nobody said anything to still stays
    // inside its box, which is what makes it safe for an author who has never read this file.
    let by_default = drawn(Label::new(path), 10.0);
    assert!(
        by_default.starts_with("projects") && by_default.ends_with('…'),
        "unset means Ellipsis::End: {by_default:?}",
    );
    // …and the opt-out is the way back to overflowing, said out loud.
    assert_eq!(drawn(Label::new(path).truncate(Ellipsis::None), 10.0), path);
    let plain = Label::new(path);
    assert!(
        plain.base().style.layout.flex_shrink.is_none()
            && plain.base().style.layout.min_width.is_none(),
        "a label declares neither: shrinking is the engine's default and its floor is its own \
         min-content answer",
    );
    // **A cutting label declares nothing about sizing any more**, and does not need to: the engine
    // shrinks by default, and the label's own "how narrow can you get?" answer — one character — is
    // its floor. Forcing that floor to zero is what cost a `Card` its title.
    let cut = Label::new(path).truncate(Ellipsis::End);
    assert!(
        cut.base().style.layout.flex_shrink.is_none()
            && cut.base().style.layout.min_width.is_none(),
    );
}

/// **The container shrinks it.** The paint-time cut is only ever reached if the layout can hand the
/// label less than its natural width — so this is the test that matters: a real `Flex` too narrow
/// for the text, laid out by the engine, with nothing set by hand.
#[test]
fn a_truncating_label_shrinks_inside_a_container_that_is_too_narrow() {
    use heca_grid_ui::Ellipsis;
    let theme = Theme::default();
    let text = "projects/heca/src/widgets/label.rs";
    let paint_in_row = |label: Label| {
        let mut row = Flex::row()
            .width(Length::Px(120.0))
            .height(Length::Px(40.0))
            .child(label);
        LayoutEngine::new().compute(&mut row, Size::new(120.0, 40.0));
        let child_w = row.base().children[0].base().bounds.size.w;
        let scene = common::paint(&row, &theme);
        let drawn = scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .expect("the label paints");
        (child_w, drawn)
    };

    let (w, drawn) = paint_in_row(Label::new(text).truncate(Ellipsis::End));
    assert!(
        w <= 120.5,
        "a truncating label must accept the box it is given, got {w}px in a 120px row",
    );
    assert!(
        drawn.ends_with('…') && drawn.chars().count() < text.chars().count(),
        "…and cut its text to it: {drawn:?}",
    );

    // The opt-out keeps its natural width and overflows — which is now something a caller asks for
    // rather than what they get by saying nothing (F003/P082/T438).
    let (w_plain, plain) = paint_in_row(Label::new(text).truncate(Ellipsis::None));
    assert_eq!(
        plain, text,
        "opted out, it draws its whole string whatever box it is given"
    );
    // Its **box** does not exceed the row, whatever it opts out of: shrinking is the engine's
    // default and nothing is wider than what holds it (`max-width: 100%`, applied in the layout
    // pass). Opting out of the *cut* is a statement about the text, not a licence for the box —
    // the text simply paints past its own edge, which is what asking not to be cut means.
    let (w_rigid, rigid) = paint_in_row(Label::new(text).truncate(Ellipsis::None).shrink(0.0));
    assert_eq!(rigid, text, "still whole");
    assert!(
        w_rigid <= 120.5 && w_plain <= 120.5,
        "neither box exceeds the 120px row ({w_rigid}px, {w_plain}px)",
    );
}

/// A rule under a truncated label spans the **drawn** glyphs, not the width the full string wanted.
#[test]
fn label_decorations_follow_a_truncated_run() {
    use heca_grid_ui::Ellipsis;
    let theme = Theme::default();
    let mut label = Label::new("projects/heca/src/widgets")
        .truncate(Ellipsis::End)
        .underline(true);
    LayoutEngine::new().compute(&mut label, Size::new(400.0, 40.0));
    let w = 10.0 * label.base().font as f64 * heca_grid_ui::font::MONO_ADVANCE_RATIO as f64;
    label.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(w, 40.0));
    let scene = common::paint(&label, &theme);
    let rule = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Rect(r) => Some(r.rect),
            _ => None,
        })
        .expect("the underline is painted");
    assert!(
        rule.size.w <= w + 0.5,
        "the rule spans the cut run ({}), not the full string's width",
        rule.size.w,
    );
}

/// Paint a wrapping label inside a column `width` px wide and report the lines drawn (in paint
/// order, with their y), the height the layout gave the label, and how many monospace cells fit —
/// **read from the font the engine resolved**, not from the default. The label inherits a
/// size-variant-scaled font, so a hardcoded 15px cell measures the box a character wide.
fn wrapped(text: &str, width: f64) -> (Vec<(String, f64)>, f64, usize) {
    wrapped_label(Label::new(text).wrap(true), width)
}

/// [`wrapped`] for a label the caller configured (a line cap, say).
fn wrapped_label(label: Label, width: f64) -> (Vec<(String, f64)>, f64, usize) {
    let theme = Theme::default();
    let mut col = Flex::column().width(Length::Px(width as f32)).child(label);
    LayoutEngine::new().compute(&mut col, Size::new(800.0, 600.0));
    let label = col.base().children[0].base();
    let height = label.bounds.size.h;
    let cells = (label.bounds.size.w
        / (label.font as f64 * heca_grid_ui::font::MONO_ADVANCE_RATIO as f64))
        as usize;
    let scene = common::paint(&col, &theme);
    let lines = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some((t.text.clone(), t.rect.loc.y)),
            _ => None,
        })
        .collect();
    (lines, height, cells)
}

/// **The measure follows the width.** A wrapped label is the one widget whose height cannot be known
/// before layout, so this asserts the two halves together: the lines actually drawn, and the box the
/// engine sized for them. A test that checked only a `wrap` flag would pass with the text painted
/// outside its own bounds.
#[test]
fn a_wrapping_label_reflows_and_its_height_follows_its_width() {
    let text = "the quick brown fox jumps over the lazy dog";

    // Roomy: one line, one line's height — a wrapping label in a wide box looks like a plain one.
    let (wide, wide_h, _) = wrapped(text, 600.0);
    assert_eq!(wide.len(), 1, "nothing to wrap at 600px: {wide:?}");
    assert_eq!(wide[0].0, text);

    // Narrow: several lines, stacked downward, and the height is exactly that many lines.
    let (narrow, narrow_h, cells) = wrapped(text, 120.0);
    assert!(narrow.len() > 1, "120px must force a wrap, got {narrow:?}");
    assert!(
        narrow.windows(2).all(|p| p[1].1 > p[0].1),
        "each line sits below the last: {narrow:?}",
    );
    // Taffy rounds each box to whole pixels, so allow a pixel of slack rather than exact equality.
    assert!(
        (narrow_h - wide_h * narrow.len() as f64).abs() <= 1.5,
        "{} lines must measure about {} px, got {narrow_h}",
        narrow.len(),
        wide_h * narrow.len() as f64,
    );

    // The text survives the break: same words, same order, nothing dropped or duplicated.
    let rejoined: Vec<&str> = narrow.iter().flat_map(|(l, _)| l.split(' ')).collect();
    assert_eq!(
        rejoined.join(" "),
        text,
        "wrapping is not allowed to lose text"
    );

    // Every line fits the box it was measured against.
    for (line, _) in &narrow {
        assert!(
            line.chars().count() <= cells,
            "{line:?} overflows {cells} cells"
        );
    }
}

/// A word longer than the line is **hard-broken**, not allowed to overflow. A path or a URL with no
/// spaces is common enough that refusing to break it means refusing to fit at all.
#[test]
fn a_wrapping_label_hard_breaks_a_word_too_long_for_the_line() {
    let word = "supercalifragilisticexpialidocious";
    let (lines, _, cells) = wrapped(word, 120.0);
    assert!(
        lines.len() > 1,
        "an unbreakable word must still be broken: {lines:?}"
    );
    for (line, _) in &lines {
        assert!(
            line.chars().count() <= cells,
            "{line:?} overflows {cells} cells"
        );
    }
    assert_eq!(
        lines.iter().map(|(l, _)| l.as_str()).collect::<String>(),
        word,
        "the pieces must still spell the word",
    );
}

/// **Nothing else moves.** A label that sets neither `wrap` nor `truncate` keeps the fixed
/// single-line measure every existing layout in the app depends on — the measure path must be
/// reachable only by opting in.
#[test]
fn a_plain_label_is_untouched_by_the_measure_path() {
    use heca_grid_ui::style::Length as L;
    // The label that stays out of the measure path is the one that opted OUT of cutting: it reports
    // one fixed width and has nothing to answer (F003/P082/T438 made cutting the default).
    let plain = Label::new("some text").truncate(Ellipsis::None);
    assert!(
        matches!(plain.base().style.layout.height, L::Px(_)),
        "a plain label still reports its own height, not `auto`",
    );
    assert!(
        plain.measure_text().is_none(),
        "…and is not handed to taffy's measure path"
    );

    let wrapping = Label::new("some text").wrap(true);
    assert!(
        matches!(wrapping.base().style.layout.height, L::Auto),
        "a wrapping label must report no height of its own, or taffy believes the one-line answer",
    );
    assert!(wrapping.measure_text().is_some());
    assert_eq!(wrapping.base().style.layout.flex_shrink, Some(1.0));
    assert!(
        wrapping.base().style.layout.min_width.is_some(),
        "a label that cannot be handed less than its natural width never wraps",
    );
}

/// Paint `label` in a box `cells` characters wide and report the text runs it drew, with colours.
fn runs_of(label: Label, cells: f64) -> Vec<(String, Color)> {
    let theme = Theme::default();
    let mut label = label;
    // Lay out first: the cell the label marks against comes from the font the engine resolves.
    LayoutEngine::new().compute(&mut label, Size::new(400.0, 40.0));
    let w = cells * label.base().font as f64 * heca_grid_ui::font::MONO_ADVANCE_RATIO as f64;
    label.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(w, 40.0));
    let scene = common::paint(&label, &theme);
    scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some((t.text.clone(), t.color)),
            _ => None,
        })
        .collect()
}

/// **Marks are the widget's, not the caller's.** The matched characters come out in the mark colour
/// and the rest in the label's — and an unmarked label is still one single run, because splitting
/// every label into pieces would cost the whole tree for a feature almost nothing uses.
#[test]
fn a_label_marks_the_characters_it_was_given() {
    let mark = Color::rgb(255, 0, 0);

    // No marks ⇒ exactly one run, unchanged.
    let plain = runs_of(Label::new("close pane"), 40.0);
    assert_eq!(plain.len(), 1, "an unmarked label is one run: {plain:?}");

    // Marked ⇒ **the whole line is still one run**, with the marked characters over-drawn on top.
    // Splitting the line at the mark boundaries loses a run's leading space in the shaper, which
    // walks every space in the label — so the text must never be cut into pieces.
    let marked = runs_of(
        Label::new("close pane").marks([0, 1, 6]).mark_color(mark),
        40.0,
    );
    assert_eq!(
        marked.first().map(|(t, _)| t.as_str()),
        Some("close pane"),
        "the line is drawn whole, so its spacing is the font's own: {marked:?}",
    );
    let in_mark: String = marked
        .iter()
        .filter(|(_, c)| *c == mark)
        .map(|(t, _)| t.as_str())
        .collect();
    assert_eq!(
        in_mark, "clp",
        "exactly the marked characters are accented: {marked:?}"
    );
}

/// A mark follows **its character** through a cut. `Ellipsis::Start` drops the head, so every
/// surviving index shifts — counting through the drawn string instead would stamp the accent onto
/// whatever letter happened to land there.
#[test]
fn marks_survive_a_truncation_at_either_end() {
    use heca_grid_ui::Ellipsis;
    let mark = Color::rgb(255, 0, 0);
    let text = "projects/heca/src";
    // Mark the final three characters, "src" — indices 14, 15, 16.
    let marks = [14usize, 15, 16];

    // Cutting the head keeps them: they are the tail the ellipsis preserved.
    let start = runs_of(
        Label::new(text)
            .truncate(Ellipsis::Start)
            .marks(marks)
            .mark_color(mark),
        10.0,
    );
    let kept: String = start
        .iter()
        .filter(|(_, c)| *c == mark)
        .map(|(t, _)| t.as_str())
        .collect();
    assert_eq!(
        kept, "src",
        "the marks moved with their characters: {start:?}"
    );

    // Cutting the tail throws those characters away, so nothing is marked — and nothing is stamped
    // onto the ellipsis.
    let end = runs_of(
        Label::new(text)
            .truncate(Ellipsis::End)
            .marks(marks)
            .mark_color(mark),
        10.0,
    );
    assert!(
        !end.iter().any(|(_, c)| *c == mark),
        "characters that were cut cannot still be marked: {end:?}",
    );
}

/// A mark follows its character across a **reflow** too: wrapping collapses whitespace, so counting
/// through the wrapped output would drift a character per line break.
#[test]
fn marks_survive_a_wrap() {
    let theme = Theme::default();
    let mark = Color::rgb(255, 0, 0);
    let text = "the quick brown fox jumps over the lazy dog";
    // "lazy" occupies indices 35..39 — and it is on the last wrapped line, which is the point:
    // counting through the wrapped output would have drifted by the collapsed line breaks.
    let marks: Vec<usize> = (35..39).collect();
    let mut col = Flex::column()
        .width(Length::Px(120.0))
        .child(Label::new(text).wrap(true).marks(marks).mark_color(mark));
    LayoutEngine::new().compute(&mut col, Size::new(800.0, 600.0));
    let scene = common::paint(&col, &theme);
    let accented: String = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) if t.color == mark => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        accented, "lazy",
        "the mark landed on the line its characters wrapped to"
    );
}

/// A wrapped label's rules follow **each line**, not one rule spanning the whole box.
#[test]
fn decorations_follow_every_wrapped_line() {
    let theme = Theme::default();
    let mut col = Flex::column().width(Length::Px(120.0)).child(
        Label::new("the quick brown fox jumps over the lazy dog")
            .wrap(true)
            .underline(true),
    );
    LayoutEngine::new().compute(&mut col, Size::new(800.0, 600.0));
    let scene = common::paint(&col, &theme);
    let texts = common::texts(&scene).len();
    let rules: Vec<Rectangle> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r) if r.rect.size.h < 4.0 => Some(r.rect),
            _ => None,
        })
        .collect();
    assert!(texts > 1, "the fixture must actually wrap");
    assert_eq!(
        rules.len(),
        texts,
        "one rule per drawn line, not one for the box"
    );
    assert!(
        rules.windows(2).all(|p| p[1].loc.y > p[0].loc.y),
        "each rule sits under its own line: {rules:?}",
    );
}

#[test]
fn label_weight_and_slant_are_font_attributes_decorations_are_rects() {
    let theme = Theme::default();
    let paint = |label: Label| {
        let mut label = label;
        LayoutEngine::new().compute(&mut label, Size::new(200.0, 40.0));
        let scene = common::paint(&label, &theme);
        let runs: Vec<TextStyle> = scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) => Some(t.style),
                _ => None,
            })
            .collect();
        // The label paints no background of its own, so every rect it emits is a decoration.
        let rules: Vec<Rectangle> = scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Rect(r) => Some(r.rect),
                _ => None,
            })
            .collect();
        (runs, rules, label.base().bounds, label.base().font)
    };

    // Weight + slant reach the shaper as font attributes on the run…
    let (runs, rules, ..) = paint(Label::new("STATUS").bold(true).italic(true));
    assert_eq!(runs, vec![TextStyle::REGULAR.bold(true).italic(true)]);
    assert!(rules.is_empty(), "no decoration ⇒ no rects");

    // …while the decorations never touch it: they are rects the widget draws.
    let (runs, rules, bounds, font) = paint(Label::new("STATUS").underline(true));
    assert_eq!(
        runs,
        vec![TextStyle::REGULAR],
        "a rule is not a font attribute"
    );
    assert_eq!(rules.len(), 1, "the underline");
    let rule = rules[0];
    let mid = bounds.loc.y + bounds.size.h / 2.0;
    assert!(rule.loc.y > mid, "the underline sits below the text centre");
    assert!(
        (rule.size.w - bounds.size.w).abs() < 0.5,
        "it spans the text run, which for a Start-aligned label is its whole box",
    );
    assert!(
        rule.size.h >= 1.0,
        "never thinner than a pixel: {}",
        rule.size.h
    );

    // Strikethrough goes through the text; both together draw two rules.
    let (_, rules, bounds, _) = paint(Label::new("STATUS").strikethrough(true));
    let mid = bounds.loc.y + bounds.size.h / 2.0;
    assert!(rules[0].loc.y < mid, "the strike sits at/above the centre");
    let (_, rules, ..) = paint(Label::new("STATUS").underline(true).strikethrough(true));
    assert_eq!(rules.len(), 2, "both rules");

    // An empty label has a zero-width run, so it draws no rule at all.
    let (_, rules, ..) = paint(Label::new("").underline(true));
    assert!(rules.is_empty(), "nothing to underline");
    let _ = font;
}

#[test]
fn label_decorations_follow_the_text_run_not_the_box() {
    let theme = Theme::default();
    // A label normally hugs its text (`remeasure` sizes the box to the run), but a *container* can
    // widen a child's bounds — a `Select` does exactly that to its option rows, so its pill spans
    // the panel. In that box, `align` decides where the run sits, and the rule must follow the run:
    // an underline spanning the whole box, most of it empty, would be plainly wrong.
    let mut label = Label::new("HI").align(TextAlign::End).underline(true);
    LayoutEngine::new().compute(&mut label, Size::new(300.0, 40.0));
    let run_w = label.base().bounds.size.w;
    label.base_mut().bounds.size.w = 300.0;

    let scene = common::paint(&label, &theme);
    let rule = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Rect(r) => Some(r.rect),
            _ => None,
        })
        .expect("the underline is painted");
    let bounds = label.base().bounds;
    // Within a pixel: the *box* is whole pixels (a text measure rounds up, because half a character
    // is not drawable), while the glyph run underneath it is not — two characters of an 8.1px cell
    // are 16.2 in a 17px box.
    assert!(
        (rule.size.w - run_w).abs() <= 1.0,
        "the rule is as wide as the two-character run ({run_w}), not the 300px box: {}",
        rule.size.w,
    );
    assert!(
        (rule.loc.x + rule.size.w - (bounds.loc.x + bounds.size.w)).abs() < 0.5,
        "End-aligned: the run — and its rule — sit at the right edge of the box",
    );
}

/// **A path folds at its directories.** A word too long for the line is broken after a path
/// separator when one lies inside the line, so the reader gets whole directory names, not a path
/// cut mid-word — and when there is none it is still hard-broken (the test above).
#[test]
fn a_wrapping_label_folds_a_long_path_after_a_separator() {
    let path = "/Users/antonio/projects/gleam/tutorial/src/main";
    let (lines, _, cells) = wrapped(path, 190.0);
    assert!(
        lines.len() > 1,
        "the path is longer than the line: {lines:?}"
    );
    let (last, rest) = lines.split_last().expect("lines");
    for (line, _) in rest {
        assert!(
            line.ends_with('/'),
            "{line:?} must end at a separator: {lines:?}"
        );
    }
    for (line, _) in &lines {
        assert!(
            line.chars().count() <= cells,
            "{line:?} overflows {cells} cells"
        );
    }
    assert_eq!(
        lines.iter().map(|(l, _)| l.as_str()).collect::<String>(),
        path,
        "nothing lost: {last:?}",
    );
}

/// **A line cap stops the growth and says so.** Text that needs more lines than the cap ends its
/// last line with `…`; the box is exactly the capped number of lines tall (measure and paint agree);
/// text that fits is not marked.
#[test]
fn a_capped_wrapping_label_ends_with_an_ellipsis_and_is_that_many_lines_tall() {
    let text = "the quick brown fox jumps over the lazy dog and keeps on running far away";
    let (_, one_h, _) = wrapped("one line", 600.0);

    let (lines, height, cells) = wrapped_label(Label::new(text).wrap(true).max_lines(2), 120.0);
    assert_eq!(lines.len(), 2, "capped at two lines: {lines:?}");
    assert!(
        lines[1].0.ends_with('…'),
        "the cut is marked: {:?}",
        lines[1].0
    );
    assert!(
        lines[1].0.chars().count() <= cells,
        "the mark fits the line"
    );
    assert!(
        (height - one_h * 2.0).abs() <= 1.5,
        "the box is two lines tall, got {height}"
    );

    // Fits within the cap: no mark, nothing lost.
    let (lines, _, _) = wrapped_label(Label::new("a few words").wrap(true).max_lines(2), 600.0);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, "a few words");
}
