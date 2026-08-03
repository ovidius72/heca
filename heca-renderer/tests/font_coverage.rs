//! **A glyph the font does not have renders as tofu.** These tests read the embedded faces' `cmap`
//! and prove every codepoint the widget layer names is really there.
//!
//! It lives here because this crate owns the font *bytes*; the names live in `heca-grid-ui`. The
//! check is worth its own file: the bug it catches is invisible to every other test — the code
//! compiles, the run is queued, the renderer draws it, and the user sees an empty box. It was found
//! by eye once (a shortcut spelled with `⌃ ⌥ ⌘ ⎋`, none of which the UI face has), which is exactly
//! once too often.

use heca_grid_ui::widgets::{Glyph, NfGlyph};

/// Every codepoint in `face` that has a glyph.
fn has_glyph(font: &[u8], codepoint: u32) -> bool {
    let face = ttf_parser::Face::parse(font, 0).expect("the embedded face parses");
    char::from_u32(codepoint)
        .and_then(|c| face.glyph_index(c))
        .is_some()
}

/// The Nerd Font keyboard set backing [`NfIcon`](heca_grid_ui::widgets::NfIcon) — every glyph a
/// keycap can draw.
///
/// This proves **presence**, not identity: the font names its glyphs `uniF0636`, so no test can say
/// that the codepoint called `Shift` draws a shift key. That one is confirmed by eye in the showcase
/// glyph gallery (see `NfGlyph`).
#[test]
fn every_nerd_font_glyph_is_in_the_embedded_face() {
    for &g in NfGlyph::ALL {
        assert!(
            has_glyph(heca_renderer::font::DEFAULT_TERMINAL_BYTES, g.codepoint()),
            "{}: U+{:05X} is not in the embedded Nerd Font — it would render as tofu",
            g.name(),
            g.codepoint(),
        );
    }
}

/// The same guard for the icon font, whose curated set carries two codepoints that were **inferred**
/// from Phosphor's alphabetical layout rather than read from a table (`Glyph::CaretLeft`,
/// `Glyph::CaretUp` say so in their docs). An inferred codepoint that lands outside the font is the
/// failure this catches.
#[test]
fn every_icon_glyph_is_in_the_embedded_icon_face() {
    for &g in Glyph::ALL {
        // `primary_char` is the public half of the duotone pair; the secondary layer is one below.
        let primary = g.primary_char().expect("a glyph is a valid codepoint") as u32;
        for cp in [primary - 1, primary] {
            assert!(
                has_glyph(heca_grid_ui::font::ICON_FONT_BYTES, cp),
                "{g:?}: U+{cp:04X} is not in Phosphor",
            );
        }
    }
}

/// The UI face has **no keyboard symbols** worth relying on — the fact that sent keycaps to the Nerd
/// Font in the first place. Pinned as a test so a future "just draw ⌘ as text" is a red test, not a
/// discovery in a screenshot.
#[test]
fn the_ui_face_cannot_spell_a_shortcut() {
    let ui = heca_grid_ui::font::DEFAULT_MONO_BYTES;
    for (name, cp) in [
        ("⌃ control", 0x2303u32),
        ("⌥ option", 0x2325),
        ("⌘ command", 0x2318),
        ("⎋ escape", 0x238B),
    ] {
        assert!(
            !has_glyph(ui, cp),
            "{name} is in the UI face now — keycaps could use text after all, so revisit NfGlyph",
        );
    }
    // …while the prefix symbol it *does* use is there, which is why `λ` stays plain text.
    assert!(has_glyph(ui, 0x03BB), "the UI face must have λ (PREFIX_SYMBOL)");
}
