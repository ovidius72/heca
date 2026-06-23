//! [`Icon`] — a single glyph from the embedded **Phosphor Duotone** icon font.
//!
//! Duotone glyphs render as **two stacked layers**: a *secondary* background
//! layer (Phosphor's `:before` codepoint, drawn dimmed) and a *primary*
//! foreground layer (the next codepoint, full strength) overlaid at the same
//! spot. The widget emits both runs via [`PaintCx::icon`](crate::component::PaintCx::icon),
//! shaped with the icon family.
//!
//! Colors are **theme/config-driven**, never baked in: the primary defaults to
//! the theme foreground, and the secondary defaults to the primary at the theme's
//! [`icon_secondary_alpha`](crate::theme::Theme::icon_secondary_alpha). A caller
//! (or a Dock) can override either with [`color`](Icon::color) /
//! [`secondary_color`](Icon::secondary_color) to tint icons by state. Only the
//! font *bytes* are static (embedded); the look is configurable.

use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::{Signal, SignalGet, signal};
use crate::style::Length;

/// Offset from a duotone glyph's secondary (`:before`) codepoint to its primary
/// (`:after`) layer — Phosphor pairs them consecutively.
const PRIMARY_OFFSET: u32 = 1;

/// A curated set of Phosphor icons, by name. Each value is the **secondary**
/// (`:before`) codepoint; the primary layer is `secondary + 1`. Use
/// [`Icon::from_codepoint`] for any glyph outside this set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Folder,
    FolderOpen,
    File,
    FileCode,
    GitBranch,
    GitCommit,
    GitMerge,
    GitPullRequest,
    Terminal,
    Gear,
    Search,
    Close,
    Check,
    CaretRight,
    CaretDown,
    Play,
    Pause,
    Stop,
    Warning,
    WarningCircle,
    Info,
    Circle,
    Lightning,
    List,
    Sidebar,
    DotsThreeVertical,
    ArrowRight,
    ArrowLineLeft,
    ArrowLineRight,
    Plus,
    Minus,
    SquareSplitVertical,
    XSquare,
    FrameCorners,
    Cards,
}

impl Glyph {
    /// The secondary-layer (`:before`) codepoint; the primary layer is this `+ 1`.
    const fn secondary(self) -> u32 {
        match self {
            Glyph::Folder => 0xe24a,
            Glyph::FolderOpen => 0xe256,
            Glyph::File => 0xe230,
            Glyph::FileCode => 0xe914,
            Glyph::GitBranch => 0xe278,
            Glyph::GitCommit => 0xe27a,
            Glyph::GitMerge => 0xe280,
            Glyph::GitPullRequest => 0xe282,
            Glyph::Terminal => 0xeae8,
            Glyph::Gear => 0xe272,
            Glyph::Search => 0xe30c,
            Glyph::Close => 0xe4f6,
            Glyph::Check => 0xe182,
            Glyph::CaretRight => 0xe13a,
            Glyph::CaretDown => 0xe136,
            Glyph::Play => 0xe3d0,
            Glyph::Pause => 0xe39e,
            Glyph::Stop => 0xe46c,
            Glyph::Warning => 0xe4e0,
            Glyph::WarningCircle => 0xe4e2,
            Glyph::Info => 0xe2ce,
            Glyph::Circle => 0xe18a,
            Glyph::Lightning => 0xe2de,
            Glyph::List => 0xe2f0,
            Glyph::Sidebar => 0xec24,
            Glyph::DotsThreeVertical => 0xe208,
            Glyph::ArrowRight => 0xe06c,
            Glyph::ArrowLineLeft => 0xe062,
            Glyph::ArrowLineRight => 0xe064,
            Glyph::Plus => 0xe3d4,
            Glyph::Minus => 0xe32a,
            Glyph::SquareSplitVertical => 0xe874,
            Glyph::XSquare => 0xe4fa,
            Glyph::FrameCorners => 0xe626,
            Glyph::Cards => 0xe0f8,
        }
    }

    /// The **primary-layer** character (the full-strength glyph), for callers that
    /// draw a single-layer icon manually via [`PaintCx::icon`](crate::component::PaintCx::icon)
    /// (e.g. command-palette rows). Duotone rendering uses both layers; this is the
    /// foreground one.
    pub fn primary_char(self) -> Option<char> {
        char::from_u32(self.secondary() + PRIMARY_OFFSET)
    }
}

/// A duotone icon glyph. Sizes to a square of the resolved font size (or an
/// explicit [`size`](Icon::size)); colors come from the theme unless overridden.
pub struct Icon {
    base: Base,
    glyph: Signal<Glyph>,
    seen_glyph: Glyph,
    /// Explicit glyph size (px); otherwise the inherited font size.
    size: Option<f32>,
    /// Primary-layer color override (default: theme foreground).
    color: Option<Color>,
    /// Secondary-layer color override (default: primary at theme alpha).
    secondary: Option<Color>,
}

impl Icon {
    /// A new icon for a named [`Glyph`].
    pub fn new(glyph: Glyph) -> Self {
        let mut icon = Self {
            base: Base::new(),
            glyph: signal(glyph),
            seen_glyph: glyph,
            size: None,
            color: None,
            secondary: None,
        };
        icon.remeasure();
        icon
    }

    /// A new icon from a raw **secondary** (`:before`) codepoint — for glyphs
    /// outside the [`Glyph`] set. The primary layer is `secondary_cp + 1`.
    pub fn from_codepoint(secondary_cp: u32) -> Self {
        Self::new(Glyph::from_secondary(secondary_cp))
    }

    /// Handle to the icon's glyph signal so hosts can update it live.
    pub fn glyph_signal(&self) -> Signal<Glyph> {
        self.glyph
    }

    /// Explicit glyph size in logical px (overrides the inherited font size).
    pub fn size(mut self, px: f32) -> Self {
        self.size = Some(px);
        self.remeasure();
        self
    }

    /// Primary-layer color (default: theme foreground). The secondary layer
    /// follows it (dimmed) unless set via [`secondary_color`](Icon::secondary_color).
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    /// Explicit secondary-layer color (default: the primary color at the theme's
    /// [`icon_secondary_alpha`](crate::theme::Theme::icon_secondary_alpha)).
    pub fn secondary_color(mut self, c: Color) -> Self {
        self.secondary = Some(c);
        self
    }

    fn glyph_size(&self) -> f32 {
        match self.size {
            // An explicit px still tracks the size variant (Small/Normal/Large) by
            // its font scale — otherwise icon-only buttons wouldn't resize. The
            // font-driven path already includes the variant via `base.font`.
            Some(px) => px * self.base.style.size.font_scale(),
            None => self.base.font,
        }
    }
}

impl Component for Icon {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Lay out as a square of the glyph size (font-driven unless explicit).
    fn remeasure(&mut self) {
        let s = self.glyph_size();
        self.base.style.width = Length::Px(s);
        self.base.style.height = Length::Px(s);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let size = self.glyph_size();
        let primary = self.color.unwrap_or_else(|| cx.theme().foreground);
        let secondary = self.secondary.unwrap_or_else(|| {
            let a = (cx.theme().icon_secondary_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
            primary.with_alpha(a)
        });
        let rect = self.base.bounds;
        let secondary_cp = self.glyph.get_untracked().secondary();

        // Secondary (background) layer first, then the primary layer on top.
        if let Some(c) = char::from_u32(secondary_cp) {
            cx.icon(rect, &c.to_string(), secondary, size);
        }
        if let Some(c) = char::from_u32(secondary_cp + PRIMARY_OFFSET) {
            cx.icon(rect, &c.to_string(), primary, size);
        }
    }

    fn tick(&mut self, _dt: f32) -> bool {
        let next = self.glyph.get_untracked();
        if next != self.seen_glyph {
            self.seen_glyph = next;
            self.base.mark_needs_paint();
        }
        false
    }
}

impl Glyph {
    /// Convert a raw Phosphor secondary codepoint into the matching [`Glyph`].
    pub fn from_secondary(secondary_cp: u32) -> Self {
        match secondary_cp {
            0xe24a => Glyph::Folder,
            0xe256 => Glyph::FolderOpen,
            0xe230 => Glyph::File,
            0xe914 => Glyph::FileCode,
            0xe278 => Glyph::GitBranch,
            0xe27a => Glyph::GitCommit,
            0xe280 => Glyph::GitMerge,
            0xe282 => Glyph::GitPullRequest,
            0xeae8 => Glyph::Terminal,
            0xe272 => Glyph::Gear,
            0xe30c => Glyph::Search,
            0xe4f6 => Glyph::Close,
            0xe182 => Glyph::Check,
            0xe13a => Glyph::CaretRight,
            0xe136 => Glyph::CaretDown,
            0xe3d0 => Glyph::Play,
            0xe39e => Glyph::Pause,
            0xe46c => Glyph::Stop,
            0xe4e0 => Glyph::Warning,
            0xe4e2 => Glyph::WarningCircle,
            0xe2ce => Glyph::Info,
            0xe18a => Glyph::Circle,
            0xe2de => Glyph::Lightning,
            0xe2f0 => Glyph::List,
            0xec24 => Glyph::Sidebar,
            0xe208 => Glyph::DotsThreeVertical,
            0xe06c => Glyph::ArrowRight,
            0xe3d4 => Glyph::Plus,
            0xe32a => Glyph::Minus,
            0xe874 => Glyph::SquareSplitVertical,
            0xe4fa => Glyph::XSquare,
            0xe626 => Glyph::FrameCorners,
            0xe0f8 => Glyph::Cards,
            // Unknown Phosphor codepoints fall back to the generic terminal glyph so
            // callers still get a stable icon instead of a missing-glyph square.
            _ => Glyph::Terminal,
        }
    }
}
