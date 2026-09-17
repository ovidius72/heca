//! [`Pane`] — a generic container with configurable frame decoration.
//!
//! Supports three frame modes via [`FrameStyle`]:
//!
//! | Mode | Visual |
//! |------|--------|
//! | [`FrameStyle::None`] | Background fill only — no border, no brackets |
//! | [`FrameStyle::Bordered`] | A clean border from `style.border` |
//! | [`FrameStyle::Bracketed`] | The self-contained accent corner-bracket reticle (no `style.border`) |
//!
//! Default mode is [`FrameStyle::Bordered`] — a fill-only container that
//! becomes a bordered panel once `.border(color, width)` is called.
//!
//! Use `.bracketed()` to opt into the decorative corner-accent look.
//!
//! ## Header / info bar
//!
//! A [`Pane`] carries no built-in title. The app's in-pane **info bar** is a
//! separate [`Tag`](super::Tag) chip composed *inside* the pane top (see the
//! showcase demo and `docs/widgets.md`) — keeping the frame decoration and the
//! header strictly independent.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::color::Color;
use crate::component::{Base, Component, PaintCx, paint_child};
use crate::reactive::SignalGet;
use crate::style::Direction;
use crate::theme::FrameStyle;

/// Rest-glow spread radius (px) — the pane's share of the theme rest halo
/// (`PaintCx::rest_glow` carries color + intensity).
const GLOW_RADIUS: f32 = 12.0;

/// A generic container with configurable frame decoration.
///
/// Default [`FrameStyle`] is [`FrameStyle::Bordered`] — just a fill until
/// `.border(color, width)` is supplied.
pub struct Pane {
    base: Base,
    frame: FrameStyle,
    /// Optional per-widget [`FrameStyle::Bordered`] border width override (logical
    /// px). `None` → the live `theme.colors.border_width` (the default, so the global
    /// BORDER control still drives every pane). `Some(w)` lets one pane carry its
    /// own frame width independent of the theme (e.g. a self-themed sidebar shell).
    border_width: Option<f32>,
}

#[heca_grid_ui_macros::props]
impl Pane {
    /// A new vertical (column) pane with the default [`FrameStyle::Bordered`].
    /// Content is inset by 8.0 px by default — override with `.padding(x)`.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.layout.direction = Direction::Column;
        base.style.layout.padding = (8.0).into();
        Self {
            base,
            frame: FrameStyle::Bordered,
            border_width: None,
        }
    }

    /// A horizontal (row) pane.
    pub fn row() -> Self {
        let mut pane = Self::new();
        pane.base.style.layout.direction = Direction::Row;
        pane
    }

    /// **The kind of border this pane draws** — none, a continuous line, or the corner-bracket
    /// reticle. It is a border *style*, the way CSS `border-style` is, which is why it is not
    /// called `frame`: a caller who reads `.frame(..)` has to go and find out what a frame is.
    ///
    /// It is `border_style` and **not** `border` because
    /// [`StyleExt::border`](crate::builders::StyleExt::border) already takes the colour and the
    /// width. CSS splits the three for the same reason.
    ///
    /// Takes [`FrameStyle`](crate::theme::FrameStyle) — the theme's own vocabulary, which
    /// [`Overlay`](crate::widgets::Overlay) already uses for its edge, and which the app's
    /// `[appearance] border_style` converts into. So a config value goes straight in and no
    /// caller translates:
    ///
    /// The app's `[appearance] border_style` converts into it, so a config value goes in with
    /// nothing to translate:
    ///
    /// ```ignore
    /// Pane::new().border_style(appearance.effective_pane_border_style().into())
    /// ```
    #[heca_grid_ui_macros::prop]
    pub fn border_style(mut self, style: FrameStyle) -> Self {
        self.frame = style;
        self
    }

    /// Shorthand: set frame to [`FrameStyle::Bordered`].
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    pub fn bordered(mut self) -> Self {
        self.frame = FrameStyle::Bordered;
        self
    }

    /// Shorthand: set frame to [`FrameStyle::Bracketed`].
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    pub fn bracketed(mut self) -> Self {
        self.frame = FrameStyle::Bracketed;
        self
    }

    /// Shorthand: set frame to [`FrameStyle::None`].
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    pub fn frameless(mut self) -> Self {
        self.frame = FrameStyle::None;
        self
    }

    /// Override the [`FrameStyle::Bordered`] border width (logical px), independent
    /// of the theme. `None` (the default) keeps the live `theme.colors.border_width` so the
    /// global BORDER control drives the pane; `Some(w)` pins this pane's frame width
    /// (e.g. a sidebar shell that wants its own thickness). No effect on the
    /// `Bracketed`/`None` frames. The border color still comes from `.border(color, _)`
    /// when set, else the theme border color.
    #[heca_grid_ui_macros::host_only("unsupported argument type (impl Into<Option<f32>>)")]
    pub fn border_width(mut self, width: impl Into<Option<f32>>) -> Self {
        self.border_width = width.into();
        self
    }
}

impl Component for Pane {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let b = self.base.bounds;
        let fill = self.base.style.visual.fill;

        // Radius: per-widget override (> 0), else theme fallback.
        let radius = if self.base.style.visual.radius > 0.0 {
            self.base.style.visual.radius
        } else {
            cx.theme().colors.border_radius
        };
        // Surface glow: an explicit `.glow(..)` (StyleExt) wins; otherwise the
        // theme rest glow (`PaintCx::rest_glow`) gives the pane the shared neon
        // identity at rest, scaled by the `glow_size` setting (T011).
        let glow = self.base.style.visual.glow.or_else(|| cx.rest_glow(GLOW_RADIUS));

        match self.frame {
            FrameStyle::None => {
                // Fill only — no border, no brackets.
                if let Some(f) = fill {
                    cx.rect(b, f, None, radius, glow);
                }
            }
            FrameStyle::Bordered => {
                // A clean border whose WIDTH always comes from the live
                // `theme.colors.border_width` (the global border control) — read at paint
                // time so it tracks the control immediately and is gone at
                // `border_width == 0`. An explicit `.border(color, _)` only supplies
                // the COLOR; its width is ignored in favour of the theme (so a
                // build-time width literal can't get "stuck" past a slider change).
                // No explicit color ⇒ the theme border color.
                let (tb_color, tb_width) = {
                    let t = cx.theme();
                    (t.colors.border, t.colors.border_width)
                };
                let color = self.base.style.visual.border.map_or(tb_color, |bd| bd.color);
                // Width: per-widget override (`.border_width(w)`) when set, else the
                // live theme width (so the global BORDER control still drives panes
                // that don't opt out).
                let width = self.border_width.unwrap_or(tb_width);
                let border =
                    (width > 0.0).then_some(crate::scene::Border { color, width });
                if let Some(f) = fill {
                    cx.rect(b, f, border, radius, glow);
                } else if border.is_some() {
                    cx.rect(b, Color::TRANSPARENT, border, radius, glow);
                }
            }
            FrameStyle::Bracketed => {
                // Fill only; the frame is the self-contained corner-bracket reticle
                // drawn by `bracket_frame` (bright rounded corners + a dimmed
                // continuous line) — matching Modal/Toast. Drawing a full
                // `style.border` here too would wash the reticle into a plain
                // border (indistinguishable from `Bordered`). The per-widget
                // `.border_width(w)`/`.radius(r)` overrides size the reticle (else
                // the theme drives it), so a self-themed surface (e.g. the sidebar)
                // controls its bracket thickness + corner rounding too.
                if let Some(f) = fill {
                    cx.rect(b, f, None, radius, glow);
                }
                let width = self.border_width.unwrap_or(cx.theme().colors.border_width);
                cx.bracket_frame_with(b, width, radius);
            }
        }

        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }
    }
}

impl Default for Pane {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExt for Pane {}
impl StyleExt for Pane {}
impl Parent for Pane {}
