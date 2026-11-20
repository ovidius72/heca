use super::emit::{EmitKey, align_bits, glow_bits};
use super::TextGlow;
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
    let quad = glyph_quad(rect, 2.0, [0.0, 0.0, 1.0, 1.0], [1.0; 4], 0.0);
    let corners: Vec<[f32; 2]> = quad.iter().map(|v| v.position).collect();
    assert_eq!(
        corners,
        vec![[11.0, 20.0], [15.0, 20.0], [13.0, 28.0], [9.0, 28.0]],
        "the top moves right by half the slant and the bottom left by half"
    );
    let mut ink = Ink::new();
    ink.add(rect, 2.0);
    assert_eq!(ink.bbox(0.0), [9.0, 20.0, 6.0, 8.0]);
    assert_eq!(ink.bbox(1.0), [8.0, 19.0, 8.0, 10.0], "a halo's reach grows every side");
}

/// **Upright text does not lean; a faux italic leans by a share of its line, at least a pixel.**
#[test]
fn a_faux_italic_leans_by_a_share_of_its_line() {
    use super::shape::faux_italic_slant;
    assert_eq!(faux_italic_slant(40.0, 2.0, false), 0.0);
    assert!((faux_italic_slant(40.0, 2.0, true) - 2.8).abs() < 1e-5);
    assert_eq!(faux_italic_slant(4.0, 2.0, true), 1.0, "never less than a pixel");
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
    for weight in [FontWeight::Regular, FontWeight::Medium, FontWeight::SemiBold, FontWeight::Bold] {
        let face = Face {
            family: heca_grid_ui::font::DEFAULT_MONO_FAMILY,
            weight: weight.value(),
            italic: false,
            letter_spacing: 0.0,
            ligatures: true,
        };
        let buffer = shape(&mut fonts, "M", 24.0, &face);
        let id = buffer.layout_runs().next().and_then(|run| run.glyphs.first()).map(|g| g.font_id);
        let id = id.expect("the UI family shapes an M");
        let found = fonts.db().face(id).expect("a loaded face");
        assert_eq!(found.weight.0, weight.value(), "{weight:?} shaped with a {}-weight face", found.weight.0);
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
        shape(fonts, "abcd", 20.0, &face).layout_runs().next().map_or(0.0, |run| run.line_w)
    };
    let plain = width(&mut fonts, 0.0);
    let spaced = width(&mut fonts, 0.1);
    assert!(
        (spaced - plain - 4.0 * 0.1 * 20.0).abs() < 0.5,
        "four glyphs, each 0.1 em (2px) wider: {plain} → {spaced}"
    );
}
