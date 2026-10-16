use super::*;
use crate::actions::{ActionCatalog, ActionCategory, ActionMeta};
use crate::app::interaction::ActionPolicy;

fn owner(mount: &str, title: &str, focused: bool) -> OwnerInfo {
    OwnerInfo {
        mount: mount.to_string(),
        title: title.to_string(),
        focused,
    }
}

fn component_action(catalog: &mut ActionCatalog, name: &str, label: &str, owner: &str) {
    let _ = catalog.insert_dynamic(ActionMeta {
        name: name.to_string(),
        label: label.to_string(),
        description: format!("{label} — what it does."),
        category: ActionCategory::Chrome,
        owner: Some(owner.to_string()),
        icon: None,
        policy: ActionPolicy::ContainerFocused,
        args: Vec::new(),
        confirm: None,
    });
}

fn find<'a>(rows: &'a [PaletteEntry], id: &str) -> &'a PaletteEntry {
    rows.iter()
        .find(|e| e.id == id)
        .unwrap_or_else(|| panic!("{id} is not in the palette"))
}

/// A component's action is labelled with its component's title and carries the composite —
/// choosing it focuses the placement that owns it, then acts.
#[test]
fn a_components_action_is_scoped_and_focuses_first() {
    let mut catalog = ActionCatalog::with_builtins();
    component_action(
        &mut catalog,
        "docker.restart",
        "Restart Container",
        "docker",
    );
    let owners = std::collections::HashMap::from([(
        "docker.restart".to_string(),
        owner("docker.right", "Containers", false),
    )]);

    let rows = entries(&catalog, &owners, &crate::chrome::ActionShortcuts::default());
    let entry = find(&rows, "docker.restart");
    assert_eq!(entry.label, "Containers › Restart Container");
    assert_eq!(entry.description, "Restart Container — what it does.");
    match &entry.intent {
        InteractionIntent::FocusContainerThenAction { container, action } => {
            assert_eq!(
                container, "docker.right",
                "the placement, not the component"
            );
            assert_eq!(action.action, "docker.restart");
        }
        other => panic!("expected the focus-then-act composite, got {other:?}"),
    }
}

/// A session with two named panes in one workspace, and a second, unnamed workspace.
fn session_with_panes() -> heca_core::layout::testing::Windowed {
    use heca_core::layout::{Pane, PaneId, Size};
    let mut session = heca_core::layout::testing::Windowed::new(Size::new(800.0, 600.0), 1.0);
    session.session.workspaces[0].name = Some("Editing".to_string());
    // **The shape a running app actually has**: `title` empty, the name carried by the live
    // runtime. Reading `title` here produced a list of blank rows in the app while the sidebar
    // beside it read `zsh`, so the fixture has to be built this way or the test proves nothing.
    let mut running = Pane::new(PaneId(1), "");
    running.runtime.program = Some("zsh".to_string());
    running.runtime.cwd = Some(std::path::PathBuf::from("/tmp/project"));
    session.m().add_pane(running, None, false);
    // A renamed pane: `custom_name` overrides the process-derived name everywhere it is shown.
    let mut renamed = Pane::new(PaneId(2), "");
    renamed.runtime.program = Some("nvim".to_string());
    renamed.custom_name = Some("logs".to_string());
    session.m().add_pane(renamed, None, false);
    session.m().add_workspace();
    session
}

#[test]
fn adding_one_mode_declares_its_rows_and_both_ways_to_open_it() {
    use heca_grid_ui::{Event, WidgetIntent};
    use std::{cell::RefCell, rc::Rc};

    let session = session_with_panes();
    let catalog = ActionCatalog::with_builtins();
    let owners = std::collections::HashMap::new();
    let shortcuts = crate::chrome::ActionShortcuts::default();
    let programs = heca_config::programs::ProgramsConfig::default();
    let sources = modes::RowSources {
        catalog: &catalog,
        owners: &owners,
        shortcuts: &shortcuts,
        layout: session.l(),
        programs: &programs,
        add_process_name: false,
        last_focused: None,
        last_visited: None,
    };
    let mut declared = modes::MODES.to_vec();
    declared.push(modes::Mode {
        sigil: '!',
        scope: "custom",
        rows: |sources| {
            workspace_entries(sources.layout, None)
                .into_iter()
                .map(|mut row| {
                    row.id = format!("custom:{}", row.id);
                    row
                })
                .collect()
        },
    });
    for named in [false, true] {
        let ran = Rc::new(RefCell::new(String::new()));
        let (palette, rows) = modes::populate(CommandPalette::new(), &declared, &sources, |row| {
            let ran = ran.clone();
            let id = row.id.clone();
            Command::new(row.label.clone(), move || *ran.borrow_mut() = id.clone())
        });
        assert!(rows.iter().any(|row| row.id == "custom:ws:0"));
        assert_eq!(rows.len(), palette.label_signals().len());
        let mut palette = if named {
            palette.query_in(Some("custom"), "edit").default_open(true)
        } else {
            palette.query("!edit").default_open(true)
        };
        heca_grid_ui::dispatch(&mut palette, &Event::Widget(WidgetIntent::Activate));
        assert_eq!(*ran.borrow(), "custom:ws:0", "named={named}");
    }
}

#[test]
fn rpc_keeps_the_mode_and_query_on_the_existing_palette_action() {
    let catalog = ActionCatalog::with_builtins();
    for scope in ["command", "pane", "workspace", "unknown"] {
        let (intent, dock) = crate::rpc::parse_rpc(&format!(
            "command_palette mode={scope} query=nvim",
        ))
        .expect("the RPC line parses")
        .into_intent(&catalog)
        .expect("the existing catalog names both optional arguments");
        let args = intent.args.iter().map(|(key, value)| {
            let PropValue::Text(value) = value else { panic!("RPC argument is not text") };
            (key.clone(), value.clone())
        }).collect();
        assert_eq!(dock, None);
        assert_eq!(
            crate::input::resolve_action(&intent.action, &args),
            Some(WmAction::CommandPalette {
                mode: Some(scope.to_string()),
                query: Some("nvim".to_string()),
            }),
        );
    }
}

/// **A floating pane is in the list too** — the palette lists every pane a workspace holds,
/// not only the ones in its columns.
#[test]
fn a_floating_pane_has_a_row() {
    use heca_core::layout::{PaneId, Point, Rectangle, Size};
    let mut session = session_with_panes();
    let rect = Rectangle::new(Point::new(10.0, 10.0), Size::new(200.0, 100.0));
    let editing = &mut session.session.workspaces[0];
    assert!(editing.float_tiled_pane(PaneId(2), rect).is_some());
    assert!(
        editing
            .find_pane(PaneId(2))
            .is_some_and(|_| editing.floating_panes.len() == 1)
    );

    let rows = pane_entries(
        session.l(),
        &heca_config::programs::ProgramsConfig::default(),
        true,
        None,
    );
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["pane:1", "pane:2"]);
}

/// A pane row is named the way the sidebar names it, and is ranked by that **name** — the id
/// is minted from a counter that restarts every launch, so counting it would file today's
/// habits against tomorrow's different pane.
#[test]
fn a_pane_row_is_ranked_by_its_display_name_not_its_id() {
    let session = session_with_panes();
    let rows = pane_entries(
        session.l(),
        &heca_config::programs::ProgramsConfig::default(),
        true,
        None,
    );

    let running = find(&rows, "pane:1");
    assert_eq!(
        running.label, "zsh",
        "the name comes from the live runtime through pane_info_view — `title` is empty in a \
         running app, and reading it left every row blank",
    );
    assert_eq!(
        running.rank_id.as_deref(),
        Some("zsh"),
        "ranked by that name"
    );
    assert_eq!(
        running.description, "/tmp/project · Editing",
        "where it is — the workspace alone leaves several `zsh` rows identical",
    );

    // `custom_name` wins over the process-derived name, and with the setting on the program
    // follows it — exactly as in the sidebar. The icon is the program's, not a generic one.
    let renamed = find(&rows, "pane:2");
    assert_eq!(renamed.label, "logs (Neovim)");
    assert_eq!(renamed.icon, Some(Glyph::FileCode));
    assert_eq!(
        renamed.rank_id.as_deref(),
        Some("logs"),
        "ranked by the name alone"
    );
    assert_eq!(renamed.description, "Editing", "no cwd: just the workspace");

    match renamed.intent {
        InteractionIntent::FocusPane { pane_id } => {
            assert_eq!(
                pane_id,
                heca_core::layout::PaneId(2),
                "runs FocusPane on that pane"
            );
        }
        ref other => panic!("expected FocusPane, got {other:?}"),
    }
}

/// **Exactly one row is the one you are on**, in each list — the accent means nothing if two
/// rows wear it.
#[test]
fn the_focused_pane_and_the_active_workspace_are_marked_current() {
    let session = session_with_panes();

    let panes = pane_entries(
        session.l(),
        &heca_config::programs::ProgramsConfig::default(),
        true,
        None,
    );
    let current: Vec<&str> = panes
        .iter()
        .filter(|r| r.current)
        .map(|r| r.id.as_str())
        .collect();
    assert_eq!(current.len(), 1, "one pane is current, got {current:?}");

    let workspaces = workspace_entries(session.l(), None);
    assert!(find(&workspaces, "ws:0").current, "workspace 0 is active");
    assert!(!find(&workspaces, "ws:1").current, "and it is the only one");
}

/// **Only a named workspace has an identity to rank.** An index is reused, so a persisted count
/// would attach itself to whatever workspace later takes that slot.
#[test]
fn only_a_named_workspace_carries_a_rank_id() {
    let session = session_with_panes();
    let rows = workspace_entries(session.l(), None);

    let named = find(&rows, "ws:0");
    assert_eq!(named.label, "Editing");
    assert_eq!(named.rank_id.as_deref(), Some("Editing"));

    let unnamed = find(&rows, "ws:1");
    assert_eq!(unnamed.label, "Workspace 2", "its position, spelled out");
    assert_eq!(
        unnamed.rank_id, None,
        "no durable identity: 'Workspace 2' is the index, and the index is reused",
    );
    match unnamed.intent {
        InteractionIntent::ActivateAction(WmAction::FocusWorkspace { ws_idx }) => {
            assert_eq!(ws_idx, 1);
        }
        ref other => panic!("expected FocusWorkspace, got {other:?}"),
    }
}

/// The action's own listing must not change: it is still offered, still runs bare, and both
/// new arguments are optional — a required one would take it out of the palette entirely.
#[test]
fn command_palette_stays_bindable_and_offerable_with_no_arguments() {
    use crate::input::{WmAction, resolve_action};
    assert_eq!(
        resolve_action("command_palette", &std::collections::HashMap::new()),
        Some(WmAction::CommandPalette {
            mode: None,
            query: None
        }),
        "the bare name still parses, and to the same behaviour",
    );
    let catalog = ActionCatalog::with_builtins();
    let meta = catalog
        .all()
        .find(|m| m.name == "command_palette")
        .expect("the action is declared");
    assert!(
        !meta.args.iter().any(|a| a.required),
        "both arguments are optional — a required one is filtered out of the palette",
    );
}

/// An action whose only required argument is a word list is offered **once per word** — and
/// once per *distinct* word: a region's three spellings are one command, not three.
#[test]
fn a_word_list_argument_is_offered_once_per_distinct_word() {
    let catalog = ActionCatalog::with_builtins();
    let rows = entries(
        &catalog,
        &std::collections::HashMap::new(),
        &crate::chrome::ActionShortcuts::default(),
    );
    let regions: Vec<&str> = rows
        .iter()
        .filter(|r| r.id.starts_with("set_region_visible "))
        .map(|r| r.label.as_str())
        .collect();
    assert_eq!(
        regions,
        [
            "Set Region Visibility › sidebar.left",
            "Set Region Visibility › sidebar.right",
            "Set Region Visibility › bar.top",
            "Set Region Visibility › bar.bottom",
        ],
    );
    let row = rows
        .iter()
        .find(|r| r.id == "set_region_visible region=bar.top")
        .expect("the top bar is offered");
    let InteractionIntent::View(intent) = &row.intent else {
        panic!("a built-in runs as a plain intent, got {:?}", row.intent);
    };
    assert_eq!(intent.action, "set_region_visible");
    assert_eq!(
        intent.args.get("region"),
        Some(&PropValue::Text("bar.top".into()))
    );
    // A target that is not a word list is still not offered at all.
    assert!(!rows.iter().any(|r| r.id.starts_with("close_pane_by_id")));
}

/// A built-in is the app's own: no scope prefix, and no container to focus.
#[test]
fn a_builtin_carries_no_scope_and_no_focus_step() {
    let catalog = ActionCatalog::with_builtins();
    let rows = entries(
        &catalog,
        &std::collections::HashMap::new(),
        &crate::chrome::ActionShortcuts::default(),
    );
    let entry = find(&rows, "close");
    assert!(!entry.label.contains('›'), "no component owns `close`");
    assert!(matches!(entry.intent, InteractionIntent::View(_)));
}

/// The focused component's actions sort first — and **nothing is hidden**: an unfocused
/// component's actions are still listed, because choosing one focuses it.
#[test]
fn the_focused_components_actions_come_first_and_the_rest_stay_listed() {
    let mut catalog = ActionCatalog::with_builtins();
    component_action(&mut catalog, "docker.restart", "Restart", "docker");
    component_action(&mut catalog, "ws.delete", "Delete Row", "workspaces");
    let owners = std::collections::HashMap::from([
        (
            "docker.restart".to_string(),
            owner("docker", "Containers", false),
        ),
        (
            "ws.delete".to_string(),
            owner("workspaces", "Workspaces", true),
        ),
    ]);

    let rows = entries(&catalog, &owners, &crate::chrome::ActionShortcuts::default());
    assert_eq!(rows[0].id, "ws.delete", "the focused component leads");
    assert!(
        rows.iter().any(|e| e.id == "docker.restart"),
        "an unfocused component's actions are listed, not filtered out",
    );
    // Everything else keeps the catalog's own order behind them.
    let builtins: Vec<&str> = rows.iter().skip(1).take(2).map(|e| e.id.as_str()).collect();
    assert_eq!(
        builtins,
        catalog
            .all()
            .filter(|m| m.owner.is_none())
            .take(2)
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
    );
}
