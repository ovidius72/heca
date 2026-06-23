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
use crate::component::{paint_child, Base, Component, PaintCx};
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
                let border = (tb_width > 0.0)
                    .then_some(crate::scene::Border { color, width: tb_width });
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
                // border (indistinguishable from `Bordered`).
                if let Some(f) = fill {
                    cx.rect(b, f, None, radius, self.base.style.glow);
                }
                cx.bracket_frame(b);
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
