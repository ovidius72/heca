use super::TextGlow;
use super::emit::{EmitKey, align_bits, glow_bits};
use heca_grid_ui::scene::TextAlign;

#[test]
fn emit_key_distinguishes_color_and_position() {
    let base = EmitKey {
        text: "hi".into(),
        size_bits: 14.0f32.to_bits(),
        weight: 400,
        letter_spacing_bits: 0,
        icon: false,
        italic: false,
        faux_italic: false,
        color_bits: [1.0, 1.0, 1.0, 1.0].map(f32::to_bits),
        box_bits: [10.0, 20.0, 0.0, 0.0].map(f32::to_bits),
        align: align_bits(TextAlign::Start),
        centered: false,
        line_box: false,
        font_family: None,
        scale_bits: 2.0f32.to_bits(),
        run: false,
        cell_w_bits: 0,
        run_colors_hash: 0,
        ligatures: true,
        glow_bits: glow_bits(None),
        glow_mode_bits: 0,
    };
    assert_eq!(base, base.clone(), "identical inputs ⇒ a cache hit");

    // Color is baked into the vertices, so a recolor must miss (rebuild).
    let mut recolored = base.clone();
    recolored.color_bits = [1.0, 0.0, 0.0, 1.0].map(f32::to_bits);
    assert_ne!(base, recolored, "a color change is a different emission");

    // Position is baked into the vertices, so a move must miss (rebuild).
    let mut moved = base.clone();
    moved.box_bits = [11.0, 20.0, 0.0, 0.0].map(f32::to_bits);
    assert_ne!(base, moved, "a move is a different emission");
}

/// The halo quads are baked into the cached vertices, so changing the glow must
/// miss the retained-geometry cache — otherwise a glyph would keep the halo it
/// had when it was first emitted.
#[test]
fn emit_key_distinguishes_the_glow() {
    let flat = glow_bits(None);
    let lit = glow_bits(Some(TextGlow {
        color: [0.0, 1.0, 1.0, 1.0],
        radius: 6.0,
        intensity: 0.5,
    }));
    assert_ne!(flat, lit);
}

/// A fully transparent glow emits no halo, so it must hash like no glow at all —
/// otherwise identical output would rebuild every frame.
#[test]
fn emit_key_treats_a_transparent_glow_as_no_glow() {
    let transparent = glow_bits(Some(TextGlow {
        color: [0.0, 1.0, 1.0, 0.0],
        radius: 0.0,
        intensity: 0.0,
    }));
    assert_eq!(glow_bits(None), transparent);
}

#[test]
fn align_bits_are_distinct() {
    let mut v = vec![
        align_bits(TextAlign::Start),
        align_bits(TextAlign::Center),
        align_bits(TextAlign::End),
    ];
    v.sort_unstable();
    v.dedup();
    assert_eq!(
        v.len(),
        3,
        "each alignment hashes to a distinct discriminant"
    );
}

/// **The vertex layout the shader reads** — position, texcoord, color, additive, packed in that
/// order. The named attribute list must place each field exactly where the struct puts it.
#[test]
fn the_vertex_layout_matches_the_vertex() {
    use super::pipeline::TextVertex;
    let attrs = TextVertex::ATTRIBUTES;
    let offsets: Vec<u64> = attrs.iter().map(|a| a.offset).collect();
    assert_eq!(offsets, vec![0, 8, 16, 32]);
    let locations: Vec<u32> = attrs.iter().map(|a| a.shader_location).collect();
    assert_eq!(locations, vec![0, 1, 2, 3]);
    assert_eq!(std::mem::size_of::<TextVertex>(), 36);
}

/// **A glyph's quad leans by the slant, and its ink box covers the lean** — the one quad both
/// labels and terminal runs draw with.
#[test]
fn a_quad_leans_and_its_ink_box_covers_it() {
    use super::TextBox;
    use super::emit::{Ink, glyph_quad};
    let rect = TextBox {
        x: 10.0,
        y: 20.0,
        w: 4.0,
        h: 8.0,
    };
    let quad = glyph_quad(rect, 2.0, [0.0, 0.0, 1.0, 1.0], [1.0; 4], 0.0, false);
    let corners: Vec<[f32; 2]> = quad.iter().map(|v| v.position).collect();
    assert_eq!(
        corners,
        vec![[11.0, 20.0], [15.0, 20.0], [13.0, 28.0], [9.0, 28.0]],
        "the top moves right by half the slant and the bottom left by half"
    );
    let mut ink = Ink::new();
    ink.add(rect, 2.0);
    assert_eq!(ink.bbox(0.0), [9.0, 20.0, 6.0, 8.0]);
    assert_eq!(
        ink.bbox(1.0),
        [8.0, 19.0, 8.0, 10.0],
        "a halo's reach grows every side"
    );
}

/// **Upright text does not lean; a faux italic leans by a share of its line, at least a pixel.**
#[test]
fn a_faux_italic_leans_by_a_share_of_its_line() {
    use super::shape::faux_italic_slant;
    assert_eq!(faux_italic_slant(40.0, 2.0, false), 0.0);
    assert!((faux_italic_slant(40.0, 2.0, true) - 2.8).abs() < 1e-5);
    assert_eq!(
        faux_italic_slant(4.0, 2.0, true),
        1.0,
        "never less than a pixel"
    );
}

/// **One way to shape**: a line is 1.2× its size, and turning ligatures off changes the glyphs `=>`
/// shapes to — for every caller, since all of them shape through here.
#[test]
fn shaping_says_the_line_and_the_ligatures_once() {
    use super::shape::{Face, metrics, shape};
    assert!((metrics(20.0).line_height - 24.0).abs() < 1e-5);
    let mut fonts = crate::font::embedded_font_system();
    let glyphs = |fonts: &mut cosmic_text::FontSystem, ligatures| {
        let face = Face {
            family: crate::font::DEFAULT_TERMINAL_FAMILY,
            weight: 400,
            italic: false,
            letter_spacing: 0.0,
            ligatures,
        };
        shape(fonts, "=>", 24.0, &face)
            .layout_runs()
            .flat_map(|run| run.glyphs.iter().map(|g| g.glyph_id).collect::<Vec<_>>())
            .collect::<Vec<u16>>()
    };
    assert_ne!(glyphs(&mut fonts, true), glyphs(&mut fonts, false));
}

/// **Every [`FontWeight`](heca_grid_ui::FontWeight) is a real face of the embedded UI family** — a
/// weight asks the family for that face, and Medium and SemiBold are not quietly drawn as Regular
/// or Bold.
#[test]
fn each_ui_weight_shapes_with_its_own_face() {
    use super::shape::{Face, shape};
    use heca_grid_ui::FontWeight;
    let mut fonts = crate::font::embedded_font_system();
    let mut faces = Vec::new();
    for weight in [
        FontWeight::Regular,
        FontWeight::Medium,
        FontWeight::SemiBold,
        FontWeight::Bold,
    ] {
        let face = Face {
            family: heca_grid_ui::font::DEFAULT_MONO_FAMILY,
            weight: weight.value(),
            italic: false,
            letter_spacing: 0.0,
            ligatures: true,
        };
        let buffer = shape(&mut fonts, "M", 24.0, &face);
        let id = buffer
            .layout_runs()
            .next()
            .and_then(|run| run.glyphs.first())
            .map(|g| g.font_id);
        let id = id.expect("the UI family shapes an M");
        let found = fonts.db().face(id).expect("a loaded face");
        assert_eq!(
            found.weight.0,
            weight.value(),
            "{weight:?} shaped with a {}-weight face",
            found.weight.0
        );
        faces.push(id);
    }
    faces.dedup();
    assert_eq!(faces.len(), 4, "four weights, four faces");
}

/// **Letter spacing moves every glyph after the first** by its share of the font size — the same
/// `em` the label measured its box with.
#[test]
fn letter_spacing_widens_a_shaped_run_by_its_em() {
    use super::shape::{Face, shape};
    let mut fonts = crate::font::embedded_font_system();
    let width = |fonts: &mut cosmic_text::FontSystem, letter_spacing| {
        let face = Face {
            family: heca_grid_ui::font::DEFAULT_MONO_FAMILY,
            weight: 400,
            italic: false,
            letter_spacing,
            ligatures: true,
        };
        shape(fonts, "abcd", 20.0, &face)
            .layout_runs()
            .next()
            .map_or(0.0, |run| run.line_w)
    };
    let plain = width(&mut fonts, 0.0);
    let spaced = width(&mut fonts, 0.1);
    assert!(
        (spaced - plain - 4.0 * 0.1 * 20.0).abs() < 0.5,
        "four glyphs, each 0.1 em (2px) wider: {plain} → {spaced}"
    );
}

/// **A colour glyph keeps its colours, a plain one is white to be tinted.** ⏺ is not in the
/// terminal's font, so it falls back to the system's emoji font, which draws it in colour —
/// read as a mask it used to fill its whole box. Needs a GPU and an emoji font; skipped without.
#[test]
fn an_emoji_glyph_enters_the_atlas_with_its_own_colours() {
    let (device, queue) = crate::gpu::headless_device_or_skip!();
    let mut fonts = crate::font::embedded_font_system();
    let mut swash = cosmic_text::SwashCache::new();
    let mut atlas = crate::atlas::Atlas::new(&device);
    let face = super::shape::Face {
        family: crate::font::DEFAULT_TERMINAL_FAMILY,
        weight: 400,
        italic: false,
        letter_spacing: 0.0,
        ligatures: false,
    };
    let glyph_of = |fonts: &mut cosmic_text::FontSystem, text: &str| {
        let buffer = super::shape::shape(fonts, text, 32.0, &face);
        let run = buffer.layout_runs().next().expect("a line");
        run.glyphs[0].physical((0.0, run.line_y), 1.0).cache_key
    };
    let letter = glyph_of(&mut fonts, "a");
    let entry = atlas
        .glyph(&queue, &mut fonts, &mut swash, letter)
        .expect("a has a bitmap");
    assert!(
        !entry.own_colours,
        "a letter is tinted by the text's colour"
    );
    let is_colour =
        |fonts: &mut cosmic_text::FontSystem, swash: &mut cosmic_text::SwashCache, key| {
            matches!(
                swash.get_image(fonts, key).as_ref().map(|img| img.content),
                Some(cosmic_text::SwashContent::Color)
            )
        };
    // An emoji by default (😀) is drawn by the colour face, in its own colours.
    let emoji = glyph_of(&mut fonts, "\u{1f600}");
    let Some(entry) = atlas.glyph(&queue, &mut fonts, &mut swash, emoji) else {
        return; // no fallback font for it on this machine
    };
    let colour = is_colour(&mut fonts, &mut swash, emoji);
    // macOS always has its colour emoji font, so there the case is certain, not skipped.
    #[cfg(target_os = "macos")]
    assert!(colour, "on macOS 😀 is drawn by the colour emoji font");
    assert_eq!(
        entry.own_colours, colour,
        "a colour glyph keeps its colours"
    );

    // **A text-by-default emoji (⏺) comes from a text face**, as in every terminal — a round dot
    // in the text's colour, not the colour face's framed button. Asked for with the emoji selector,
    // it is the colour one again.
    let dot = glyph_of(&mut fonts, "\u{23fa}");
    #[cfg(target_os = "macos")]
    assert!(
        !is_colour(&mut fonts, &mut swash, dot),
        "on macOS ⏺ is drawn from a text face (STIX Two Math has it)"
    );
    let _ = dot;
    let asked = glyph_of(&mut fonts, "\u{23fa}\u{fe0f}");
    #[cfg(target_os = "macos")]
    assert!(
        is_colour(&mut fonts, &mut swash, asked),
        "⏺ with the emoji selector is the colour emoji"
    );
    let _ = asked;
}
