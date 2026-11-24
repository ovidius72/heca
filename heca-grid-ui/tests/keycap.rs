//! **One key, drawn as a chip** — `Keycap`.

mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{Flex, KeyCap, Keycap, NfGlyph, keycap_size};
use heca_grid_ui::{DrawCommand, LayoutEngine, Size, Theme};

/// **A chip is the size of its key** at the font it inherits — the same size the picker's letters
/// use for the same text — and a size variant resizes it with nothing else written.
#[test]
fn a_keycap_is_the_size_of_its_key_at_its_font() {
    let theme = Theme::default();
    let mut key = Keycap::new("Esc");
    LayoutEngine::new()
        .theme(&theme)
        .compute(&mut key, Size::new(400.0, 100.0));
    // The engine lays every box out on whole pixels.
    let want = keycap_size(key.base().font, "Esc");
    let got = key.base().bounds.size;
    assert_eq!(
        (got.w, got.h),
        (want.w.round(), want.h.round()),
        "{got:?} vs {want:?}"
    );

    let mut small = Keycap::new("Esc").size(WidgetSize::Caption);
    LayoutEngine::new()
        .theme(&theme)
        .compute(&mut small, Size::new(400.0, 100.0));
    assert!(
        small.base().bounds.size.w < key.base().bounds.size.w,
        "a smaller font, a smaller chip: {:?} vs {:?}",
        small.base().bounds.size,
        key.base().bounds.size
    );
}

/// **A shortcut is a row of keys**, laid out by the engine: each chip draws its key, in order,
/// side by side — nothing measures or positions a chip by hand.
#[test]
fn a_shortcut_is_a_row_of_keycaps_each_drawing_its_key() {
    let theme = Theme::default();
    let mut shortcut = Flex::row()
        .gap(Spacing::Xs)
        .child([Keycap::new(NfGlyph::Command), Keycap::new("K")]);
    LayoutEngine::new()
        .theme(&theme)
        .compute(&mut shortcut, Size::new(400.0, 100.0));
    let (command, k) = (
        shortcut.base().children[0].base().bounds,
        shortcut.base().children[1].base().bounds,
    );
    assert!(
        k.loc.x >= command.loc.x + command.size.w,
        "side by side: {command:?} {k:?}"
    );

    let texts: Vec<String> = common::paint(&shortcut, &theme)
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        texts.iter().any(|t| t == "K"),
        "the letter is drawn: {texts:?}"
    );
    assert_eq!(
        Keycap::new(NfGlyph::Command).cap(),
        &KeyCap::Nf(NfGlyph::Command)
    );
}
