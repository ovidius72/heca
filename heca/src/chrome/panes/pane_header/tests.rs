use super::*;
use heca_core::runtime::{ContentKind, GitInfo};
use heca_grid_ui::builders::HintExt as _;
use heca_grid_ui::widgets::Label;

fn facts_for(cwd: &str) -> super::super::pane_items::PaneFacts {
    let rt = PaneRuntime {
        program: Some("nvim".into()),
        cwd: Some(std::path::PathBuf::from(cwd)),
        ..PaneRuntime::default()
    };
    super::super::pane_items::PaneFacts::of(
        PaneId(1),
        &ProgramsConfig::default(),
        "shell",
        None,
        Some(&rt),
    )
}

/// The header preserves values and their fitting hints; labels own shortening and tooltips.
#[test]
fn a_narrow_bar_keeps_full_values_for_the_widgets_tooltips() {
    let chips = super::super::pane_items::PaneChips::default();
    let defs: Vec<_> = ["location", "app_name"]
        .iter()
        .map(|n| chips.get(n).expect("shipped"))
        .collect();
    let facts = facts_for("/a/very/long/path/to/some/project/directory");

    let roomy = segment_items(&chips, &defs, &facts);
    let tight = segment_items(&chips, &defs, &facts);

    assert_eq!(roomy[0].chip.text, "/a/very/long/path/to/some/project/directory");
    assert_eq!(tight[0].chip.text, roomy[0].chip.text, "shortening belongs to Label, preserving the tooltip");
    assert_eq!(tight[1].chip.text, roomy[1].chip.text, "the program name is untouched");
    assert_eq!(tight.len(), 2);
}

/// A pane_name chip builds a bar for a renamed pane and for one that is not renamed.
#[test]
fn a_bar_is_built_from_what_the_chips_say() {
    let chips = super::super::pane_items::PaneChips::default();
    let defs = [chips.get("pane_name").expect("shipped")];
    let facts = facts_for("/tmp");
    let items = segment_items(&chips, &defs, &facts);
    assert!(build_pane_info_bar(items).is_some());
    assert!(
        build_pane_info_bar(Vec::new()).is_none(),
        "no chips, no bar"
    );
}

#[test]
fn header_segments_keep_full_hover_values_and_theme_colours_when_squeezed() {
    use heca_grid_ui::{DrawCommand, Flex, Length, builders::{LayoutExt, Parent}};
    let chips = super::super::pane_items::PaneChips::default();
    let defs: Vec<_> = ["location", "app_name", "git_branch"].iter()
        .map(|name| chips.get(name).unwrap()).collect();
    let mut facts = facts_for("/a/very/long/path/to/the/project/directory");
    facts.git = Some(GitInfo { branch: Some("feature/a-long-branch-name".into()), ..Default::default() });
    let tag = build_pane_info_bar(segment_items(&chips, &defs, &facts)).unwrap();
    let mut root = Flex::row().width(Length::FULL).child(tag);
    let theme = GuiTheme::default();
    for width in [280.0, 480.0] {
        let scene = crate::chrome::paint_chrome_root(&mut root, width, 100.0, &theme);
        assert!(scene.iter().any(|cmd| matches!(cmd, DrawCommand::Text(text) if text.text == facts.app_name)),
            "the short application name should remain readable");
        let tag = &root.base().children[0];
        let path_tip = tag.base().children[0].base().tooltip.as_ref().unwrap();
        let branch_tip = tag.base().children[2].base().tooltip.as_ref().unwrap();
        assert_eq!(path_tip.text.get_untracked(), "/a/very/long/path/to/the/project/directory");
        assert_eq!(branch_tip.text.get_untracked(), "feature/a-long-branch-name");
        for (index, expected) in [(0, theme.colors.accent), (2, theme.colors.warning)] {
            let segment = tag.base().children[index].base().bounds;
            assert!(scene.iter().any(|cmd| matches!(cmd, DrawCommand::Text(text)
                if segment.contains(text.rect.loc) && text.color == expected)));
        }
        for cmd in scene.iter() {
            if let DrawCommand::Text(text) = cmd {
                assert!(text.rect.loc.x + text.rect.size.w <= f64::from(width) + 1.0, "header text escaped: {text:?}");
            }
        }
    }
    heca_grid_ui::set_text_by_key(&root, &segment_text_key("location"), "/another/complete/path");
    let tag = &root.base().children[0];
    assert_eq!(tag.base().children[0].base().tooltip.as_ref().unwrap().text.get_untracked(), "/another/complete/path");
}

/// **Which actions are destructive is the action's own declaration, not a surface's.**
///
/// It was written into this file as a `matches!` on the close button — a styling rule keyed to a
/// name, in a file that should know nothing about which acts cannot be undone. Every surface that
/// renders an action reads the same answer now, exactly as they already do for its icon and its
/// label, so a header button, a menu entry and the palette cannot disagree about what is dangerous.
#[test]
fn a_headers_danger_hue_comes_from_the_action_not_from_its_name() {
    let catalog = crate::actions::ActionCatalog::with_builtins();
    assert!(
        catalog.destructive("close"),
        "closing a pane is declared destructive by the action"
    );
    for safe in ["zoom_column", "float", "add_pane_to_column"] {
        assert!(
            !catalog.destructive(safe),
            "{safe} is not destructive, so nothing should draw it as though it were"
        );
    }
}

/// **A chrome button's pick says what it is, so the picker can refuse it**.
///
/// A button hands over the same closure for its click and its pick — they are one gesture for a
/// button — and a closure is opaque, so the policy had nothing to ask about. `prefix+/`
/// therefore lettered the sidebar toggles while a pane was floating, even though `sidebar_left`
/// is `TiledOnly` and the click was already refused: a letter that did nothing.
///
/// It is named here because this is the one place a chrome button's action name is known — the
/// same place the tooltip and its live keybinding come from. If that ever goes, every chrome
/// button silently escapes the filter again and nothing else would notice.
#[test]
fn a_buttons_pick_is_named_from_its_action() {
    let shortcuts = ActionShortcuts::default();
    let button = Label::new("×").on_hint(|| {});
    let tip = action_tooltip(button, "close", "Close", &shortcuts);

    // Read straight off the widget: the tip is a **property** now, so what comes back is the
    // button itself rather than a wrapper around it — which is exactly what lets a pane header
    // button go into a `ButtonGroup`.
    let named = tip
        .base()
        .hint
        .as_ref()
        .expect("the button declares a pick")
        .intent
        .as_ref()
        .expect("…and it is named");
    assert_eq!(named.action, "close");
}

/// A pick that already said what it is keeps it — the caller knows more than the action name
/// alone (a pane button carries the pane it acts on).
#[test]
fn a_pick_that_already_named_itself_is_left_alone() {
    let shortcuts = ActionShortcuts::default();
    let button = Label::new("×").on_hint(heca_grid_ui::Hint::of(
        heca_view::Intent::new("close_pane_by_id").arg("pane_id", heca_view::PropValue::Int(7)),
        || {},
    ));
    let tip = action_tooltip(button, "close", "Close", &shortcuts);

    let named = tip.base().hint.as_ref().unwrap().intent.as_ref().unwrap();
    assert_eq!(
        named.action, "close_pane_by_id",
        "the caller's own naming wins"
    );
}

#[test]
fn pane_info_view_resolves_program_status_and_git_segments() {
    let programs = ProgramsConfig::default();
    let runtime = PaneRuntime {
        program: Some("v".into()),
        status: ProcessStatus::Running,
        cwd: None,
        exit_code: None,
        git: Some(GitInfo {
            branch: Some("main".into()),
            ahead: 0,
            behind: 0,
            added: 2,
            modified: 3,
            deleted: 1,
            dirty: true,
        }),
        kind: ContentKind::Terminal,
        agent: None,
    };

    // No custom name → title is the program name, no process hint (the title IS the process).
    let view = pane_info_view(&programs, "shell", None, Some(&runtime), true);
    assert_eq!(view.title, "Neovim");
    assert_eq!(view.process_hint, None);

    // Custom name + setting on → title is the custom name, hint carries the program name.
    let view = pane_info_view(&programs, "shell", Some("Editor"), Some(&runtime), true);
    assert_eq!(view.title, "Editor");
    assert_eq!(view.process_hint.as_deref(), Some("Neovim"));
    // Custom name + setting off → no hint.
    let view = pane_info_view(&programs, "shell", Some("Editor"), Some(&runtime), false);
    assert_eq!(view.title, "Editor");
    assert_eq!(view.process_hint, None);

    let view = pane_info_view(&programs, "shell", None, Some(&runtime), false);

    assert_eq!(view.icon, Glyph::FileCode);
    assert_eq!(view.title, "Neovim");
    assert_eq!(view.status, ProcessStatus::Running);
    assert_eq!(view.git_branch.as_deref(), Some("main"));
    assert_eq!(view.git_added.as_deref(), Some("+2"));
    assert_eq!(view.git_modified.as_deref(), Some("~3"));
    assert_eq!(view.git_deleted.as_deref(), Some("-1"));
}

#[test]
fn pane_info_view_uses_shell_fallbacks_without_git() {
    let programs = ProgramsConfig::default();
    let runtime = PaneRuntime {
        program: Some("zsh".into()),
        status: ProcessStatus::Idle,
        ..PaneRuntime::default()
    };

    let view = pane_info_view(&programs, "pane", None, Some(&runtime), false);

    assert_eq!(view.icon, Glyph::Terminal);
    assert_eq!(view.title, "zsh");
    assert_eq!(view.status, ProcessStatus::Idle);
    assert_eq!(view.git_branch, None);
    assert_eq!(view.git_added, None);
    assert_eq!(view.git_modified, None);
    assert_eq!(view.git_deleted, None);
}

#[test]
fn pane_header_key_tracks_its_shape_never_its_words_or_width() {
    let programs = ProgramsConfig::default();
    let chips = super::PaneChips::default();
    let segments: Vec<&crate::chrome::panes::pane_items::PaneChipDef> = ["app_name", "git_branch"]
        .iter()
        .map(|n| chips.get(n).expect("shipped"))
        .collect();
    let buttons = super::PaneButtons::default();
    let actions: Vec<&crate::chrome::panes::pane_items::PaneButtonDef> = ["split", "close"]
        .iter()
        .map(|n| buttons.get(n).expect("shipped"))
        .collect();
    let runtime = PaneRuntime {
        program: Some("zsh".into()),
        status: ProcessStatus::Idle,
        git: Some(GitInfo {
            branch: Some("main".into()),
            ..GitInfo::default()
        }),
        ..PaneRuntime::default()
    };
    let hints = ActionShortcuts::default();
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let facts_of = |rt: &PaneRuntime| {
        crate::chrome::panes::pane_items::PaneFacts::of(
            PaneId(1),
            &programs,
            "shell",
            None,
            Some(rt),
        )
    };
    #[allow(clippy::too_many_arguments)]
    fn content<'a>(
        chips: &'a super::PaneChips,
        segments: &'a [&'a crate::chrome::panes::pane_items::PaneChipDef],
        actions: &'a [&'a crate::chrome::panes::pane_items::PaneButtonDef],
        facts: &'a crate::chrome::panes::pane_items::PaneFacts,
        hints: &'a ActionShortcuts,
        catalog: &'a crate::actions::ActionCatalog,
        col_idx: usize,
    ) -> super::PaneHeaderContent<'a> {
        super::PaneHeaderContent {
            facts,
            segments,
            chips,
            actions,
            ws_idx: 0,
            col_idx,
            zoomed: false,
            floating: false,
            shortcuts: hints,
            catalog,
        }
    }
    let base = super::pane_header_key(
        &content(
            &chips,
            &segments,
            &actions,
            &facts_of(&runtime),
            &hints,
            &catalog,
            0,
        ),
        15.0,
        300.0,
    );
    // Same inputs ⇒ same key (no needless rebuild).
    assert_eq!(
        base,
        super::pane_header_key(
            &content(
                &chips,
                &segments,
                &actions,
                &facts_of(&runtime),
                &hints,
                &catalog,
                0
            ),
            15.0,
            300.0
        )
    );
    // A different column ⇒ different key (re-bakes the split action's col_idx).
    assert_ne!(
        base,
        super::pane_header_key(
            &content(
                &chips,
                &segments,
                &actions,
                &facts_of(&runtime),
                &hints,
                &catalog,
                1
            ),
            15.0,
            300.0
        )
    );
    // **A different branch ⇒ the SAME key**. A branch is *words*, and words are
    // a per-frame input written into the retained tree, not part of what the header is. While
    // they were part of its identity, every command rebuilt the whole bar — twice, at the
    // command's start and finish — and a rebuilt widget paints nothing until the layout walk
    // gives it a box, so the buttons blinked out and back both times.
    let mut other = runtime.clone();
    other.git = Some(GitInfo {
        branch: Some("dev".into()),
        ..GitInfo::default()
    });
    assert_eq!(
        base,
        super::pane_header_key(
            &content(
                &chips,
                &segments,
                &actions,
                &facts_of(&other),
                &hints,
                &catalog,
                0
            ),
            15.0,
            300.0
        ),
        "changing a segment's words rebuilt the header instead of rewriting them",
    );

    // **A segment that GOES ⇒ a different key.** That is a change of shape, not of words: the
    // bar has one section fewer, so it genuinely is a different header. This is the other half
    // of the rule, and without it "words never rebuild" would be satisfied by never rebuilding.
    let mut no_branch = runtime.clone();
    no_branch.git = None;
    assert_ne!(
        base,
        super::pane_header_key(
            &content(
                &chips,
                &segments,
                &actions,
                &facts_of(&no_branch),
                &hints,
                &catalog,
                0
            ),
            15.0,
            300.0
        ),
        "a segment disappearing must rebuild — it changes what the bar IS",
    );

    // **And the status changes nothing at all.** It is carried on the projection and rendered
    // nowhere in this bar, so Idle → Running → Success used to churn the identity while
    // changing nothing a user could see. That was half of every command's rebuilds.
    let mut running = runtime.clone();
    running.status = heca_core::runtime::ProcessStatus::Running;
    assert_eq!(
        base,
        super::pane_header_key(
            &content(
                &chips,
                &segments,
                &actions,
                &facts_of(&running),
                &hints,
                &catalog,
                0
            ),
            15.0,
            300.0
        ),
        "a status this bar does not render must not rebuild it",
    );
    // **A width change ⇒ the SAME key.** A pane's width is a per-frame layout input, not part of
    // what the header is — the same rule the pane shell states for its own rect. The bar used to
    // re-run width arithmetic of its own, so a resize had to rebuild it; `Label` truncates
    // itself, so nothing does now. Keying on width meant every step of a drag threw the header
    // away and built a new one: a burst of CPU and a visible flicker in the buttons. It re-lays
    // out instead.
    for w in [120.0, 295.0, 1600.0] {
        assert_eq!(
            base,
            super::pane_header_key(
                &content(
                    &chips,
                    &segments,
                    &actions,
                    &facts_of(&runtime),
                    &hints,
                    &catalog,
                    0
                ),
                15.0,
                w
            ),
            "a resize to {w} rebuilt the header instead of re-laying it out"
        );
    }
}
