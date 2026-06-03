//! Default embedded monospace font.
//!
//! The Grid look ships with a default mono family so it renders identically
//! everywhere without depending on installed system fonts.
//!
//! Chosen face: **Geist Mono** (SIL Open Font License 1.1 — free to embed and
//! redistribute). <https://fonts.google.com/specimen/Geist+Mono>
//!
//! The renderer loads the embedded faces into its `cosmic-text` font system so
//! `Geist Mono` resolves without a system install. These are *defaults*, not
//! hardcoded overrides: `Theme.font_family` / `Theme.font_size` (and a future
//! weight/style token) stay configurable.
//!
//! ## Weights & italic
//! Both [`DEFAULT_MONO_BYTES`] (Regular) and [`DEFAULT_MONO_BOLD_BYTES`] (Bold)
//! register under the same family `"Geist Mono"`; `cosmic-text` selects between
//! them via `Attrs::weight(...)`. Geist Mono ships **no italic face**, so italic
//! is rendered as a **synthesized oblique** (skew) — request it with
//! `Attrs::style(cosmic_text::Style::Italic)` and let the shaper fake-slant the
//! upright glyphs, or apply [`OBLIQUE_SKEW`] manually.

/// Family name of the default embedded monospace font (shared by all weights).
pub const DEFAULT_MONO_FAMILY: &str = "Geist Mono";

/// Embedded bytes of the default monospace font (Geist Mono **Regular**, OFL 1.1).
/// Load once into the renderer's font database:
/// ```ignore
/// font_system.db_mut().load_font_data(DEFAULT_MONO_BYTES.to_vec());
/// font_system.db_mut().load_font_data(DEFAULT_MONO_BOLD_BYTES.to_vec());
/// ```
pub const DEFAULT_MONO_BYTES: &[u8] = include_bytes!("../assets/GeistMono-Regular.ttf");

/// Embedded bytes of the **Bold** weight (Geist Mono Bold, OFL 1.1). Registers
/// under the same family as [`DEFAULT_MONO_BYTES`]; selected via `Attrs::weight`.
pub const DEFAULT_MONO_BOLD_BYTES: &[u8] = include_bytes!("../assets/GeistMono-Bold.ttf");

/// Horizontal skew (tangent of the slant angle, ~12°) for synthesizing an
/// oblique/italic from the upright faces, since Geist Mono has no italic face.
pub const OBLIQUE_SKEW: f32 = 0.21;

/// Approximate advance width of a monospace glyph as a fraction of font size.
/// Used for the Phase-A naive text measure until `cosmic-text` shaping lands.
pub const MONO_ADVANCE_RATIO: f32 = 0.6;

/// Approximate line height as a fraction of font size (Phase-A measure only).
pub const MONO_LINE_RATIO: f32 = 1.4;
