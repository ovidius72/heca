//! Pane and workspace rows from the current layout.

use super::*;

/// Every open pane, as palette rows — **pure**, over the session, so the naming and the rank ids
/// are testable without an `AppState` (which needs a window).
///
/// A pane's name is whatever [`pane_info_view`] says it is — **the one name
/// composer**, the same call the sidebar card and the pane header make.
///
/// `Pane::title` is *not* that name. It is a fallback, and in a running app it is empty: the name
/// on screen is resolved from the live `PaneRuntime`'s program through
/// [`ProgramsConfig`](heca_config::programs::ProgramsConfig) (so `v` reads as `nvim`), with a
/// `custom_name` overriding it. Reading the field directly produced a pane list of blank rows while
/// the sidebar beside it read `zsh` — the two names have to come from one place.
pub(crate) fn pane_entries(
    layout: heca_core::layout::Layout<'_>,
    programs: &heca_config::programs::ProgramsConfig,
    add_process_name: bool,
    last_focused: Option<heca_core::layout::PaneId>,
) -> Vec<PaletteEntry> {
    // The pane you are standing in: the active workspace's active pane. Only one row in the whole
    // list can be it, which is what makes the accent mean something.
    let focused = layout
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.id);
    let mut rows = Vec::new();
    for (ws_idx, ws) in layout.session().workspaces.iter().enumerate() {
        let ws_name = workspace_name(ws, ws_idx);
        for pane in ws.panes() {
            let info = pane_info_view(
                programs,
                &pane.title,
                pane.custom_name.as_deref(),
                Some(&pane.runtime),
                add_process_name,
            );
            rows.push(PaletteEntry {
                id: format!("pane:{}", pane.id.0),
                // The **name**, not the id: `PaneId` is minted from a counter that restarts at
                // 1 every launch, so a persisted count would rank a pane that no longer exists.
                rank_id: Some(info.title.clone()),
                pane: Some(pane.id),
                current: Some(pane.id) == focused,
                preselect: Some(pane.id) == last_focused,
                label: info.label(),
                // Where it is. The workspace alone leaves several `zsh` rows identical, which
                // is exactly the list this is meant to pick from, so the working directory
                // leads when there is one — the same `~/…` the pane header shows.
                description: match pane.runtime.cwd.as_deref() {
                    Some(cwd) => format!("{} · {ws_name}", home_relative_path(cwd)),
                    None => ws_name.clone(),
                },
                icon: Some(info.icon),
                description_path: false,
                detail: None,
                shortcuts: Vec::new(),
                intent: InteractionIntent::FocusPane { pane_id: pane.id },
            });
        }
    }
    rows
}

/// Every workspace, as palette rows — pure, for the same reason as [`pane_entries`].
pub(crate) fn workspace_entries(
    layout: heca_core::layout::Layout<'_>,
    last_visited: Option<usize>,
) -> Vec<PaletteEntry> {
    layout
        .session()
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
            pane: None,
            current: ws_idx == layout.active_workspace_idx(),
            preselect: Some(ws_idx) == last_visited,
            label: workspace_name(ws, ws_idx),
            description: match ws.scrolling.columns.len() {
                1 => "1 column".to_string(),
                n => format!("{n} columns"),
            },
            icon: Some(Glyph::Cards),
            description_path: false,
            detail: None,
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
