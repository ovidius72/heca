//! Each mode's sigil, search scope and row source, declared together.

use super::*;

pub(super) const MODE_COMMAND: &str = "command";
pub(super) const MODE_PANE: &str = "pane";
pub(super) const MODE_WORKSPACE: &str = "workspace";
pub(super) const MODE_FOLLOW: &str = "follow";
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

/// One declaration connects the input sigil and search scope to their rows — and says how the mode
/// is searched and what entering it does, so no call site chooses either.
#[derive(Clone, Copy)]
pub(super) struct Mode {
    pub sigil: char,
    pub scope: &'static str,
    pub rows: fn(&RowSources<'_>) -> Vec<PaletteEntry>,
    /// Is the row's second line (a folder, a message body) searched as well as its title? Off for a
    /// mode whose second line only explains a row already found.
    pub search_description: bool,
    /// The action to run each time the mode is entered, for a mode whose rows are not known until
    /// asked for. `None` for a mode built from what the window already holds.
    pub on_enter: Option<fn() -> WmAction>,
}

/// Declaration order decides the default mode and the order of the flat row list.
pub(super) const MODES: &[Mode] = &[
    Mode {
        sigil: ':',
        scope: MODE_COMMAND,
        rows: |sources| entries(sources.catalog, sources.owners, sources.shortcuts),
        search_description: false,
        on_enter: None,
    },
    Mode {
        sigil: '@',
        scope: MODE_PANE,
        rows: |sources| {
            pane_entries(
                sources.layout,
                sources.programs,
                sources.panes,
                sources.add_process_name,
                sources.last_focused,
            )
        },
        search_description: false,
        on_enter: None,
    },
    Mode {
        sigil: '$',
        scope: MODE_WORKSPACE,
        rows: |sources| workspace_entries(sources.layout, sources.last_visited),
        search_description: false,
        on_enter: None,
    },
    // A session is found by its name or by its folder.
    Mode {
        sigil: '#',
        scope: "session",
        rows: |sources| sources.sessions.to_vec(),
        search_description: true,
        on_enter: None,
    },
    Mode {
        sigil: '~',
        scope: MODE_FOLLOW,
        rows: |sources| sources.follow.to_vec(),
        search_description: false,
        on_enter: None,
    },
    // A notification is found by its title or by its body, and the list is the server's: every
    // entry into the mode asks it again, so what is shown is never an old answer.
    Mode {
        sigil: '!',
        scope: MODE_NOTIFICATION,
        rows: |sources| sources.notifications.to_vec(),
        search_description: true,
        on_enter: Some(|| WmAction::NotificationHistory),
    },
];

/// Build each declared list once, keeping row signals and overlay results in that order.
///
/// `enter` turns a mode's `on_enter` action into what the palette runs when the mode is entered.
pub(super) fn populate(
    mut palette: CommandPalette,
    modes: &[Mode],
    sources: &RowSources<'_>,
    mut command: impl FnMut(&PaletteEntry) -> Command,
    enter: impl Fn(WmAction) -> Box<dyn Fn()>,
) -> (CommandPalette, Vec<PaletteEntry>) {
    let mut rows = Vec::new();
    for mode in modes {
        let batch = (mode.rows)(sources);
        palette = palette.mode_with_commands(
            mode.sigil,
            mode.scope,
            batch.iter().map(&mut command),
        );
        if mode.search_description {
            palette = palette.match_description_in(mode.scope);
        }
        if let Some(action) = mode.on_enter {
            let run = enter(action());
            palette = palette.on_mode_enter(mode.scope, run);
        }
        rows.extend(batch);
    }
    (palette, rows)
}
