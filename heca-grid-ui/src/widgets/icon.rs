//! [`Icon`] — a single glyph from the embedded **Phosphor Duotone** icon font.
//!
//! Duotone glyphs render as **two stacked layers**: a *secondary* background
//! layer (Phosphor's `:before` codepoint, drawn dimmed) and a *primary*
//! foreground layer (the next codepoint, full strength) overlaid at the same
//! spot. The widget emits both runs via [`PaintCx::icon`](crate::component::PaintCx::icon),
//! shaped with the icon family.
//!
//! Colors are **theme/config-driven**, never baked in. With no explicit
//! [`color`](Icon::color) the primary layer takes the
//! [content color](crate::component::PaintCx::with_content_color) inherited from an enclosing
//! control — so an icon composed inside a [`Button`](super::Button) tints with that button's
//! hover/disabled state — falling back to the theme foreground when there is none. The secondary
//! layer follows the primary at the theme's
//! [`icon_secondary_alpha`](crate::theme::Theme::icon_secondary_alpha) unless set via
//! [`secondary_color`](Icon::secondary_color). Only the font *bytes* are static (embedded); the
//! look is configurable.

use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::{Signal, SignalGet, signal};
use crate::scene::Glow;
use crate::style::Length;

/// Offset from a duotone glyph's secondary (`:before`) codepoint to its primary
/// (`:after`) layer — Phosphor pairs them consecutively.
const PRIMARY_OFFSET: u32 = 1;

/// A curated set of Phosphor icons, by name. Each value is the **secondary**
/// (`:before`) codepoint; the primary layer is `secondary + 1`. Iterate
/// [`Glyph::ALL`] for the whole set (e.g. to render an icon gallery).
#[derive(Debug, Clone, Copy, PartialEq, Eq, heca_grid_ui_macros::PropName)]
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
    /// Left chevron — "collapse this row" / "out to the parent", the mirror of
    /// [`CaretRight`](Glyph::CaretRight). Inferred codepoint (`0xe138`): Phosphor lays the caret
    /// family out alphabetically two codepoints apart (`caret-down` `0xe136`, `caret-left`,
    /// `caret-right` `0xe13a`, `caret-up` `0xe13c`), so this is the slot between down and right —
    /// confirm visually in the showcase glyph grid, as [`CaretUp`](Glyph::CaretUp) also asks.
    CaretLeft,
    CaretDown,
    /// Up chevron — the polished form of the macOS Control symbol (`⌃`), used to render a
    /// button's `Ctrl` accelerator. Inferred codepoint (`0xe13c`) — confirm visually in the
    /// showcase glyph grid.
    CaretUp,
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
    // Action-menu / pane-header glyphs (Phosphor names differ from these enum names;
    // see `secondary()` for the mapping). Added for the context-menu + button action icons.
    Pencil,
    NotePencil,
    Backspace,
    Trash,
    XCircle,
    PlusCircle,
    FolderSimpleMinus,
    FolderSimplePlus,
    StackPlus,
    StackMinus,
    ColumnsPlusLeft,
    ColumnsPlusRight,
    SquareHalf,
    SquareSplitHorizontal,
    SquareHalfBottom,
}

#[heca_grid_ui_macros::props]
impl Glyph {
    /// Every curated glyph, in enum order — the single enumerable source of the icon
    /// set. Rust can't iterate enum variants without a macro/dependency, so this list
    /// is the one place they're collected (a unit test asserts it stays complete). Use
    /// it to render a full gallery or document the set; `secondary()` gives each one's
    /// codepoint.
    pub const ALL: &'static [Glyph] = &[
        Glyph::Folder, Glyph::FolderOpen, Glyph::File, Glyph::FileCode,
        Glyph::GitBranch, Glyph::GitCommit, Glyph::GitMerge, Glyph::GitPullRequest,
        Glyph::Terminal, Glyph::Gear, Glyph::Search, Glyph::Close,
        Glyph::Check, Glyph::CaretRight, Glyph::CaretLeft, Glyph::CaretDown, Glyph::CaretUp,
        Glyph::Play,
        Glyph::Pause, Glyph::Stop, Glyph::Warning, Glyph::WarningCircle,
        Glyph::Info, Glyph::Circle, Glyph::Lightning, Glyph::List,
        Glyph::Sidebar, Glyph::DotsThreeVertical, Glyph::ArrowRight, Glyph::ArrowLineLeft,
        Glyph::ArrowLineRight, Glyph::Plus, Glyph::Minus, Glyph::SquareSplitVertical,
        Glyph::XSquare, Glyph::FrameCorners, Glyph::Cards, Glyph::Pencil,
        Glyph::NotePencil, Glyph::Backspace, Glyph::Trash, Glyph::XCircle,
        Glyph::PlusCircle, Glyph::FolderSimpleMinus, Glyph::FolderSimplePlus,
        Glyph::StackPlus, Glyph::StackMinus, Glyph::ColumnsPlusLeft, Glyph::ColumnsPlusRight,
        Glyph::SquareHalf, Glyph::SquareSplitHorizontal, Glyph::SquareHalfBottom,
    ];

    /// The secondary-layer (`:before`) codepoint; the primary layer is this `+ 1`.
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
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
            Glyph::CaretLeft => 0xe138,
            Glyph::CaretDown => 0xe136,
            Glyph::CaretUp => 0xe13c,
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
            // Phosphor Duotone v2.1 `:before` codepoints (verified against the embedded font).
            Glyph::Pencil => 0xe3ae,                 // pencil
            Glyph::NotePencil => 0xe34c,             // note-pencil (the "edit" icon)
            Glyph::Backspace => 0xe0ae,              // backspace
            Glyph::Trash => 0xe4a6,                  // trash
            Glyph::XCircle => 0xe4f8,                // x-circle
            Glyph::PlusCircle => 0xe3d6,             // plus-circle
            Glyph::FolderSimpleMinus => 0xe25c,      // folder-simple-minus
            Glyph::FolderSimplePlus => 0xe25e,       // folder-simple-plus
            Glyph::StackPlus => 0xedf6,              // stack-plus
            Glyph::StackMinus => 0xedf4,             // stack-minus
            Glyph::ColumnsPlusLeft => 0xe544,        // columns-plus-left
            Glyph::ColumnsPlusRight => 0xe542,       // columns-plus-right
            Glyph::SquareHalf => 0xe462,             // square-half
            Glyph::SquareSplitHorizontal => 0xe870,  // square-split-horizontal
            Glyph::SquareHalfBottom => 0xeb16,       // square-half-bottom
        }
    }

    /// The **primary-layer** character (the full-strength glyph), for callers that
    /// draw a single-layer icon manually via [`PaintCx::icon`](crate::component::PaintCx::icon)
    /// (e.g. command-palette rows). Duotone rendering uses both layers; this is the
    /// foreground one.
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
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
    /// Opt-in resting halo (see [`Icon::glow`]).
    glow: bool,
}

/// Halo falloff as a fraction of the glyph's own size.
///
/// Derived rather than fixed, because an icon is not a fixed size: a 34px glyph and
/// a 16px glyph need proportionally different reach, and a constant would make one
/// of them wrong.
///
/// **Deliberately wider than a surface's rest glow, not equal to it.** Perceived glow
/// is total emitted light, which scales with the emitting *perimeter* — a glyph's is
/// roughly a quarter of a button's, so matching the radius (which an earlier version
/// did, at 0.35 ≈ the button's 12px) left the glyph visibly dimmer and tighter beside
/// a lit control. The extra reach buys back the area a small emitter cannot.
const HALO_RADIUS_FRAC: f32 = 0.55;

#[heca_grid_ui_macros::props]
impl Icon {
    /// A new icon for a named [`Glyph`].
    pub fn new(glyph: Glyph) -> Self {
        let mut icon = Self {
            base: Base::new(),
            glyph: signal(glyph),
            seen_glyph: glyph,
            size: None,
            color: None,
            glow: false,
            secondary: None,
        };
        icon.remeasure();
        icon
    }

    /// Handle to the icon's glyph signal so hosts can update it live.
    pub fn glyph_signal(&self) -> Signal<Glyph> {
        self.glyph
    }

    /// Explicit glyph size in logical px (overrides the inherited font size).
    #[heca_grid_ui_macros::prop]
    pub fn size(mut self, px: f32) -> Self {
        self.size = Some(px);
        self.remeasure();
        self
    }

    /// Primary-layer color (default: theme foreground). The secondary layer
    /// follows it (dimmed) unless set via [`secondary_color`](Icon::secondary_color).
    #[heca_grid_ui_macros::prop]
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    /// Explicit secondary-layer color (default: the primary color at the theme's
    /// [`icon_secondary_alpha`](crate::theme::Theme::icon_secondary_alpha)).
    #[heca_grid_ui_macros::prop]
    pub fn secondary_color(mut self, c: Color) -> Self {
        self.secondary = Some(c);
        self
    }

    /// Give the glyph the theme's resting halo (default `false` — flat).
    ///
    /// This is the glyph counterpart of the rest glow every bordered *surface*
    /// carries: it reads `interaction.control_rest_glow` and scales with `glow_size`
    /// exactly as a surface's does, so `glow_size = none` removes it along with every
    /// other halo in the UI.
    ///
    /// It is **opt-in rather than automatic**, because most icons in the app sit
    /// inside a control that is already glowing — a Button's leading icon, a pane
    /// header action — and haloing those too would double the light. Turn it on for a
    /// glyph that stands alone on the background and should still read as lit.
    ///
    /// A glyph inside a control that publishes a content glow (see
    /// [`PaintCx::with_content_glow`](crate::component::PaintCx::with_content_glow))
    /// inherits one without this flag; the flag is the standalone case.
    #[heca_grid_ui_macros::prop]
    pub fn glow(mut self, glow: bool) -> Self {
        self.glow = glow;
        self
    }

    fn glyph_size(&self) -> f32 {
        match self.size {
            // An explicit px still tracks the size variant (Small/Normal/Large) by
            // its font scale — otherwise icon-only buttons wouldn't resize. The
            // font-driven path already includes the variant via `base.font`.
            Some(px) => px * self.base.style.layout.size.font_scale(),
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
        self.base.style.layout.width = Length::Px(s);
        self.base.style.layout.height = Length::Px(s);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let size = self.glyph_size();
        // Own color → the enclosing control's inherited content color → the theme foreground, so a
        // glyph composed inside a control (a Button's leading icon, an accelerator) tracks that
        // control's hover/disabled state. The secondary layer keeps following the primary.
        let primary = self
            .color
            .or_else(|| cx.content_color())
            .unwrap_or_else(|| cx.theme().colors.foreground);
        let secondary = self.secondary.unwrap_or_else(|| {
            let a = (cx.theme().colors.icon_secondary_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
            primary.with_alpha(a)
        });
        let rect = self.base.bounds;
        let secondary_cp = self.glyph.get_untracked().secondary();

        // Own opt-in → the enclosing control's inherited content glow → flat. Same
        // resolution order as the colour above, for the same reason: a glyph composed
        // inside a control (a `RailCell`'s bare-icon rest state) cannot be reached and
        // styled by that control, so the control publishes and the glyph pulls.
        //
        // The reach is always ours, even on the inherited path: a parent knows it
        // wants the glyph lit, but only the glyph knows how big it is.
        let halo_radius = size * HALO_RADIUS_FRAC;
        let glow = if self.glow {
            cx.rest_glow(halo_radius)
        } else {
            cx.content_glow().map(|g| Glow {
                radius: halo_radius,
                ..g
            })
        };

        // Secondary (background) layer first, then the primary layer on top. Only the
        // primary carries the halo: haloing both would double the light on every
        // two-layer glyph and cost a second set of taps for no visible gain.
        if let Some(c) = char::from_u32(secondary_cp) {
            cx.icon(rect, &c.to_string(), secondary, size);
        }
        if let Some(c) = char::from_u32(secondary_cp + PRIMARY_OFFSET) {
            cx.icon_glowing(rect, &c.to_string(), primary, size, glow);
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

#[cfg(test)]
mod tests {
    use super::Glyph;

    /// `Glyph::ALL` must list **every** variant exactly once — it's the hand-kept list
    /// the compiler can't verify. Guard it by codepoint: each glyph's `secondary()` is
    /// unique, so a missing/duplicated entry shows up as a count or uniqueness failure.
    #[test]
    fn all_glyphs_have_unique_codepoints() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for &g in Glyph::ALL {
            let cp = g.secondary();
            assert!(
                seen.insert(cp),
                "duplicate secondary codepoint {cp:#06x} for {g:?}"
            );
        }
        assert_eq!(seen.len(), Glyph::ALL.len());
    }

    mod glow {
        use super::super::Icon;
        use crate::component::{Component, PaintCx};
        use crate::scene::{DrawCommand, Scene};
        use crate::theme::{GlowLevel, Theme};

        /// Paint one icon and report the halo on its **primary** glyph run (the last
        /// text command it emits).
        fn primary_glow(icon: Icon, glow_size: GlowLevel) -> Option<crate::scene::Glow> {
            let mut theme = Theme::default();
            theme.colors.glow_size = glow_size;
            let mut scene = Scene::new();
            {
                let mut cx = PaintCx::new(&mut scene, &theme);
                icon.paint(&mut cx);
            }
            scene
                .iter()
                .filter_map(|cmd| match cmd {
                    DrawCommand::Text(t) => Some(t.glow),
                    _ => None,
                })
                .last()
                .expect("an icon paints at least one glyph run")
        }

        /// An icon is flat unless it asks for a halo — most icons sit inside a control
        /// that is already glowing, so haloing every one would double the light.
        #[test]
        fn an_icon_is_flat_by_default() {
            let glow = primary_glow(Icon::new(super::Glyph::Search), GlowLevel::Medium);
            assert!(glow.is_none(), "unglowing icon emitted {glow:?}");
        }

        /// Opting in puts the halo on the glyph run for the renderer to realize.
        #[test]
        fn opting_in_carries_a_halo_on_the_glyph_run() {
            let glow = primary_glow(Icon::new(super::Glyph::Search).glow(true), GlowLevel::Medium);
            assert!(glow.is_some(), "glow(true) emitted no halo");
        }

        /// **The chokepoint.** A glyph halo is not a second glow system: it goes
        /// through the same `scaled_glow` as every surface, so the `glow_size` setting
        /// removes it along with the rest.
        #[test]
        fn glow_size_none_removes_the_glyph_halo() {
            let glow = primary_glow(Icon::new(super::Glyph::Search).glow(true), GlowLevel::None);
            assert!(glow.is_none(), "glow_size=none still emitted {glow:?}");
        }

        /// …and the level scales it, rather than the glyph baking a fixed intensity.
        #[test]
        fn glow_size_scales_the_glyph_halo() {
            let thin = primary_glow(Icon::new(super::Glyph::Search).glow(true), GlowLevel::Thin)
                .expect("thin halo");
            let large = primary_glow(Icon::new(super::Glyph::Search).glow(true), GlowLevel::Large)
                .expect("large halo");
            assert!(
                large.radius > thin.radius,
                "large halo radius {} should exceed thin's {}",
                large.radius,
                thin.radius
            );
        }
    }
}
