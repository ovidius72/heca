//! Structured font configuration, decoupled from color themes.
//!
//! Fonts are **system-local, not theme-portable**: a color theme that ships
//! `font_family = "Maple Mono Normal NF"` breaks on a system without that font
//! installed. Colors/palettes travel with a theme; font families and sizes do
//! not. So font configuration lives here in a dedicated `[font]` block of
//! `config.toml`, not in [`heca_theme::Theme`].
//!
//! ## Schema
//! ```toml
//! [font.family.ui]
//! normal = "Geist Mono"
//! # bold / italic / bold_italic are optional — unset falls back to `normal`
//! # and the renderer selects the face via weight/style within the family.
//!
//! [font.family.terminal]
//! normal = "Maple Mono Normal NF"
//!
//! [font.size]
//! ui = 15.0
//! terminal = 14.0
//! ```
//!
//! ## Per-style fallback
//! `bold` / `italic` / `bold_italic` are optional family slots. When unset,
//! [`FontFamilyGroup::resolve`] falls back to `normal` and the renderer picks
//! the face with `Weight::BOLD` / `Style::Italic` (or a synthesized oblique
//! when the family has no italic face). When set, the renderer uses the named
//! family for that style — so a user can point bold/italic at a different
//! installed font without touching the regular family.

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
//  Default family/size helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Embedded UI/chrome family — `Geist Mono` (regular + bold faces registered
/// under this name in the renderer). System-safe: the renderer embeds it, so
/// it resolves without a system install. A `const` (not a `fn`) so the name is
/// lifted once and the intent — a static fallback, not a configurable value —
/// is explicit.
const DEFAULT_UI_FAMILY: &str = "Geist Mono";

/// Embedded terminal family — `Maple Mono Normal NF` (regular + bold faces
/// embedded). System-safe for the same reason.
const DEFAULT_TERMINAL_FAMILY: &str = "Maple Mono Normal NF";

fn default_ui_size() -> f32 {
    15.0
}

fn default_terminal_size() -> f32 {
    14.0
}

// ═══════════════════════════════════════════════════════════════════════════════
//  FontFamilyGroup
// ═══════════════════════════════════════════════════════════════════════════════

/// Per-style font family slots for one surface (UI or terminal).
///
/// `normal` is the base family. `bold` / `italic` / `bold_italic` are optional;
/// when unset they fall back to `normal` (and the renderer selects the face via
/// weight/style within the `normal` family). When set, the renderer uses the
/// named family for that style.
///
/// Use [`FontFamilyGroup::resolve`] to pick the family for a given
/// (bold, italic) combination, and [`FontFamilyGroup::has_distinct_italic`] to
/// decide between a real italic face and a synthesized oblique.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FontFamilyGroup {
    /// Base (regular) family. Unset (None) → the renderer falls back to the
    /// **surface-appropriate embedded font** (Geist Mono for UI, Maple Mono
    /// Normal NF for terminal) via [`FontFamilies::ui_normal`] /
    /// [`FontFamilies::terminal_normal`]. This keeps a theme independent of
    /// system-installed fonts: omit `normal` and you get the bundled font for
    /// that surface.
    #[serde(default)]
    pub normal: Option<String>,
    /// Optional distinct family for bold runs. Unset → `normal` + weight-based
    /// bold face selection within the family.
    #[serde(default)]
    pub bold: Option<String>,
    /// Optional distinct family for italic runs. Unset → `normal` + synthesized
    /// oblique (the family has no italic face) or `Style::Italic` synthesis.
    #[serde(default)]
    pub italic: Option<String>,
    /// Optional distinct family for bold-italic runs. Unset → falls back to
    /// `italic`, then `bold`, then `normal`.
    #[serde(default)]
    pub bold_italic: Option<String>,
}

impl FontFamilyGroup {
    /// Default UI family group: `Geist Mono` regular, with bold/italic/bold_italic
    /// unset (the renderer picks the bold face within the family via weight, and
    /// synthesizes the oblique — Geist Mono ships no italic face).
    pub fn ui_default() -> Self {
        Self {
            normal: Some(DEFAULT_UI_FAMILY.to_string()),
            bold: None,
            italic: None,
            bold_italic: None,
        }
    }

    /// Default terminal family group: `Maple Mono Normal NF` regular, with
    /// bold/italic/bold_italic unset (embedded regular + bold faces; italic is
    /// synthesized).
    pub fn terminal_default() -> Self {
        Self {
            normal: Some(DEFAULT_TERMINAL_FAMILY.to_string()),
            bold: None,
            italic: None,
            bold_italic: None,
        }
    }

    /// Resolve the family name for a given (bold, italic) style combination,
    /// applying the fallback chain:
    /// - bold + italic → `bold_italic` → `italic` → `bold` → `normal`
    /// - bold only     → `bold` → `normal`
    /// - italic only   → `italic` → `normal`
    /// - neither       → `normal`
    ///
    /// `fallback_normal` is used when `normal` is unset (None) — callers pass
    /// the surface-appropriate embedded family via [`FontFamilies::ui_normal`]
    /// / [`FontFamilies::terminal_normal`] so a group with no `normal` still
    /// resolves to the correct bundled font for its surface.
    pub fn resolve<'a>(&'a self, bold: bool, italic: bool, fallback_normal: &'a str) -> &'a str {
        let normal = self.normal.as_deref().unwrap_or(fallback_normal);
        match (bold, italic) {
            (true, true) => self
                .bold_italic
                .as_deref()
                .or(self.italic.as_deref())
                .or(self.bold.as_deref())
                .unwrap_or(normal),
            (true, false) => self.bold.as_deref().unwrap_or(normal),
            (false, true) => self.italic.as_deref().unwrap_or(normal),
            (false, false) => normal,
        }
    }

    /// Whether a distinct italic family (or bold-italic family) is configured,
    /// vs. falling back to `normal` + synthesized oblique. The renderer uses
    /// this to decide between `Style::Italic` on a real italic face and a
    /// manually skewed oblique.
    pub fn has_distinct_italic(&self) -> bool {
        self.italic.is_some() || self.bold_italic.is_some()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  FontFamilies / FontSizes / FontConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// Font family groups for the two rendering surfaces: UI/chrome and terminal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FontFamilies {
    /// UI/chrome font families (labels, sidebar, status bar, chrome widgets).
    #[serde(default = "FontFamilyGroup::ui_default")]
    pub ui: FontFamilyGroup,
    /// Terminal pane font families.
    #[serde(default = "FontFamilyGroup::terminal_default")]
    pub terminal: FontFamilyGroup,
}

impl Default for FontFamilies {
    fn default() -> Self {
        Self {
            ui: FontFamilyGroup::ui_default(),
            terminal: FontFamilyGroup::terminal_default(),
        }
    }
}

impl FontFamilies {
    /// Resolved UI normal/regular family: the configured `normal` if set,
    /// otherwise the embedded **Geist Mono** fallback. This is the surface-aware
    /// default so omitting `[font.family.ui].normal` (or the whole table) keeps
    /// the bundled UI font instead of leaking the terminal font or vice-versa.
    pub fn ui_normal(&self) -> &str {
        self.ui.normal.as_deref().unwrap_or(DEFAULT_UI_FAMILY)
    }

    /// Resolved terminal normal/regular family: the configured `normal` if set,
    /// otherwise the embedded **Maple Mono Normal NF** fallback. See
    /// [`FontFamilies::ui_normal`] for why the fallback is surface-specific.
    pub fn terminal_normal(&self) -> &str {
        self.terminal
            .normal
            .as_deref()
            .unwrap_or(DEFAULT_TERMINAL_FAMILY)
    }
}

/// Font sizes (logical px) for the two rendering surfaces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FontSizes {
    /// UI/chrome font size.
    #[serde(default = "default_ui_size")]
    pub ui: f32,
    /// Terminal pane font size.
    #[serde(default = "default_terminal_size")]
    pub terminal: f32,
}

impl Default for FontSizes {
    fn default() -> Self {
        Self {
            ui: default_ui_size(),
            terminal: default_terminal_size(),
        }
    }
}

/// Structured font configuration — the single source of truth for font families
/// and sizes, independent of the color [`heca_theme::Theme`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct FontConfig {
    /// Per-surface family groups (UI + terminal).
    #[serde(default)]
    pub family: FontFamilies,
    /// Per-surface sizes (UI + terminal).
    #[serde(default)]
    pub size: FontSizes,
}

impl FontConfig {
    /// Validate the font config: sizes must be finite and positive; any set
    /// family name must be non-empty. Returns an error message on failure.
    pub fn validate(&self) -> Result<(), String> {
        fn check_size(name: &str, size: f32) -> Result<(), String> {
            if !size.is_finite() || size <= 0.0 {
                return Err(format!(
                    "{name} must be a finite positive number (got {size})"
                ));
            }
            Ok(())
        }
        fn check_group(name: &str, g: &FontFamilyGroup) -> Result<(), String> {
            // `normal` may be unset (None) — that means "use the embedded
            // fallback for this surface", which is valid. Only reject an
            // *explicitly empty* normal.
            if let Some(fam) = &g.normal
                && fam.trim().is_empty()
            {
                return Err(format!("{name}.normal must not be empty when set"));
            }
            for (slot, val) in [
                ("bold", &g.bold),
                ("italic", &g.italic),
                ("bold_italic", &g.bold_italic),
            ] {
                if let Some(fam) = val
                    && fam.trim().is_empty()
                {
                    return Err(format!("{name}.{slot} must not be empty when set"));
                }
            }
            Ok(())
        }
        check_size("font.size.ui", self.size.ui)?;
        check_size("font.size.terminal", self.size.terminal)?;
        check_group("font.family.ui", &self.family.ui)?;
        check_group("font.family.terminal", &self.family.terminal)?;
        Ok(())
    }

    /// Approximate terminal cell metrics for the terminal font size, used as a
    /// heuristic fallback when the real font can't be measured. Width ≈ 0.58em,
    /// height ≈ 1.28em — the same ratios the old `Theme::terminal_cell_size`
    /// used, relocated here since font size no longer lives on `Theme`.
    pub fn terminal_cell_size(&self) -> (f32, f32) {
        self.terminal_cell_size_for(self.size.terminal)
    }

    /// Same heuristic as [`terminal_cell_size`](Self::terminal_cell_size) but for
    /// an explicit font size — used by per-pane font zoom, where a pane's size can
    /// differ from the configured `size.terminal`.
    pub fn terminal_cell_size_for(&self, size: f32) -> (f32, f32) {
        const TERMINAL_CELL_WIDTH_RATIO: f32 = 0.58;
        const TERMINAL_CELL_HEIGHT_RATIO: f32 = 1.28;
        (
            size * TERMINAL_CELL_WIDTH_RATIO,
            size * TERMINAL_CELL_HEIGHT_RATIO,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_config_default_uses_embedded_families() {
        let fc = FontConfig::default();
        assert_eq!(fc.family.ui_normal(), "Geist Mono");
        assert_eq!(fc.family.terminal_normal(), "Maple Mono Normal NF");
        assert_eq!(fc.size.ui, 15.0);
        assert_eq!(fc.size.terminal, 14.0);
        // No per-style overrides by default → renderer uses weight/style within
        // the normal family.
        assert!(fc.family.ui.bold.is_none());
        assert!(fc.family.terminal.italic.is_none());
    }

    #[test]
    fn resolve_falls_back_to_normal() {
        let g = FontFamilyGroup::terminal_default();
        assert_eq!(
            g.resolve(false, false, "Maple Mono Normal NF"),
            "Maple Mono Normal NF"
        );
        assert_eq!(
            g.resolve(true, false, "Maple Mono Normal NF"),
            "Maple Mono Normal NF"
        );
        assert_eq!(
            g.resolve(false, true, "Maple Mono Normal NF"),
            "Maple Mono Normal NF"
        );
        assert_eq!(
            g.resolve(true, true, "Maple Mono Normal NF"),
            "Maple Mono Normal NF"
        );
        assert!(!g.has_distinct_italic());
    }

    #[test]
    fn resolve_uses_surface_fallback_when_normal_unset() {
        // `normal` omitted but the table is present: resolve must use the
        // surface-appropriate embedded fallback, NOT the other surface's font.
        let g = FontFamilyGroup {
            normal: None,
            bold: None,
            italic: None,
            bold_italic: None,
        };
        assert_eq!(
            g.resolve(false, false, "Maple Mono Normal NF"),
            "Maple Mono Normal NF"
        );
        assert_eq!(g.resolve(true, true, "Geist Mono"), "Geist Mono");
    }

    #[test]
    fn resolve_uses_distinct_slots_when_set() {
        let g = FontFamilyGroup {
            normal: Some("Regular".to_string()),
            bold: Some("Bold".to_string()),
            italic: Some("Italic".to_string()),
            bold_italic: Some("BoldItalic".to_string()),
        };
        assert_eq!(g.resolve(false, false, "fallback"), "Regular");
        assert_eq!(g.resolve(true, false, "fallback"), "Bold");
        assert_eq!(g.resolve(false, true, "fallback"), "Italic");
        assert_eq!(g.resolve(true, true, "fallback"), "BoldItalic");
        assert!(g.has_distinct_italic());
    }

    #[test]
    fn bold_italic_falls_back_through_italic_then_bold() {
        let g = FontFamilyGroup {
            normal: Some("Regular".to_string()),
            bold: Some("Bold".to_string()),
            italic: Some("Italic".to_string()),
            bold_italic: None,
        };
        // bold_italic unset → italic wins (italic is a closer match than bold).
        assert_eq!(g.resolve(true, true, "fallback"), "Italic");
        assert!(g.has_distinct_italic());
    }

    #[test]
    fn italic_only_distinct_still_reports_distinct_italic() {
        let g = FontFamilyGroup {
            normal: Some("Regular".to_string()),
            bold: Some("Bold".to_string()),
            italic: None,
            bold_italic: Some("BoldItalic".to_string()),
        };
        // bold_italic set → distinct italic available even though `italic` is None.
        assert!(g.has_distinct_italic());
        assert_eq!(g.resolve(false, true, "fallback"), "Regular");
        assert_eq!(g.resolve(true, true, "fallback"), "BoldItalic");
    }

    #[test]
    fn parse_full_font_config() {
        // Parsed as a bare `FontConfig`: top-level keys are `family` / `size`
        // (the `[font]` prefix only appears when embedded in the full `Config`).
        let toml = r#"
[family.ui]
normal = "Iosevka"
bold = "Iosevka Bold"

[family.terminal]
normal = "Iosevka Term"
italic = "Iosevka Term Italic"

[size]
ui = 16.0
terminal = 14.0
"#;
        let cfg: FontConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.family.ui_normal(), "Iosevka");
        assert_eq!(cfg.family.ui.bold.as_deref(), Some("Iosevka Bold"));
        assert_eq!(
            cfg.family.terminal.italic.as_deref(),
            Some("Iosevka Term Italic")
        );
        assert_eq!(cfg.size.ui, 16.0);
        assert_eq!(cfg.size.terminal, 14.0);
        cfg.validate().unwrap();
    }

    #[test]
    fn parse_partial_font_config_uses_defaults() {
        // Only terminal family specified; ui + sizes fall back to defaults.
        let toml = r#"
[family.terminal]
normal = "JetBrains Mono"
"#;
        let cfg: FontConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.family.terminal_normal(), "JetBrains Mono");
        assert_eq!(cfg.family.ui_normal(), "Geist Mono");
        assert_eq!(cfg.size.ui, 15.0);
        assert_eq!(cfg.size.terminal, 14.0);
    }

    #[test]
    fn parse_omitted_normal_uses_surface_embedded_fallback() {
        // `[font.family.terminal]` present but `normal` omitted (only `bold`
        // set): the terminal surface must fall back to its OWN embedded font
        // (Maple Mono Normal NF), not the UI embedded font (Geist Mono).
        let toml = r#"
[family.terminal]
bold = "Maple Mono Bold NF"
"#;
        let cfg: FontConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.family.terminal_normal(), "Maple Mono Normal NF");
        assert_eq!(
            cfg.family.terminal.bold.as_deref(),
            Some("Maple Mono Bold NF")
        );
        cfg.validate().unwrap();
    }

    #[test]
    fn validate_rejects_non_positive_sizes() {
        let mut cfg = FontConfig::default();
        cfg.size.ui = 0.0;
        assert!(cfg.validate().is_err());
        cfg.size.ui = -1.0;
        assert!(cfg.validate().is_err());
        cfg.size.ui = f32::NAN;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_empty_family_names() {
        let mut cfg = FontConfig::default();
        cfg.family.ui.normal = Some("  ".to_string());
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn terminal_cell_size_tracks_terminal_font_size() {
        let mut cfg = FontConfig::default();
        cfg.size.terminal = 20.0;
        let (w, h) = cfg.terminal_cell_size();
        assert!((w - 20.0 * 0.58).abs() < f32::EPSILON);
        assert!((h - 20.0 * 1.28).abs() < f32::EPSILON);
    }
}
