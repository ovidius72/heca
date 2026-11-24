//! heca's own palette lists, declared through `palette_list`, and the one loop that builds every
//! declared list into the palette.

use super::*;

pub(super) const MODE_COMMAND: &str = "command";
pub(super) const MODE_PANE: &str = "pane";
pub(super) const MODE_WORKSPACE: &str = "workspace";
pub(super) const MODE_FOLLOW: &str = "follow";
pub(super) const MODE_SESSION: &str = "session";
pub(crate) const MODE_NOTIFICATION: &str = "notification";

/// Plain data needed by the palette's row sources.
pub(super) struct RowSources<'a> {
    pub sessions: &'a [PaletteEntry],
    pub follow: &'a [PaletteEntry],
    pub notifications: &'a [PaletteEntry],
    pub catalog: &'a crate::actions::ActionCatalog,
    pub owners: &'a std::collections::HashMap<String, OwnerInfo>,
    pub shortcuts: &'a ActionShortcuts,
    pub layout: heca_core::layout::Layout<'a>,
    pub programs: &'a heca_config::programs::ProgramsConfig,
    pub panes: &'a crate::chrome::PaneRuntimeStore,
    /// The user's `[settings] pane_renamed_add_process_name`: a renamed pane also names its program.
    pub add_process_name: bool,
    pub last_focused: Option<heca_core::layout::PaneId>,
    pub last_visited: Option<usize>,
}

/// **heca's own lists, declared through the door every list uses** (`palette_list`) — once per
/// thread, and the names returned in their order: the first is the default list. An extension adding
/// to one of them (`palette_list("session").render_item(..)`) adds to the same declaration.
pub(super) fn declare_builtins() -> &'static [&'static str] {
    use super::lists::PaletteList;
    const OWN: &[&str] = &[
        MODE_COMMAND,
        MODE_PANE,
        MODE_WORKSPACE,
        MODE_SESSION,
        MODE_FOLLOW,
        MODE_NOTIFICATION,
    ];
    // Declared already when the first of them has its rows — asked of the registry itself, so a
    // registry cleared (a test) declares them again.
    if super::lists::has_host_rows(MODE_COMMAND) {
        return OWN;
    }
    PaletteList::new(MODE_COMMAND)
        .sigil(':')
        .host_rows(|sources| entries(sources.catalog, sources.owners, sources.shortcuts));
    PaletteList::new(MODE_PANE).sigil('@').host_rows(|sources| {
        pane_entries(
            sources.layout,
            sources.programs,
            sources.panes,
            sources.add_process_name,
            sources.last_focused,
        )
    });
    PaletteList::new(MODE_WORKSPACE)
        .sigil('$')
        .host_rows(|sources| workspace_entries(sources.layout, sources.last_visited));
    // A session is found by its name or by its folder.
    PaletteList::new(MODE_SESSION)
        .sigil('#')
        .search_descriptions(true)
        .host_rows(|sources| sources.sessions.to_vec());
    PaletteList::new(MODE_FOLLOW)
        .sigil('~')
        .host_rows(|sources| sources.follow.to_vec());
    // A notification is found by its title or by its body, and the list is the server's: every
    // entry into the list asks it again, so what is shown is never an old answer.
    PaletteList::new(MODE_NOTIFICATION)
        .sigil('!')
        .search_descriptions(true)
        .host_rows(|sources| sources.notifications.to_vec())
        .on_enter(|| WmAction::NotificationHistory);
    OWN
}

/// **Every declared list, built into the palette** — heca's rows from `sources`, an extension's from
/// its `items` — each shown through `command`, with what `enter` makes of a list's `on_enter`.
pub(super) fn populate(
    mut palette: CommandPalette,
    sources: &RowSources<'_>,
    command: &impl Fn(&PaletteEntry, &str) -> Command,
    enter: impl Fn(WmAction) -> Box<dyn Fn()>,
) -> (CommandPalette, Vec<PaletteEntry>) {
    let mut rows = Vec::new();
    for list in super::lists::declared() {
        let Some(sigil) = list.sigil else {
            continue;
        };
        let batch: Vec<PaletteEntry> = match (&list.host_rows, &list.items) {
            (Some(host), _) => host(sources),
            (None, Some(items)) => items()
                .into_iter()
                .enumerate()
                .map(|(at, item)| PaletteEntry {
                    id: format!("{}#{at}", list.name),
                    ..item.entry
                })
                .collect(),
            (None, None) => continue,
        };
        palette = palette.mode_with_commands(
            sigil,
            list.name.clone(),
            batch.iter().map(|row| command(row, &list.name)),
        );
        if list.search_descriptions {
            palette = palette.match_description_in(list.name.clone());
        }
        if let Some(action) = list.on_enter {
            palette = palette.on_mode_enter(list.name.clone(), enter(action()));
        }
        rows.extend(batch);
    }
    (palette, rows)
}
