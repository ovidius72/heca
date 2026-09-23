//! Guards the `terminal_ligatures` toggle: disabling `calt`/`liga`/`clig` (as the
//! terminal run path does when `ligatures = false`) must actually change the
//! shaped glyphs for a ligature sequence in the embedded terminal font — and
//! leaving them on must keep the ligature. GPU-free (cosmic-text shaping only).

use cosmic_text::{Attrs, Buffer, Family, FeatureTag, FontFeatures, FontSystem, Metrics, Shaping};

fn glyph_ids(fs: &mut FontSystem, text: &str, ligatures: bool) -> Vec<u16> {
    let mut buffer = Buffer::new(fs, Metrics::new(24.0, 28.8));
    buffer.set_size(Some(10000.0), Some(10000.0));
    let mut attrs = Attrs::new().family(Family::Name("Maple Mono Normal NF"));
    if !ligatures {
        let mut features = FontFeatures::new();
        features
            .disable(FeatureTag::CONTEXTUAL_ALTERNATES)
            .disable(FeatureTag::STANDARD_LIGATURES)
            .disable(FeatureTag::CONTEXTUAL_LIGATURES);
        attrs = attrs.font_features(features);
    }
    buffer.set_text(text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(fs, false);
    buffer
        .layout_runs()
        .flat_map(|r| r.glyphs.iter())
        .map(|g| g.glyph_id)
        .collect()
}

#[test]
fn disabling_features_turns_off_ligatures() {
    let mut fs = FontSystem::new();
    fs.db_mut()
        .load_font_data(heca_renderer::font::DEFAULT_TERMINAL_BYTES.to_vec());

    // With ligatures on, `=>` is substituted; with them off it matches the two
    // characters shaped standalone.
    let on = glyph_ids(&mut fs, "=>", true);
    let off = glyph_ids(&mut fs, "=>", false);
    let standalone: Vec<u16> = [
        glyph_ids(&mut fs, "=", false),
        glyph_ids(&mut fs, ">", false),
    ]
    .concat();

    assert_ne!(
        on, off,
        "disabling calt/liga/clig must change the shaped glyphs"
    );
    assert_eq!(
        off, standalone,
        "with ligatures off, `=>` must shape as the standalone `=` + `>` glyphs"
    );
}
