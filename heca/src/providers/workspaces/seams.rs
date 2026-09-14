//! **The app edges this dock binds, as one group** (F006/P032/T429).
//!
//! AGENTS.md § 0b-bis step 3: every app concept a surface binds travels in a single struct built
//! once, and every component takes `&` it. Threading them one argument at a time is what produced
//! `pane_card()`'s eleven positional parameters, where no call site could be read without counting.
//!
//! They are split in two because they differ in **kind**, not in size:
//!
//! - [`DockSeams`] is what a component *reads* — the theme, the emitter, the catalog, this
//!   placement's id. Shared, so every component holds `&` it at once.
//! - [`DockRegistries`] is what a component *registers into* — host ids that must stay monotonic
//!   and are released with the tree that made them. Exclusive by nature, so it travels `&mut`.
//!
//! Neither carries `AppState`, or the `ChromeCtx` that reaches it: a component that takes one
//! needs a window, so it is a component nobody can test. `mod.rs` gathers both from the contexts
//! and hands them down — the single rule that decides whether this surface can be checked without
//! the maintainer photographing the app.

use crate::actions::ActionCatalog;
use crate::chrome::{
    ChromeIntentEmitter, ChromeSignals, DragItemRegistry, WorkspacesContainerState,
};
use heca_config::programs::ProgramsConfig;
use heca_core::layout::PaneId;
use heca_grid_ui::theme::Theme as GuiTheme;
use std::rc::Rc;

/// What every row of this dock reads, gathered once in `mod.rs`.
pub(crate) struct DockSeams<'a> {
    /// This placement's **mount id**. Every row's cursor signal is registered under it, and the
    /// intents a row fires are stamped with it, so two seatings of this container light different
    /// rows (F003/P085/T354).
    pub(crate) mount: &'a str,
    /// Every colour, size and radius. A component names a token, never a literal.
    pub(crate) theme: &'a GuiTheme,
    /// How a program is recognised and what icon it gets. The `Rc` travels, not a bare
    /// reference: a row keeps it alive inside the subscription that recomputes its labels, which
    /// outlives this borrow.
    pub(crate) programs: &'a Rc<ProgramsConfig>,
    /// Where a row's declared gesture goes. A row names an action; it never runs one.
    pub(crate) emit: &'a ChromeIntentEmitter,
    /// What an action **looks like** — the one thing a row cannot answer for itself, needed
    /// because a row declares its own menu (F004/P084/T395).
    pub(crate) catalog: &'a ActionCatalog,
    /// This component's own state: the active pane, the collapse set, the display settings.
    pub(crate) ws_state: &'a WorkspacesContainerState,
    /// The pane holding focus, read once at the top rather than by each row.
    pub(crate) active_pane: Option<PaneId>,
}

impl DockSeams<'_> {
    /// **What a pick on this row does** — the caller says only WHICH row.
    ///
    /// `chrome::picks` takes the mount and the emitter alongside the intent, and both of those are
    /// already in here: this group exists so a component is handed the app's edges once instead of
    /// threading them (AGENTS.md § 0b-bis rule 3). Every call site was unpacking the group to pass
    /// two of its own fields back, in a fixed order, with nothing to catch a swap.
    ///
    /// ```ignore
    /// .on_hint(seams.picks(row_hint(workspace_key(ws_id))))
    /// ```
    ///
    /// The intent stays visible because it is the part that differs and the part that says what
    /// the pick *is*; the seating and the channel are never the caller's choice.
    pub(crate) fn picks(&self, intent: heca_view::Intent) -> heca_grid_ui::Hint {
        crate::chrome::picks(self.mount, intent, self.emit)
    }
}

/// What every row of this dock registers into.
///
/// `&mut`, and passed beside [`DockSeams`] rather than inside it, because these are written to.
/// Disjoint fields for the reason `BuildCx` has them as fields too: two accessor calls in one
/// expression would be two overlapping `&mut` borrows.
pub(crate) struct DockRegistries<'a> {
    /// The signals the host flips in place. **Every state that changes with focus goes here** —
    /// see [`DockSeams`]' module note and `PaneRow`'s.
    pub(crate) signals: &'a mut ChromeSignals,
    /// Drag sources and drop targets, which hand back an opaque id the registry can resolve.
    pub(crate) drag: &'a mut DragItemRegistry,
}
