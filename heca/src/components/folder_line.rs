//! [`FolderLine`] — a working directory, as a line of metadata under a name.

use heca_grid_ui::builders::{LayoutExt, Parent};
use heca_grid_ui::component::{Base, Component, PaintCx};
use heca_grid_ui::reactive::{Signal, SignalGet, signal};
use heca_grid_ui::style::{Length, Spacing, WidgetSize};
use heca_grid_ui::widgets::{Ellipsis, Flex, Glyph, Icon, Label};

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
/// let line = FolderLine::new().path("~/projects/heca").size(WidgetSize::Caption).indent(Spacing::Sm);
/// let path = line.path_signal();            // keep it to follow a `cd` without a rebuild
/// let card = Flex::column().child(name).child(line);
/// path.set("~/projects/other".into());      // the line updates in place
/// path.set(String::new());                  // an empty path hides the line, and its gap
/// ```
///
/// **An empty path is no line at all**, not an empty row with a folder in it: the line takes no
/// space while it has nothing to say, and comes back when a path arrives — so place it always,
/// even before the directory is known.
///
/// It is a widget like any other, so every common builder works on it — `.area(..)` in a `Grid`,
/// `.grow(..)` in a `Flex`, widths. It is transparent to layout: placed in an area or a track, it is
/// the line itself that sits there.
pub struct FolderLine {
    base: Base,
    path: Signal<String>,
    size: WidgetSize,
    indent: Spacing,
    /// Whether the line is shown, as last applied.
    shown: bool,
}

impl FolderLine {
    /// A line with no path yet, in the text colour of where it is placed — the theme's foreground
    /// unless a parent publishes another — read when it is drawn, so it follows a theme reload.
    /// Readable, because "where is this" is a primary fact, not a muted one.
    pub fn new() -> Self {
        let mut base = Base::container();
        base.transparent = true;
        Self {
            base,
            path: signal(String::new()),
            size: WidgetSize::Caption,
            indent: Spacing::None,
            shown: false,
        }
    }

    /// The path to show, as the user should read it (heca shows a pane's directory home-relative)
    /// — fixed, or a value that changes (a signal, or a function reading signals), followed in
    /// place. Default: empty, which hides the line.
    pub fn path(self, path: impl heca_grid_ui::builders::IntoTextValue) -> Self {
        path.into_text_value().show_in(self.path);
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

    /// The glyph and the path, built once, the first time the line is laid out — after every builder
    /// has spoken.
    fn line(&self) -> Flex {
        let label = Label::new("")
            .text(self.path)
            // **Cut from the FRONT, so the tail survives**: `…/projects/heca` says more than
            // `~/projects/he…`, and a path's last components are the ones that tell two apart.
            .truncate(Ellipsis::Start);
        Flex::row()
            .align("center")
            .size(self.size)
            .gap(Spacing::Sm)
            // Inside the line, so the inset disappears with it — see `indent`.
            .padding_x(self.indent)
            // Never wider than what holds it — see `PaneName`. The line is commonly centred,
            // and a centred child is sized by its content unless it says otherwise.
            .max_width(Length::FULL)
            // **This line absorbs the squeeze** (`Style::flex_shrink`: nothing shrinks unless it
            // says so). A path is the longest thing on a card and the first that should give way.
            .shrink(1.0)
            .child(Icon::new(Glyph::Folder))
            .child(label)
    }

    /// **No path, no line — this widget's box as well as the line's**, so a parent spends no gap on
    /// it (the same rule as `Visibility`, which this widget is for its one line).
    fn show(&mut self, shown: bool) {
        self.shown = shown;
        self.base.set_hidden(!shown);
        for child in self.base.children.iter_mut() {
            child.base_mut().set_hidden(!shown);
        }
    }
}

impl Default for FolderLine {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for FolderLine {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Build the line on the first layout pass, transparently, so the line takes the place this
    /// widget was given; then show it or not from the path.
    fn remeasure(&mut self) {
        if self.base.children.is_empty() {
            let line = self.line();
            heca_grid_ui::component::wrap_transparently(&mut self.base, &line);
            self.base
                .children
                .push(heca_grid_ui::WidgetModel::new(line));
        }
        self.show(!self.path.get_untracked().is_empty());
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.shown {
            return;
        }
        for child in &self.base.children {
            heca_grid_ui::component::paint_child(child.as_ref(), cx);
        }
    }

    /// A path that arrives or goes after the line is on screen shows or hides it, in place.
    fn tick(&mut self, dt: f32) -> bool {
        let shown = !self.path.get_untracked().is_empty();
        if shown != self.shown {
            self.show(shown);
            self.base.mark_needs_layout();
        }
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::LayoutEngine;
    use heca_grid_ui::reactive::SignalUpdate;

    fn line(path: &str) -> FolderLine {
        FolderLine::new().path(path)
    }

    fn lay_out(root: &mut dyn Component) {
        LayoutEngine::new()
            .base_font(15.0)
            .compute(root, heca_grid_ui::Size::new(400.0, 200.0));
    }

    /// **Metadata is smaller than the name above it** — measured, not assumed.
    #[test]
    fn the_line_is_smaller_than_the_name_it_sits_under() {
        let mut column = Flex::column()
            .child(Label::new("name"))
            .child(line("~/projects/heca"));
        lay_out(&mut column);
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
        let mut widget = line("~/projects/heca");
        lay_out(&mut widget);
        assert!(widget.shown);
        assert_eq!(
            widget.base().children[0].base().children.len(),
            2,
            "a glyph and a path"
        );
    }

    /// **A cut path offers the whole path on hover, and follows a `cd`**; a path that fits offers
    /// nothing — the label does both by itself, nothing is written here.
    #[test]
    fn a_cut_path_offers_the_whole_path_and_follows_directory_changes() {
        let path_tip = |width: f32| {
            let widget = line("~/projects/a/long/folder");
            let path = widget.path_signal();
            let mut root = Flex::column().width(width).child(widget);
            LayoutEngine::new()
                .base_font(15.0)
                .compute(&mut root, heca_grid_ui::Size::new(f64::from(width), 200.0));
            let label = &root.base().children[0].base().children[0].base().children[1];
            (label.base().tooltip.as_ref().map(|tip| tip.text), path)
        };
        let (tip, path) = path_tip(80.0);
        let tip = tip.expect("cut, the path offers itself whole");
        assert_eq!(tip.get_untracked(), "~/projects/a/long/folder");
        path.set("~/projects/another/folder".into());
        assert_eq!(
            tip.get_untracked(),
            "~/projects/another/folder",
            "and follows the directory"
        );
        assert!(
            path_tip(600.0).0.is_none(),
            "a path that fits has nothing to add"
        );
    }

    /// **Nothing to say, nothing shown, and no gap spent on it** — and a path that arrives later
    /// shows the line, in place.
    #[test]
    fn an_empty_path_hides_the_line_and_its_gap_until_one_arrives() {
        let folder = line("");
        let path = folder.path_signal();
        let mut column = Flex::column()
            .gap(Spacing::Md)
            .child(Label::new("name"))
            .child(folder);
        lay_out(&mut column);
        let alone = column.base().bounds.size.h;
        let mut bare = Flex::column().gap(Spacing::Md).child(Label::new("name"));
        lay_out(&mut bare);
        assert_eq!(
            alone,
            bare.base().bounds.size.h,
            "an empty line takes no room and no gap"
        );

        path.set("~/x".into());
        column.tick(0.0);
        lay_out(&mut column);
        assert!(
            column.base().bounds.size.h > alone,
            "a directory reported later shows it"
        );
    }

    /// **Every common builder works on it** — here a grid area, the reason it became a widget.
    #[test]
    fn it_takes_a_grid_area_like_any_widget() {
        use heca_grid_ui::builders::PlaceExt;
        use heca_grid_ui::widgets::Grid;
        let mut grid = Grid::new()
            .template_column("auto 1fr")
            .template_area(["name folder"])
            .child(Label::new("name").area("name"))
            .child(line("~/x").area("folder"));
        lay_out(&mut grid);
        let name = grid.base().children[0].base().bounds;
        let folder = grid.base().children[1].base().bounds;
        assert!(
            folder.loc.x >= name.loc.x + name.size.w,
            "the line sits in its own area, after the name"
        );
    }

    /// **The line takes its colour when it is drawn**: the same widget, painted under two themes,
    /// writes its path in each theme's text colour — nothing was copied in when it was built.
    #[test]
    fn the_line_follows_the_theme_it_is_painted_with() {
        let mut widget = line("~/projects/heca");
        lay_out(&mut widget);
        let colour_of_path = |foreground: heca_grid_ui::Color| {
            let mut theme = heca_grid_ui::Theme::default();
            theme.colors.foreground = foreground;
            let mut scene = heca_grid_ui::Scene::new();
            heca_grid_ui::paint_child(&widget, &mut PaintCx::new(&mut scene, &theme));
            scene.iter().find_map(|c| match c {
                heca_grid_ui::DrawCommand::Text(t) if t.text.contains("heca") => Some(t.color),
                _ => None,
            })
        };
        let (one, two) = (
            heca_grid_ui::Color::rgb(0x11, 0x22, 0x33),
            heca_grid_ui::Color::rgb(0xdd, 0xcc, 0xbb),
        );
        assert_eq!(colour_of_path(one), Some(one));
        assert_eq!(colour_of_path(two), Some(two), "the new theme, no rebuild");
    }
}
