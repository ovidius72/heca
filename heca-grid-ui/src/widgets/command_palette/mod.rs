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
//! The existing drawing code still positions the panel and forwards input to an off-tree
//! query field. Those paths live in `geometry`, `paint` and `component`; mode declarations
//! and named prefill live in `modes`.

use crate::builders::{LayoutExt, Parent};
use crate::component::{Base, Component, Event, Handled, PaintCx, WidgetIntent};
use crate::font::MONO_LINE_RATIO;
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::Glow;
use crate::search::{Ranked, SearchAction, SearchModel};
use crate::style::WidgetSize;
use crate::style::{Direction, Length};
use crate::widgets::{
    Ellipsis, Flex, Glyph, Input, KeyCap, KeycapVariant, Label, PanelChrome, PanelElevation,
    paint_panel_chrome,
};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::Cell;

mod command;
mod component;
mod geometry;
mod metrics;
mod modes;
mod paint;
mod search;

pub use command::Command;
use metrics::*;

/// A fuzzy command launcher.
pub struct CommandPalette {
    base: Base,
    commands: Vec<Command>,
    /// Sigil → scope, in declaration order; **the first declared is the default mode**, the one a
    /// query with no sigil is in.
    ///
    /// Empty means the palette has one mode — [`search`](Self::search)'s own scope, reachable
    /// without a sigil — which is exactly what a palette written before modes existed does.
    modes: Vec<(char, String)>,
    /// Builder input kept until a named mode can be resolved. The query field
    /// remains the sole mode selector; editing or replacing it clears this input.
    named_prefill: Option<(String, String)>,
    /// The value of the query Input, which lives after the result rows in `Base.children`.
    query: Signal<String>,
    /// A typed edit asks the palette to reset ranking/history before its next read or action.
    query_changed: Signal<bool>,
    selected: usize,
    scroll: usize,
    placeholder: String,
    /// How roomy the panel is — width and row count, never font. Set from the app's
    /// `[settings] command_palette_size`.
    panel_size: WidgetSize,
    open: Signal<bool>,
    on_dismiss: Option<Box<dyn Fn()>>,
    viewport: Cell<Size>,
    /// Panel rect cached at paint, so the caret blink can damage just the panel
    /// (the palette paints on the overlay layer, away from its layout `bounds`).
    panel: Cell<Rectangle>,
    /// Each row's **measured** height, captured post-layout while the children are still at their
    /// natural, engine-computed sizes — the same trick [`Select`](super::Select) uses.
    ///
    /// This is what makes a wrapping description reflow the list: the palette no longer computes a
    /// row's height from a line count, it reads what the engine measured, so a description that
    /// wrapped onto three lines moves the rows below it without a line of arithmetic here.
    natural_h: Vec<f64>,
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
    /// The wrap signal of each row's description, where it has one. **Only the selected row's
    /// description reflows**; the rest are cut to one line, which is what keeps the list from
    /// exploding to three lines a row and is why this is per-row state rather than a build-time
    /// property.
    desc_wrap: Vec<Option<Signal<bool>>>,
}

#[heca_grid_ui_macros::props]
impl CommandPalette {
    /// A new, empty (closed) palette.
    pub fn new() -> Self {
        let open = signal(false);
        let mut base = Base::new();
        // The palette owns modal containment; its child Input owns the keyboard.
        base.follow_focus_modal(open);
        // **A palette locks because it is a palette.** It has the keyboard for its query line,
        // and nothing behind it should be acted on while you are typing into it.
        base.lock = true;
        let query_changed = signal(false);
        let placeholder = String::from("Type a command…");
        let query_field = Self::query_field(open, query_changed, String::new(), placeholder.clone());
        let query = query_field.text();
        base.children.push(crate::WidgetModel::new(query_field));
        Self {
            base,
            commands: Vec::new(),
            modes: Vec::new(),
            named_prefill: None,
            query,
            query_changed,
            selected: 0,
            scroll: 0,
            placeholder,
            panel_size: WidgetSize::Normal,
            open,
            on_dismiss: None,
            viewport: Cell::new(Size::new(f64::MAX, f64::MAX)),
            panel: Cell::new(Rectangle::from_size(Size::new(0.0, 0.0))),
            search: SearchModel::detached("command"),
            natural_h: Vec::new(),
            title_marks: Vec::new(),
            title_text: Vec::new(),
            desc_wrap: Vec::new(),
        }
    }

    /// Build the one query control; all editing stays in Input.
    fn query_field(open: Signal<bool>, changed: Signal<bool>, text: String, placeholder: String) -> Input {
        let mut field = Input::new().value(text).placeholder(placeholder).placeholder_on_focus(true)
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
    /// Its text becomes **real children** — a title [`Label`] and, when the command has one, a
    /// description `Label` — so the cut, the reflow and the match marks are the label's properties
    /// rather than three things painted by hand here. The icon, the keycap chips and the selection
    /// chrome stay drawn by the palette: the chips already go through the shared keycap primitive,
    /// and the rest is panel chrome, not text.
    #[heca_grid_ui_macros::host_only("a composed value, not a scalar — built from `children`")]
    pub fn command(mut self, c: Command) -> Self {
        let title = Label::new(c.label.clone()).truncate(Ellipsis::End);
        self.title_marks.push(title.marks_signal());
        // The row's text signal, kept so a host can rename a row **in place**. The label is a
        // `Signal<String>` already, so writing it re-marks that one label instead of rebuilding the
        // palette — which is what keeps a live list from flashing.
        self.title_text.push(title.text_signal());
        let mut column = Flex::column().child(title);
        self.desc_wrap.push(None);
        if let Some(desc) = &c.description {
            // Cut by default, reflowed while selected — `sync_wrap` flips it. Because the height is
            // measured, the reflow moves every row below it.
            let d = Label::new(desc.clone())
                .truncate(Ellipsis::End)
                .wrap(false)
                .muted(true);
            *self.desc_wrap.last_mut().expect("just pushed") = Some(d.wrap_signal());
            column = column.child(d);
        }
        self.base.children.insert(self.commands.len(), crate::WidgetModel::new(column));
        self.commands.push(c);
        self
    }
    /// How roomy the panel is: `Small` / `Normal` / `Large` decide its **maximum width and how many
    /// rows it shows**, and nothing else — the text stays at reading size at every size, which is why
    /// this is its own property rather than [`LayoutExt::size`],
    /// whose font scaling would cascade through the whole panel.
    ///
    /// Whatever the variant, the panel never exceeds `PANEL_VIEWPORT_FRAC` of the window, so a
    /// small screen is safe by construction.
    #[heca_grid_ui_macros::prop]
    pub fn panel_size(mut self, size: WidgetSize) -> Self {
        self.panel_size = size;
        self
    }

    /// Set the empty-query placeholder text.
    #[heca_grid_ui_macros::prop]
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        let field = Self::query_field(self.open, self.query_changed, self.query_text(), self.placeholder.clone());
        self.query = field.text();
        *self.base.children.last_mut().expect("palette owns a query child") = crate::WidgetModel::new(field);
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
        // Mark, place the selection and reflow now, so this does not depend on being called before
        // `open` — properties are order-independent, and a mode set after it would otherwise paint
        // the wrong list once.
        self.sync_marks();
        self.sync_initial_selection();
        self.sync_wrap();
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
        self.sync_wrap();
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
