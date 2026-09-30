//! **The lines under a pane's name in the workspaces dock** — the working directory, the git state,
//! and the text line another crate adds — each a self-contained widget that follows its pane.
//!
//! A line is built once, subscribes to the pane's facts, and rewrites its own words and visibility
//! in place (change-guarded), which is why a `cd` or a checkout moves it without the sidebar being
//! rebuilt. They are registered through the same [`PaneLineDef`] another crate's line uses (see
//! `chrome/pane_items/lines.rs`), so `cwd` and `git` are the first users of the registry rather than
//! a special case in the row.

use std::rc::Rc;

use heca_grid_ui::Component;
use heca_grid_ui::builders::{LayoutExt, Parent};
use heca_grid_ui::reactive::{create_effect, signal};
use heca_grid_ui::widgets::{Flex, Glyph, Icon, Label, Tooltip, TooltipSide, Visibility};

use super::pane_row::{META_INDENT, set_if_changed};
use crate::chrome::CARD_META_FONT_SCALE;
use crate::chrome::pane_items::{BuildLine, LineCx, PaneLineDef, ProduceLine};

/// heca's own two lines, in the order the row has always shown them.
pub(crate) fn builtin_row_lines() -> Vec<PaneLineDef> {
    let own = |name: &str, build: fn(&LineCx<'_>) -> Box<dyn Component>| PaneLineDef {
        name: name.to_string(),
        build: Rc::new(build),
        from_extension: false,
    };
    vec![own("cwd", cwd_line), own("git", git_line)]
}

/// **Where the pane is**: a folder glyph and the home-relative path. The one shape the exposé's card
/// shares (`components::FolderLine`). Shown while the pane has a directory, read inside this line's own
/// subscription so a `cd` moves it. Whether the line exists at all is `[appearance.sidebar]
/// pane_lines`, and nothing else.
fn cwd_line(cx: &LineCx<'_>) -> Box<dyn Component> {
    let line = crate::components::FolderLine {
        path: None,
        show: false,
        font_scale: CARD_META_FONT_SCALE,
        indent: META_INDENT,
        theme: cx.theme,
    }
    .build();
    let (text, visible) = (line.text, line.visible);
    let facts = cx.facts.clone();
    create_effect(move |_| {
        let cwd = facts().cwd;
        // Cut to a home-relative form here, at the binding, so there is one spelling of "where this
        // pane is".
        set_if_changed(
            text,
            cwd.as_deref()
                .map(crate::chrome::home_relative_path)
                .unwrap_or_default(),
        );
        set_if_changed(visible, cwd.is_some());
    });
    Box::new(line.widget)
}

/// **The git state**: branch (cut for a narrow sidebar, the full name on hover) and the counts of
/// added, modified and deleted files. Absent outside a repository; a count of nothing is absent.
fn git_line(cx: &LineCx<'_>) -> Box<dyn Component> {
    let theme = cx.theme;
    let branch_label = Label::new("")
        .color(theme.colors.foreground)
        .font_scale(0.8);
    let branch_shown = branch_label.text_signal();
    let branch_full = signal(String::new());

    // One count: a glyph and its words, hidden while there is nothing to say.
    let count = |glyph: Glyph, color| {
        let label = Label::new("").color(color).font_scale(0.8);
        let text = label.text_signal();
        let segment = Visibility::new(
            Flex::row()
                .align("center")
                .gap(4.0)
                .child(Icon::new(glyph).size(12.0).color(color))
                .child(label),
            false,
        );
        let visible = segment.visible_signal();
        (segment, text, visible)
    };
    let (added, added_text, added_visible) = count(Glyph::Plus, theme.colors.success);
    let (modified, modified_text, modified_visible) = count(Glyph::Warning, theme.colors.warning);
    let (deleted, deleted_text, deleted_visible) = count(Glyph::Minus, theme.colors.danger);

    let row = Visibility::new(
        Flex::row()
            .align("center")
            .gap(6.0)
            .padding_x(META_INDENT)
            .child(
                Icon::new(Glyph::GitBranch)
                    .size(12.0)
                    .color(theme.colors.warning),
            )
            .child(
                Tooltip::new_signal(branch_label, branch_full)
                    .side(TooltipSide::Bottom)
                    .delay(0.25),
            )
            .child(added)
            .child(modified)
            .child(deleted),
        false,
    );
    let visible = row.visible_signal();

    let facts = cx.facts.clone();
    create_effect(move |_| {
        let git = facts().git;
        set_if_changed(visible, git.is_some());
        let branch = git
            .as_ref()
            .map(crate::chrome::pane_header::git_branch)
            .unwrap_or_default();
        set_if_changed(
            branch_shown,
            crate::chrome::truncate_sidebar_git_branch(&branch),
        );
        set_if_changed(branch_full, branch);
        for (visible, text, value) in [
            (
                added_visible,
                added_text,
                git.as_ref().and_then(crate::chrome::pane_header::git_added),
            ),
            (
                modified_visible,
                modified_text,
                git.as_ref()
                    .and_then(crate::chrome::pane_header::git_modified),
            ),
            (
                deleted_visible,
                deleted_text,
                git.as_ref()
                    .and_then(crate::chrome::pane_header::git_deleted),
            ),
        ] {
            set_if_changed(visible, value.is_some());
            set_if_changed(text, value.unwrap_or_default());
        }
    });
    Box::new(row)
}

/// **A line another crate added**: an icon and some text, shown while `produce` has something to
/// say for the pane. It runs again when the pane's facts change — that is what the subscription is.
pub(crate) fn text_line(produce: Rc<ProduceLine>) -> Rc<BuildLine> {
    Rc::new(move |cx: &LineCx<'_>| {
        let label = Label::new("")
            .color(cx.theme.colors.foreground)
            .font_scale(CARD_META_FONT_SCALE);
        let text = label.text_signal();
        let icon = Icon::new(Glyph::Circle)
            .size(12.0)
            .color(cx.theme.colors.foreground);
        let glyph = icon.glyph_signal();
        let row = Visibility::new(
            Flex::row()
                .align("center")
                .gap(6.0)
                .padding_x(META_INDENT)
                .child(icon)
                .child(label),
            false,
        );
        let visible = row.visible_signal();
        let facts = cx.facts.clone();
        let produce = produce.clone();
        create_effect(move |_| match produce(&facts()) {
            Some(line) => {
                set_if_changed(text, line.text);
                set_if_changed(glyph, line.icon);
                set_if_changed(visible, true);
            }
            None => set_if_changed(visible, false),
        });
        Box::new(row) as Box<dyn Component>
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::PaneFacts;
    use crate::chrome::pane_items::{PaneLine, PaneRowLines};
    use heca_config::programs::ProgramsConfig;
    use heca_core::layout::PaneId;
    use heca_grid_ui::reactive::{SignalGet, SignalUpdate};

    /// **An added line follows its pane and hides itself when it has nothing to say** — with no
    /// rebuild: the line subscribed to the pane's facts when it was built, and rewrites itself.
    #[test]
    fn an_added_line_follows_its_panes_facts_and_hides_when_it_has_nothing_to_say() {
        let theme = heca_grid_ui::theme::Theme::default();
        let title = signal(String::from("plain"));
        let facts: Rc<dyn Fn() -> PaneFacts> = Rc::new(move || {
            PaneFacts::of(
                PaneId(1),
                &ProgramsConfig::default(),
                "shell",
                Some(&title.get()),
                None,
            )
        });
        let build = text_line(Rc::new(|f: &PaneFacts| {
            (f.title == "agent").then(|| PaneLine::new(Glyph::Terminal, format!("is {}", f.title)))
        }));
        let line = build(&LineCx {
            theme: &theme,
            facts,
        });
        // Visibility is applied at layout, so each check lays the row out first.
        let mut root: Box<dyn Component> = Box::new(Flex::column().child(line));
        let hidden = |root: &mut Box<dyn Component>| {
            heca_grid_ui::LayoutEngine::new()
                .compute(root.as_mut(), heca_grid_ui::Size::new(300.0, 100.0));
            root.base().children[0].base().style.layout.hidden
        };

        assert!(hidden(&mut root), "nothing to say yet, so nothing shows");
        title.set("agent".into());
        assert!(
            !hidden(&mut root),
            "the pane became one it has something to say about"
        );
        title.set("plain".into());
        assert!(
            hidden(&mut root),
            "and hides again when that stops being true"
        );
    }

    /// **The user's list decides which lines show and in what order** — `cwd` and `git` are named
    /// like any other.
    #[test]
    fn the_users_list_decides_which_lines_show_and_in_what_order() {
        let lines = PaneRowLines::default();
        let names = |cfg: &[&str]| -> Vec<String> {
            let cfg: Vec<String> = cfg.iter().map(|s| s.to_string()).collect();
            lines.shown(&cfg).names().map(str::to_string).collect()
        };
        assert_eq!(names(&["cwd", "git"]), ["cwd", "git"], "the default");
        assert_eq!(names(&["git", "cwd"]), ["git", "cwd"], "in the order given");
        assert_eq!(names(&["git"]), ["git"]);
        assert!(names(&[]).is_empty(), "an empty list shows the name alone");
        assert_eq!(
            names(&["nothing.here", "cwd"]),
            ["cwd"],
            "an unknown name is skipped"
        );
    }
}
