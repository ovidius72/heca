//! Each mode's sigil, search scope and row source, declared together.

use super::*;

pub(super) const MODE_COMMAND: &str = "command";
pub(super) const MODE_PANE: &str = "pane";
pub(super) const MODE_WORKSPACE: &str = "workspace";

/// Plain data needed by the palette's row sources.
pub(super) struct RowSources<'a> {
    pub catalog: &'a crate::actions::ActionCatalog,
    pub owners: &'a std::collections::HashMap<String, OwnerInfo>,
    pub shortcuts: &'a ActionShortcuts,
    pub layout: heca_core::layout::Layout<'a>,
    pub programs: &'a heca_config::programs::ProgramsConfig,
    pub last_focused: Option<heca_core::layout::PaneId>,
    pub last_visited: Option<usize>,
}

/// One declaration connects the input sigil and search scope to their rows.
#[derive(Clone, Copy)]
pub(super) struct Mode {
    pub sigil: char,
    pub scope: &'static str,
    pub rows: fn(&RowSources<'_>) -> Vec<PaletteEntry>,
}

/// Declaration order decides the default mode and the order of the flat row list.
pub(super) const MODES: &[Mode] = &[
    Mode {
        sigil: ':',
        scope: MODE_COMMAND,
        rows: |sources| entries(sources.catalog, sources.owners, sources.shortcuts),
    },
    Mode {
        sigil: '@',
        scope: MODE_PANE,
        rows: |sources| pane_entries(sources.layout, sources.programs, sources.last_focused),
    },
    Mode {
        sigil: '$',
        scope: MODE_WORKSPACE,
        rows: |sources| workspace_entries(sources.layout, sources.last_visited),
    },
];

/// Build each declared list once, keeping row signals and overlay results in that order.
pub(super) fn populate(
    mut palette: CommandPalette,
    modes: &[Mode],
    sources: &RowSources<'_>,
    mut command: impl FnMut(&PaletteEntry) -> Command,
) -> (CommandPalette, Vec<PaletteEntry>) {
    let mut rows = Vec::new();
    for mode in modes {
        let batch = (mode.rows)(sources);
        palette = palette.mode_with_commands(
            mode.sigil,
            mode.scope,
            batch.iter().map(&mut command),
        );
        rows.extend(batch);
    }
    (palette, rows)
}
