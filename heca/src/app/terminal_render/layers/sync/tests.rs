use super::{retained_terminal_texture_size, terminal_layer_render_key};
use heca_renderer::terminal::{TerminalFontFamilies, TerminalStyle};

// --- `terminal-00c` app-path retained-presentation coverage -----------------
//
// The retained-content foundation (`terminal-00b`) keeps the last frame's
// content in a per-pane layer and only re-renders dirty rows. These tests
// pin the pure policy that protects unchanged rows: `retained_damage_to_apply`
// decides skip / Full / passthrough, `retained_terminal_texture_size` sizes
// the offscreen scratch, and `terminal_layer_render_key` detects style changes.

fn style(font_size: f32, surface_alpha: f32) -> TerminalStyle<'static> {
    TerminalStyle {
        font_size,
        families: TerminalFontFamilies {
            normal: "Maple Mono Normal NF",
            bold: None,
            italic: None,
            bold_italic: None,
        },
        surface_alpha,
        ligatures: true,
        hyperlink_style: heca_renderer::terminal::HyperlinkDecor::Underline,
        hyperlink_color: [0.4, 0.6, 1.0, 1.0],
    }
}

#[test]
fn retained_terminal_texture_size_scales_and_rounds_up() {
    use heca_core::layout::{Point, Rectangle, Size};
    let rect = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 40.0));
    // scale 2.0 → 200x80, ceiled.
    let sz = retained_terminal_texture_size(rect, 2.0);
    assert_eq!((sz.width, sz.height), (200, 80));
    // Fractional physical pixels round up (.ceil) so partial rows aren't lost.
    let sz = retained_terminal_texture_size(
        Rectangle::new(Point::new(0.0, 0.0), Size::new(10.5, 5.25)),
        2.0,
    );
    assert_eq!((sz.width, sz.height), (21, 11));
}

#[test]
fn retained_terminal_texture_size_never_zero_for_positive_rect() {
    use heca_core::layout::{Point, Rectangle, Size};
    // A sub-pixel pane still yields at least 1x1 so the texture is valid.
    let sz = retained_terminal_texture_size(
        Rectangle::new(Point::new(0.0, 0.0), Size::new(0.1, 0.1)),
        1.0,
    );
    assert_eq!((sz.width, sz.height), (1, 1));
}

#[test]
fn terminal_layer_render_key_is_stable_for_identical_style() {
    assert_eq!(
        terminal_layer_render_key(&style(14.0, 0.8)),
        terminal_layer_render_key(&style(14.0, 0.8))
    );
}

#[test]
fn terminal_layer_render_key_changes_with_font_size() {
    assert_ne!(
        terminal_layer_render_key(&style(14.0, 0.8)),
        terminal_layer_render_key(&style(15.0, 0.8))
    );
}

#[test]
fn terminal_layer_render_key_changes_with_surface_alpha() {
    assert_ne!(
        terminal_layer_render_key(&style(14.0, 0.8)),
        terminal_layer_render_key(&style(14.0, 0.6))
    );
}

#[test]
fn terminal_layer_render_key_changes_with_font_family() {
    let a = TerminalStyle {
        font_size: 14.0,
        families: TerminalFontFamilies {
            normal: "Mono A",
            bold: None,
            italic: None,
            bold_italic: None,
        },
        surface_alpha: 0.8,
        ligatures: true,
        hyperlink_style: heca_renderer::terminal::HyperlinkDecor::Underline,
        hyperlink_color: [0.4, 0.6, 1.0, 1.0],
    };
    let b = TerminalStyle {
        font_size: 14.0,
        families: TerminalFontFamilies {
            normal: "Mono B",
            bold: None,
            italic: None,
            bold_italic: None,
        },
        surface_alpha: 0.8,
        ligatures: true,
        hyperlink_style: heca_renderer::terminal::HyperlinkDecor::Underline,
        hyperlink_color: [0.4, 0.6, 1.0, 1.0],
    };
    assert_ne!(terminal_layer_render_key(&a), terminal_layer_render_key(&b));
}

#[test]
fn terminal_layer_render_key_changes_with_ligatures() {
    // Toggling ligatures must change the render key so the retained terminal
    // layer re-renders (the `terminal_ligatures` setting applies live).
    let on = style(14.0, 0.8);
    let mut off = style(14.0, 0.8);
    off.ligatures = false;
    assert_ne!(
        terminal_layer_render_key(&on),
        terminal_layer_render_key(&off)
    );
}
