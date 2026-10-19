//! GPU rendering of a terminal snapshot: the render types, the colour helpers,
//! the [`TerminalRenderer`] itself, and the glyph-cell decorations.

mod decorations;
mod render;

pub use render::TerminalRenderer;

/// Hyperlink decoration the renderer applies to OSC 8 link spans. Renderer-side
/// mirror of `heca_config::appearance::HyperlinkStyle` (kept config-free here).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HyperlinkDecor {
    /// No special rendering.
    None,
    /// Recolor only.
    Color,
    /// Recolor + straight underline.
    Underline,
    /// Recolor + wavy undercurl.
    Undercurl,
}

/// Host-level selection overlay parameters for terminal panes.
///
/// The renderer is agnostic of the shared selection model; it receives
/// pre-computed row spans and a color from the app layer. The app layer is
/// responsible for converting anchor/focus cell coordinates into row-wise
/// spans that follow terminal text-flow semantics.
#[derive(Clone, Copy, Debug)]
pub struct SelectionOverlaySpan {
    pub row: usize,
    pub start_col: usize,
    pub end_col: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct CaretIndicator {
    pub row: usize,
    pub col: usize,
    /// When true, the caret marks the active endpoint of an existing
    /// selection (thicker, more prominent). When false, the caret is in
    /// caret-only state (thin bar, no selection yet).
    pub is_selection_endpoint: bool,
}

#[derive(Clone, Debug)]
pub struct SelectionOverlay {
    pub spans: Vec<SelectionOverlaySpan>,
    pub color: [f32; 4],
    /// Caret indicator: drawn as a blinking cursor-like block when in
    /// caret-only state (selection mode entered but no selection started).
    pub caret: Option<CaretIndicator>,
}

impl SelectionOverlay {
    pub fn new(spans: Vec<SelectionOverlaySpan>, color: [f32; 4]) -> Self {
        Self {
            spans,
            color,
            caret: None,
        }
    }

    pub fn with_caret(mut self, caret: CaretIndicator) -> Self {
        self.caret = Some(caret);
        self
    }
}

#[derive(Clone, Copy)]
pub struct TerminalStyle<'a> {
    pub font_size: f32,
    pub families: TerminalFontFamilies<'a>,
    /// Alpha multiplier for the terminal surface background.
    /// 1.0 = fully opaque (no transparency). When < 1.0, the default-bg fill
    /// becomes translucent so a frosted backdrop can show through.
    /// Derived from `AppearanceConfig::opacity()`.
    pub surface_alpha: f32,
    /// Whether coding-font ligatures (`calt`/`liga`/`clig`) are applied when
    /// shaping terminal runs. `false` disables them so each glyph stands alone.
    pub ligatures: bool,
    /// Decoration applied to OSC 8 hyperlink spans.
    pub hyperlink_style: HyperlinkDecor,
    /// Color used to recolor hyperlink text + its decoration (resolved from
    /// `[appearance.terminal] hyperlink_color` → `theme.accent`).
    pub hyperlink_color: [f32; 4],
}

/// Per-style font family slots for terminal rendering, mirroring the app's
/// `heca_config::font::FontFamilyGroup` but living in the renderer so the
/// renderer stays config-free. Optional slots fall back to `normal` via
/// [`TerminalFontFamilies::resolve`]; the renderer then selects the face with
/// `Weight::BOLD` / `Style::Italic` (or a synthesized oblique when the resolved
/// family is `normal` and no distinct italic face exists).
#[derive(Clone, Copy)]
pub struct TerminalFontFamilies<'a> {
    /// Base (regular) family.
    pub normal: &'a str,
    /// Optional distinct family for bold runs. Unset → `normal` + weight-based
    /// bold face selection within the family.
    pub bold: Option<&'a str>,
    /// Optional distinct family for italic runs. Unset → `normal` + synthesized
    /// oblique.
    pub italic: Option<&'a str>,
    /// Optional distinct family for bold-italic runs. Unset → `italic` → `bold`
    /// → `normal`.
    pub bold_italic: Option<&'a str>,
}

impl<'a> TerminalFontFamilies<'a> {
    /// Resolve the family name for a given (bold, italic) style combination,
    /// applying the fallback chain: bold_italic → italic → bold → normal.
    pub fn resolve(&self, bold: bool, italic: bool) -> &'a str {
        match (bold, italic) {
            (true, true) => self
                .bold_italic
                .or(self.italic)
                .or(self.bold)
                .unwrap_or(self.normal),
            (true, false) => self.bold.unwrap_or(self.normal),
            (false, true) => self.italic.unwrap_or(self.normal),
            (false, false) => self.normal,
        }
    }
}

fn with_surface_alpha(color: [f32; 4], surface_alpha: f32) -> [f32; 4] {
    [
        color[0],
        color[1],
        color[2],
        color[3] * surface_alpha.clamp(0.0, 1.0),
    ]
}

fn is_default_bg(bg: [f32; 4], default_bg: [f32; 4]) -> bool {
    const EPS: f32 = 1e-6;
    (bg[0] - default_bg[0]).abs() < EPS
        && (bg[1] - default_bg[1]).abs() < EPS
        && (bg[2] - default_bg[2]).abs() < EPS
        && (bg[3] - default_bg[3]).abs() < EPS
}

#[cfg(test)]
mod tests {
    use super::{TerminalFontFamilies, is_default_bg, with_surface_alpha};

    #[test]
    fn with_surface_alpha_scales_alpha_only() {
        let color = [0.25, 0.5, 0.75, 0.8];
        let scaled = with_surface_alpha(color, 0.5);
        assert_eq!(scaled[0], color[0]);
        assert_eq!(scaled[1], color[1]);
        assert_eq!(scaled[2], color[2]);
        assert!((scaled[3] - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn with_surface_alpha_clamps_input() {
        let color = [1.0, 1.0, 1.0, 0.8];
        assert!((with_surface_alpha(color, 2.0)[3] - 0.8).abs() < f32::EPSILON);
        assert!((with_surface_alpha(color, -1.0)[3] - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn is_default_bg_uses_exact_visual_match() {
        let bg = [0.1, 0.2, 0.3, 1.0];
        assert!(is_default_bg(bg, bg));
        assert!(!is_default_bg(bg, [0.1, 0.2, 0.31, 1.0]));
    }

    #[test]
    fn terminal_families_resolve_falls_back_to_normal() {
        // No per-style overrides: every style resolves to `normal`.
        let f = TerminalFontFamilies {
            normal: "Maple Mono Normal NF",
            bold: None,
            italic: None,
            bold_italic: None,
        };
        assert_eq!(f.resolve(false, false), "Maple Mono Normal NF");
        assert_eq!(f.resolve(true, false), "Maple Mono Normal NF");
        assert_eq!(f.resolve(false, true), "Maple Mono Normal NF");
        assert_eq!(f.resolve(true, true), "Maple Mono Normal NF");
    }

    #[test]
    fn terminal_families_resolve_bold_italic_falls_back_to_bold_when_italic_unset() {
        // Edge case from the review: bold+italic with bold_italic=None and
        // italic=None but bold=Some → resolve to the bold family (not normal).
        // The render loop then sets `is_faux_italic = resolved == normal` →
        // false, so the bold family is used with a real italic request rather
        // than a faux-italic on normal. This is the improvement over the old
        // `italic_font_family == font_family` comparison.
        let f = TerminalFontFamilies {
            normal: "Maple Mono Normal NF",
            bold: Some("Maple Mono Bold NF"),
            italic: None,
            bold_italic: None,
        };
        assert_eq!(f.resolve(true, true), "Maple Mono Bold NF");
        assert_eq!(f.resolve(true, false), "Maple Mono Bold NF");
        // italic-only still falls back to normal (faux italic).
        assert_eq!(f.resolve(false, true), "Maple Mono Normal NF");
    }

    #[test]
    fn terminal_families_resolve_full_chain() {
        // All four slots set: each style picks its own family.
        let f = TerminalFontFamilies {
            normal: "Regular",
            bold: Some("Bold"),
            italic: Some("Italic"),
            bold_italic: Some("BoldItalic"),
        };
        assert_eq!(f.resolve(false, false), "Regular");
        assert_eq!(f.resolve(true, false), "Bold");
        assert_eq!(f.resolve(false, true), "Italic");
        assert_eq!(f.resolve(true, true), "BoldItalic");
    }
}
