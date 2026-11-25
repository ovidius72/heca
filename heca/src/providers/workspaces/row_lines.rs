//! **The lines under a pane's name in the workspaces dock** — the working directory, the git state,
//! and the text line another crate adds — each a self-contained widget that follows its pane.
//!
//! A line is built once, subscribes to the pane's facts, and rewrites its own words and visibility
//! in place (change-guarded), which is why a `cd` or a checkout moves it without the sidebar being
//! rebuilt. They are registered through the same [`PaneLineDef`] another crate's line uses (see
//! `chrome/pane_items/lines.rs`), so `cwd` and `git` are the first users of the registry rather than
//! a special case in the row.

use std::rc::Rc;

use heca_grid_ui::Tone;
use heca_grid_ui::builders::{LayoutExt, Parent, ToneExt};
use heca_grid_ui::reactive::{SignalGet, create_memo};
use heca_grid_ui::style::{Length, Spacing, WidgetSize};
use heca_grid_ui::widgets::{Ellipsis, Flex, Glyph, Icon, Label, Visibility};

use super::pane_row::META_INDENT;
use crate::chrome::panes::pane_items::{BuildLine, LineCx, PaneLineDef, ProduceLine};

/// The built-in lines; configuration chooses their order and visibility.
pub(crate) fn builtin_row_lines() -> Vec<PaneLineDef> {
    let own = |name: &str, build: fn(&LineCx) -> heca_grid_ui::WidgetModel| PaneLineDef {
        name: name.to_string(),
        build: Rc::new(build),
        from_extension: false,
    };
    vec![
        own(super::TITLE_LINE, |cx| {
            heca_grid_ui::WidgetModel::new(title_line(cx))
        }),
        own("cwd", |cx| heca_grid_ui::WidgetModel::new(cwd_line(cx))),
        own("git", |cx| heca_grid_ui::WidgetModel::new(git_line(cx))),
        own("agent", |cx| {
            heca_grid_ui::WidgetModel::new(super::agent_line::build(cx))
        }),
        own("agent_session", |cx| {
            heca_grid_ui::WidgetModel::new(super::agent_line::session_line(cx))
        }),
    ]
}

/// **The pane's title line**: the status pip, the program's icon, the pane's name — it cuts rather
/// than spilling (`components::PaneName`) — and the dimmed `(process)` of a renamed pane. Built from
/// the row's own worked-out parts ([`RowHead`](crate::chrome::panes::pane_items::RowHead)), so building it
/// again for a view shows the same values. The arrangement is the library's `Tile`.
fn title_line(cx: &LineCx) -> heca_grid_ui::widgets::Tile {
    use heca_grid_ui::widgets::{StatusDot, Tile};
    let head = cx.head;
    let name = crate::components::PaneName::new("")
        .build()
        .text(move || head.get().title);
    // Smaller than the name and in the foreground colour (`muted` is unreadable on a selected row);
    // the size and the parentheses make it read as secondary, and `Flex` puts both runs on one
    // baseline.
    let hint = Visibility::new()
        .visible(move || head.get().hint.is_some())
        .child(
            Label::new("")
                .size(WidgetSize::Caption)
                .text(move || head.get().hint.unwrap_or_default()),
        );
    Tile::new()
        .status(
            StatusDot::new(heca_grid_ui::DotStatus::default()).status(move || head.get().status),
        )
        .icon(Icon::new(Glyph::Terminal).glyph(move || head.get().icon))
        .title(name)
        .suffix(hint)
}

/// **Where the pane is**: a folder glyph and the home-relative path. The one shape the exposé's card
/// shares (`components::FolderLine`). Shown while the pane has a directory, read inside this line's own
/// subscription so a `cd` moves it. Whether the line exists at all is the sidebar view's list, and
/// nothing else.
fn cwd_line(cx: &LineCx) -> crate::components::FolderLine {
    let facts = cx.facts.clone();
    // Cut to a home-relative form here, at the binding, so there is one spelling of "where this
    // pane is". No directory is an empty path, which hides the line.
    crate::components::FolderLine::new()
        .size(WidgetSize::Caption)
        .indent(META_INDENT)
        .path(move || {
            facts()
                .cwd
                .as_deref()
                .map(crate::chrome::home_relative_path)
                .unwrap_or_default()
        })
}

/// **The git state**: branch (cut for a narrow sidebar, the full name on hover) and the counts of
/// added, modified and deleted files. Absent outside a repository; a count of nothing is absent.
fn git_line(cx: &LineCx) -> Visibility {
    use crate::chrome::panes::pane_header::{git_added, git_branch, git_deleted, git_modified};
    // The pane's git state, read once per change and shared by every part of the row below.
    let facts = cx.facts.clone();
    let git = create_memo(move |_| facts().git);
    // Colours are named by meaning and read from the theme when drawn: the branch in the text
    // colour (no colour of its own), its glyph and the counts in their tones.
    //
    // One count: a glyph and its words, hidden while there is nothing to say.
    let count =
        |glyph: Glyph, tone: Tone, words: fn(&heca_core::runtime::GitInfo) -> Option<String>| {
            let value = move || git.get().as_ref().and_then(words);
            // **A count is never cut**: it is the part of the row that is short and that says the most,
            // so it keeps its own width and the branch beside it is what gives way.
            Visibility::new().visible(move || value().is_some()).child(
                Flex::row()
                    .align("center")
                    .gap(Spacing::Xs)
                    .shrink(0.0)
                    .content_tone(tone)
                    .child(Icon::new(glyph))
                    .child(
                        Label::new("")
                            .text(move || value().unwrap_or_default())
                            .truncate(Ellipsis::None),
                    ),
            )
        };
    Visibility::new()
        .visible(move || git.get().is_some())
        .child(
            Flex::row()
                .align("center")
                .size(WidgetSize::Caption)
                .gap(Spacing::Sm)
                .padding_x(META_INDENT)
                // Never wider than the card: the row is as wide as its content up to the card, and then
                // the branch — the one child that can — takes the squeeze.
                .max_width(Length::FULL)
                .child(
                    Icon::new(Glyph::GitBranch)
                        .content_tone(Tone::Warning)
                        .shrink(0.0),
                )
                // **Cut from the front by the row's own box**, so the tail — the meaningful end of a
                // branch, `…security-upgrade` — survives; cut, it offers the whole name on hover.
                .child(
                    Label::new("")
                        .text(move || git.get().as_ref().map(git_branch).unwrap_or_default())
                        .truncate(Ellipsis::Start),
                )
                .child(count(Glyph::Plus, Tone::Success, git_added))
                .child(count(Glyph::Warning, Tone::Warning, git_modified))
                .child(count(Glyph::Minus, Tone::Danger, git_deleted)),
        )
}

/// **A line another crate added**: an icon and some text, shown while `produce` has something to
/// say for the pane. It runs again when the pane's facts change — that is what the subscription is.
pub(crate) fn text_line(produce: Rc<ProduceLine>) -> Rc<BuildLine> {
    Rc::new(move |cx: &LineCx| {
        let facts = cx.facts.clone();
        let produce = produce.clone();
        // What the line says for the pane now, read once per change and shared by its parts.
        let line = create_memo(move |_| produce(&facts()));
        // No colour of its own: the line takes the text colour where it is drawn.
        heca_grid_ui::WidgetModel::new(
            Visibility::new()
                .visible(move || line.get().is_some())
                .child(
                    Flex::row()
                        .align("center")
                        .size(WidgetSize::Caption)
                        .gap(Spacing::Sm)
                        .padding_x(META_INDENT)
                        .child(
                            Icon::new(Glyph::Circle)
                                .glyph(move || line.get().map_or(Glyph::Circle, |line| line.icon)),
                        )
                        .child(
                            Label::new("")
                                .text(move || line.get().map(|line| line.text).unwrap_or_default()),
                        ),
                ),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::PaneFacts;
    use crate::chrome::panes::pane_items::{PaneLine, PaneRowLines};
    use heca_config::programs::ProgramsConfig;
    use heca_core::layout::PaneId;
    use heca_grid_ui::reactive::{SignalGet, SignalUpdate, signal};

    /// **An added line follows its pane and hides itself when it has nothing to say** — with no
    /// rebuild: the line subscribed to the pane's facts when it was built, and rewrites itself.
    #[test]
    fn an_added_line_follows_its_panes_facts_and_hides_when_it_has_nothing_to_say() {
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
            head: Default::default(),
            facts,
            agent_statuses: signal(Rc::new(
                heca_config::settings::AgentsConfig::default().statuses,
            )),
            agent_session_line: signal(Default::default()),
        });
        // Visibility is applied at layout, so each check lays the row out first.
        let mut root: heca_grid_ui::WidgetModel =
            heca_grid_ui::WidgetModel::new(Flex::column().child(line));
        let hidden = |root: &mut heca_grid_ui::WidgetModel| {
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

    /// **In the git row the branch gives way and the counts never do** — measured in a card too
    /// narrow for the whole row: every count is drawn whole, the branch is the part that is cut.
    #[test]
    fn a_narrow_git_row_cuts_the_branch_and_keeps_the_counts_whole() {
        use heca_core::runtime::{GitInfo, PaneRuntime};
        let theme = heca_grid_ui::theme::Theme::default();
        let runtime = PaneRuntime {
            git: Some(GitInfo {
                branch: Some("feature/a-very-long-branch-name-for-dependency-upgrades".into()),
                added: 12,
                modified: 34,
                deleted: 5,
                ..Default::default()
            }),
            ..Default::default()
        };
        let facts: Rc<dyn Fn() -> PaneFacts> = Rc::new(move || {
            PaneFacts::of(
                PaneId(1),
                &ProgramsConfig::default(),
                "shell",
                None,
                Some(&crate::chrome::PanePresentation::immediate(runtime.clone())),
            )
        });
        let line = git_line(&LineCx {
            head: Default::default(),
            facts,
            agent_statuses: signal(Rc::new(
                heca_config::settings::AgentsConfig::default().statuses,
            )),
            agent_session_line: signal(Default::default()),
        });
        // A card about as wide as a sidebar row, narrower than the row's natural width.
        let mut root = Flex::column()
            .width(heca_grid_ui::style::Length::Px(220.0))
            .child(line);
        let scene = crate::chrome::paint_chrome_root(&mut root, 220.0, 100.0, &theme);
        let drawn: Vec<String> = scene
            .iter()
            .filter_map(|c| match c {
                heca_grid_ui::DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();
        for count in ["12", "34", "5"] {
            assert!(
                drawn.iter().any(|t| t.contains(count) && !t.contains('…')),
                "the count {count} is drawn whole: {drawn:?}"
            );
        }
        assert!(
            drawn
                .iter()
                .any(|t| t.contains('…') && t.contains("upgrades")),
            "the branch is the part that is cut, from the front: {drawn:?}"
        );
    }

    /// **Each view's list decides which lines show and in what order** — the title is named like
    /// any other line; an unknown name is skipped; the current pane has its own list or follows.
    #[test]
    fn each_views_list_decides_which_lines_show_and_in_what_order() {
        use heca_config::appearance::{PaneView, SidebarAppearance};
        let lines = PaneRowLines::default();
        let owned = |cfg: &[&str]| -> Vec<String> { cfg.iter().map(|s| s.to_string()).collect() };
        let sidebar = SidebarAppearance {
            pane_compact_view: owned(&["title"]),
            pane_normal_view: owned(&["git", "title", "cwd"]),
            pane_extended_view: owned(&["nothing.here", "cwd"]),
            pane_current_view: Some(owned(&["agent", "title"])),
            ..SidebarAppearance::default()
        };
        let views = lines.views(&sidebar);
        let names = |view: PaneView, current: bool| -> Vec<String> {
            views
                .lines(view, current)
                .names()
                .map(str::to_string)
                .collect()
        };
        assert_eq!(names(PaneView::Compact, false), ["title"]);
        assert_eq!(
            names(PaneView::Normal, false),
            ["git", "title", "cwd"],
            "in the order given"
        );
        assert_eq!(
            names(PaneView::Extended, false),
            ["cwd"],
            "an unknown name is skipped"
        );
        assert_eq!(
            names(PaneView::Compact, true),
            ["agent", "title"],
            "the current pane's own"
        );
        let follows = lines.views(&SidebarAppearance::default());
        assert_eq!(
            follows.lines(PaneView::Normal, true),
            follows.lines(PaneView::Normal, false),
            "with no list of its own, the current pane follows the view"
        );
    }

    /// **The git row takes its colours from the theme it is painted with**, by meaning: the branch
    /// in the text colour, its glyph and the modified count in `warning`, added in `success`,
    /// deleted in `danger`. Built under one theme and painted under another, it shows the second —
    /// nothing was copied in when it was built.
    #[test]
    fn the_git_row_takes_its_colours_from_the_theme_it_is_painted_with() {
        use heca_core::runtime::{GitInfo, PaneRuntime};
        let mut painted_with = heca_grid_ui::theme::Theme::default();
        painted_with.colors.foreground = heca_grid_ui::Color::rgb(0x10, 0x20, 0x30);
        painted_with.colors.success = heca_grid_ui::Color::rgb(0x11, 0x22, 0x33);
        painted_with.colors.warning = heca_grid_ui::Color::rgb(0x44, 0x55, 0x66);
        painted_with.colors.danger = heca_grid_ui::Color::rgb(0x77, 0x88, 0x99);
        let runtime = PaneRuntime {
            git: Some(GitInfo {
                branch: Some("main".into()),
                added: 1,
                modified: 2,
                deleted: 3,
                ..Default::default()
            }),
            ..Default::default()
        };
        let facts: Rc<dyn Fn() -> PaneFacts> = Rc::new(move || {
            PaneFacts::of(
                PaneId(1),
                &ProgramsConfig::default(),
                "shell",
                None,
                Some(&crate::chrome::PanePresentation::immediate(runtime.clone())),
            )
        });
        let line = git_line(&LineCx {
            head: Default::default(),
            facts,
            agent_statuses: signal(Rc::new(
                heca_config::settings::AgentsConfig::default().statuses,
            )),
            agent_session_line: signal(Default::default()),
        });
        let mut root = Flex::column().child(line);
        let scene = crate::chrome::paint_chrome_root(&mut root, 400.0, 100.0, &painted_with);
        let colour_of = |text: &str| {
            scene.iter().find_map(|c| match c {
                heca_grid_ui::DrawCommand::Text(t) if t.text == text => Some(t.color),
                _ => None,
            })
        };
        let c = &painted_with.colors;
        assert_eq!(colour_of("main"), Some(c.foreground), "the branch");
        assert_eq!(colour_of("+1"), Some(c.success), "added");
        assert_eq!(colour_of("~2"), Some(c.warning), "modified");
        assert_eq!(colour_of("-3"), Some(c.danger), "deleted");
        // A glyph is drawn in two layers (a faint one under a full one): the full one says its tone.
        let branch_glyph = Glyph::GitBranch.primary_char().expect("a glyph");
        let glyph_drawn_in = |colour: heca_grid_ui::Color| {
            scene.iter().any(|cmd| match cmd {
                heca_grid_ui::DrawCommand::Text(t) => {
                    t.color == colour
                        && t.text
                            .chars()
                            .next()
                            .is_some_and(|ch| (ch as u32).abs_diff(branch_glyph as u32) <= 1)
                }
                _ => false,
            })
        };
        assert!(glyph_drawn_in(c.warning), "the branch glyph");
    }
}
