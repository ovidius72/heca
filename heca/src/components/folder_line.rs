//! [`FolderLine`] — a working directory, as a line of metadata under a name.

use heca_grid_ui::WidgetModel;
use heca_grid_ui::builders::{IntoComponent, LayoutExt, Parent, TooltipExt};
use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate, create_effect, signal};
use heca_grid_ui::style::{Length, Spacing, WidgetSize};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{Ellipsis, Flex, Glyph, Icon, Label, Visibility};

/// **Where something is** — a folder glyph and a path beside it, cut from the front so the end of
/// the path survives, with the whole path on hover.
///
/// heca's sidebar pane row and the exposé's pane card show a pane's directory with it, and an
/// extension shows a folder the same way by placing the same line — one shape, so two surfaces
/// cannot describe the same folder two different ways.
///
/// ```ignore
/// use heca::FolderLine;
/// use heca_grid_ui::style::{Spacing, WidgetSize};
///
/// let line = FolderLine::new(theme).path("~/projects/heca").size(WidgetSize::Caption).indent(Spacing::Sm);
/// let path = line.path_signal();            // keep it to follow a `cd` without a rebuild
/// let card = Flex::column().child(name).child(line);
/// path.set("~/projects/other".into());      // the line updates in place
/// path.set(String::new());                  // an empty path hides the line, and its gap
/// ```
///
/// **An empty path is no line at all**, not an empty row with a folder in it: the line takes no
/// space while it has nothing to say, and comes back when a path arrives — so place it always,
/// even before the directory is known.
pub struct FolderLine {
    path: Signal<String>,
    size: WidgetSize,
    indent: Spacing,
    color: heca_grid_ui::Color,
}

impl FolderLine {
    /// A line with no path yet, in the theme's foreground colour — readable, because "where is
    /// this" is a primary fact, not a muted one.
    pub fn new(theme: &GuiTheme) -> Self {
        Self {
            path: signal(String::new()),
            size: WidgetSize::Caption,
            indent: Spacing::None,
            color: theme.colors.foreground,
        }
    }

    /// The path to show, as the user should read it (heca shows a pane's directory home-relative).
    /// Default: empty, which hides the line.
    pub fn path(self, path: impl Into<String>) -> Self {
        self.path.set(path.into());
        self
    }

    /// The size variant. Default [`WidgetSize::Caption`]: metadata sits under a name, never beside
    /// it in weight. The glyph and the path both take it, so they cannot disagree.
    pub fn size(mut self, size: WidgetSize) -> Self {
        self.size = size;
        self
    }

    /// **How far the line sits in from the name above it.** Default [`Spacing::None`]. It is
    /// applied inside the line, so the inset disappears with it — a wrapper around the line would
    /// keep its box when the line hides.
    pub fn indent(mut self, indent: Spacing) -> Self {
        self.indent = indent;
        self
    }

    /// **The live path**: set it and the line follows, in place, without rebuilding the tree; set it
    /// empty and the line hides.
    pub fn path_signal(&self) -> Signal<String> {
        self.path
    }

    /// The line as a widget, for a caller that has to name its type.
    pub(crate) fn widget(self) -> Visibility {
        let label = Label::new("")
            .with_text_signal(self.path)
            .color(self.color)
            // **Cut from the FRONT, so the tail survives**: `…/projects/heca` says more than
            // `~/projects/he…`, and a path's last components are the ones that tell two apart.
            .truncate(Ellipsis::Start);
        let label = label.tooltip_signal(self.path).tooltip_quick(true);
        let widget = Visibility::new(
            Flex::row()
                .align("center")
                .size(self.size)
                .gap(Spacing::Sm)
                // Inside the `Visibility`, never around it — see `indent`.
                .padding_x(self.indent)
                // Never wider than what holds it — see `PaneName`. The line is commonly centred,
                // and a centred child is sized by its content unless it says otherwise.
                .max_width(Length::FULL)
                // **This line absorbs the squeeze**, which is how a widget asks for it here
                // (`Style::flex_shrink`: nothing shrinks unless it says so). A path is the longest
                // thing on a card and the first that should give way — without this the line keeps
                // its natural width and takes the card with it.
                .shrink(1.0)
                .child(Icon::new(Glyph::Folder).color(self.color))
                .child(label),
            !self.path.get_untracked().is_empty(),
        );
        let (path, visible) = (self.path, widget.visible_signal());
        create_effect(move |_| {
            let shown = !path.get().is_empty();
            if visible.get_untracked() != shown {
                visible.set(shown);
            }
        });
        widget
    }
}

impl IntoComponent for FolderLine {
    fn into_component(self) -> WidgetModel {
        WidgetModel::new(self.widget())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::Component;

    fn line(path: &str) -> FolderLine {
        FolderLine::new(&GuiTheme::default()).path(path)
    }

    /// **Metadata is smaller than the name above it** — measured, not assumed: the line's text and
    /// glyph resolve to a smaller font than a label beside it that asked for nothing.
    #[test]
    fn the_line_is_smaller_than_the_name_it_sits_under() {
        use heca_grid_ui::LayoutEngine;
        let mut column = Flex::column().child(Label::new("name")).child(line("~/projects/heca"));
        LayoutEngine::new()
            .base_font(15.0)
            .compute(&mut column, heca_grid_ui::Size::new(400.0, 200.0));
        let name = column.base().children[0].base().font;
        let row = &column.base().children[1].base().children[0];
        let glyph = row.base().children[0].base().font;
        let path = row.base().children[1].base().font;
        assert!(glyph < name, "the glyph is smaller: {glyph} vs {name}");
        assert!(path < name, "the path is smaller: {path} vs {name}");
    }

    /// It draws the path it was given, and it is a glyph plus that path — nothing else.
    #[test]
    fn it_shows_the_folder_and_the_path() {
        let widget = line("~/projects/heca").widget();
        assert!(widget.visible_signal().get_untracked());
        let row = &widget.base().children[0];
        assert_eq!(row.base().children.len(), 2, "a glyph and a path");
    }

    #[test]
    fn the_tooltip_keeps_the_full_path_and_follows_directory_changes() {
        let line = line("~/projects/a/long/folder");
        let path = line.path_signal();
        let widget = line.widget();
        let label = &widget.base().children[0].base().children[1];
        let tip = label.base().tooltip.as_ref().expect("the path owns its tooltip");
        assert_eq!(tip.text.get_untracked(), "~/projects/a/long/folder");
        path.set("~/projects/another/folder".into());
        assert_eq!(tip.text.get_untracked(), "~/projects/another/folder");
    }

    /// **Nothing to say, nothing shown** — and a path that arrives later shows the line, in place.
    #[test]
    fn an_empty_path_hides_the_line_until_one_arrives() {
        let line = line("");
        let path = line.path_signal();
        let widget = line.widget();
        let visible = widget.visible_signal();
        assert!(!visible.get_untracked(), "no path yet");
        path.set("~/x".into());
        assert!(visible.get_untracked(), "a directory reported later shows it");
        path.set(String::new());
        assert!(!visible.get_untracked(), "and an empty one hides it again");
    }
}
