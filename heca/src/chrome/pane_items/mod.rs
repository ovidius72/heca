//! **Pane items — what a pane's header and row can show, by name.**
//!
//! A pane's header shows *buttons* (right) and *chips* (left), and the sidebar's pane row shows
//! *lines*. Each of those used to be a closed list in config and a `match` in the
//! code that drew it, so nothing outside heca could add one. They are **named items in a registry**
//! now: heca's own are registered the same way a program built on heca registers its own, and
//! config lists them **by name** to decide which show and in what order.
//!
//! *Client side*: every item here is drawn, so it lives in a window. What it reads about a pane is
//! plain data the window is handed — after the server/client split (F012/P104) that is what the
//! server sends.
//!
//! | kind | added with | shown by |
//! |---|---|---|
//! | header button | `pro.pane_button("show_notes")` | `[appearance.pane] title_actions` |
//! | header chip | `pro.pane_chip("status", \|facts\| ..)` | `[appearance.pane] title_segments` |
//! | sidebar row line | `pro.pane_line("status", \|facts\| ..)` | `[appearance.sidebar] pane_lines` |
//!
//! **What an extension adds is a default.** It is shown while the user's list is still the built-in
//! one; once they write their own list only their list counts, and an item they do not show is said
//! once — "`pro.show_notes` is available; add it to `title_actions` to show it" — so it can still be
//! found. The user is always the one who decides.

mod buttons;
mod chips;
mod lines;
mod listing;

pub use buttons::PANE_ARG;
pub(crate) use buttons::{ButtonEmit, PaneButtonDef, PaneButtons, PaneIds, add_extension_button};
pub(crate) use chips::{Fit, PaneChipDef, PaneChips, add_extension_chip};
pub use chips::{PaneChip, PaneFacts};
pub(crate) use lines::{
    BuildLine, LineCx, PaneLineDef, PaneRowLines, ProduceLine, add_extension_line,
};
pub use lines::{PaneLine, PaneLineSet};

/// **Say what the user's lists leave out or get wrong**, for every kind of pane item — at startup,
/// once every action a button names exists, and again on each reload. Nothing is silent.
pub(crate) fn report_pane_items(state: &crate::app_state::AppState) {
    let pane = &state.appearance.pane;
    state
        .pane_buttons
        .report(&pane.title_actions, &state.action_catalog);
    state.pane_chips.report(&pane.title_segments);
    state
        .pane_lines
        .report(&state.appearance.sidebar.pane_lines);
}
