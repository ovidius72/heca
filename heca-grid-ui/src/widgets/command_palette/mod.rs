//! [`CommandPalette`] — a fuzzy command launcher with named search modes.
//!
//! [`Command`] supplies each row's data and callback. [`SearchModel`] owns matching,
//! ranking and per-scope history; the query is an [`Input`], and row text uses [`Label`]
//! children. Navigation comes from configurable [`WidgetIntent`] events.
//!
//! Declare a mode and its rows once; named prefills and typed sigils share that declaration:
//!
//! ```
//! use heca_grid_ui::{Command, CommandPalette};
//! let palette = CommandPalette::new()
//!     .mode_with_commands(':', "command", [Command::new("Close", || {})])
//!     .mode_with_commands('!', "reports", [Command::new("Build report", || {})])
//!     .query_in(Some("reports"), "build")
//!     .default_open(true);
//! let open = palette.open_signal();
//! ```
//!
//! The first mode is the default. No declared modes preserves a single list in the
//! search model's scope. Query edits select modes only through their leading sigil;
//! there is no separate live mode state. [`CommandPalette::query_in`] only composes
//! that query, with unknown names leaving the supplied text unchanged.
//!
//! Declarative Heca hosts open their native palette through the same action, for example:
//!
//! ```text
//! command_palette mode=pane query=nvim
//! ```
//!
//! There is no serialized `CommandPalette` widget kind. A described button emits that
//! action as an `Intent`; the native host supplies live rows and callbacks. The complete
//! native and declarative examples and API tables are in `docs/widgets.md`.
//!
//! **Composed, not drawn.** The palette is a layer that holds the keyboard, the open state and the
//! selection, around an [`Overlay`](crate::widgets::Overlay) whose panel is a column of the query
//! `Input` and a `ScrollRegion` of rows (`build`, `row`). The overlay places and caps the panel, the
//! engine lays the rows out, the region scrolls and keeps the selected row in view. What stays the
//! palette's own: fuzzy filtering (`filter`, a pure function), the match marks, reflowing the
//! selected description and moving the cursor (`search`); mode declarations and named prefill live
//! in `modes`.

use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, Handled, WidgetIntent};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::search::{Ranked, SearchAction, SearchModel};
use crate::style::WidgetSize;
use crate::widgets::{Glyph, Input, KeyCap};
use std::cell::Cell;

mod build;
mod chord;
mod command;
mod component;
mod extent;
mod filter;
mod modes;
#[cfg(test)]
mod pointer_tests;
mod row;
mod search;
mod source;
#[cfg(test)]
mod tests;

pub use command::Command;

/// A fuzzy command launcher.
pub struct CommandPalette {
    base: Base,
    commands: Vec<Command>,
    source: Option<source::CandidateSource>,
    /// Sigil → scope, in declaration order; **the first declared is the default mode**, the one a
    /// query with no sigil is in.
    ///
    /// Empty means the palette has one mode — [`search`](Self::search)'s own scope, reachable
    /// without a sigil — which is exactly what a palette written before modes existed does.
    modes: Vec<(char, String)>,
    /// Builder input kept until a named mode can be resolved. The query field
    /// remains the sole mode selector; editing or replacing it clears this input.
    named_prefill: Option<(String, String)>,
    /// The value of the query Input, which sits above the rows in the panel.
    query: Signal<String>,
    /// A typed edit asks the palette to reset ranking/history before its next read or action.
    query_changed: Signal<bool>,
    /// The cursor: a position in the ranked results.
    selected: usize,
    /// Did the keyboard (or the palette itself) move the cursor, and not the pointer? Only then may
    /// a row ask its scroll region to bring it into view — pointing at a row is seeing it already,
    /// and scrolling it from under the pointer would move the row that was pointed at.
    keyboard_moved: Signal<bool>,
    /// What the rows' own handlers have noticed the pointer do, read at the next event or tick.
    pointer: row::PointerInbox,
    placeholder: String,
    /// How roomy the panel is — width and how tall it may grow, never font. Set from the app's
    /// `[settings] command_palette_size`.
    panel_size: WidgetSize,
    open: Signal<bool>,
    on_dismiss: Option<Box<dyn Fn()>>,
    /// Matching, ranking by past use, and the query history — **embedded, not implemented**. Every
    /// call below delegates; this widget owns no matcher and walks no history.
    search: SearchModel,
    /// The live signals of each row, in `commands` order — what moves the marks, the cursor and
    /// the reflow on every keystroke without rebuilding a hundred rows.
    rows: Vec<row::RowHandle>,
    /// The text signal of each row's title, in `commands` order — handed to a host that wants a row
    /// to follow something live (a pane's process, a rename). Writing one is a text swap on that
    /// label alone; the palette reads them back when it ranks, so what is matched is what is shown.
    title_text: Vec<Signal<String>>,
    /// Scopes whose rows are matched on their description as well as their title.
    describe_scopes: Vec<String>,
    /// What to do when a mode is entered ([`on_mode_enter`](Self::on_mode_enter)), by scope.
    mode_enter: Vec<modes::ModeEnter>,
    /// The scope the open palette was last seen in, so entering is announced once, not on every key.
    entered: std::cell::RefCell<Option<String>>,
}

#[heca_grid_ui_macros::props]
impl CommandPalette {
    /// A new, empty (closed) palette.
    pub fn new() -> Self {
        let query_changed = signal(false);
        let placeholder = String::from("Type a command…");
        let panel_size = WidgetSize::Normal;
        let mut base = Base::viewport_layer();
        let open = base.open;
        // Modal containment is the composed overlay's (it follows `open` and gives the keyboard
        // back to the opener); the query Input inside it owns the keyboard.
        // **A palette locks because it is a palette.** It has the keyboard for its query line, and
        // nothing behind it should be acted on while you are typing into it.
        base.lock = true;
        let query_field = Self::query_field(open, query_changed, String::new(), placeholder.clone());
        let query = query_field.text();
        base.children.push(build::overlay(query_field, open, panel_size));
        Self {
            base,
            commands: Vec::new(),
            source: None,
            modes: Vec::new(),
            named_prefill: None,
            query,
            query_changed,
            selected: 0,
            keyboard_moved: signal(true),
            pointer: row::PointerInbox::new(),
            placeholder,
            panel_size,
            open,
            on_dismiss: None,
            search: SearchModel::detached("command"),
            rows: Vec::new(),
            title_text: Vec::new(),
            describe_scopes: Vec::new(),
            mode_enter: Vec::new(),
            entered: Default::default(),
        }
    }

    /// Build the one query control; all editing stays in Input.
    fn query_field(open: Signal<bool>, changed: Signal<bool>, text: String, placeholder: String) -> Input {
        let mut field = Input::new().value(text).placeholder(placeholder).placeholder_on_focus(true)
            // Home and End are the list's (`menu_first` / `menu_last`), not the caret's.
            .line_keys(false)
            .on_change(move |_| changed.set(true));
        field.base_mut().follow_focus(open);
        field
    }

    fn sync_query(&mut self) {
        if self.query_changed.get_untracked() {
            self.query_changed.set(false);
            self.on_query_changed();
        }
    }

    /// Add a command.
    ///
    /// It becomes a **row of the list** — a grid of an icon, the title and description `Label`s and
    /// the keycap chords — laid out by the engine, so the cut, the reflow, the match marks and the
    /// shortcut column are properties of widgets rather than things painted here.
    #[heca_grid_ui_macros::host_only("a composed value, not a scalar — built from `children`")]
    pub fn command(mut self, c: Command) -> Self {
        self.push_command(c);
        self
    }

    fn push_command(&mut self, c: Command) {
        let (row, handle, title) = row::build(&c, self.commands.len(), self.keyboard_moved, self.pointer);
        self.list_mut().children.push(crate::WidgetModel::new(row));
        self.rows.push(handle);
        // Kept so a host can rename a row **in place**: the label is a `Signal<String>` already, so
        // writing it re-marks that one label instead of rebuilding the palette — which is what
        // keeps a live list from flashing.
        self.title_text.push(title);
        self.commands.push(c);
    }

    /// How roomy the panel is: `Small` / `Normal` / `Large` decide the panel's **own width and how
    /// much of the window's height it may take**, and nothing else — the text stays at reading size
    /// at every size, which is why this is its own property rather than [`LayoutExt::size`], whose
    /// font scaling would cascade through the whole panel.
    ///
    /// Whatever the variant, the panel never exceeds the window: the overlay it sits in caps it, so
    /// a small screen is safe by construction.
    #[heca_grid_ui_macros::prop]
    pub fn panel_size(mut self, size: WidgetSize) -> Self {
        self.panel_size = size;
        self.resize_panel(size);
        self
    }

    /// Set the empty-query placeholder text.
    #[heca_grid_ui_macros::prop]
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        let field = Self::query_field(self.open, self.query_changed, self.query_text(), self.placeholder.clone());
        self.query = field.text();
        self.set_query_field(field);
        self
    }

    /// Open with the query already filled in — including its sigil, which is how a caller opens the
    /// palette **in a mode**: `"@"` is the pane list, `"@nvim"` is the pane list already narrowed.
    ///
    /// [`Self::query_in`] composes this string from a mode name using the declared
    /// sigils. Both builders set this same field; there is no separate live mode.
    #[heca_grid_ui_macros::prop]
    pub fn query(mut self, text: impl Into<String>) -> Self {
        self.named_prefill = None;
        self.query.set(text.into());
        self.refresh_source();
        // Filter, mark, place the selection and reflow now, so this does not depend on being called
        // before `open` — properties are order-independent, and a mode set after it would otherwise
        // paint the wrong list once.
        self.sync_initial_selection();
        self.sync_rows();
        self.announce_mode();
        self
    }

    /// Set the initial open state.
    #[heca_grid_ui_macros::prop]
    pub fn default_open(mut self, open: bool) -> Self {
        self.open.set(open);
        // Filter and mark for the (empty) query now, so the first paint is not a frame behind, and
        // place the selection on whichever row asked to start there.
        self.sync_initial_selection();
        self.sync_rows();
        self.announce_mode();
        self
    }

    /// The search memory this palette ranks and recalls from — a host-owned store, so what the user
    /// has searched and chosen survives the palette being rebuilt on every open.
    ///
    /// Unset, the palette gets a private store: it behaves identically, it simply forgets.
    #[heca_grid_ui_macros::host_only("a shared model, not a scalar")]
    pub fn search(mut self, model: SearchModel) -> Self {
        self.search = model;
        self
    }

    /// The open-state signal — the host binds a trigger (e.g. Ctrl+K) to it.
    pub fn open_signal(&self) -> Signal<bool> {
        self.open
    }

    /// Live query, also used by a host's named form reader.
    pub fn query_signal(&self) -> Signal<String> { self.query }

    /// Update the query as an edit, retaining the input and resetting its selection/history cursor.
    /// History recall uses its own native path, so recalling does not restart history browsing.
    pub fn query_writer(&self) -> impl Fn(String) + 'static {
        let query = self.query;
        let changed = self.query_changed;
        move |text| {
            if query.get_untracked() != text { query.set(text); changed.set(true); }
        }
    }

    /// Observe user cancellation. Accepting a command closes without a dismissal callback.
    #[heca_grid_ui_macros::host_only("an intent callback in a description")]
    pub fn on_dismiss(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Box::new(callback)); self
    }

    fn dismiss(&mut self) {
        self.close();
        if let Some(callback) = &self.on_dismiss { callback(); }
    }

    /// Each row's title signal, in the order the commands were added — for a host whose rows track
    /// something that changes while the palette is open (a pane's running program, a rename).
    ///
    /// Writing one **renames that row in place**: the label re-marks itself and nothing else is
    /// rebuilt, so the list never flashes. `results` reads these back, so a row
    /// renamed this way is also matched by its new name rather than the one it was built with.
    pub fn label_signals(&self) -> &[Signal<String>] {
        &self.title_text
    }

    fn is_open(&self) -> bool {
        self.open.get_untracked()
    }

    /// The current query text (read from the [`Input`]).
    fn query_text(&self) -> String {
        self.query.get_untracked()
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}


impl LayoutExt for CommandPalette {}
