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

use std::rc::Rc;

use heca_grid_ui::reactive::{create_effect, SignalGet};
use heca_grid_ui::widgets::{Command, CommandPalette, Glyph};

use super::{ChromeIntentEmitter, LayerBand, LayerKind, ModalResult, OverlayId};
use crate::app::events::AppEvent;
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
    /// The action's name — the entry's identity when the choice comes back through the overlay.
    pub(crate) id: String,
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
pub(crate) fn open_command_palette(state: &mut AppState) -> OverlayId {
    let id = OverlayId(state.layers.reserve_id());
    let source = InteractionSource::Keyboard;

    let event_proxy = state.event_proxy.clone();
    let emit: ChromeIntentEmitter = Rc::new(move |intent| {
        let _ = event_proxy.send_event(AppEvent::ChromeIntent { source, intent });
    });

    let owners = owners(state);
    let rows = entries(&state.action_catalog, &owners, &state.action_shortcuts);
    let mut palette = CommandPalette::new()
        .placeholder("Type a command…")
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
        let mut command =
            Command::new(row.label.clone(), move || emit_run(carrier.clone()))
                // The action's name: the stable identity past choices are counted against. The
                // label cannot serve — it carries the owning component's title, so it changes with
                // mounting.
                .id(row.id.clone())
                .group(group_of(row, &owners))
                .description(row.description.clone());
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
    let palette = palette.open(true);

    // The widget closes **itself** — on Esc, on a click outside, and after running a command — by
    // flipping its own open signal. That flip is the one dismissal signal there is, so the layer is
    // popped from it rather than from three separate callbacks. `resolve` is double-resolve safe, so
    // the close that follows a chosen command is a no-op after the `SubmitOverlay` already popped it.
    let open = palette.open_signal();
    let emit_close = emit.clone();
    let close = InteractionIntent::ActivateAction(WmAction::CloseOverlay { overlay: id });
    create_effect(move |_| {
        if !open.get() {
            emit_close(close.clone());
        }
    });

    // Modal + covering: an open palette captures the keyboard for the whole viewport (that is how
    // typing reaches its query line through the widget-keymap path) and nothing behind it should be
    // acted on — `Domain::Overlay` says so for policy, `covers_content` is the fact it reads.
    state.layers.insert(
        id.0,
        LayerBand::Overlay,
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
