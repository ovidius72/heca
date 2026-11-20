//! **How a label's text is set** — weight, letter spacing, line height, case — and that each one
//! reaches the measure, the cut, the wrap and the paint together, so the box and the glyphs never
//! disagree.

mod common;

use heca_grid_ui::font::MONO_ADVANCE_RATIO;
use heca_grid_ui::prelude::*;
use heca_grid_ui::{LayoutEngine, Size, Theme};

/// Lay a label out alone in a roomy box: its bounds and its resolved font.
fn laid_out(label: Label) -> (Label, f64) {
    let mut label = label;
    LayoutEngine::new().compute(&mut label, Size::new(600.0, 200.0));
    let font = f64::from(label.base().font);
    (label, font)
}

/// The text runs a widget paints, in order.
fn runs(widget: &dyn Component) -> Vec<heca_grid_ui::scene::TextCmd> {
    common::texts(&common::paint(widget, &Theme::default()))
}

#[test]
fn letter_spacing_widens_the_label_by_its_step_on_every_character() {
    let (plain, font) = laid_out(Label::new("MEMBERS").truncate(Ellipsis::None));
    let (wide, _) = laid_out(
        Label::new("MEMBERS")
            .truncate(Ellipsis::None)
            .letter_spacing(LetterSpacing::Wide),
    );
    let chars = 7.0;
    let added = chars * font * f64::from(LetterSpacing::Wide.em());
    assert!(
        // The layout snaps each box to whole pixels, so the two widths can each be off by under one.
        (wide.base().bounds.size.w - plain.base().bounds.size.w - added).abs() < 1.0,
        "seven characters, each {} em wider: {} vs {}",
        LetterSpacing::Wide.em(),
        wide.base().bounds.size.w,
        plain.base().bounds.size.w,
    );
    let run = &runs(&wide)[0];
    assert_eq!(
        run.style.letter_spacing,
        LetterSpacing::Wide.em(),
        "the renderer spaces the glyphs by the same step the box was measured with",
    );
}

/// A box that holds the text at the font's own spacing does not hold it spaced out — the cut is made
/// on the wider cell, so the spaced text is cut instead of drawn past the box.
#[test]
fn a_spaced_label_is_cut_on_its_wider_cell() {
    let text = "projects";
    // A row exactly as wide as the text at the font's own spacing.
    let (_, font) = laid_out(Label::new(text));
    let width = (text.chars().count() as f64 * font * f64::from(MONO_ADVANCE_RATIO)).ceil();
    let fits_plain = |label: Label| {
        let mut row = Flex::row().width(Length::Px(width as f32)).child(label);
        LayoutEngine::new().compute(&mut row, Size::new(600.0, 200.0));
        runs(&row)[0].text.clone()
    };
    assert_eq!(fits_plain(Label::new(text)), text, "the box is the text's own width");
    let spaced = fits_plain(Label::new(text).letter_spacing(LetterSpacing::Wider));
    assert!(spaced.ends_with('…'), "spaced out, the same text no longer fits: {spaced:?}");
}

#[test]
fn line_height_sets_a_one_line_label_and_the_pitch_of_a_wrapped_one() {
    let (normal, font) = laid_out(Label::new("x"));
    let (relaxed, _) = laid_out(Label::new("x").line_height(LineHeight::Relaxed));
    assert!((normal.base().bounds.size.h - font * f64::from(LineHeight::Normal.ratio())).abs() < 0.5);
    assert!((relaxed.base().bounds.size.h - font * f64::from(LineHeight::Relaxed.ratio())).abs() < 0.5);

    // Wrapped onto several lines: the box is that many lines tall, and each line is drawn one line
    // below the last.
    let mut col = Flex::column().width(Length::Px(120.0)).child(
        Label::new("the quick brown fox jumps over the lazy dog")
            .wrap(true)
            .line_height(LineHeight::Loose),
    );
    LayoutEngine::new().compute(&mut col, Size::new(600.0, 600.0));
    let label = col.base().children[0].base();
    let line = f64::from(label.font * LineHeight::Loose.ratio());
    let drawn = runs(&col);
    assert!(drawn.len() > 1, "the fixture must wrap");
    assert!(
        (label.bounds.size.h - line * drawn.len() as f64).abs() < 0.5,
        "{} lines of {line}px in a {}px box",
        drawn.len(),
        label.bounds.size.h,
    );
    for pair in drawn.windows(2) {
        assert!((pair[1].rect.loc.y - pair[0].rect.loc.y - line).abs() < 0.5, "line pitch");
    }
}

#[test]
fn a_transform_changes_what_is_shown_not_what_the_label_holds() {
    let (label, _) = laid_out(Label::new("members · 4").transform(TextTransform::Uppercase));
    assert_eq!(runs(&label)[0].text, "MEMBERS · 4");
    assert_eq!(
        label.text_summary().as_deref(),
        Some("members · 4"),
        "the accessible name keeps the casing it was written in",
    );
    assert_eq!(label.text_signal().get_untracked(), "members · 4");

    // The box is measured on the shown text: `ß` shows as `SS`, one character more.
    let (written, _) = laid_out(Label::new("straße").truncate(Ellipsis::None));
    let (shown, _) = laid_out(
        Label::new("straße")
            .truncate(Ellipsis::None)
            .transform(TextTransform::Uppercase),
    );
    assert_eq!(runs(&shown)[0].text, "STRASSE");
    assert!(shown.base().bounds.size.w > written.base().bounds.size.w);
}

#[test]
fn font_weight_reaches_the_run_and_the_old_bold_still_draws_bold() {
    let (label, _) = laid_out(Label::new("Title").font_weight(FontWeight::SemiBold));
    assert_eq!(runs(&label)[0].style.weight, FontWeight::SemiBold);

    // The weight is a signal: a parent re-weights the label in place, no rebuild.
    label.font_weight_signal().set(FontWeight::Medium);
    assert_eq!(runs(&label)[0].style.weight, FontWeight::Medium);

    #[allow(deprecated, reason = "the old builder must keep drawing bold")]
    let (old, _) = laid_out(Label::new("Title").bold(true));
    assert_eq!(runs(&old)[0].style.weight, FontWeight::Bold);
}

/// An active `Item` bolds its label through the weight signal, and gives the weight back when it
/// stops being active.
#[test]
fn an_active_item_bolds_its_label_and_lets_go() {
    let weight = |item: &Item| {
        runs(item)
            .into_iter()
            .find(|run| run.text == "row")
            .map(|run| run.style.weight)
            .expect("the item paints its label")
    };
    let mut item = Item::new("row");
    LayoutEngine::new().compute(&mut item, Size::new(300.0, 60.0));
    assert_eq!(weight(&item), FontWeight::Regular);
    item.state().set(true);
    item.tick(0.0);
    assert_eq!(weight(&item), FontWeight::Bold);
    item.state().set(false);
    item.tick(0.0);
    assert_eq!(weight(&item), FontWeight::Regular);
}

/// Spacing and line height are steps of the font size: double the font, double the step — no caller
/// writes a number, and a font-size change carries them along.
#[test]
fn the_steps_follow_the_font_size() {
    let width = |size: f32| {
        let (label, _) = laid_out(
            // Twenty characters, so a step that did not scale would be off by many pixels.
            Label::new("abcdefghijklmnopqrst")
                .truncate(Ellipsis::None)
                .font_size(size)
                .letter_spacing(LetterSpacing::Wide),
        );
        label.base().bounds.size.w
    };
    // Whole-pixel boxes: each width is within a pixel of its exact value.
    assert!((width(30.0) - 2.0 * width(15.0)).abs() < 2.0, "{} vs {}", width(30.0), width(15.0));
}
