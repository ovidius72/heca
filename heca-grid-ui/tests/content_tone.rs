//! **The colour of the text inside a container, by meaning** — CSS's inherited `color`
//! (`content_tone`), and the dialog that uses it to say a destructive consequence.

mod common;

use heca_core::layout::Size;
use heca_grid_ui::widgets::{Button, Dialog, Flex, Label};
use heca_grid_ui::{
    Color, Component, ComponentExt, DrawCommand, LayoutEngine, Parent, Theme, Tone,
};

/// Lay `w` out in a 600×400 viewport, paint it through `paint_child` — the one path every widget is
/// drawn by, and the one that applies a published tone — and report each text run's colour.
fn text_colours(w: &mut dyn Component) -> Vec<(String, Color)> {
    LayoutEngine::new().compute(w, Size::new(600.0, 400.0));
    let theme = Theme::default();
    let scene = common::paint_via_child(w, &theme);
    scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some((t.text.clone(), t.color)),
            _ => None,
        })
        .collect()
}

fn colour_of(runs: &[(String, Color)], text: &str) -> Color {
    runs.iter()
        .find(|(t, _)| t == text)
        .map(|(_, c)| *c)
        .unwrap_or_else(|| panic!("`{text}` was not painted: {runs:?}"))
}

#[test]
fn text_inside_a_toned_container_takes_the_tone() {
    let danger = Theme::default().colors.danger;
    let mut toned = Flex::column()
        .content_tone(Tone::Danger)
        .child(Label::new("inside"));
    let runs = text_colours(&mut toned);
    assert_eq!(colour_of(&runs, "inside"), danger);
}

#[test]
fn a_label_with_its_own_colour_keeps_it() {
    let own = Color::rgb(10, 200, 120);
    let mut toned = Flex::column()
        .content_tone(Tone::Danger)
        .child(Label::new("own").color(own));
    let runs = text_colours(&mut toned);
    assert_eq!(colour_of(&runs, "own"), own);
}

/// Show the dialog the way a host does, through its handle, and let it settle.
fn open(mut d: Dialog) -> Dialog {
    d.handle().show();
    d.tick(0.016);
    d
}

/// A destructive dialog says its consequence in the danger colour; the title does not change, so
/// the question still reads as the question.
#[test]
fn a_destructive_dialog_says_its_message_in_the_danger_colour() {
    let theme = Theme::default();
    let mut d = open(
        Dialog::new("Delete pane?")
            .body(Label::new("This cannot be undone."))
            .danger(true)
            .action(Button::new("Cancel")),
    );
    let runs = text_colours(&mut d);
    assert_eq!(
        colour_of(&runs, "This cannot be undone."),
        theme.colors.danger
    );
    assert_ne!(colour_of(&runs, "Delete pane?"), theme.colors.danger);
}

#[test]
fn danger_before_the_body_is_the_same_dialog() {
    let theme = Theme::default();
    let mut d = open(
        Dialog::new("Delete pane?")
            .danger(true)
            .body(Label::new("This cannot be undone.")),
    );
    let runs = text_colours(&mut d);
    assert_eq!(
        colour_of(&runs, "This cannot be undone."),
        theme.colors.danger
    );
}

#[test]
fn an_ordinary_dialog_is_unchanged() {
    let theme = Theme::default();
    let mut d = open(Dialog::new("Rename pane").body(Label::new("New name")));
    let runs = text_colours(&mut d);
    assert_ne!(colour_of(&runs, "New name"), theme.colors.danger);
}
