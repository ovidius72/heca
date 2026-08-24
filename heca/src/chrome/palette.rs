//! The **command palette** — every registered action, searchable, from one key (F003/P085/T358).
//!
//! The palette is not a second action system: it is a *view* of the one
//! [`ActionCatalog`](crate::actions::ActionCatalog), rendered by the library's
//! [`CommandPalette`](heca_grid_ui::CommandPalette) widget and dispatched through the one door
//! ([`dispatch_intent`]). Everything an entry shows — icon, label, description, shortcut — is read
//! from the action's metadata, so a rebind, a new plugin action or a relabelled built-in reaches the
//! palette without a line being written here.
//!
//! **What it lists: everything registered.** Registration follows mounting, so an action belonging
//! to a component that is not mounted does not exist to be listed — no filter, it falls out. A
//! mounted-but-unfocused component's actions are shown, scoped by their component's title
//! (`"Workspaces › Delete Row"`), and choosing one **focuses that component and then performs**,
//! which is exactly what [`ActionPolicy::ContainerFocused`](crate::app::interaction::ActionPolicy)
//! requires — the precedent being `FocusPaneThenAction`, which pane-header buttons have used for the
//! same problem all along.
//!
//! It is built like [`open_dropdown`](super::overlay::open_dropdown), the app's other list overlay:
//! a widget pushed as an `Overlay`-band layer whose chosen entry comes back as a `SubmitOverlay`
//! resolved by the overlay host, so the entry's intent is policy-routed and confirm-gated like any
//! other dispatch.


use heca_grid_ui::reactive::{create_effect, SignalGet, SignalUpdate};
use heca_grid_ui::widgets::{Command, CommandPalette, Glyph};

use super::{ChromeIntentEmitter, LayerKind, ModalResult, OverlayId};
use crate::app::interaction::{dispatch_intent, InteractionIntent, InteractionSource};
use crate::app_state::AppState;
use crate::chrome::Intent;
use crate::input::WmAction;

/// One row of the palette: what it shows, and what it runs.
///
/// Assembled in [`entries`] from the catalog and resolved once at open time rather than per
/// keystroke — the widget re-filters the list on every character, and re-deriving which placement
/// owns an action inside that loop would make the answer depend on how fast someone types.
#[derive(Debug, Clone)]
pub(crate) struct PaletteEntry {
    /// The entry's identity for the overlay round-trip, unique across **every** mode — an action's
    /// name, or `"pane:3"` / `"ws:0"`. It is not what frecency counts; see
    /// [`rank_id`](Self::rank_id).
    pub(crate) id: String,
    /// What past use is counted against, when the entry has a durable identity at all.
    ///
    /// Separate from [`id`](Self::id) because the two answer different questions, and for actions
    /// they happened to be the same string. A `PaneId` restarts at 1 every launch and a workspace
    /// index is reused, so counting either would attach yesterday's habits to a **different** pane
    /// or workspace — the name is the only part that means the same thing tomorrow. `None` for an
    /// unnamed workspace, which has nothing durable to be: the row still matches and sorts, it
    /// simply carries no boost.
    pub(crate) rank_id: Option<String>,
    /// Which mode lists it — the widget's scope name, `None` for the default (actions) mode.
    pub(crate) mode: Option<String>,
    /// The pane this row stands for, when it is a pane row — the handle used to keep its label
    /// following the pane's live name while the palette is open.
    pub(crate) pane: Option<heca_core::layout::PaneId>,
    /// Is this the pane/workspace the user is already on? Drawn in the accent, so a list of
    /// near-identical names still says where you are standing before you pick where to go.
    pub(crate) current: bool,
    /// Should the selection start here? The pane/workspace you were **last** in — so Enter takes
    /// you back where you just were, instead of re-focusing what is already focused.
    pub(crate) preselect: bool,
    /// What the user reads: the action's label, prefixed with its component's title when it belongs
    /// to one (`"Workspaces › Delete Row"`).
    pub(crate) label: String,
    /// The action's description — the muted second line.
    pub(crate) description: String,
    pub(crate) icon: Option<Glyph>,
    /// Every binding of the action, already display-formatted (`"λ ⇧ e"`), one per row of keycaps —
    /// empty when unbound. The list, not a joined line: an action bound twice shows two rows.
    pub(crate) shortcuts: Vec<String>,
    /// What choosing it dispatches. A built-in is a plain `View` intent; a component's action is
    /// aimed at the placement that owns it, focus first.
    pub(crate) intent: InteractionIntent,
}

/// What is known about the component owning an action, at the moment the palette opens.
///
/// Resolved from the host by [`owners`] so [`entries`] stays a pure function of its inputs — it can
/// be tested without an `AppState`, which cannot be built without a window.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OwnerInfo {
    /// The mount the action would land on — `owning_mount`'s answer, the one rule shared with the
    /// `perform` bridge and RPC.
    pub(crate) mount: String,
    /// The component's own title, as it names itself. The palette never invents a scope name.
    pub(crate) title: String,
    /// Does this mount currently hold chrome focus? Its actions sort first.
    pub(crate) focused: bool,
}

/// Build the palette's rows from the catalog — **pure**, so the ordering and labelling rules are
/// testable without a window.
///
/// `owners` maps an action name to the placement that would receive it; an action absent from the
/// map is the app's own. The order is: the focused component's actions first, then everything else
/// in the catalog's stable order. Nothing is hidden — an action whose component is mounted but not
/// focused is listed exactly like the rest, because choosing it focuses that component first.
pub(crate) fn entries(
    catalog: &crate::actions::ActionCatalog,
    owners: &std::collections::HashMap<String, OwnerInfo>,
    shortcuts: &super::ActionShortcuts,
) -> Vec<PaletteEntry> {
    let mut rows: Vec<(bool, PaletteEntry)> = catalog
        .all()
        .filter(|meta| offerable(meta))
        .map(|meta| {
            let owner = owners.get(&meta.name);
            let label = match owner {
                Some(o) => format!("{} › {}", o.title, meta.label),
                None => meta.label.clone(),
            };
            let intent = match owner {
                Some(o) => InteractionIntent::FocusContainerThenAction {
                    container: o.mount.clone(),
                    action: Intent::new(&meta.name),
                },
                None => InteractionIntent::View(Intent::new(&meta.name)),
            };
            (
                owner.is_some_and(|o| o.focused),
                PaletteEntry {
                    id: meta.name.clone(),
                    // For an action the two coincide: a name is both unique and durable.
                    rank_id: Some(meta.name.clone()),
                    mode: None,
                    pane: None,
                    current: false,
                    preselect: false,
                    label,
                    description: meta.description.clone(),
                    // Through the catalog, not off the meta: the catalog answers with the action's
                    // own glyph *or its category's*, so no row is ever blank and the palette makes
                    // no decision of its own about iconography.
                    icon: catalog.icon(&meta.name),
                    shortcuts: shortcuts.chords(&meta.name).to_vec(),
                    intent,
                },
            )
        })
        .collect();
    // Stable: within each group the catalog's own order survives.
    rows.sort_by_key(|(focused, _)| !*focused);
    rows.into_iter().map(|(_, entry)| entry).collect()
}

/// The scope name of the pane mode — the search memory panes are filed under, and the `scope=` a
/// `clear_search_history` line names.
pub(crate) const MODE_PANE: &str = "pane";
/// The scope name of the workspace mode.
pub(crate) const MODE_WORKSPACE: &str = "workspace";

/// Every open pane, as palette rows — **pure**, over the session, so the naming and the rank ids
/// are testable without an `AppState` (which needs a window).
///
/// A pane's name is whatever [`pane_info_view`](super::pane_info_view) says it is — **the one name
/// composer**, the same call the sidebar card and the pane header make.
///
/// `Pane::title` is *not* that name. It is a fallback, and in a running app it is empty: the name
/// on screen is resolved from the live `PaneRuntime`'s program through
/// [`ProgramsConfig`](heca_config::programs::ProgramsConfig) (so `v` reads as `nvim`), with a
/// `custom_name` overriding it. Reading the field directly produced a pane list of blank rows while
/// the sidebar beside it read `zsh` — the two names have to come from one place.
pub(crate) fn pane_entries(
    session: &heca_core::layout::Session,
    programs: &heca_config::programs::ProgramsConfig,
    last_focused: Option<heca_core::layout::PaneId>,
) -> Vec<PaletteEntry> {
    // The pane you are standing in: the active workspace's active pane. Only one row in the whole
    // list can be it, which is what makes the accent mean something.
    let focused = session.active_workspace().and_then(|ws| ws.active_pane()).map(|p| p.id);
    let mut rows = Vec::new();
    for (ws_idx, ws) in session.workspaces.iter().enumerate() {
        let ws_name = workspace_name(ws, ws_idx);
        for col in ws.scrolling.columns.iter() {
            for pane in col.panes.iter() {
                let info = super::pane_info_view(
                    programs,
                    &pane.title,
                    pane.custom_name.as_deref(),
                    Some(&pane.runtime),
                    // The `(process)` suffix is a sidebar affordance and lands in `process_hint`,
                    // never in the title — so this only says the palette does not want it.
                    false,
                );
                rows.push(PaletteEntry {
                    id: format!("pane:{}", pane.id.0),
                    // The **name**, not the id: `PaneId` is minted from a counter that restarts at
                    // 1 every launch, so a persisted count would rank a pane that no longer exists.
                    rank_id: Some(info.title.clone()),
                    mode: Some(MODE_PANE.to_string()),
                    pane: Some(pane.id),
                    current: Some(pane.id) == focused,
                    preselect: Some(pane.id) == last_focused,
                    label: info.title,
                    // Where it is. The workspace alone leaves several `zsh` rows identical, which
                    // is exactly the list this is meant to pick from, so the working directory
                    // leads when there is one — the same `~/…` the pane header shows.
                    description: match pane.runtime.cwd.as_deref() {
                        Some(cwd) => {
                            format!("{} · {ws_name}", super::home_relative_path(cwd))
                        }
                        None => ws_name.clone(),
                    },
                    icon: Some(Glyph::Terminal),
                    shortcuts: Vec::new(),
                    intent: InteractionIntent::FocusPane { pane_id: pane.id },
                });
            }
        }
    }
    rows
}

/// Every workspace, as palette rows — pure, for the same reason as [`pane_entries`].
pub(crate) fn workspace_entries(
    session: &heca_core::layout::Session,
    last_visited: Option<usize>,
) -> Vec<PaletteEntry> {
    session
        .workspaces
        .iter()
        .enumerate()
        .map(|(ws_idx, ws)| PaletteEntry {
            id: format!("ws:{ws_idx}"),
            // **Only a named workspace has an identity.** An index is reused, so a persisted count
            // would attach itself to whatever workspace later takes that slot — actively wrong
            // rather than merely useless. `"Workspace 3"` is that index spelled out and is no more
            // durable than the index itself.
            rank_id: ws.name.clone(),
            mode: Some(MODE_WORKSPACE.to_string()),
            pane: None,
            current: ws_idx == session.active_workspace_idx,
            preselect: Some(ws_idx) == last_visited,
            label: workspace_name(ws, ws_idx),
            description: match ws.scrolling.columns.len() {
                1 => "1 column".to_string(),
                n => format!("{n} columns"),
            },
            icon: Some(Glyph::Cards),
            shortcuts: Vec::new(),
            intent: InteractionIntent::ActivateAction(WmAction::FocusWorkspace { ws_idx }),
        })
        .collect()
}

/// What a workspace is called: its own name, or its position spelled out — the sidebar's rule.
fn workspace_name(ws: &heca_core::layout::Workspace, ws_idx: usize) -> String {
    ws.name
        .clone()
        .unwrap_or_else(|| format!("Workspace {}", ws_idx + 1))
}

/// Which block an entry sorts into **while the palette's query is empty** — 0 for the focused
/// component's actions, 1 for everything else.
///
/// The focused-first rule (P085) exists so that focusing a component puts *its* verbs at hand, and
/// with nothing typed that focus is the only context there is. Once something is typed the query is
/// better context and the widget drops the blocks entirely — an action whose name the user spelled
/// correctly must not sit below one they did not, merely because its component holds focus.
fn group_of(entry: &PaletteEntry, owners: &std::collections::HashMap<String, OwnerInfo>) -> u16 {
    match owners.get(&entry.id) {
        Some(o) if o.focused => 0,
        _ => 1,
    }
}

/// Can this action be **invoked with no arguments**? Only those belong in a palette.
///
/// A palette offers a name and nothing else, so an action that requires arguments cannot be run
/// from one — and listing it is worse than useless, because it looks like a working command and does
/// nothing. Thirty-seven built-ins are in that category: every by-id / by-index form
/// (`close_pane_by_id`, `add_pane_to_column`, `move_column`, `float_at`, …). They are not
/// duplicates to be pruned by hand — each is the **targeted** form of an act whose unit form is
/// already listed, and it exists for the surfaces that *do* carry a target: a header button, a
/// context-menu entry, a drag, an RPC line with arguments.
///
/// This is arity, **not policy**. What the current domain permits changes moment to moment and is
/// judged at dispatch (and a component's `ContainerFocused` verb is reached by focusing it first);
/// what an action *takes* is fixed at its declaration. So the rule reads the `args` every action is
/// already required to declare, and no hand-written list can drift from it.
///
/// The palette also does not offer the action that opens it.
fn offerable(meta: &crate::actions::ActionMeta) -> bool {
    meta.name != "command_palette" && !meta.args.iter().any(|arg| arg.required)
}

/// Which component owns each declared action right now, and where it would land.
///
/// The owner *kind* is on the action's metadata (stamped at registration); the **placement** is
/// decided here, per open, by `owning_mount` — the same rule a click inside the component and an
/// RPC line without `--dock` use. An action whose component has since unmounted resolves to nothing
/// and is simply not offered.
pub(crate) fn owners(state: &AppState) -> std::collections::HashMap<String, OwnerInfo> {
    let focused = state.chrome_state.focused_container();
    state
        .action_catalog
        .all()
        .filter(|meta| meta.owner.is_some())
        .filter_map(|meta| {
            let mount = crate::providers::owning_mount(state, &meta.name)?;
            let title = state.chrome_host.provider(&mount)?.title().to_string();
            Some((
                meta.name.clone(),
                OwnerInfo {
                    focused: focused.as_deref() == Some(mount.as_str()),
                    mount,
                    title,
                },
            ))
        })
        .collect()
}

/// Open the command palette as an overlay layer.
///
/// The source is [`InteractionSource::Keyboard`]: the palette is a keyboard surface, and what it
/// dispatches should be judged exactly as the key for that action would be — a `TiledOnly` entry
/// chosen while a pane floats is refused here too, rather than the palette becoming a way around
/// the policy.
/// The sigil that reaches `mode`, so the app composes the same query a user would type.
///
/// The one place the app's own table is written down; the widget is told the same pairs in
/// [`open_command_palette`]. An unknown mode name has no sigil and lands in the default list —
/// better than refusing to open at all for a typo in a config line.
fn sigil_for(mode: &str) -> Option<char> {
    match mode {
        MODE_PANE => Some('@'),
        MODE_WORKSPACE => Some('$'),
        "command" => Some(':'),
        _ => None,
    }
}

/// Open the command palette, optionally in a mode and/or with a query already typed.
///
/// The two **compose into one prefilled query** — `mode=pane, query="nvim"` is `"@nvim"` — because
/// the sigil is the mode selector. There is no second mechanism and the widget needs no "open in a
/// mode" entry point.
pub(crate) fn open_command_palette(
    state: &mut AppState,
    mode: Option<&str>,
    query: Option<&str>,
) -> OverlayId {
    let id = OverlayId(state.layers.reserve_id());
    let prefill = format!(
        "{}{}",
        mode.and_then(sigil_for).map(String::from).unwrap_or_default(),
        query.unwrap_or_default(),
    );
    let source = InteractionSource::Keyboard;

    let emit = ChromeIntentEmitter::new(&state.event_proxy, source);

    let owners = owners(state);
    let mut rows = entries(&state.action_catalog, &owners, &state.action_shortcuts);
    rows.extend(pane_entries(&state.session, &state.programs, state.last_focused));
    rows.extend(workspace_entries(&state.session, state.last_visited_ws_idx));
    let mut palette = CommandPalette::new()
        .placeholder("Type a command…")
        // The sigil **is** the mode selector, so opening in a mode is opening with a prefilled
        // query and there is no second mechanism. Actions are declared first, which makes them the
        // default — `:` is theirs only so the user can come back to them explicitly.
        .mode(':', "command")
        .mode('@', MODE_PANE)
        .mode('$', MODE_WORKSPACE)
        // The app's search memory, so what was searched and chosen outlives this palette — it is
        // built from scratch on every open.
        .search(
            heca_grid_ui::search::SearchModel::new("command", state.search_store.clone())
                // The user's `[settings] search_case`, as the library's own vocabulary.
                .case(match state.search_case {
                    heca_config::settings::SearchCase::Smart => {
                        heca_grid_ui::search::MatchCase::Smart
                    }
                    heca_config::settings::SearchCase::Sensitive => {
                        heca_grid_ui::search::MatchCase::Sensitive
                    }
                    heca_config::settings::SearchCase::Insensitive => {
                        heca_grid_ui::search::MatchCase::Insensitive
                    }
                }),
        )
        // The user's `[settings] command_palette_size`, as a semantic variant — the widget owns what
        // each one means in pixels.
        .panel_size(match state.command_palette_size {
            heca_config::settings::PaletteSize::Small => heca_grid_ui::WidgetSize::Small,
            heca_config::settings::PaletteSize::Normal => heca_grid_ui::WidgetSize::Normal,
            heca_config::settings::PaletteSize::Large => heca_grid_ui::WidgetSize::Large,
        });
    for row in &rows {
        let carrier = InteractionIntent::ActivateAction(WmAction::SubmitOverlay {
            overlay: id,
            action: row.id.clone(),
        });
        let emit_run = emit.clone();
        let mut command = Command::new(row.label.clone(), move || emit_run.fire(carrier.clone()))
            .group(group_of(row, &owners))
            .current(row.current)
            .preselect(row.preselect)
            .description(row.description.clone());
        // The identity past choices are counted against — **not** the row's id. For an action they
        // are the same string; for a pane they are not, and assuming they were is what would file
        // a habit under a number that means something different tomorrow. An entry without one
        // still matches and sorts, it simply carries no boost.
        if let Some(rank_id) = &row.rank_id {
            command = command.id(rank_id.clone());
        }
        if let Some(mode) = &row.mode {
            command = command.mode(mode.clone());
        }
        if let Some(glyph) = row.icon {
            command = command.icon(glyph);
        }
        // One keycap row per binding, each split into the tokens a cap is drawn per (`λ`, `⇧`, `e`)
        // by the same seam that formatted it.
        for chord in &row.shortcuts {
            command = command.keys(crate::shortcut::chord_caps(chord));
        }
        palette = palette.command(command);
    }
    let palette = palette.query(prefill).open(true);

    // **A pane row follows its pane.** The chrome store already mirrors each pane's `program` and
    // `custom_name` as signals, and the row's title is a signal too, so this is one effect per pane
    // row and nothing else: rename a pane or start `nvim` in it and that row's text changes in
    // place, with no rebuild and no flash. The widget ranks off the same signals, so a row renamed
    // this way is also *found* by its new name and not the one it was built with.
    let programs = state.programs.clone();
    for (i, row) in rows.iter().enumerate() {
        let Some(pane) = row.pane else { continue };
        let Some((program, custom_name)) = state
            .chrome_state
            .workspaces
            .with_pane_runtime(pane, |signals| signals.map(|s| (s.program, s.custom_name)))
        else {
            continue;
        };
        let label = palette.label_signals()[i];
        let programs = programs.clone();
        create_effect(move |_| {
            // Only the program matters to the title; the rest of a `PaneRuntime` is read for the
            // fields the palette does not show.
            let runtime = heca_core::runtime::PaneRuntime {
                program: program.get(),
                ..Default::default()
            };
            let info = super::pane_info_view(
                &programs,
                "",
                custom_name.get().as_deref(),
                Some(&runtime),
                false,
            );
            label.set(info.title);
        });
    }

    // The widget closes **itself** — on Esc, on a click outside, and after running a command — by
    // flipping its own open signal. That flip is the one dismissal signal there is, so the layer is
    // popped from it rather than from three separate callbacks. `resolve` is double-resolve safe, so
    // the close that follows a chosen command is a no-op after the `SubmitOverlay` already popped it.
    let open = palette.open_signal();
    let emit_close = emit.clone();
    let close = InteractionIntent::ActivateAction(WmAction::CloseOverlay { overlay: Some(id) });
    create_effect(move |_| {
        if !open.get() {
            emit_close.fire(close.clone());
        }
    });

    // Modal + covering: an open palette captures the keyboard for the whole viewport (that is how
    // typing reaches its query line through the widget-keymap path) and nothing behind it should be
    // acted on — `Domain::Overlay` says so for policy, `covers_content` is the fact it reads.
    state.layers.insert(
        id.0,
        state.layers.current(),
        LayerKind::OnDemand,
        true,
        true,
        Box::new(palette),
    );

    state.overlays.on_resolve(id, move |state, registry, result| {
        if let ModalResult::Action { id: chosen, .. } = result
            && let Some(entry) = rows.iter().find(|e| e.id == chosen)
        {
            dispatch_intent(state, registry, source, entry.intent.clone());
            // Not the palette's business *what* was recorded or whether anything was: the shared
            // helper saves when the store says it changed, so any future search surface is
            // persisted by the same call without knowing this one exists.
            crate::search_state::persist_if_changed(state);
        }
    });
    state.needs_redraw = true;
    id
}

#[cfg(test)]
mod tests {
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
        component_action(&mut catalog, "docker.restart", "Restart Container", "docker");
        let owners = std::collections::HashMap::from([(
            "docker.restart".to_string(),
            owner("docker.right", "Containers", false),
        )]);

        let rows = entries(&catalog, &owners, &super::super::ActionShortcuts::default());
        let entry = find(&rows, "docker.restart");
        assert_eq!(entry.label, "Containers › Restart Container");
        assert_eq!(entry.description, "Restart Container — what it does.");
        match &entry.intent {
            InteractionIntent::FocusContainerThenAction { container, action } => {
                assert_eq!(container, "docker.right", "the placement, not the component");
                assert_eq!(action.action, "docker.restart");
            }
            other => panic!("expected the focus-then-act composite, got {other:?}"),
        }
    }

    /// A session with two named panes in one workspace, and a second, unnamed workspace.
    fn session_with_panes() -> heca_core::layout::Session {
        use heca_core::layout::{LayoutOptions, Pane, PaneId, Rectangle, SessionId, Size};
        let viewport = Size::new(800.0, 600.0);
        let mut session = heca_core::layout::Session::new(
            SessionId(0),
            viewport,
            1.0,
            LayoutOptions::default(),
        );
        session.workspaces[0].name = Some("Editing".to_string());
        // **The shape a running app actually has**: `title` empty, the name carried by the live
        // runtime. Reading `title` here produced a list of blank rows in the app while the sidebar
        // beside it read `zsh`, so the fixture has to be built this way or the test proves nothing.
        let mut running = Pane::new(PaneId(1), "");
        running.runtime.program = Some("zsh".to_string());
        running.runtime.cwd = Some(std::path::PathBuf::from("/tmp/project"));
        session.add_pane(running, None, false);
        // A renamed pane: `custom_name` overrides the process-derived name everywhere it is shown.
        let mut renamed = Pane::new(PaneId(2), "");
        renamed.runtime.program = Some("zsh".to_string());
        renamed.custom_name = Some("logs".to_string());
        session.add_pane(renamed, None, false);
        session.add_workspace(Rectangle::from_size(viewport));
        session
    }

    /// A pane row is named the way the sidebar names it, and is ranked by that **name** — the id
    /// is minted from a counter that restarts every launch, so counting it would file today's
    /// habits against tomorrow's different pane.
    #[test]
    fn a_pane_row_is_ranked_by_its_display_name_not_its_id() {
        let session = session_with_panes();
        let rows = pane_entries(&session, &heca_config::programs::ProgramsConfig::default(), None);

        let running = find(&rows, "pane:1");
        assert_eq!(
            running.label, "zsh",
            "the name comes from the live runtime through pane_info_view — `title` is empty in a \
             running app, and reading it left every row blank",
        );
        assert_eq!(running.rank_id.as_deref(), Some("zsh"), "ranked by that name");
        assert_eq!(running.mode.as_deref(), Some(MODE_PANE));
        assert_eq!(
            running.description, "/tmp/project · Editing",
            "where it is — the workspace alone leaves several `zsh` rows identical",
        );

        // `custom_name` wins over the process-derived name, exactly as in the sidebar.
        let renamed = find(&rows, "pane:2");
        assert_eq!(renamed.label, "logs");
        assert_eq!(renamed.rank_id.as_deref(), Some("logs"));
        assert_eq!(renamed.description, "Editing", "no cwd: just the workspace");

        match renamed.intent {
            InteractionIntent::FocusPane { pane_id } => {
                assert_eq!(pane_id, heca_core::layout::PaneId(2), "runs FocusPane on that pane");
            }
            ref other => panic!("expected FocusPane, got {other:?}"),
        }
    }

    /// **Exactly one row is the one you are on**, in each list — the accent means nothing if two
    /// rows wear it.
    #[test]
    fn the_focused_pane_and_the_active_workspace_are_marked_current() {
        let session = session_with_panes();

        let panes = pane_entries(&session, &heca_config::programs::ProgramsConfig::default(), None);
        let current: Vec<&str> = panes
            .iter()
            .filter(|r| r.current)
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(current.len(), 1, "one pane is current, got {current:?}");

        let workspaces = workspace_entries(&session, None);
        assert!(find(&workspaces, "ws:0").current, "workspace 0 is active");
        assert!(!find(&workspaces, "ws:1").current, "and it is the only one");
    }

    /// **Only a named workspace has an identity to rank.** An index is reused, so a persisted count
    /// would attach itself to whatever workspace later takes that slot.
    #[test]
    fn only_a_named_workspace_carries_a_rank_id() {
        let session = session_with_panes();
        let rows = workspace_entries(&session, None);

        let named = find(&rows, "ws:0");
        assert_eq!(named.label, "Editing");
        assert_eq!(named.rank_id.as_deref(), Some("Editing"));
        assert_eq!(named.mode.as_deref(), Some(MODE_WORKSPACE));

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

    /// A mode name composes with a query into the one prefilled string the widget reads — the
    /// sigil is the mode selector, so there is no second mechanism.
    #[test]
    fn a_mode_and_a_query_compose_into_one_prefilled_query() {
        let prefill = |mode: Option<&str>, query: Option<&str>| {
            format!(
                "{}{}",
                mode.and_then(sigil_for).map(String::from).unwrap_or_default(),
                query.unwrap_or_default(),
            )
        };
        assert_eq!(prefill(None, None), "", "bare is what it always was");
        assert_eq!(prefill(Some("pane"), None), "@", "a mode alone lists it whole");
        assert_eq!(prefill(Some("pane"), Some("nvim")), "@nvim");
        assert_eq!(prefill(Some("workspace"), Some("ed")), "$ed");
        assert_eq!(prefill(None, Some("close")), "close", "a query with no mode is a search");
        assert_eq!(
            prefill(Some("nonsense"), Some("x")), "x",
            "an unknown mode has no sigil and lands in the default list, rather than refusing to \
             open over a typo in a config line",
        );
    }

    /// The action's own listing must not change: it is still offered, still runs bare, and both
    /// new arguments are optional — a required one would take it out of the palette entirely.
    #[test]
    fn command_palette_stays_bindable_and_offerable_with_no_arguments() {
        use crate::input::{action_from_name, WmAction};
        assert_eq!(
            action_from_name("command_palette"),
            Some(WmAction::CommandPalette { mode: None, query: None }),
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

    /// A built-in is the app's own: no scope prefix, and no container to focus.
    #[test]
    fn a_builtin_carries_no_scope_and_no_focus_step() {
        let catalog = ActionCatalog::with_builtins();
        let rows = entries(
            &catalog,
            &std::collections::HashMap::new(),
            &super::super::ActionShortcuts::default(),
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
            ("docker.restart".to_string(), owner("docker", "Containers", false)),
            ("ws.delete".to_string(), owner("workspaces", "Workspaces", true)),
        ]);

        let rows = entries(&catalog, &owners, &super::super::ActionShortcuts::default());
        assert_eq!(rows[0].id, "ws.delete", "the focused component leads");
        assert!(
            rows.iter().any(|e| e.id == "docker.restart"),
            "an unfocused component's actions are listed, not filtered out",
        );
        // Everything else keeps the catalog's own order behind them.
        let builtins: Vec<&str> = rows
            .iter()
            .skip(1)
            .take(2)
            .map(|e| e.id.as_str())
            .collect();
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
}
