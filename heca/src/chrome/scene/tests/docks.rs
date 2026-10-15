//! A sidebar shell and the docks seated in it: what they host, how they are outlined and lettered.

use super::*;
use heca_config::appearance::BorderStyle;

/// A sidebar look for a test shell: 8px gap, 12px corners, the frame's default colour.
fn shell_look(border_style: BorderStyle, border_width: f32, background: Color) -> SidebarLook {
    SidebarLook {
        background,
        gap: 8.0,
        border_style,
        border_width,
        border_radius: 12.0,
        border_color: None,
    }
}

#[test]
fn sidebar_shell_hosts_the_provider_mounted_container() {
    use heca_grid_ui::Component;

    let tree = one_pane_tree(1);
    let theme = GuiTheme::default();
    let chrome = SharedChromeState::new(280.0, 260.0);
    chrome
        .workspaces
        .set_active_pane(Some(heca_core::layout::PaneId(1)));
    // The shell wraps a bracketed Pane that holds just the container the region's
    // provider built (the collapse toggle moved to the top bar, sidebar-fu-14); the
    // container hosts a dock per workspace (so the tree's text is visible).
    let content = region_body(
        &tree,
        &theme,
        &chrome,
        &mut super::ChromeSignals::default(),
        &mut super::DragItemRegistry::default(),
    );
    assert!(
        content.is_some(),
        "the workspaces provider is mounted in the left region, so it must contribute a body",
    );
    let shell = super::build_sidebar_shell(
        280.0,
        600.0,
        &shell_look(BorderStyle::Bracketed, 1.0, theme.colors.background),
        content,
    );
    assert_eq!(
        shell.base().children.len(),
        1,
        "sidebar shell wraps a single full-height background surface",
    );
    let surface = &shell.base().children[0];
    assert_eq!(
        surface.base().children.len(),
        1,
        "the background surface wraps a single bracketed Pane",
    );
    let pane = &surface.base().children[0];
    assert_eq!(
        pane.base().children.len(),
        1,
        "the shell Pane holds just the mounted container's body (the collapse toggle \
         moved to the top bar, sidebar-fu-14)",
    );
    let container = &pane.base().children[0];
    assert!(
        !container.base().children.is_empty(),
        "the WorkspacesContainer must host a dock per workspace",
    );
}

#[test]
fn bordered_sidebar_paints_border_color_at_configured_width() {
    // Regression: with `sidebar_border_style = "bordered"`, the shell must paint
    // a visible frame in the (global) border color at the configured
    // `sidebar_border_width`. Previously the bordered sidebar drew nothing /
    // ignored the color because its width was theme-locked.
    use heca_grid_ui::DrawCommand;

    let tree = one_pane_tree(1);

    // A distinct border color so we can prove it reached the painted frame.
    let mut theme = GuiTheme::default();
    theme.colors.border = Color::new(0x40, 0xe0, 0xff, 0xff);
    let border_w = 4.0_f32;

    let chrome = SharedChromeState::new(280.0, 260.0);
    let content = region_body(
        &tree,
        &theme,
        &chrome,
        &mut super::ChromeSignals::default(),
        &mut super::DragItemRegistry::default(),
    );
    let mut shell = super::build_sidebar_shell(
        280.0,
        600.0,
        &shell_look(BorderStyle::Bordered, border_w, theme.colors.background),
        content,
    );

    let scene = super::paint_chrome_root(&mut shell, 280.0, 600.0, &theme);
    let found = scene.iter().any(|c| match c {
        DrawCommand::Rect(r) => r.border.is_some_and(|b| {
            b.color == theme.colors.border && (b.width - border_w).abs() < 0.01
        }),
        _ => false,
    });
    assert!(
        found,
        "bordered sidebar must paint a {border_w}px frame in the configured border color",
    );
}

/// The focused dock is **outlined**, and an unfocused one is not (F003/P011/T020).
///
/// Focus with nothing to show for it tells the user nothing, so this asserts the drawn ring
/// rather than a flag — and asserts that it follows the store *without a rebuild*, because the
/// host flips a signal rather than rebuilding the chrome tree.
///
/// Whether the ring reads well at that width and colour is a visual judgement, and the user makes
/// it in the app. What is pinned here is that it is drawn at all, on the right dock.
#[test]
fn the_focused_dock_is_outlined_and_follows_the_store() {
    use heca_grid_ui::DrawCommand;

    let tree = one_pane_tree(1);
    let theme = GuiTheme::default();
    let chrome = SharedChromeState::new(280.0, 260.0);
    let content = region_body(
        &tree,
        &theme,
        &chrome,
        &mut super::ChromeSignals::default(),
        &mut super::DragItemRegistry::default(),
    );
    let mut shell = super::build_sidebar_shell(
        280.0,
        600.0,
        &shell_look(BorderStyle::Bracketed, 1.0, theme.colors.background),
        content,
    );

    let ring = theme.colors.effective_focus_ring();
    let outlined = |shell: &mut Flex| {
        super::paint_chrome_root(shell, 280.0, 600.0, &theme)
            .iter()
            .any(|c| match c {
                DrawCommand::Rect(r) => r.border.is_some_and(|b| b.color == ring),
                _ => false,
            })
    };

    assert!(
        !outlined(&mut shell),
        "no dock has focus yet, so nothing is outlined",
    );
    assert!(heca_grid_ui::focus_scope(&mut shell, "workspaces"));
    assert!(
        outlined(&mut shell),
        "the dock the keyboard is in is outlined in the theme's focus colour — read off the tree",
    );
    heca_grid_ui::release_scope(&mut shell, "workspaces");
    assert!(
        !outlined(&mut shell),
        "and the keyboard leaving it takes the outline away",
    );
}

/// **A mounted dock does not spend one of `prefix+/`'s letters.**
///
/// The host wraps every mounted container in a focus scope that clicks to `FocusDock`. That
/// makes the whole dock actionable, so the ordinary picker lettered it — a letter per
/// *placement*, pointing at a keyboard destination `prefix+Shift+e` already offers on every
/// dock, while the rows inside it were the things worth reaching.
///
/// Naming its scope is what keeps it out. Its own pick still reaches it by name — that is
/// `a_docks_pick_letter_is_stamped_over_it`, and the two must stay true together.
#[test]
fn a_mounted_dock_is_not_a_target_of_the_ordinary_picker() {
    let tree = one_pane_tree(1);
    let theme = GuiTheme::default();
    let chrome = SharedChromeState::new(280.0, 260.0);
    let mut signals = super::ChromeSignals::default();
    let content = region_body(
        &tree,
        &theme,
        &chrome,
        &mut signals,
        &mut super::DragItemRegistry::default(),
    );
    let shell = super::build_sidebar_shell(
        280.0,
        600.0,
        &shell_look(BorderStyle::Bracketed, 1.0, theme.colors.background),
        content,
    );

    let named: Vec<String> = heca_grid_ui::collect_hints(&shell)
        .into_iter()
        .filter_map(|(path, _)| heca_grid_ui::identity_of(&shell, &path))
        .collect();
    assert!(
        !named.iter().any(|n| n.ends_with("workspaces")),
        "the dock itself must not be offered by the ordinary picker: {named:?}",
    );
    assert!(
        named.iter().any(|n| n.contains("ws:")),
        "…while the rows inside it still are: {named:?}",
    );
}

/// A dock's pick letter reaches the tree: the host registers the signal, and setting it stamps a
/// keycap over that dock. The letters themselves come from the pick candidates
/// (`dock_keycap`), projected each frame by `sync_chrome_signals`.
#[test]
fn a_docks_pick_letter_is_stamped_over_it() {
    use heca_grid_ui::DrawCommand;

    let tree = one_pane_tree(1);
    let theme = GuiTheme::default();
    let chrome = SharedChromeState::new(280.0, 260.0);
    let mut signals = super::ChromeSignals::default();
    let content = region_body(
        &tree,
        &theme,
        &chrome,
        &mut signals,
        &mut super::DragItemRegistry::default(),
    );
    let mut shell = super::build_sidebar_shell(
        280.0,
        600.0,
        &shell_look(BorderStyle::Bracketed, 1.0, theme.colors.background),
        content,
    );

    // **The dock is lettered by the identity it declares**, through the one door every other
    // target uses — `offer_hint_by_key`, matching its `scope_key`. No per-widget signal list,
    // and nothing projected onto it every frame (F003/P082/T427).
    let letter_drawn = |shell: &mut Flex| {
        super::paint_chrome_root(shell, 280.0, 600.0, &theme)
            .iter()
            .any(|c| matches!(c, DrawCommand::Text(t) if t.text == "a"))
    };
    assert!(!letter_drawn(&mut shell), "no pick open, no keycap");
    assert!(
        heca_grid_ui::offer_hint_by_key(&shell, "workspaces", Some("a".to_string())),
        "the mounted container answers to its own mount id",
    );
    assert!(
        letter_drawn(&mut shell),
        "the dock's letter is stamped over it while the pick is open",
    );
    heca_grid_ui::offer_hint_by_key(&shell, "workspaces", None);
    assert!(
        !letter_drawn(&mut shell),
        "and withdrawn when the pick ends"
    );
}

#[test]
fn drag_registry_captures_pane_column_and_workspace() {
    let tree = one_pane_tree(7);
    let theme = GuiTheme::default();
    let chrome = SharedChromeState::new(280.0, 260.0);
    let mut drag = super::DragItemRegistry::default();
    // Drag items are registered while the region's mounted container builds its body,
    // so go through the host path and inspect what landed in the registry.
    let _ = region_body(
        &tree,
        &theme,
        &chrome,
        &mut super::ChromeSignals::default(),
        &mut drag,
    );

    let items = drag.items();
    // The drag framework decides kind by the side-map, not by raw ids — so a pane,
    // its column, and its workspace must each be registered (F4.5 step 2 scope C).
    assert!(items.contains(&super::ChromeDragItem::Pane(heca_core::layout::PaneId(7))));
    assert!(items.contains(&super::ChromeDragItem::Column { ws: 0, col: 0 }));
    assert!(items.contains(&super::ChromeDragItem::Workspace { ws: 0 }));
}

/// **`[appearance.sidebar] border_color` colours the sidebar frame**, brackets included. Unset, the
/// brackets keep the accent.
#[test]
fn the_sidebar_border_colour_from_config_reaches_the_frame() {
    use heca_grid_ui::DrawCommand;

    let red = Color::new(255, 0, 0, 255);
    let theme = GuiTheme::default();
    let frame_colours = |border_color: Option<Color>| {
        let mut appearance = heca_config::appearance::AppearanceConfig::default();
        appearance.sidebar.border_style = Some(BorderStyle::Bracketed);
        appearance.sidebar.border_color = border_color;
        let look = SidebarLook::from_config(
            &appearance,
            &heca_config::theme::Theme::default(),
            theme.colors.background,
        );
        let mut shell = super::build_sidebar_shell(280.0, 600.0, &look, None);
        super::paint_chrome_root(&mut shell, 280.0, 600.0, &theme)
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Rect(r) => r.border.map(|b| b.color.with_alpha(255)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    let set = frame_colours(Some(red));
    assert!(!set.is_empty() && set.iter().all(|c| *c == red), "the brackets are red");

    let accent = theme.colors.accent.with_alpha(255);
    let unset = frame_colours(None);
    assert!(!unset.is_empty() && unset.iter().all(|c| *c == accent), "unset, the accent");
}
