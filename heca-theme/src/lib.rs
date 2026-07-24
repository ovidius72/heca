//! # heca-theme
//!
//! Unified theme types and bundled palettes for heca.
//!
//! This crate provides the single source of truth for all theming:
//! - [`Theme`] struct with palette colors, typography, effect tokens, and terminal config
//! - [`Color`] type with hex parsing and serde support
//! - [`Intensity`] and [`GlowLevel`] effect enums
//! - Bundled themes: `grid_tron` (dark, default), `mocha` (dark), `latte` (light)
//! - [`loader::load_theme`] for user-config → bundled → fallback resolution

pub mod color;
pub mod loader;
pub mod theme;

pub use color::Color;
pub use loader::{available_themes, config_dir, load_theme};
pub use theme::{FrameStyle, GlowLevel, Intensity, Shadow, Theme};
