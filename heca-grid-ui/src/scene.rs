//! The display list — `heca-grid-ui`'s GPU-free visual vocabulary.
//!
//! Components never call the GPU. Instead they emit [`DrawCommand`]s into a
//! [`Scene`], and `heca-renderer` rasterizes them (SDF glow, scanline, and text
//! pipelines). This keeps the component crate headless and unit-testable while
//! still driving real GPU effects: a component states the *intent*
//! (`Rect { glow: Some(..) }`), the renderer owns the *shader*.
//!
//! Coordinates are **f64 logical pixels** (reusing `heca-core` geometry); the
//! renderer scales to physical pixels by the display `scale_factor`, preserving
//! HiDPI crispness — consistent with the project's "f64 logical, f32 at the GPU
//! boundary" rule.

use crate::color::Color;
use heca_core::layout::Rectangle;

/// A retained list of draw commands, rebuilt each repaint.
#[derive(Debug, Default, Clone)]
pub struct Scene {
    commands: Vec<DrawCommand>,
}

impl Scene {
    /// An empty scene.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a command.
    pub fn push(&mut self, cmd: DrawCommand) {
        self.commands.push(cmd);
    }

    /// Clear all commands (reuse the allocation across frames).
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Number of queued commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether the scene has no commands.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Iterate the commands in submission order (consumed by the renderer).
    pub fn iter(&self) -> impl Iterator<Item = &DrawCommand> {
        self.commands.iter()
    }
}

/// One drawable element. Adding a new effect is one variant here plus one branch
/// in the renderer — component code is untouched.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    /// Solid or rounded rectangle, with optional border and additive outer glow.
    Rect(RectCmd),
    /// L-shaped corner brackets framing a rectangle (Tron reticle).
    Brackets(BracketCmd),
    /// A positioned text run.
    Text(TextCmd),
    /// A scanline overlay confined to a region.
    Scanline(ScanlineCmd),
    /// Push a clip rectangle; subsequent commands are clipped to it.
    PushClip(Rectangle),
    /// Pop the most recent clip rectangle.
    PopClip,
}

/// A filled/rounded rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectCmd {
    pub rect: Rectangle,
    pub fill: Color,
    pub border: Option<Border>,
    /// Corner radius in logical pixels (0.0 = sharp).
    pub radius: f32,
    pub glow: Option<Glow>,
}

/// A rectangle outline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    pub color: Color,
    pub width: f32,
}

/// An additive neon glow halo around a shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glow {
    pub color: Color,
    /// Falloff radius in logical pixels.
    pub radius: f32,
    /// Multiplier on glow strength (driven by theme intensity).
    pub intensity: f32,
}

/// Corner brackets framing a rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BracketCmd {
    pub rect: Rectangle,
    pub color: Color,
    /// Length of each bracket arm in logical pixels.
    pub len: f32,
    pub thickness: f32,
    pub glow: Option<Glow>,
}

/// Horizontal text alignment within the target rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

/// A run of text positioned within a rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct TextCmd {
    pub rect: Rectangle,
    pub text: String,
    pub color: Color,
    pub size: f32,
    pub align: TextAlign,
    /// Render with the bold weight.
    pub bold: bool,
}

/// A scanline overlay confined to a region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScanlineCmd {
    pub rect: Rectangle,
    pub color: Color,
    /// Vertical spacing between lines in logical pixels.
    pub spacing: f32,
    /// Per-line opacity multiplier (0.0..=1.0).
    pub opacity: f32,
}
