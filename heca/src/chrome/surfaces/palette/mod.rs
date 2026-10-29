//! The **command palette** — every registered action, searchable, from one key.
//!
//! The palette is not a second action system: it is a *view* of the one
//! [`ActionCatalog`](crate::actions::ActionCatalog), rendered by the library's
//! [`CommandPalette`] widget and dispatched through the one door
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
//! It is built like [`open_dropdown`](crate::chrome::open_dropdown), the app's other list overlay:
//! a widget pushed as an `Overlay`-band layer whose chosen entry comes back as a `SubmitOverlay`
//! resolved by the overlay host, so the entry's intent is policy-routed and confirm-gated like any
//! other dispatch.

use heca_grid_ui::reactive::{SignalGet, SignalUpdate, create_effect};
use heca_grid_ui::widgets::{Command, CommandPalette, Glyph};

use crate::app::interaction::{InteractionIntent, InteractionSource, dispatch_intent};
use crate::app_state::AppState;
use crate::chrome::{ChromeIntentEmitter, LayerKind, ModalResult, OverlayId};
use crate::chrome::{Intent, PropValue};
use crate::input::WmAction;

mod actions;
mod model;
mod modes;
mod targets;

use actions::{entries, group_of};
use model::{OwnerInfo, PaletteEntry};
use targets::{pane_entries, workspace_entries};
use crate::chrome::{ActionShortcuts, home_relative_path, pane_info_view};

/// Which component owns each declared action right now, and where it would land.
///
/// The owner *kind* is on the action's metadata (stamped at registration); the **placement** is
/// decided here, per open, by `owning_mount` — the same rule a click inside the component and an
/// RPC line without `--dock` use. An action whose component has since unmounted resolves to nothing
/// and is simply not offered.
pub(crate) fn owners(state: &AppState) -> std::collections::HashMap<String, OwnerInfo> {
    let focused = crate::app::tree_focus::focused_dock(state);
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

/// Open the command palette, optionally in a mode and/or with a query already typed.
///
/// The two **compose into one prefilled query** — `mode=pane, query="nvim"` is `"@nvim"` — because
/// the sigil is the mode selector. The widget resolves the name from its own mode table.
pub(crate) fn open_command_palette(
    state: &mut AppState,
    mode: Option<&str>,
    query: Option<&str>,
) -> OverlayId {
    let id = OverlayId(state.layers.reserve_id());
    let source = InteractionSource::Keyboard;

    let emit = ChromeIntentEmitter::new(&state.event_proxy, source);

    let owners = owners(state);
    let palette = CommandPalette::new()
        .placeholder("Type a command…")
        // The app's search memory, so what was searched and chosen outlives this palette — it is
        // built from scratch on every open.
        .search(
            heca_grid_ui::search::SearchModel::new(modes::MODE_COMMAND, state.search_store.clone())
                // The user's `[settings] search_case`, as the library's own vocabulary.
                .case(match state.settings.search_case {
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
        .panel_size(match state.settings.command_palette_size {
            heca_config::settings::PaletteSize::Small => heca_grid_ui::WidgetSize::Small,
            heca_config::settings::PaletteSize::Normal => heca_grid_ui::WidgetSize::Normal,
            heca_config::settings::PaletteSize::Large => heca_grid_ui::WidgetSize::Large,
        });
    let sources = modes::RowSources {
        catalog: &state.action_catalog,
        owners: &owners,
        shortcuts: &state.action_shortcuts,
        layout: state.layout(),
        programs: &state.client.programs,
        add_process_name: state.settings.pane_renamed_add_process_name,
        last_focused: state.last_focused,
        last_visited: state.last_visited_ws_idx,
    };
    let (palette, rows) = modes::populate(palette, modes::MODES, &sources, |row| {
        let mut command = Command::new(row.label.clone(), id.submitter(row.id.clone(), &emit))
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
        if let Some(glyph) = row.icon {
            command = command.icon(glyph);
        }
        // One keycap row per binding, each split into the tokens a cap is drawn per (`λ`, `⇧`, `e`)
        // by the same seam that formatted it.
        for chord in &row.shortcuts {
            command = command.keys(crate::shortcut::chord_caps(chord));
        }
        command
    });
    let palette = palette.query_in(mode, query.unwrap_or_default()).default_open(true);

    // **A pane row follows its pane.** The chrome store already mirrors each pane's `program` and
    // `custom_name` as signals, and the row's title is a signal too, so this is one effect per pane
    // row and nothing else: rename a pane or start `nvim` in it and that row's text changes in
    // place, with no rebuild and no flash. The widget ranks off the same signals, so a row renamed
    // this way is also *found* by its new name and not the one it was built with.
    let programs = state.client.programs.clone();
    let add_process_name = state.settings.pane_renamed_add_process_name;
    for (i, row) in rows.iter().enumerate() {
        let Some(pane) = row.pane else { continue };
        let Some((program, custom_name)) = state
            .chrome_state
            .panes
            .with(pane, |signals| signals.map(|s| (s.program, s.custom_name)))
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
            let info = pane_info_view(
                &programs,
                "",
                custom_name.get().as_deref(),
                Some(&runtime),
                add_process_name,
            );
            label.set(info.label());
        });
    }

    // The widget closes **itself** — on Esc, on a click outside, and after running a command — by
    // flipping its own open signal. That flip is the one dismissal signal there is, so the layer is
    // popped from it rather than from three separate callbacks. `resolve` is double-resolve safe, so
    // the close that follows a chosen command is a no-op after the `SubmitOverlay` already popped it.
    let open = palette.open_signal();
    let close = id.closer(&emit);
    create_effect(move |_| {
        if !open.get() {
            close();
        }
    });

    // Modal + covering: an open palette captures the keyboard for the whole viewport (that is how
    // typing reaches its query line through the widget-keymap path) and nothing behind it should be
    // acted on — `Domain::Overlay` says so for policy, `lock` is the fact it reads.
    let parent = state.layers.current();
    state.layers.insert(
        id.0,
        parent,
        LayerKind::OnDemand,
        heca_grid_ui::WidgetModel::new(palette),
        &mut state.window_root,
    );

    state
        .overlays
        .on_resolve(id, move |state, registry, result| {
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
mod tests;
