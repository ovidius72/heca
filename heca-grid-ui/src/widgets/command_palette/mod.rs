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
//! **It is composed, not drawn**: an [`Overlay`] holds the query field over a scrolling list of
//! rows that follow the ranked results — see `view`. Nothing in the palette positions, measures or
//! hit-tests; mode declarations and named prefill live in `modes`.

use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, Handled, WidgetIntent};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::search::{Ranked, SearchAction, SearchModel};
use crate::style::{Length, WidgetSize};
use crate::widgets::{Glyph, KeyCap};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

mod command;
mod component;
mod metrics;
mod modes;
mod search;
mod source;
mod view;

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
    /// The value of the query [`Input`].
    query: Signal<String>,
    /// A typed edit asks the palette to reset ranking/history before its next read or action.
    query_changed: Signal<bool>,
    /// The selected row, as a position in the ranked results.
    selected: usize,
    /// The query field's placeholder — the field follows it.
    placeholder: Signal<String>,
    /// How roomy the panel is — width and row count, never font. Set from the app's
    /// `[settings] command_palette_size`; the panel's body and list read it for their own size.
    panel_size: Signal<WidgetSize>,
    open: Signal<bool>,
    /// Whether it was open at the last tick, so closing — by any path — resets it once.
    was_open: bool,
    on_dismiss: Option<Box<dyn Fn()>>,
    /// What each row shows, by command index — read by the view to build a row.
    rows: Rc<RefCell<Vec<view::RowData>>>,
    /// The commands shown, in rank order, for the view's rows to follow.
    shown: Signal<Vec<(u64, usize)>>,
    /// Bumped when a live source replaces the commands, so the rows are made again.
    generation: u64,
    /// The selected command, by index — what each row's "am I selected" reads.
    selected_command: Signal<Option<usize>>,
    /// The selected row by position — the list's selection, which the pointer moves too.
    list_selected: Signal<Option<usize>>,
    /// What a pointer did to a row since the last tick.
    picks: Rc<Cell<Option<view::Pick>>>,
    /// Matching, ranking by past use, and the query history — **embedded, not implemented**. Every
    /// call below delegates; this widget owns no matcher and walks no history.
    search: SearchModel,
    /// The marks signal of each row's title label, so the match highlights can be moved on every
    /// keystroke without rebuilding a hundred labels.
    title_marks: Vec<Signal<Vec<usize>>>,
    /// The text signal of each row's title, in `commands` order — handed to a host that wants a row
    /// to follow something live (a pane's process, a rename). Writing one is a text swap on that
    /// label alone; the palette reads them back when it ranks, so what is matched is what is shown.
    title_text: Vec<Signal<String>>,
    /// The marks signal of each row's description label, where it has one — what shows the query's
    /// match **in the description** for a mode that matches it
    /// ([`match_description_in`](Self::match_description_in)).
    desc_marks: Vec<Option<Signal<Vec<usize>>>>,
    /// Scopes whose rows are matched on their description as well as their title.
    describe_scopes: Vec<String>,
    /// What to do when a mode is entered ([`on_mode_enter`](Self::on_mode_enter)), by scope.
    mode_enter: Vec<modes::ModeEnter>,
    /// The scope the open palette was last seen in, so entering is announced once, not on every key.
    entered: std::cell::RefCell<Option<String>>,
    /// What the panel is built from.
    view: Rc<RefCell<view::ViewState>>,
}

#[heca_grid_ui_macros::props]
impl CommandPalette {
    /// A new, empty (closed) palette.
    pub fn new() -> Self {
        let mut base = Base::container();
        // It fills whatever holds it; the overlay inside centres the panel in the viewport.
        base.style.layout.width = Length::FULL;
        base.style.layout.height = Length::FULL;
        let open = base.open;
        let query_changed = signal(false);
        let placeholder = signal(String::from("Type a command…"));
        let query = signal(String::new());
        let rows = Rc::new(RefCell::new(Vec::new()));
        let shown = signal(Vec::new());
        let selected_command = signal(None);
        let list_selected = signal(None);
        let picks = Rc::new(Cell::new(None));
        let panel_size = signal(WidgetSize::Normal);
        let view = view::ViewState {
            rows: rows.clone(),
            results: shown,
            selected: selected_command,
            list_selected,
            picks: picks.clone(),
            size: panel_size,
            open,
            query,
            query_changed,
            placeholder,
        };
        let view = Rc::new(RefCell::new(view));
        Self::attach_panel(&mut base, &view);
        Self {
            base,
            commands: Vec::new(),
            source: None,
            modes: Vec::new(),
            named_prefill: None,
            query,
            query_changed,
            selected: 0,
            placeholder,
            panel_size,
            open,
            was_open: false,
            on_dismiss: None,
            rows,
            shown,
            generation: 0,
            selected_command,
            list_selected,
            picks,
            search: SearchModel::detached("command"),
            title_marks: Vec::new(),
            title_text: Vec::new(),
            desc_marks: Vec::new(),
            describe_scopes: Vec::new(),
            mode_enter: Vec::new(),
            entered: Default::default(),
            view,
        }
    }

    fn sync_query(&mut self) {
        if self.query_changed.get_untracked() {
            self.query_changed.set(false);
            self.on_query_changed();
        }
    }

    /// Add a command.
    ///
    /// Its row is composed, like everything else in the palette: an [`Icon`](crate::widgets::Icon),
    /// a title [`Label`](crate::widgets::Label) and a description `Label` (the cut, the reflow and
    /// the match marks are theirs), its bindings as [`Keycap`](crate::widgets::Keycap)s, and the
    /// selection is the [`List`](crate::widgets::List)'s. Only the rows on screen are built.
    #[heca_grid_ui_macros::host_only("a composed value, not a scalar — built from `children`")]
    pub fn command(mut self, c: Command) -> Self {
        self.push_command(c);
        self
    }

    fn push_command(&mut self, c: Command) {
        let title = signal(String::new());
        c.label.clone().show_in(title);
        let title_marks = signal(Vec::new());
        let description_marks = signal(Vec::new());
        self.title_text.push(title);
        self.title_marks.push(title_marks);
        self.desc_marks
            .push(c.description.is_some().then_some(description_marks));
        self.rows.borrow_mut().push(view::RowData {
            title,
            title_marks,
            description: c.description.clone().map(|text| (text, c.description_path)),
            description_marks,
            detail: c.detail.clone(),
            icon: c.icon,
            chords: c.chords.clone(),
            trailing: c.trailing.clone(),
            content: c.content.clone(),
        });
        self.commands.push(c);
    }
    /// How roomy the panel is: `Small` / `Normal` / `Large` decide its **maximum width and how many
    /// rows it shows**, and nothing else — the text stays at reading size at every size, which is why
    /// this is its own property rather than [`LayoutExt::size`],
    /// whose font scaling would cascade through the whole panel.
    ///
    /// Whatever the variant, the panel never exceeds `PANEL_VIEWPORT_FRAC` of the window, so a
    /// small screen is safe by construction.
    #[heca_grid_ui_macros::prop]
    pub fn panel_size(self, size: WidgetSize) -> Self {
        self.panel_size.set(size);
        self
    }

    /// **The empty-query placeholder** — a fixed text, or one that changes (a `Signal<String>`, a
    /// function reading signals), which the query field follows with nothing rebuilt: a palette
    /// whose hint names the list it is in changes the hint, not the field.
    #[heca_grid_ui_macros::prop]
    pub fn placeholder(self, text: impl crate::builders::IntoTextValue) -> Self {
        text.into_text_value().show_in(self.placeholder);
        self
    }

    /// **What opens the palette** — your `Signal<bool>`, or the name of the action that opens it
    /// (`.open("palette.open")`: a key, RPC, a plugin). It closes itself on its dismiss key, a
    /// choice or a click outside, and writes your signal `false` when it does. The same `.open`
    /// every surface takes — see [`IntoOpen`](crate::builders::IntoOpen).
    #[heca_grid_ui_macros::prop]
    pub fn open(self, by: impl crate::builders::IntoOpen) -> Self {
        let own = self.open;
        by.into_open().apply(own, Self::follow_open, self)
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
        // Mark, place the selection and reflow now, so this does not depend on being called before
        // `open` — properties are order-independent, and a mode set after it would otherwise paint
        // the wrong list once.
        self.sync_marks();
        self.sync_initial_selection();
        self.sync_view();
        self.announce_mode();
        self
    }

    /// Set the initial open state.
    #[heca_grid_ui_macros::prop]
    pub fn default_open(mut self, open: bool) -> Self {
        self.open.set(open);
        // Mark the matches for the (empty) query now, so the first paint is not a frame behind, and
        // place the selection on whichever row asked to start there.
        self.sync_marks();
        self.sync_initial_selection();
        self.sync_view();
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
    pub fn query_signal(&self) -> Signal<String> {
        self.query
    }

    /// Update the query as an edit, retaining the input and resetting its selection/history cursor.
    /// History recall uses its own native path, so recalling does not restart history browsing.
    pub fn query_writer(&self) -> impl Fn(String) + 'static {
        let query = self.query;
        let changed = self.query_changed;
        move |text| {
            if query.get_untracked() != text {
                query.set(text);
                changed.set(true);
            }
        }
    }

    /// Observe user cancellation. Accepting a command closes without a dismissal callback.
    #[heca_grid_ui_macros::host_only("an intent callback in a description")]
    pub fn on_dismiss(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Box::new(callback));
        self
    }

    fn dismiss(&mut self) {
        self.close();
        if let Some(callback) = &self.on_dismiss {
            callback();
        }
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

    /// Whether its panel is on screen — open, or still dissolving after it closed.
    fn panel_shown(&self) -> bool {
        self.base.children.iter().any(|panel| panel.is_shown())
    }

    /// The current query text (read from the [`Input`]).
    fn query_text(&self) -> String {
        self.query.get_untracked()
    }
}

impl CommandPalette {
    /// Follow `open`: the panel is built on it, so it is up exactly while it is true.
    fn follow_open(mut self, open: Signal<bool>) -> Self {
        self.open = open;
        self.base.open = open;
        self.view.borrow_mut().open = open;
        self
    }

    /// **The palette's one child, its panel — built the first time it is needed**: the first
    /// layout, or the first event, whichever comes first, when every builder has run. So the panel
    /// is built once, on the `open` the palette ended up following, and nothing is built again or
    /// reached into to change it. (The same door as any child that follows a value: a list of one.)
    fn attach_panel(base: &mut Base, view: &Rc<RefCell<view::ViewState>>) {
        let view = view.clone();
        crate::builders::IntoChildren::attach(
            crate::builders::BoundChildren::new(|| vec![()], move |_| view::view(&view.borrow())),
            base,
        );
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExt for CommandPalette {}
