//! **The kept chrome tree** — what it holds beside the widgets, and when it is rebuilt.

use crate::chrome::{ChromeConfig, ChromeSignals, DragItemRegistry};

/// A **retained** chrome tree + the signature of the state that produced it. The
/// tree is rebuilt only when [`chrome_signature`] changes; otherwise it is just
/// re-laid-out and painted each frame. This keeps the widget signals alive across
/// frames (no per-frame signal churn) and gives a live tree to dispatch events into
/// (F4.2). The collapsed sidebar rail is still hand-drawn in `render.rs`.
pub(crate) struct RetainedChrome {
    pub(crate) sig: u64,
    /// Handles to the tree's **value** signals (selection + status), so they update
    /// in place via [`sync_chrome_signals`](crate::chrome::sync_chrome_signals) instead of forcing a rebuild.
    pub(crate) signals: ChromeSignals,
    /// Maps each draggable/droppable row's **own name** back to *what it is* (pane / column /
    /// workspace) — the component's own knowledge, kept beside the tree that wrote it. Populated
    /// during [`build_chrome_root`](crate::chrome::build_chrome_root) and read back when a drag starts and
    /// when the framework hands back a drop. A name is never parsed here.
    pub(crate) drag_items: DragItemRegistry,
    /// **The [`InteractionSource`](crate::app::interaction::InteractionSource) every intent from
    /// this tree is dispatched with** — taken from the emitter that built it, never restated.
    ///
    /// It is here so that *asking whether a gesture would be allowed* and *running it* ask the
    /// identical question. The `prefix+/` picker judged its candidates as `Keyboard` while this
    /// tree dispatches as `MouseLeftSidebar`; the two resolved to different domains, so the picker
    /// offered letters in `Container` that execution then refused in `Floating` — a letter that did
    /// nothing. Guessing the source in the filter was the bug; there is one authority and this is a
    /// copy of it, made at construction.
    pub(crate) intent_source: crate::app::interaction::InteractionSource,
}

/// Hash of what the chrome tree shows that changes while the app runs: the window size, the
/// app-wide font zoom, the sidebar widths and the sidebar content. When it changes the retained tree
/// is rebuilt; otherwise the tree is reused (re-laid-out and painted only).
///
/// **Style is not here.** The theme and the appearance settings change only on a config reload,
/// and a reload drops the whole tree, so a new setting needs nothing added here.
pub(crate) fn chrome_signature(state: &crate::app_state::AppState, chrome: ChromeConfig) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hsh = std::collections::hash_map::DefaultHasher::new();
    let phys = state.window.inner_size();
    phys.width.hash(&mut hsh);
    phys.height.hash(&mut hsh);
    state.scale_factor.to_bits().hash(&mut hsh);
    // App-wide font zoom scales the chrome font, so a change must rebuild the tree.
    state.app_font_zoom.to_bits().hash(&mut hsh);
    chrome.left_sidebar_width.to_bits().hash(&mut hsh);
    chrome.right_sidebar_width.to_bits().hash(&mut hsh);
    chrome.sidebar_gap.to_bits().hash(&mut hsh);
    // Which lines show under a pane's name is baked into each row when it is built.
    for name in state
        .pane_lines
        .shown(&state.appearance.sidebar.pane_lines)
        .names()
    {
        name.hash(&mut hsh);
    }
    // The active workspace (which gets the accent wash + count badge) is structural
    // enough to rebuild on a workspace SWITCH — but pane-to-pane focus *within* a
    // workspace must NOT rebuild: pane/column `active` + the status text are bound
    // signals (`sync_chrome_signals`), deliberately excluded from this signature.
    //
    // **A pane's runtime is not in here.** Every metadata line on a sidebar card is attached
    // always and shows or hides itself, so a runtime change needs no rebuild.
    state.layout().active_workspace_idx().hash(&mut hsh);
    for ws in &state.chrome_state.workspaces.tree().workspaces {
        ws.ws_idx.hash(&mut hsh);
        ws.name.hash(&mut hsh);
        ws.collapsed.hash(&mut hsh);
        for c in &ws.columns {
            c.col_idx.hash(&mut hsh);
            c.collapsed.hash(&mut hsh);
            for p in &c.panes {
                p.pane_id.0.hash(&mut hsh);
                p.name.hash(&mut hsh);
            }
            u8::MAX.hash(&mut hsh); // column separator in the hash stream
        }
        for p in &ws.floating_panes {
            p.pane_id.0.hash(&mut hsh);
            p.name.hash(&mut hsh);
        }
        u64::MAX.hash(&mut hsh); // workspace separator
    }
    hsh.finish()
}
