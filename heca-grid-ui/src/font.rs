//! Default embedded monospace font.
//!
//! The Grid look ships with a default mono family so it renders identically
//! everywhere without depending on installed system fonts.
//!
//! Chosen face: **Geist Mono** (SIL Open Font License 1.1 — free to embed and
//! redistribute). <https://fonts.google.com/specimen/Geist+Mono>
//!
//! ## Status
//! Phase A wires the family *name* only. The actual font bytes are embedded and
//! loaded into the renderer's `cosmic-text` font system in Phase B:
//!
//! ```ignore
//! // Phase B (heca-renderer), once assets/GeistMono-Regular.ttf is vendored:
//! pub const DEFAULT_MONO_BYTES: &[u8] =
//!     include_bytes!("../assets/GeistMono-Regular.ttf");
//! // font_system.db_mut().load_font_data(DEFAULT_MONO_BYTES.to_vec());
//! ```

/// Family name of the default embedded monospace font.
pub const DEFAULT_MONO_FAMILY: &str = "Geist Mono";

/// Approximate advance width of a monospace glyph as a fraction of font size.
/// Used for the Phase-A naive text measure until `cosmic-text` shaping lands.
pub const MONO_ADVANCE_RATIO: f32 = 0.6;

/// Approximate line height as a fraction of font size (Phase-A measure only).
pub const MONO_LINE_RATIO: f32 = 1.4;
