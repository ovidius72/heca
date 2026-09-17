//! [`FolderLine`] — a pane's working directory, as a line of metadata under its name.

use heca_grid_ui::builders::{LayoutExt, Parent};
use heca_grid_ui::reactive::Signal;
use heca_grid_ui::style::{Length, Spacing};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{Ellipsis, Flex, Glyph, Icon, Label, Visibility};

/// Gap between the folder glyph and the path, and the glyph's size — the line's own proportions,
/// not the caller's.
const GAP: f32 = 6.0;
const ICON: f32 = 12.0;

/// **Where a pane is** — a folder glyph and the home-relative path beside it.
///
/// The sidebar's pane row and the exposé's pane card both show it, which is why it lives here
/// rather than inside either of them: one shape, so the map and the dock cannot end up describing
/// the same pane two different ways (`components` — the second caller is the move).
///
/// The **text and the visibility come back as signals**, because a cwd changes without the tree
/// being rebuilt: a `cd` in the shell updates the path in place, and `[settings] pane_show_cwd`
/// turns the line on and off the same way. A caller that has nothing to bind can ignore them.
pub(crate) struct FolderLine<'a> {
    /// The path to show, already home-relative (`crate::chrome::home_relative_path`). `None` draws
    /// nothing — a pane whose backend never reported a directory.
    pub(crate) path: Option<&'a str>,
    /// Whether the line is shown at all — `[settings] pane_show_cwd` and the caller's own reasons.
    pub(crate) show: bool,
    /// Size relative to the surrounding text: metadata sits under the name, never beside it in
    /// weight.
    pub(crate) font_scale: f32,
    /// **How far the line sits in from the name above it** — a token, resolved from the font, and
    /// applied *inside* this line's own visibility so the inset disappears with the line.
    ///
    /// It is a property because it is the surrounding card's taste, not the line's: the dock's
    /// card steps its metadata in under the name, the exposé's centres it and steps nothing.
    /// It lives here rather than in a wrapper the caller adds because a wrapper does not vanish
    /// when the line hides — a row holding a hidden line is still a row, and the column above it
    /// still spends a gap on it.
    pub(crate) indent: Spacing,
    pub(crate) theme: &'a GuiTheme,
}

/// What [`FolderLine::build`] hands back: the widget, plus the two signals a host binds when the
/// path or its visibility can change under it.
pub(crate) struct BuiltFolderLine {
    pub(crate) widget: Visibility,
    pub(crate) text: Signal<String>,
    pub(crate) visible: Signal<bool>,
}

impl FolderLine<'_> {
    pub(crate) fn build(self) -> BuiltFolderLine {
        // Readable, matching the sibling git-branch line: `muted` is too dim for a primary piece of
        // information about a pane, which is what "where am I" is.
        // **Cut from the FRONT, so the tail survives**: `~/projects/heca` says less than
        // `…/projects/heca`, and a path's last components are the ones that tell two panes apart.
        // Without it the line keeps its natural width and takes its whole card with it — a card
        // 80px wide drew a 120px path, pushing the name out with it (Antonio, driving, 2026-08-24).
        // Here rather than at each surface, for the same reason the line itself is here.
        let label = Label::new(self.path.unwrap_or_default())
            .color(self.theme.colors.foreground)
            .font_scale(self.font_scale)
            .truncate(Ellipsis::Start);
        let text = label.text_signal();
        let widget = Visibility::new(
            Flex::row()
                .align("center")
                .gap(GAP)
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
                .child(
                    Icon::new(Glyph::Folder)
                        .size(ICON)
                        .color(self.theme.colors.foreground),
                )
                .child(label),
            self.show && self.path.is_some(),
        );
        let visible = widget.visible_signal();
        BuiltFolderLine {
            widget,
            text,
            visible,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::reactive::SignalGet;
    use heca_grid_ui::Component;

    fn built(path: Option<&str>, show: bool) -> BuiltFolderLine {
        FolderLine {
            path,
            show,
            font_scale: 0.85,
            indent: Spacing::None,
            theme: &GuiTheme::default(),
        }
        .build()
    }

    /// It draws the path it was given, and it is a glyph plus that path — nothing else.
    #[test]
    fn it_shows_the_folder_and_the_path() {
        let line = built(Some("~/projects/heca"), true);
        assert!(line.visible.get_untracked());
        assert_eq!(line.text.get_untracked(), "~/projects/heca");
        assert_eq!(
            line.widget.base().children[0].base().children.len(),
            2,
            "a glyph and a path",
        );
    }

    /// **Nothing to say, nothing shown** — a pane with no directory, and the setting turned off,
    /// are the same absence rather than an empty row with a folder icon in it.
    #[test]
    fn it_is_hidden_when_there_is_no_path_or_the_setting_is_off() {
        assert!(!built(None, true).visible.get_untracked(), "no cwd reported");
        assert!(!built(Some("~/x"), false).visible.get_untracked(), "setting off");
    }
}
