//! [`PaneName`] — what a pane is called, wherever a surface shows it.

use heca_grid_ui::reactive::Signal;
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::builders::LayoutExt;
use heca_grid_ui::style::Length;
use heca_grid_ui::widgets::{Ellipsis, Label};
use heca_grid_ui::Color;

/// **A pane's name, and it stays inside its box.**
///
/// The sidebar's row and the exposé's card both show it — the same line, written twice, which is
/// why it lives here beside [`FolderLine`](super::FolderLine), the metadata line under it (the
/// second caller is the move).
///
/// **It cuts rather than spills, and that is the whole reason it is a component.** A `Label` keeps
/// its natural width by default and a container too narrow for it simply overflows, so *every*
/// surface showing a name has to remember two things — `truncate`, and the shrinkability that makes
/// the cut reachable. The exposé's cards did not: narrow the window until the cards are slivers and
/// every name painted straight across its neighbours, three of them overlapping into one smear
/// (Antonio, driving, 2026-08-24). Asking here means no surface has to ask, and a surface added
/// tomorrow inherits it.
///
/// Making the cut `Label`'s own default was tried and reverted the same day: a label that may
/// shrink to nothing *does*, and a `Card`'s title vanished entirely. Cutting belongs to the kinds
/// of text that are shown in boxes they do not control — a name is one — not to every label.
pub(crate) struct PaneName<'a> {
    /// The name to show.
    pub(crate) text: &'a str,
    /// Which colour this surface gives it — the active row's accent, a card's foreground.
    pub(crate) color: Color,
    /// Names are shown bold everywhere they are shown; a caller that wants otherwise says so.
    pub(crate) bold: bool,
    /// Size relative to the surrounding text. `1.0` is the surface's own size.
    pub(crate) font_scale: f32,
    #[allow(dead_code, reason = "held for symmetry with FolderLine; colours come from the caller")]
    pub(crate) theme: &'a GuiTheme,
}

/// What [`PaneName::build`] hands back: the label, plus the signal a host binds when the name can
/// change without the tree being rebuilt — which it can, every time a shell's title changes.
pub(crate) struct BuiltPaneName {
    pub(crate) widget: Label,
    pub(crate) text: Signal<String>,
}

impl PaneName<'_> {
    pub(crate) fn build(self) -> BuiltPaneName {
        // **Cut at the end**: a name's beginning is what tells two panes apart (`server`/`serverb`),
        // so the tail is the half to lose. A path would want the other end — that is `FolderLine`'s
        // decision to make, not this one's.
        let mut widget = Label::new(self.text)
            .color(self.color)
            .bold(self.bold)
            .truncate(Ellipsis::End)
            // **Never wider than what holds it.** A *centred* child is sized by its content and is
            // free to overflow its container — which is how the exposé's cards, whose column
            // centres this, kept drawing a full name out of a card far too narrow for it. CSS's
            // `max-width: 100%`, said once here rather than at each surface.
            .max_width(Length::Pct(1.0));
        if (self.font_scale - 1.0).abs() > f32::EPSILON {
            widget = widget.font_scale(self.font_scale);
        }
        let text = widget.text_signal();
        BuiltPaneName { widget, text }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::builders::{LayoutExt, Parent};
    use heca_grid_ui::widgets::Flex;
    use heca_grid_ui::{Component, LayoutEngine, Scene, PaintCx, DrawCommand};
    use heca_core::layout::Size;

    fn built(text: &str) -> BuiltPaneName {
        PaneName {
            text,
            color: GuiTheme::default().colors.foreground,
            bold: true,
            font_scale: 1.0,
            theme: &GuiTheme::default(),
        }
        .build()
    }

    /// It shows the name it was given, and the name stays bindable.
    #[test]
    fn it_shows_the_name() {
        let name = built("editor");
        assert_eq!(
            heca_grid_ui::reactive::SignalGet::get_untracked(&name.text),
            "editor",
        );
    }

    /// **It never leaves its box** — the test the components rules ask of anything sized by its
    /// container: lay it out in a box and assert it does not exceed it, at several sizes.
    ///
    /// This is the one that was missing. A plain `Label` passes the wide cases and fails the narrow
    /// ones by painting outside itself, which no assertion about *what* was drawn ever notices.
    #[test]
    fn it_never_paints_outside_the_box_it_is_given() {
        for box_w in [400.0, 120.0, 60.0, 24.0, 8.0] {
            // **Centred**, which is how the exposé's card holds it: `align(Center)` sizes a child by
            // its content instead of the container, so a label that only *may* shrink never is
            // asked to — the cut is unreachable and the name paints straight out of the card.
            let mut root = Flex::column()
                .align(heca_grid_ui::style::Align::Center)
                .child(built("a-very-long-pane-name").widget);
            LayoutEngine::new().compute(&mut root, Size::new(box_w, 40.0));

            let mut scene = Scene::new();
            let theme = GuiTheme::default();
            root.paint(&mut PaintCx::new(&mut scene, &theme));

            for cmd in scene.iter() {
                if let DrawCommand::Text(t) = cmd {
                    assert!(
                        t.rect.loc.x + t.rect.size.w <= box_w + 0.5,
                        "at {box_w}px the name drew {:?}, out to {}",
                        t.text,
                        t.rect.loc.x + t.rect.size.w,
                    );
                }
            }
        }
    }
}
