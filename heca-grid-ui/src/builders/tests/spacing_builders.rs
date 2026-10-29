use crate::builders::*;
use crate::component::Component;
use crate::style::{Align, Direction, Justify, Space, Spacing};
use crate::widgets::Flex;

/// **A step and a number go through the same builder into the same setting** — which is the
/// whole point: one place holds whichever kind was given, so nothing can be set twice.
#[test]
fn one_builder_writes_whichever_kind_it_was_given() {
    let px = Flex::row().gap(8);
    assert_eq!(px.base().style.layout.gap, Space::Px(8.0));

    let step = Flex::row().gap(Spacing::Sm);
    assert_eq!(step.base().style.layout.gap, Space::Step(Spacing::Sm));
}

/// **Saying it again the other way replaces it.** Two settings let both stand at once with the
/// step silently winning at layout; one cannot.
#[test]
fn a_number_after_a_step_really_is_a_number() {
    let w = Flex::row().gap(Spacing::Lg).gap(4);
    assert_eq!(
        w.base().style.layout.gap,
        Space::Px(4.0),
        "the step has to be gone, or layout resolves it over the number",
    );
}

/// Padding behaves the same, on every axis.
#[test]
fn padding_takes_either_kind_per_axis() {
    let w = Flex::row().padding_x(12).padding_y(Spacing::Xs);
    assert_eq!(w.base().style.layout.padding_x, Some(Space::Px(12.0)));
    assert_eq!(
        w.base().style.layout.padding_y,
        Some(Space::Step(Spacing::Xs))
    );
}

/// **A step only becomes pixels where the font is known**, and it is rounded there — the two
/// sides of a boundary between siblings must not round in different directions.
#[test]
fn a_step_resolves_against_the_font_and_lands_on_a_whole_pixel() {
    // Xs is a quarter of the font, so 14 gives 3.5 — the half-pixel case the rounding exists
    // for. A pixel count is already what it says and is left alone.
    assert_eq!(Space::Step(Spacing::Xs).resolve(14.0), 4.0);
    assert_eq!(Space::Px(3.5).resolve(14.0), 3.5);
}

/// **The authored step survives layout.** It used to be copied into the px setting on every
/// pass, which destroyed it — so a font change had nothing left to re-resolve and the spacing
/// stayed at the old size.
#[test]
fn resolving_a_step_does_not_consume_it() {
    let w = Flex::row().gap(Spacing::Md);
    let gap = w.base().style.layout.gap;
    assert_eq!(gap.resolve(12.0), 10.0);
    assert_eq!(
        gap.resolve(24.0),
        20.0,
        "the same step, against a bigger font"
    );
    assert_eq!(
        w.base().style.layout.gap,
        Space::Step(Spacing::Md),
        "still a step after being resolved twice",
    );
}

/// Every spelling a description travels in reaches the same value, through one parser.
#[test]
fn a_space_reads_every_spelling() {
    assert_eq!(Space::from("sm"), Space::Step(Spacing::Sm));
    assert_eq!(Space::from("8px"), Space::Px(8.0));
    assert_eq!(Space::from("8"), Space::Px(8.0));
    assert_eq!(
        Space::from("nonsense"),
        Space::Px(0.0),
        "an unreadable spelling costs its author a gap, never the host",
    );
}

/// **A track reads every spelling a stylesheet writes**, through the one parser on the type.
///
/// ⚠️ Ran red first: the vocabulary lived in `heca-view-realize`, so a described grid could
/// say `"1fr"` and native code could not — it wrote `Track::Fr(1.0)`, one variant per track.
#[test]
fn a_track_reads_every_spelling() {
    use crate::style::Track;
    assert_eq!(Track::from("auto"), Track::Auto);
    assert_eq!(Track::from("1fr"), Track::Fr(1.0));
    assert_eq!(Track::from("2.5fr"), Track::Fr(2.5));
    assert_eq!(Track::from("22px"), Track::Px(22.0));
    assert_eq!(Track::from("22"), Track::Px(22.0));
    assert_eq!(Track::from(22), Track::Px(22.0));
    // Both spellings of the same word: CSS writes one, serde's own names write the other.
    assert_eq!(Track::from("min-content"), Track::MinContent);
    assert_eq!(Track::from("min_content"), Track::MinContent);
    assert_eq!(Track::from("max-content"), Track::MaxContent);
    assert_eq!(
        Track::from("nonsense"),
        Track::Auto,
        "an unreadable track costs its author a size, never the host",
    );
}

/// **A whole track list reads as one stylesheet line**, which is what `.template_row(..)` and
/// `.template_column(..)` are built on — including CSS's `repeat(n, …)`.
#[test]
fn a_template_is_a_track_list_written_the_way_css_writes_one() {
    use crate::style::Track;
    assert_eq!(Track::list("auto 1fr"), vec![Track::Auto, Track::Fr(1.0)]);
    assert_eq!(
        Track::list("200px  1fr   2fr"),
        vec![Track::Px(200.0), Track::Fr(1.0), Track::Fr(2.0)],
        "any run of whitespace separates, as in CSS",
    );
    assert_eq!(
        Track::list("repeat(3, 1fr)"),
        vec![Track::Fr(1.0); 3],
        "repeat() expands as CSS expands it",
    );
    assert_eq!(
        Track::list("auto repeat(2, 1fr) auto"),
        vec![Track::Auto, Track::Fr(1.0), Track::Fr(1.0), Track::Auto],
        "and sits among ordinary tracks",
    );
    assert_eq!(
        Track::list("repeat(2, auto 1fr)"),
        vec![Track::Auto, Track::Fr(1.0), Track::Auto, Track::Fr(1.0)],
        "a repeat of several tracks repeats all of them",
    );
}

/// **A layout keyword reads the way a stylesheet writes it**, hyphen or underscore, through
/// the type's own serde names — so a word that works in a plugin's JSON works in Rust.
#[test]
fn a_layout_keyword_reads_every_spelling() {
    assert_eq!(Justify::from("space-between"), Justify::SpaceBetween);
    assert_eq!(Justify::from("space_between"), Justify::SpaceBetween);
    assert_eq!(Justify::from("center"), Justify::Center);
    assert_eq!(Align::from("stretch"), Align::Stretch);
    assert_eq!(Direction::from("column"), Direction::Column);
    assert_eq!(
        Justify::from("nonsense"),
        Justify::default(),
        "an unreadable keyword falls back to the default, never a panic",
    );
}

/// **The builders take the spelling, not only the variant** — the whole point of the parsers
/// above. A call site never writes an enum variant by hand again.
#[test]
fn the_layout_builders_take_a_spelling() {
    use crate::widgets::Flex;
    let row = Flex::row().justify("space-between").align("center");
    let layout = &row.base().style.layout;
    assert_eq!(layout.justify, Justify::SpaceBetween);
    assert_eq!(layout.align, Align::Center);
}

/// **One child or many, through one builder — on every widget, not only the grid.**
///
/// `Flex` declares no `child` of its own; it gets this from `Parent`, which is what makes the
/// answer the same wherever children go. The pair this replaced (`child` / `children`) was two
/// names for one idea, and a caller reached for whichever they had seen first.
#[test]
fn any_parent_takes_one_child_or_several() {
    use crate::widgets::{Flex, Label};
    let one = Flex::column().child(Label::new("only"));
    assert_eq!(one.base().children.len(), 1);

    let many = Flex::column().child([Label::new("a"), Label::new("b"), Label::new("c")]);
    assert_eq!(
        many.base().children.len(),
        3,
        "an array lands as three children"
    );

    let built: Vec<Label> = (0..4).map(|i| Label::new(format!("row {i}"))).collect();
    let from_vec = Flex::column().child(built);
    assert_eq!(from_vec.base().children.len(), 4, "and so does a Vec");

    // Mixed with the singular form in one chain, which is the point of one door.
    let chained = Flex::column()
        .child(Label::new("header"))
        .child([Label::new("a"), Label::new("b")]);
    assert_eq!(chained.base().children.len(), 3);
}

/// **A share is `flex: 1 1 0`** — a weight, a **zero basis**, and permission to shrink.
///
/// The zero basis is the part that is easy to leave out and hard to see missing: with the
/// default `auto` basis each item starts from its own content and only the *leftover* is
/// divided, so two items of unequal content never come out in their stated ratio. With a zero
/// basis the whole box is divided by weight, which is what a share means.
///
/// ⚠️ Ran red first: the library had no `flex-basis` at all, so every share in the app spelled
/// it with a zero *height* and a comment apologising for the substitution.
#[test]
fn a_share_divides_the_whole_box_by_weight_not_just_the_leftover() {
    use crate::style::Length;
    use crate::widgets::{Flex, Label};
    use crate::{LayoutEngine};
    use heca_core::layout::Size;

    // Unequal content is what tells the two apart: one item holds three lines, the other one.
    let content = |lines: usize| {
        let mut f = Flex::column();
        for i in 0..lines {
            f = f.child(Label::new(format!("line {i}")));
        }
        f
    };
    let heights = |basis: bool| -> Vec<f64> {
        let item = |w: f32, lines: usize| {
            let f = Flex::column().grow(w).shrink(1.0).child(content(lines));
            if basis { f.basis(0.0) } else { f }
        };
        let mut root: crate::WidgetModel = crate::WidgetModel::new(
            Flex::column()
                .height(Length::Percent(1.0))
                .child([item(2.0, 3), item(1.0, 1)]),
        );
        LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 900.0));
        root.base()
            .children
            .iter()
            .map(|c| c.base().bounds.size.h)
            .collect()
    };

    let shares = heights(true);
    assert!(
        (shares[0] - 600.0).abs() < 1.0 && (shares[1] - 300.0).abs() < 1.0,
        "a zero basis divides the whole box 2:1 however much content each holds: {shares:?}",
    );

    let leftover = heights(false);
    assert!(
        (leftover[0] - 600.0).abs() > 1.0,
        "and without it the ratio is skewed by the content each started from — which is why \
         the basis has to be said out loud: {leftover:?}",
    );
}

/// **A child places itself, as in CSS** — `grid-column: 1 / -1` on the item, never a
/// coordinate the parent writes into it.
///
/// ⚠️ Ran red first: placement used to be `.cell(child, col, row, col_span, row_span)` on the
/// grid, so a child could not say where it went and five positional numbers said it for them.
#[test]
fn a_child_says_where_it_goes_and_a_span_of_all_follows_the_template() {
    use crate::style::{GridCell, Span};
    use crate::widgets::{Grid, Label};
    use crate::{LayoutEngine};
    use heca_core::layout::Size;

    let mut grid: crate::WidgetModel = crate::WidgetModel::new(
        Grid::new()
            .template_column("auto 1fr auto")
            .template_row("auto 1fr")
            .child([
                Label::new("icon").column(1).row(1),
                Label::new("title").column(2).row(1),
                Label::new("badge").column(3).row(1),
                // The whole width, whatever the template turns out to say.
                Label::new("body").column("1 / -1").row(2),
            ]),
    );
    LayoutEngine::new().compute(grid.as_mut(), Size::new(600.0, 400.0));

    let cell = |i: usize| -> GridCell {
        grid.base().children[i]
            .base()
            .style
            .layout
            .grid_cell
            .expect("placed")
    };
    assert_eq!((cell(0).col, cell(0).row), (1, 1));
    assert_eq!((cell(2).col, cell(2).row), (3, 1));
    assert_eq!(
        (cell(3).col, cell(3).row, cell(3).col_span),
        (1, 2, 3),
        "`1 / -1` became every column there are, resolved against the parent's template",
    );

    // And it follows the template rather than a number frozen at build time.
    let mut wider: crate::WidgetModel = crate::WidgetModel::new(
        Grid::new()
            .template_column("repeat(5, 1fr)")
            .child(Label::new("body").column("1 / -1")),
    );
    LayoutEngine::new().compute(wider.as_mut(), Size::new(600.0, 400.0));
    assert_eq!(
        wider.base().children[0]
            .base()
            .style
            .layout
            .grid_cell
            .expect("placed")
            .col_span,
        5,
        "the same child spans five columns in a five-column grid",
    );
    assert_eq!(Span::from("all").stored(), GridCell::ALL);
}
