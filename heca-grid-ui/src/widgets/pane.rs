//! [`Pane`] — a generic container with configurable frame decoration.
//!
//! Supports three frame modes via [`PaneFrame`]:
//!
//! | Mode | Visual |
//! |------|--------|
//! | [`PaneFrame::None`] | Background fill only — no border, no brackets |
//! | [`PaneFrame::Bordered`] | A clean border from `style.border` |
//! | [`PaneFrame::Bracketed`] | The self-contained accent corner-bracket reticle (no `style.border`) |
//!
//! Default mode is [`PaneFrame::Bordered`] — a fill-only container that
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

/// Frame decoration mode for a [`Pane`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneFrame {
    /// Fill only — no border, no brackets. Use when you just want the
    /// background without any frame decoration.
    None,
    /// A clean border from `style.border` (set via `.border(color, width)`).
    /// No corner brackets.
    Bordered,
    /// The self-contained accent corner-bracket reticle drawn by
    /// [`PaintCx::bracket_frame`](crate::PaintCx::bracket_frame): bright rounded
    /// corners with short arms over a dimmed continuous line. Does **not** also
    /// draw `style.border` — that would wash the reticle into a plain border.
    Bracketed,
}

/// A generic container with configurable frame decoration.
///
/// Default [`PaneFrame`] is [`PaneFrame::Bordered`] — just a fill until
/// `.border(color, width)` is supplied.
pub struct Pane {
    base: Base,
    frame: PaneFrame,
    /// Optional per-widget [`PaneFrame::Bordered`] border width override (logical
    /// px). `None` → the live `theme.border_width` (the default, so the global
    /// BORDER control still drives every pane). `Some(w)` lets one pane carry its
    /// own frame width independent of the theme (e.g. a self-themed sidebar shell).
    border_width: Option<f32>,
}

impl Pane {
    /// A new vertical (column) pane with the default [`PaneFrame::Bordered`].
    /// Content is inset by 8.0 px by default — override with `.padding(x)`.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.direction = Direction::Column;
        base.style.padding = 8.0;
        Self {
            base,
            frame: PaneFrame::Bordered,
            border_width: None,
        }
    }

    /// A horizontal (row) pane.
    pub fn row() -> Self {
        let mut pane = Self::new();
        pane.base.style.direction = Direction::Row;
        pane
    }

    /// Choose the frame decoration mode.
    pub fn frame(mut self, f: PaneFrame) -> Self {
        self.frame = f;
        self
    }

    /// Shorthand: set frame to [`PaneFrame::Bordered`].
    pub fn bordered(mut self) -> Self {
        self.frame = PaneFrame::Bordered;
        self
    }

    /// Shorthand: set frame to [`PaneFrame::Bracketed`].
    pub fn bracketed(mut self) -> Self {
        self.frame = PaneFrame::Bracketed;
        self
    }

    /// Shorthand: set frame to [`PaneFrame::None`].
    pub fn frameless(mut self) -> Self {
        self.frame = PaneFrame::None;
        self
    }

    /// Override the [`PaneFrame::Bordered`] border width (logical px), independent
    /// of the theme. `None` (the default) keeps the live `theme.border_width` so the
    /// global BORDER control drives the pane; `Some(w)` pins this pane's frame width
    /// (e.g. a sidebar shell that wants its own thickness). No effect on the
    /// `Bracketed`/`None` frames. The border color still comes from `.border(color, _)`
    /// when set, else the theme border color.
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
        let fill = self.base.style.fill;

        // Radius: per-widget override (> 0), else theme fallback.
        let radius = if self.base.style.radius > 0.0 {
            self.base.style.radius
        } else {
            cx.theme().radius
        };

        match self.frame {
            PaneFrame::None => {
                // Fill only — no border, no brackets.
                if let Some(f) = fill {
                    cx.rect(b, f, None, radius, self.base.style.glow);
                }
            }
            PaneFrame::Bordered => {
                // A clean border whose WIDTH always comes from the live
                // `theme.border_width` (the global border control) — read at paint
                // time so it tracks the control immediately and is gone at
                // `border_width == 0`. An explicit `.border(color, _)` only supplies
                // the COLOR; its width is ignored in favour of the theme (so a
                // build-time width literal can't get "stuck" past a slider change).
                // No explicit color ⇒ the theme border color.
                let (tb_color, tb_width) = {
                    let t = cx.theme();
                    (t.border, t.border_width)
                };
                let color = self.base.style.border.map_or(tb_color, |bd| bd.color);
                // Width: per-widget override (`.border_width(w)`) when set, else the
                // live theme width (so the global BORDER control still drives panes
                // that don't opt out).
                let width = self.border_width.unwrap_or(tb_width);
                let border =
                    (width > 0.0).then_some(crate::scene::Border { color, width });
                if let Some(f) = fill {
                    cx.rect(b, f, border, radius, self.base.style.glow);
                } else if border.is_some() {
                    cx.rect(b, Color::TRANSPARENT, border, radius, None);
                }
            }
            PaneFrame::Bracketed => {
                // Fill only; the frame is the self-contained corner-bracket reticle
                // drawn by `bracket_frame` (bright rounded corners + a dimmed
                // continuous line) — matching Modal/Toast. Drawing a full
                // `style.border` here too would wash the reticle into a plain
                // border (indistinguishable from `Bordered`). The per-widget
                // `.border_width(w)`/`.radius(r)` overrides size the reticle (else
                // the theme drives it), so a self-themed surface (e.g. the sidebar)
                // controls its bracket thickness + corner rounding too.
                if let Some(f) = fill {
                    cx.rect(b, f, None, radius, self.base.style.glow);
                }
                let width = self.border_width.unwrap_or(cx.theme().border_width);
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
