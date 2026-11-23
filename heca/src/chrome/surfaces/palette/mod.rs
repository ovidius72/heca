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
mod notifications;
mod sessions;
mod targets;
mod windows;
pub(crate) use modes::MODE_NOTIFICATION;
pub(crate) use sessions::open_session_folder;

use crate::chrome::{ActionShortcuts, home_relative_path, pane_info_view};
use actions::{entries, group_of};
pub(crate) use model::OpenPalette;
use model::{OwnerInfo, PaletteEntry};
use targets::{pane_entries, workspace_entries};

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
/// A prepared palette supplies different content through the same presentation and dismissal path.
pub(crate) fn open_command_palette(
    state: &mut AppState,
    mode: Option<&str>,
    query: Option<&str>,
    custom: Option<CommandPalette>,
) -> OverlayId {
    let id = OverlayId(state.layers.reserve_id());
    let source = InteractionSource::Keyboard;

    let emit = ChromeIntentEmitter::new(&state.event_proxy, source);

    // **The notifications list is the server's, and is asked for every time its mode is entered.**
    // An answer is held by the client until the palette it was asked for is built, and read here
    // once: a palette built from it already has what it asked for, any other palette's notifications
    // mode starts empty and fills when the request it makes on entering comes back.
    let answered = custom
        .is_none()
        .then(|| state.client.notification_history.take())
        .flatten();
    // A palette already up, entered the mode itself (typed `!`) and asked: the answer **replaces**
    // it, with whatever was typed meanwhile.
    let replaced = answered.as_ref().and_then(|_| {
        state
            .open_palette
            .take()
            .filter(|up| up.open.get_untracked())
    });
    // What was typed is carried over only if it was typed in the notifications mode; a palette that
    // was in some other mode when the answer came (the key was pressed over it) opens in the mode.
    let sigil = modes::MODES
        .iter()
        .find(|mode| mode.scope == modes::MODE_NOTIFICATION)
        .map(|mode| mode.sigil);
    let carried = replaced
        .as_ref()
        .map(|up| up.query.get_untracked())
        .filter(|typed| typed.chars().next() == sigil);
    if let Some(up) = &replaced {
        up.id.closer(&emit)();
    }

    let (palette, rows) = if let Some(palette) = custom {
        (palette, Vec::new())
    } else {
        standard_palette(state, id, &emit, answered)
    };
    let palette = match carried {
        Some(typed) => palette.query(typed),
        None => palette.query_in(mode, query.unwrap_or_default()),
    };
    let palette =
        palette
            .default_open(true)
            .panel_size(match state.settings.command_palette_size {
                heca_config::settings::PaletteSize::Small => heca_grid_ui::WidgetSize::Small,
                heca_config::settings::PaletteSize::Normal => heca_grid_ui::WidgetSize::Normal,
                heca_config::settings::PaletteSize::Large => heca_grid_ui::WidgetSize::Large,
            });

    // **A pane row follows its pane.** The chrome store already mirrors each pane's `program` and
    // `custom_name` as signals, and the row's title is a signal too, so this is one effect per pane
    // row and nothing else: rename a pane or start `nvim` in it and that row's text changes in
    // place, with no rebuild and no flash. The widget ranks off the same signals, so a row renamed
    // this way is also *found* by its new name and not the one it was built with.
    let programs = state.client.programs.clone();
    let add_process_name = state.settings.pane_renamed_add_process_name;
    for (i, row) in rows.iter().enumerate() {
        let Some(pane) = row.pane else { continue };
        let Some(signals) = state
            .chrome_state
            .panes
            .with(pane, |signals| signals.copied())
        else {
            continue;
        };
        let label = palette.label_signals()[i];
        let programs = programs.clone();
        create_effect(move |_| {
            let runtime = signals.presentation();
            let info = pane_info_view(
                &programs,
                "",
                signals.custom_name.get().as_deref(),
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
    state.open_palette = Some(OpenPalette {
        id,
        query: palette.query_signal(),
        open,
    });
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
                && let Some(intent) = &entry.intent
            {
                let _ = dispatch_intent(state, registry, source, intent.clone());
            }
            // Dynamic and fixed rows persist through the same search-store revision check.
            crate::search_state::persist_if_changed(state);
        });
    state.needs_redraw = true;
    id
}

fn standard_palette(
    state: &AppState,
    id: OverlayId,
    emit: &ChromeIntentEmitter,
    answered: Option<Vec<crate::notification::NotificationRecord>>,
) -> (CommandPalette, Vec<PaletteEntry>) {
    let owners = owners(state);
    let palette = CommandPalette::new()
        .placeholder("Type a command…")
        .search(search_model(
            state,
            modes::MODE_COMMAND,
            state.settings.search_case,
        ));
    let session_rows = sessions::entries(
        &state.client.sessions,
        &state.client.session,
        &state.settings.session_text,
    );
    let follow_rows = windows::entries(
        &state.windows,
        state.client.client_id,
        state.client.following(),
        &state.settings.session_text.stop_following,
    );
    let notification_rows =
        notifications::entries(answered.as_deref().unwrap_or_default(), unix_now());
    let sources = modes::RowSources {
        sessions: &session_rows,
        follow: &follow_rows,
        notifications: &notification_rows,
        catalog: &state.action_catalog,
        owners: &owners,
        shortcuts: &state.action_shortcuts,
        layout: state.layout(),
        programs: &state.client.programs,
        panes: &state.chrome_state.panes,
        add_process_name: state.settings.pane_renamed_add_process_name,
        last_focused: state.last_focused,
        last_visited: state
            .last_visited_ws
            .and_then(|ws| state.session.index_of(ws)),
    };
    let enter = entering(emit, answered.is_some());
    modes::populate(
        palette,
        modes::MODES,
        &sources,
        |row| {
            row_command(
                row,
                group_of(row, &owners),
                id.submitter(row.id.clone(), emit),
            )
        },
        enter,
    )
}

/// What the palette shows for one row, whichever mode it is in — the one place a [`PaletteEntry`]
/// becomes a [`Command`]. `on_run` is what choosing it does.
fn row_command(row: &PaletteEntry, group: u16, on_run: impl Fn() + 'static) -> Command {
    let mut command = Command::new(row.label.clone(), on_run)
        .group(group)
        .current(row.current)
        .preselect(row.preselect)
        .description(row.description.clone());
    if row.description_path {
        command = command.path(row.description.clone());
    }
    if let Some(detail) = &row.detail {
        command = command.detail(detail.clone());
    }
    // Rank against the stable choice identity, not the row's presentation id.
    if let Some(rank_id) = &row.rank_id {
        command = command.id(rank_id.clone());
    }
    if let Some(glyph) = row.icon {
        command = command.icon(glyph);
    }
    for chord in &row.shortcuts {
        command = command.keys(crate::shortcut::chord_caps(chord));
    }
    command
}

/// What a palette runs when a mode with an `on_enter` action is entered: fire that action.
///
/// `answered` is true for a palette built from the answer to that very question: entering its mode
/// the first time must not ask again, or the answer would replace itself for ever. Every later
/// entry — leaving the mode and typing its sigil again — asks.
fn entering(emit: &ChromeIntentEmitter, answered: bool) -> impl Fn(WmAction) -> Box<dyn Fn()> {
    let emit = emit.clone();
    let asked = std::rc::Rc::new(std::cell::Cell::new(answered));
    move |action| {
        let (emit, asked) = (emit.clone(), asked.clone());
        Box::new(move || {
            if !asked.replace(false) {
                emit.fire(InteractionIntent::ActivateAction(action.clone()));
            }
        })
    }
}

/// The wall clock in whole seconds since the Unix epoch — what a notification's raised time is
/// measured against.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn search_model(
    state: &AppState,
    scope: &str,
    case: heca_config::settings::SearchCase,
) -> heca_grid_ui::search::SearchModel {
    use heca_grid_ui::search::MatchCase;
    heca_grid_ui::search::SearchModel::new(scope, state.search_store.clone()).case(match case {
        heca_config::settings::SearchCase::Smart => MatchCase::Smart,
        heca_config::settings::SearchCase::Sensitive => MatchCase::Sensitive,
        heca_config::settings::SearchCase::Insensitive => MatchCase::Insensitive,
    })
}

#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod tests;
