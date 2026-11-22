//! **A label that had to cut its text offers the whole of it on hover**, with nothing written by
//! the caller — and a tooltip written by hand wins. Plus the one default delay the host sets.

mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::tooltip::set_default_delay;
use heca_grid_ui::{DrawCommand, Event, LayoutEngine, PaintCx, Point, Scene, Size, Theme};

const LONG: &str = "feature/a-branch-name-much-too-long-for-its-row";

/// Lay `root` out at `width`, rest the pointer on its first child, and return the overlay's words.
fn hover_words(root: Flex, width: f64) -> Vec<String> {
    set_default_delay(0.0);
    let mut root = root.width(width as f32);
    let viewport = Size::new(width, 200.0);
    LayoutEngine::new().compute(&mut root, viewport);
    let rect = root.base().children[0].base().bounds;
    let point = Point::new(
        rect.loc.x + rect.size.w / 2.0,
        rect.loc.y + rect.size.h / 2.0,
    );
    heca_grid_ui::dispatch(&mut root, &Event::pointer_moved(point));
    let theme = Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(viewport);
        heca_grid_ui::paint_child(&root, &mut cx);
    }
    scene
        .overlay_layer()
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCommand::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_cut_label_offers_its_whole_text_and_one_that_fits_offers_nothing() {
    let cut = hover_words(Flex::column().child(Label::new(LONG)), 120.0);
    assert!(
        cut.iter().any(|w| w == LONG),
        "the cut label shows its whole text on hover: {cut:?}"
    );

    let fits = hover_words(Flex::column().child(Label::new("main")), 600.0);
    assert!(fits.is_empty(), "nothing is cut, nothing to say: {fits:?}");
}

#[test]
fn a_tooltip_written_on_the_label_is_kept() {
    let words = hover_words(
        Flex::column().child(Label::new(LONG).tooltip("what this branch is")),
        120.0,
    );
    assert!(
        words.iter().any(|w| w == "what this branch is"),
        "{words:?}"
    );
    assert!(
        !words.iter().any(|w| w == LONG),
        "the hand-written tooltip is not replaced: {words:?}"
    );
}

#[test]
fn a_tooltip_written_on_the_row_wins_over_the_labels_cut_text() {
    let row = Flex::row()
        .tooltip("the row's details")
        .child(Label::new(LONG));
    let mut probe = Flex::column()
        .width(120.0)
        .child(Flex::row().child(Label::new(LONG)));
    LayoutEngine::new().compute(&mut probe, Size::new(120.0, 200.0));
    assert!(
        probe.base().children[0].base().children[0]
            .base()
            .tooltip
            .is_some(),
        "the label in the row is cut, so it has its own tooltip to be beaten"
    );
    let words = hover_words(Flex::column().child(row), 120.0);
    assert!(words.iter().any(|w| w == "the row's details"), "{words:?}");
    assert!(
        !words.iter().any(|w| w == LONG),
        "the label's own tooltip stays quiet under it: {words:?}"
    );
}

#[test]
fn every_tooltip_waits_the_default_the_host_sets_and_a_quick_one_half() {
    set_default_delay(1.2);
    let plain = Label::new("a").tooltip("x");
    let quick = Label::new("b").tooltip("y").tooltip_quick(true);
    let own = Label::new("c").tooltip("z").tooltip_delay(0.3);
    let delay = |w: &Label| w.base().tooltip.as_ref().map(|t| t.delay());
    assert_eq!(delay(&plain), Some(1.2));
    assert_eq!(delay(&quick), Some(0.6));
    assert_eq!(
        delay(&own),
        Some(0.3),
        "a delay of its own does not follow the default"
    );
    set_default_delay(0.8);
    assert_eq!(
        delay(&plain),
        Some(0.8),
        "a changed setting reaches a tooltip already built"
    );
}

/// **A button showing only its icon says its words on hover; with its words back it says nothing**
/// — and a tooltip written by hand stays either way.
#[test]
fn a_button_offers_its_words_only_while_it_shows_just_its_icon() {
    let words = |b: &Button| b.base().tooltip.as_ref().map(|t| t.text.get_untracked());
    let shown = Button::new("Split").icon(Glyph::SquareSplitVertical);
    assert_eq!(
        words(&shown),
        None,
        "its words are on screen: nothing to add"
    );
    let icon = Button::new("Split")
        .icon(Glyph::SquareSplitVertical)
        .icon_only(true);
    assert_eq!(
        words(&icon).as_deref(),
        Some("Split"),
        "icon alone: it says what it does"
    );
    let back = icon.icon_only(false);
    assert_eq!(words(&back), None, "words back: the tooltip goes");
    let own = Button::new("Split")
        .icon(Glyph::SquareSplitVertical)
        .tooltip("Split the column")
        .icon_only(true);
    assert_eq!(
        words(&own).as_deref(),
        Some("Split the column"),
        "a hand-written tooltip stays"
    );
}
