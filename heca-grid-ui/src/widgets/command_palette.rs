//! [`CommandPalette`] — a fuzzy command launcher (overlay).
//!
//! A centered-near-top overlay (same input-capturing contract as
//! [`Select`](super::Select): `overlay_active` +
//! `focusable` only while open, content **drawn + hit-tested manually** on the
//! overlay layer): a query line over a scrollable list of [`Command`]s. Typing
//! filters the list with a **fuzzy subsequence** match (chars in order),
//! **smart-case** (case-insensitive unless the query contains an uppercase
//! letter), ranked, with the matched characters highlighted in the accent.
//!
//! Each [`Command`] carries a label, an **optional** [`Glyph`] icon, an
//! **optional** keybinding hint, and an `on_run` callback; selecting one fires
//! the callback and closes. Navigation is built in (↑/↓ and **Ctrl+J / Ctrl+K**)
//! and also exposed as intents ([`select_next`](CommandPalette::select_next),
//! [`select_prev`](CommandPalette::select_prev),
//! [`run_selected`](CommandPalette::run_selected)) so a host can bind its own
//! configurable keys. Open/close is a host-owned [`Signal<bool>`](crate::reactive::Signal).

use crate::builders::{LayoutExt, Parent};
use crate::style::WidgetSize;
use crate::component::{Base, Component, Event, Handled, Modifiers, PaintCx, WidgetIntent};
use crate::font::MONO_LINE_RATIO;
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::search::{Ranked, SearchAction, SearchModel};
use crate::scene::{Glow, TextAlign, TextStyle};
use crate::style::{Direction, Length};
use crate::widgets::{paint_panel_chrome, Ellipsis, Flex, Glyph, Input, KeyCap, KeycapVariant, Label, PanelChrome, PanelElevation};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::{Cell, RefCell};

/// One command in a [`CommandPalette`].
pub struct Command {
    /// Stable identity, for ranking by past use. **Optional**: a command without one matches and
    /// sorts normally and simply carries no frecency boost. The label cannot serve — a host prefixes
    /// it with the owning component's title, so it changes with mounting.
    id: Option<String>,
    /// Which block this command belongs to while **nothing is typed** — lower sorts first. The
    /// caller's policy, applied only in the browsing case; once there is a query the match decides
    /// and the blocks dissolve.
    group: u16,
    label: String,
    description: Option<String>,
    icon: Option<Glyph>,
    /// One entry per **binding**, each a chord of caps (`[λ] [⇧] [e]`). Several bindings stack, one
    /// per line — an action bound twice really is bound twice, and joining them into `"λ h / λ ←"`
    /// was both wider than the label and impossible to draw as chips.
    chords: Vec<Vec<KeyCap>>,
    /// Is this the row the user is already on? Drawn in the accent so it can be found at a glance.
    /// Distinct from the *selection*, which is where the keyboard is right now.
    current: bool,
    /// Should the selection **start** here when the palette opens with nothing typed?
    preselect: bool,
    /// Which mode lists this command, named by its **search scope** — the same vocabulary the
    /// palette's sigil table and the host's `clear_search_history scope=…` already use, rather than
    /// a second spelling of the same thing. `None` means the default mode, which is what every
    /// command written before modes existed says.
    mode: Option<String>,
    on_run: Box<dyn Fn()>,
}

#[heca_grid_ui_macros::props]
impl Command {
    /// A command with `label` that runs `on_run` when selected.
    pub fn new(label: impl Into<String>, on_run: impl Fn() + 'static) -> Self {
        Self {
            id: None,
            group: 0,
            label: label.into(),
            description: None,
            icon: None,
            chords: Vec::new(),
            current: false,
            preselect: false,
            mode: None,
            on_run: Box::new(on_run),
        }
    }

    /// Mark this row as **the one the user is already on** — the focused pane, the active
    /// workspace. It is drawn in the accent, so you can see where you are in a list of near-identical
    /// names before choosing where to go.
    ///
    /// Not the same thing as the selection: the selection is where the keyboard is and moves as you
    /// type, while this does not move at all. A row can be both, and then it simply reads as
    /// selected — you are on it *and* pointing at it, and there is nothing further to distinguish.
    #[heca_grid_ui_macros::prop]
    pub fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }

    /// Start the selection on this row, when the palette opens with **nothing typed**.
    ///
    /// For a picker, that is the row that makes the default action worth pressing: the pane you
    /// were last in, not the one you are in — landing on the latter would make Enter focus what is
    /// already focused. Deliberately separate from [`current`](Self::current), which says where you
    /// *are*; the two are different rows and each wants its own answer.
    ///
    /// It applies only to an untyped list, because a query is better context than any default: the
    /// first keystroke hands the lead back to the best match. If several rows claim it the first in
    /// the list wins, since a "default" there can be more than one of is not a default.
    #[heca_grid_ui_macros::prop]
    pub fn preselect(mut self, preselect: bool) -> Self {
        self.preselect = preselect;
        self
    }

    /// Which mode this command belongs to, named by the **scope** the palette declared for it
    /// ([`CommandPalette::mode`]). Unset, it belongs to the default mode.
    ///
    /// The scope name and not the sigil: the sigil is how a *user* reaches a mode and lives in the
    /// palette's table, while the scope is what the memory is filed under — one of them is an
    /// input convention and the other is an identity, and a command declares the identity.
    #[heca_grid_ui_macros::prop]
    pub fn mode(mut self, scope: impl Into<String>) -> Self {
        self.mode = Some(scope.into());
        self
    }

    /// An optional second line, muted — what the command does.
    ///
    /// Filtering still matches the **label** only: the description explains a command the user has
    /// already found, and matching it would rank a command whose label the query never mentioned.
    #[heca_grid_ui_macros::prop]
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// A stable identity, so past choices can rank this command. Usually the action's name.
    ///
    /// Optional by design: `rank` takes `Option<&str>`, so a command without an id still matches and
    /// sorts — it just carries no boost. Nothing is forced to grow an identity to be searchable.
    #[heca_grid_ui_macros::prop]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Which block this command sorts into **while the query is empty** — lower first.
    ///
    /// The one thing a caller can say about ordering, and it is deliberately confined to the
    /// browsing case: with nothing typed the only context available is whatever the caller knows
    /// (the command palette leads with the focused component's actions), but the moment something is
    /// typed the query is better context than that and the blocks are dropped.
    #[heca_grid_ui_macros::prop]
    pub fn group(mut self, group: u16) -> Self {
        self.group = group;
        self
    }

    /// An optional leading icon.
    #[heca_grid_ui_macros::prop]
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// Add one **binding**, as the caps of its chord — `[Text("λ"), Nf(Shift), Text("e")]` draws
    /// `[λ] [⇧] [e]`, right-aligned in the list's reserved shortcut column.
    ///
    /// Call it once per binding: a second call stacks a second row of chips under the first, rather
    /// than replacing it or running the two together on one line.
    #[heca_grid_ui_macros::host_only("a composed value, not a scalar")]
    pub fn keys(mut self, chord: impl IntoIterator<Item = KeyCap>) -> Self {
        let chord: Vec<KeyCap> = chord.into_iter().collect();
        if !chord.is_empty() {
            self.chords.push(chord);
        }
        self
    }
}

/// Narrowest the panel is allowed to be — and it yields to a window narrower than itself.
const PANEL_MIN_W: f64 = 360.0;
/// The **most** of the window a panel may ever take, so it never touches the edges — the guard that
/// makes a small screen safe. Applied after the size variant's cap, and it wins.
const PANEL_VIEWPORT_FRAC: f64 = 0.92;
/// Panel top offset as a fraction of the viewport height (palettes sit high).
const TOP_FRAC: f64 = 0.12;
/// Room kept clear **below** the panel, as a fraction of the viewport height. `TOP_FRAC` is an
/// offset, not a margin: it guards the top and says nothing about the bottom, which is how the list
/// came to fill the window down to its last pixel and the final row ended up flush against the
/// screen edge.
///
/// Deliberately **wider than the gap [`PANEL_VIEWPORT_FRAC`] leaves at the sides** (4%). Matching
/// the sides was tried first and still read as touching: the panel's bottom edge landed on the top
/// of the bottom pane, so the gap disappeared into a boundary that was already there instead of
/// separating the panel from it.
const BOTTOM_FRAC: f64 = 0.08;
/// Panel inner padding.
const PAD: f64 = 12.0;
/// Vertical padding inside the query line and each result row.
const QUERY_PAD_Y: f64 = 9.0;
const ROW_PAD_Y: f64 = 6.0;
/// Horizontal inset of row content.
const ROW_PAD_X: f64 = 10.0;
/// Gap between an icon and the label.
const ICON_GAP: f64 = 10.0;
/// Command icon size as a fraction of the row font — a touch larger than the text, so the glyph
/// reads as the row's subject rather than as punctuation beside it.
const ICON_FONT_MUL: f32 = 1.3;
/// Gap between two keycap chips of one chord.
const CAP_GAP: f64 = 3.0;
/// Minimum gap between a label and the reserved shortcut column.
const SHORTCUT_GAP: f64 = 16.0;
/// Keycap font as a fraction of the row font — a chip reads as an annotation, not as content.
const KEYCAP_FONT_MUL: f32 = 0.85;
/// What a [`WidgetSize`] means for the panel: how wide it may get, and how many rows it shows.
///
/// The caller picks a **semantic** size (a `[settings] command_palette_size` of `small` / `normal` /
/// `large`); the widget owns the pixels, per the styling contract. Deliberately **width and row
/// count only** — the text stays at reading size at every variant, which is why this is a small
/// table here rather than `Style::size`, whose font scaling would cascade to the whole panel.
const fn panel_metrics(size: WidgetSize) -> (f64, usize) {
    match size {
        WidgetSize::Small => (560.0, 6),
        WidgetSize::Normal => (700.0, 8),
        WidgetSize::Large | WidgetSize::Header => (1000.0, 12),
    }
}

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
    /// The query field — a real [`Input`], so editing (selection, word/line
    /// delete, multi-click, caret) comes for free. Driven manually since the
    /// palette is overlay-drawn: its bounds/font/focus are set at paint time.
    query: RefCell<Input>,
    selected: usize,
    scroll: usize,
    placeholder: String,
    /// How roomy the panel is — width and row count, never font. Set from the app's
    /// `[settings] command_palette_size`.
    panel_size: WidgetSize,
    open: Signal<bool>,
    modifiers: Modifiers,
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

impl CommandPalette {
    /// A new, empty (closed) palette.
    pub fn new() -> Self {
        Self {
            base: Base::new(),
            commands: Vec::new(),
            modes: Vec::new(),
            query: RefCell::new(Input::new()),
            selected: 0,
            scroll: 0,
            placeholder: "Type a command…".to_string(),
            panel_size: WidgetSize::Normal,
            open: signal(false),
            modifiers: Modifiers::default(),
            viewport: Cell::new(Size::new(f64::MAX, f64::MAX)),
            panel: Cell::new(Rectangle::from_size(Size::new(0.0, 0.0))),
            search: SearchModel::detached("command"),
            natural_h: Vec::new(),
            title_marks: Vec::new(),
            title_text: Vec::new(),
            desc_wrap: Vec::new(),
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
        self.base.children.push(Box::new(column));
        self.commands.push(c);
        self
    }

    /// Declare a mode: typing `sigil` as the query's first character switches the list to the
    /// commands of `scope`, and the palette's memory to that scope's.
    ///
    /// **The first mode declared is the default** — where an unsigiled query lands — so give it a
    /// sigil too and it can be returned to explicitly. Declare none and the palette has exactly one
    /// mode, [`search`](Self::search)'s scope, with no sigil to type.
    ///
    /// The commands of every mode stay in **one flat list**, filtered: their text is built as real
    /// children indexed by position, so a list per mode would mean re-indexing them on each switch.
    #[heca_grid_ui_macros::host_only("a sigil paired with a scope, not a scalar")]
    pub fn mode(mut self, sigil: char, scope: impl Into<String>) -> Self {
        self.modes.push((sigil, scope.into()));
        self
    }

    /// The text column of row `i` — the child holding its title and description.
    fn row_child(&self, i: usize) -> &dyn Component {
        self.base.children[i].as_ref()
    }

    /// How roomy the panel is: `Small` / `Normal` / `Large` decide its **maximum width and how many
    /// rows it shows**, and nothing else — the text stays at reading size at every size, which is why
    /// this is its own property rather than [`LayoutExt::size`](crate::builders::LayoutExt::size),
    /// whose font scaling would cascade through the whole panel.
    ///
    /// Whatever the variant, the panel never exceeds [`PANEL_VIEWPORT_FRAC`] of the window, so a
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
        self
    }

    /// Open with the query already filled in — including its sigil, which is how a caller opens the
    /// palette **in a mode**: `"@"` is the pane list, `"@nvim"` is the pane list already narrowed.
    ///
    /// There is deliberately no separate "open in mode" API. The sigil is the mode selector, so a
    /// prefilled query is the only mechanism needed, and a caller that wants a mode and a caller
    /// that wants a search are doing the same thing.
    #[heca_grid_ui_macros::prop]
    pub fn query(mut self, text: impl Into<String>) -> Self {
        self.query.borrow_mut().set_value(text.into());
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
    pub fn open(mut self, open: bool) -> Self {
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

    /// Each row's title signal, in the order the commands were added — for a host whose rows track
    /// something that changes while the palette is open (a pane's running program, a rename).
    ///
    /// Writing one **renames that row in place**: the label re-marks itself and nothing else is
    /// rebuilt, so the list never flashes. [`results`](Self::results) reads these back, so a row
    /// renamed this way is also matched by its new name rather than the one it was built with.
    pub fn label_signals(&self) -> &[Signal<String>] {
        &self.title_text
    }

    fn is_open(&self) -> bool {
        self.open.get_untracked()
    }

    /// The current query text (read from the [`Input`]).
    fn query_text(&self) -> String {
        self.query.borrow().value_str()
    }

    /// The scope of the default mode — the first declared, or the model's own when none are.
    fn default_scope(&self) -> &str {
        match self.modes.first() {
            Some((_, scope)) => scope.as_str(),
            None => self.search.scope(),
        }
    }

    /// Every scope this palette can be in, default first.
    fn scopes(&self) -> Vec<String> {
        match self.modes.is_empty() {
            true => vec![self.search.scope().to_string()],
            false => self.modes.iter().map(|(_, s)| s.clone()).collect(),
        }
    }

    /// Which mode the query is in: the **sigil actually typed** (`None` when the default mode was
    /// reached without one), its scope, and the query with that sigil removed.
    ///
    /// The sigil is read off the first character and nothing else — a leading character that is not
    /// in the table is not a sigil, it is the first letter of a search, and is matched as one.
    fn active(&self) -> (Option<char>, String, String) {
        let raw = self.query_text();
        let mut rest = raw.chars();
        if let Some(first) = rest.next()
            && let Some((sigil, scope)) = self.modes.iter().find(|(s, _)| *s == first)
        {
            return (Some(*sigil), scope.clone(), rest.as_str().to_string());
        }
        (None, self.default_scope().to_string(), raw)
    }

    /// Does `cmd` belong to `scope`? A command that named no mode belongs to the default one.
    fn in_mode(&self, cmd: &Command, scope: &str) -> bool {
        match &cmd.mode {
            Some(mode) => mode == scope,
            None => scope == self.default_scope(),
        }
    }

    /// Put the active sigil back in front of `text` — what a history recall produces, since the
    /// history stores the **effective** query and the field shows the whole thing.
    fn with_sigil(sigil: Option<char>, text: &str) -> String {
        match sigil {
            Some(s) => format!("{s}{text}"),
            None => text.to_string(),
        }
    }

    /// The current filtered + ranked results — **entirely the model's answer**.
    ///
    /// Matching, smart-case and the ranking by past use all live in
    /// [`search`](crate::search); this reads the order back. The one thing decided here is the
    /// block ordering, and only while nothing is typed: with an empty query every match scores the
    /// same and the caller's groups are the only context there is, but a typed query is better
    /// context than a group and dissolves it.
    fn results(&self) -> Vec<Ranked> {
        let (_, scope, query) = self.active();
        // Only the active mode's commands are ranked, and the ids that come back are mapped to
        // indices into the **whole** list — the children, the marks and the measured heights are all
        // keyed by position in `commands`, so the filter must not renumber them.
        let of_mode: Vec<usize> = self
            .commands
            .iter()
            .enumerate()
            .filter(|(_, c)| self.in_mode(c, &scope))
            .map(|(i, _)| i)
            .collect();
        // Read each row's **live** text, not the label it was built with: a host may be following a
        // pane's process or a rename, and a list that shows one name while matching another is
        // worse than a stale one. The labels are short and there are a few hundred at most, so
        // materialising them per keystroke costs nothing measurable.
        let items: Vec<(Option<&str>, String)> = of_mode
            .iter()
            .map(|&i| (self.commands[i].id.as_deref(), self.title_text[i].get_untracked()))
            .collect();
        let mut out = self
            .search
            .with_scope(scope)
            .rank(&items, &query, |(id, text)| (*id, text.as_str()));
        for r in &mut out {
            r.index = of_mode[r.index];
        }
        if query.is_empty() {
            // Stable, so the ranking inside each block survives.
            out.sort_by_key(|r| self.commands[r.index].group);
        }
        out
    }

    /// Move the selection down (Ctrl+J / ↓), keeping it on-screen.
    pub fn select_next(&mut self) {
        let n = self.results().len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + 1).min(n - 1);
        self.follow_selection();
        self.sync_wrap();
    }

    /// Move the selection up (Ctrl+K / ↑).
    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.follow_selection();
        self.sync_wrap();
    }

    /// Run the selected command (fires its callback) and close.
    pub fn run_selected(&mut self) {
        let results = self.results();
        if let Some(m) = results.get(self.selected) {
            let cmd = &self.commands[m.index];
            // The query as typed and the command's identity — remembered only on a **run**. An
            // abandoned search is not a search anyone wants back, so `close()` records nothing.
            // **Without the sigil, into the active mode's scope**: the sigil selected which memory
            // this is, so storing it inside that memory would put it back in every recalled query.
            let (_, scope, query) = self.active();
            let id = cmd.id.clone();
            (cmd.on_run)();
            self.search.with_scope(scope).record_run(&query, id.as_deref());
        }
        self.close();
    }

    /// Keep `selected` within the visible scroll window.
    fn follow_selection(&mut self) {
        let visible = self.visible_rows(&self.results());
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + visible {
            self.scroll = self.selected + 1 - visible;
        }
    }

    /// How many rows the panel shows: the size variant's count, **and never more than the window
    /// can hold with [`BOTTOM_FRAC`] still clear beneath it**.
    ///
    /// It measures the rows it would actually draw, from `scroll` onward, instead of dividing the
    /// room by a nominal row height. A row is two lines when the list is described and taller again
    /// when an action carries several bindings, so a one-line estimate over-counted and the panel ran
    /// off the bottom of a short window — which is what this looked like in the app.
    fn visible_rows(&self, results: &[Ranked]) -> usize {
        let (_, max_rows) = panel_metrics(self.panel_size);
        let vp = self.viewport.get();
        if !vp.h.is_finite() {
            return results.len().clamp(1, max_rows);
        }
        // Everything the panel spends before the first row: its own padding above and below the
        // query line, the query line itself, and the padding under the list.
        let chrome = 3.0 * PAD + self.line_h() + 2.0 * QUERY_PAD_Y;
        // The panel starts at `vh * TOP_FRAC`, so filling the remaining `1 - TOP_FRAC` put its
        // bottom exactly on the window's edge. `BOTTOM_FRAC` is the room kept clear under it.
        let mut room = (vp.h * (1.0 - TOP_FRAC - BOTTOM_FRAC) - chrome).max(0.0);
        let mut fits = 0usize;
        for m in results.iter().skip(self.scroll).take(max_rows) {
            let h = self.row_h(m.index);
            if h > room && fits > 0 {
                break;
            }
            room -= h;
            fits += 1;
        }
        // Always at least one row: a window too short for even that is better showing a clipped row
        // than an empty panel.
        results.len().clamp(1, fits.max(1))
    }

    fn line_h(&self) -> f64 {
        (self.base.font * MONO_LINE_RATIO) as f64
    }

    /// Does any command carry a description? Then **every** row is two lines high.
    ///
    /// One height for the whole list, not per row: `row_rect` is the same rectangle the paint, the
    /// hover-select and the click hit-test all compute, and rows of differing heights would make
    /// that an index-dependent running sum in three places. A described list simply leaves the
    /// second line of an undescribed row empty.
    ///
    /// Asked of the **active mode only**: pane rows carry no description, so charging them a second
    /// line because the action list has one would leave every row in `@` mode half empty.
    fn two_line_rows(&self) -> bool {
        let (_, scope, _) = self.active();
        self.commands
            .iter()
            .filter(|c| self.in_mode(c, &scope))
            .any(|c| c.description.is_some())
    }

    /// How many lines **this** row needs: its text (label, plus a description if the list has any)
    /// or its stack of bindings, whichever is taller.
    ///
    /// Per row, **not** one height for the list. The uniform version was simpler — `row_rect` is
    /// shared by the paint, the hover-select and the click hit-test, and one height makes it
    /// arithmetic instead of a running sum — but it charges every row for the worst row: one action
    /// bound three times (`paste_clipboard` is) made all 100 rows three lines tall, which is exactly
    /// the vertical sprawl this fixes. The running sum lives in [`row_rects`](Self::row_rects) and
    /// all three callers read it there, so it is still written once.
    fn row_lines(&self, cmd: &Command) -> usize {
        let text = if self.two_line_rows() { 2 } else { 1 };
        text.max(cmd.chords.len()).max(1)
    }

    /// Height of one result row — **measured**, not computed.
    ///
    /// The text column is a real child, so its height already accounts for a description that
    /// wrapped onto three lines; reading it here is the whole of the reflow. Before the first layout
    /// pass — the palette opens and paints in the same frame — it falls back to the font-derived
    /// estimate, exactly as [`Select`](super::Select) does with its own rows.
    fn row_h(&self, cmd: usize) -> f64 {
        let measured = self.natural_h.get(cmd).copied().unwrap_or(0.0);
        let text_h = if measured > 0.0 {
            measured
        } else {
            self.line_h() * self.row_lines(&self.commands[cmd]) as f64
        };
        // A row bound three times is taller than its text: the chips stack one per line.
        let chips_h = self.line_h() * self.commands[cmd].chords.len().max(1) as f64;
        text_h.max(chips_h) + 2.0 * ROW_PAD_Y
    }

    /// The panel's width for the current viewport — extracted from [`layout`](Self::layout) because
    /// [`remeasure`](Component::remeasure) needs it too, and the two must never disagree about how
    /// wide the text is allowed to be.
    fn panel_w(&self) -> f64 {
        let (want_w, _) = panel_metrics(self.panel_size);
        let vp = self.viewport.get();
        if !vp.w.is_finite() {
            return want_w;
        }
        let room = vp.w * PANEL_VIEWPORT_FRAC;
        want_w.min(room).max(PANEL_MIN_W.min(room))
    }

    /// The icon column's width — reserved whether or not a row has an icon.
    fn icon_col(&self) -> f64 {
        (self.base.font * ICON_FONT_MUL) as f64 + ICON_GAP
    }

    /// The width the row text is laid out at: the panel's inside, less the icon column and the
    /// reserved shortcut column. **The labels cut and wrap against exactly this**, because it is the
    /// width the engine gives the text children in `remeasure`.
    fn text_w(&self) -> f64 {
        (self.panel_w()
            - 2.0 * PAD
            - 2.0 * ROW_PAD_X
            - self.icon_col()
            - self.shortcut_col_w()
            - SHORTCUT_GAP)
            .max(0.0)
    }

    /// Move each visible row's text column into its slot in the panel, and collapse the rest.
    ///
    /// The engine lays the children out in the palette's own flow and measures them there; it cannot
    /// place them in an overlay panel, because a taffy node sits where its parent puts it. So this
    /// bakes an **absolute** target into each child's bounds — the same thing
    /// [`Select::place_options`](super::Select) does, and idempotent for the same reason: re-running
    /// it never compounds an offset.
    ///
    /// Rows outside the visible window collapse to zero size, so no stale rect is left behind.
    fn place_rows(&mut self) {
        let results = self.results();
        let (panel, _query, list_top) = self.layout(&results);
        let dx = panel.loc.x + PAD + ROW_PAD_X + self.icon_col();
        let mut targets: Vec<Option<Rectangle>> = vec![None; self.base.children.len()];
        for (ri, row) in self.row_rects(&results, panel, list_top) {
            let cmd = results[ri].index;
            let h = self.base.children[cmd].base().bounds.size.h;
            targets[cmd] = Some(Rectangle::new(
                Point::new(dx, row.loc.y + ROW_PAD_Y),
                Size::new(self.text_w(), h),
            ));
        }
        for (i, target) in targets.into_iter().enumerate() {
            let Some(target) = target else {
                self.base.children[i].base_mut().bounds.size = Size::new(0.0, 0.0);
                continue;
            };
            let child = self.base.children[i].as_mut();
            let d = (
                target.loc.x - child.base().bounds.loc.x,
                target.loc.y - child.base().bounds.loc.y,
            );
            if d.0 != 0.0 || d.1 != 0.0 {
                crate::component::shift_subtree(child, d.0, d.1);
            }
            child.base_mut().bounds = target;
        }
    }

    /// Move the match marks onto the labels that matched. Cheap enough per keystroke: it writes a
    /// signal per row rather than rebuilding a hundred labels.
    /// Reflow the selected row's description and cut every other. Called wherever the selection can
    /// move, because the reflow **is** the selection's visual: the row grows and the rest slide down.
    fn sync_wrap(&mut self) {
        let selected = self.results().get(self.selected).map(|m| m.index);
        for (i, wrap) in self.desc_wrap.iter().enumerate() {
            if let Some(wrap) = wrap {
                wrap.set(Some(i) == selected);
            }
        }
    }

    /// Put the selection on the row that asked for it — **only while nothing is typed**.
    ///
    /// A query is better context than any default, so the first keystroke hands the lead back to
    /// the best match (`on_query_changed` resets to 0 and never calls this). Which makes it an
    /// open-time rule, the same shape as [`Command::group`]'s ordering.
    ///
    /// It runs after the results are known rather than over `commands`, because the row's *position*
    /// is what a selection is, and that position comes from the ranking.
    fn sync_initial_selection(&mut self) {
        let (_, _, query) = self.active();
        if !query.is_empty() {
            return;
        }
        let results = self.results();
        let Some(at) = results
            .iter()
            .position(|m| self.commands[m.index].preselect)
        else {
            return;
        };
        self.selected = at;
        // The preselected row can rank anywhere, so the list may have to open scrolled to reach it.
        self.follow_selection();
    }

    fn sync_marks(&mut self) {
        for signal in &self.title_marks {
            signal.set(Vec::new());
        }
        for m in self.results() {
            self.title_marks[m.index].set(m.hits.clone());
        }
    }

    /// The rectangle of every **visible** row, paired with its index in `results`.
    ///
    /// The one place row geometry is computed. Paint draws these, hover-select and the click
    /// hit-test read them — so a row can never be drawn in one place and clicked in another.
    fn row_rects(&self, results: &[Ranked], panel: Rectangle, list_top: f64) -> Vec<(usize, Rectangle)> {
        let mut out = Vec::new();
        let mut y = list_top;
        let window = results
            .iter()
            .enumerate()
            .skip(self.scroll)
            .take(self.visible_rows(results));
        for (ri, m) in window {
            let h = self.row_h(m.index);
            out.push((
                ri,
                Rectangle::new(
                    Point::new(panel.loc.x + PAD, y),
                    Size::new(panel.size.w - 2.0 * PAD, h),
                ),
            ));
            y += h;
        }
        out
    }

    /// Width of one chord's chips, laid end to end.
    fn chord_w(&self, chord: &[KeyCap]) -> f64 {
        let font = self.keycap_font();
        let caps: f64 = chord.iter().map(|c| c.size(font).w).sum();
        caps + CAP_GAP * chord.len().saturating_sub(1) as f64
    }

    /// The width **reserved on the right of every row** for bindings — the widest chord in the whole
    /// list, so the labels of all rows end at the same place and the chips line up in a column
    /// instead of tracking each label's length.
    ///
    /// The **active mode's** widest, for the same reason the row height is the active mode's: a
    /// pane list binds nothing, and reserving the action list's keycap column across it would spend
    /// that width on empty space.
    fn shortcut_col_w(&self) -> f64 {
        let (_, scope, _) = self.active();
        self.commands
            .iter()
            .filter(|c| self.in_mode(c, &scope))
            .flat_map(|c| c.chords.iter())
            .map(|chord| self.chord_w(chord))
            .fold(0.0, f64::max)
    }

    /// Keycap chips are drawn a touch smaller than the row text, like the context menu's quick-pick.
    fn keycap_font(&self) -> f32 {
        self.base.font * KEYCAP_FONT_MUL
    }

    fn close(&mut self) {
        self.open.set(false);
        self.query.borrow_mut().set_value("");
        self.selected = 0;
        self.scroll = 0;
    }

    /// The user typed: reset the filter/selection, move the marks with it, and **leave the history
    /// walk** — the field is theirs again.
    ///
    /// It ends the walk in **every** mode, not just the active one: there is a single field, and
    /// typing a sigil switches mode with a keystroke — leaving the mode just left parked mid-walk
    /// would make its next `Ctrl+p` resume from the middle of its history.
    fn on_query_changed(&mut self) {
        for scope in self.scopes() {
            self.search.with_scope(scope).query_changed();
        }
        self.on_query_replaced();
    }

    /// The query changed without the user typing it — a history recall. Everything
    /// [`on_query_changed`](Self::on_query_changed) does **except** ending the walk: resetting the
    /// cursor here would make a second step back start again from the newest entry, so walking the
    /// history would be impossible past its first step.
    fn on_query_replaced(&mut self) {
        self.selected = 0;
        self.scroll = 0;
        self.sync_marks();
        self.sync_wrap();
    }

    /// Panel + query + first-row geometry for the current viewport + result count.
    fn layout(&self, results: &[Ranked]) -> (Rectangle, Rectangle, f64) {
        let vp = self.viewport.get();
        let line = self.line_h();
        let query_h = line + 2.0 * QUERY_PAD_Y;
        let n_results = results.len();
        let visible = self.visible_rows(results);
        // The panel is as tall as the rows it will actually show — summed, since a row with two
        // bindings is taller than one with none.
        let empty_row = line + 2.0 * ROW_PAD_Y;
        let list_h = match n_results {
            0 => empty_row,
            _ => results
                .iter()
                .skip(self.scroll)
                .take(visible)
                .map(|m| self.row_h(m.index))
                .sum(),
        };

        // **The size decides the width; the window only takes it away** — see `panel_w`, which
        // `remeasure` shares so the text is laid out at the width it will be drawn at.
        let panel_w = self.panel_w();
        let panel_h = PAD + query_h + PAD + list_h + PAD;
        let (vw, vh) = if vp.w.is_finite() {
            (vp.w, vp.h)
        } else {
            (panel_w, panel_h)
        };
        let px = (vw - panel_w) / 2.0;
        let py = vh * TOP_FRAC;
        let panel = Rectangle::new(Point::new(px, py), Size::new(panel_w, panel_h));
        let query = Rectangle::new(
            Point::new(px + PAD, py + PAD),
            Size::new(panel_w - 2.0 * PAD, query_h),
        );
        let list_top = query.loc.y + query_h + PAD;
        (panel, query, list_top)
    }

}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for CommandPalette {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn focusable(&self) -> bool {
        self.is_open()
    }

    fn overlay_active(&self) -> bool {
        self.is_open()
    }

    /// An open palette grabs input for the whole viewport (typing, nav, outside
    /// click = dismiss), so every point is occluded while open — a host must not
    /// synthesize a page-level action (e.g. open a context menu) under it.
    fn overlay_occludes(&self, _pos: Point) -> bool {
        self.is_open()
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() || !self.is_open() {
            return;
        }
        self.viewport.set(cx.viewport());
        // NB: the panel's own surface fill + corner radius are read by the shared
        // `paint_panel_chrome`, so they are deliberately not pulled out here.
        let (background, accent, glow_c, foreground, muted, ctrl_radius) = {
            let t = cx.theme();
            (
                t.colors.background,
                t.colors.accent,
                t.colors.glow,
                t.colors.foreground,
                t.colors.muted,
                t.colors.control_radius(),
            )
        };
        let font = self.base.font;
        let line = self.line_h();
        let results = self.results();
        let (panel, query, list_top) = self.layout(&results);
        // Remember the panel so the idle caret blink can damage just this rect.
        self.panel.set(panel);

        cx.with_overlay(|cx| {
            // Scrim + panel.
            let vp = self.viewport.get();
            let scrim = if vp.w.is_finite() {
                Rectangle::new(Point::new(0.0, 0.0), vp)
            } else {
                panel
            };
            cx.rect(scrim, background.with_alpha(cx.theme().colors.interaction.scrim), None, 0.0, None);
            // The SHARED overlay panel chrome (drop shadow + theme surface fill +
            // bracket reticle) so the palette reads as the same surface as every
            // other overlay panel, plus its own accent edge and glow. The scrim
            // above is the blocking LAYER's, not part of the panel chrome.
            let panel_border = cx.border(accent.with_alpha(cx.theme().colors.interaction.panel_border));
            paint_panel_chrome(
                cx,
                panel,
                PanelChrome {
                    border: panel_border,
                    glow: Some(Glow {
                        color: glow_c,
                        radius: 12.0,
                        intensity: 0.3,
                    }),
                    elevation: PanelElevation::Panel,
                },
            );

            // Query line: a real Input, positioned + focused + painted manually
            // (it draws its own box, caret, selection, and text).
            {
                let mut q = self.query.borrow_mut();
                q.base_mut().bounds = query;
                q.base_mut().font = font;
                q.base_mut().focused.set(true); // so the caret shows + blinks
                q.paint(cx);
            }
            // The Input hides its placeholder while focused; draw ours when empty.
            if self.query_text().is_empty() {
                let q_text = Rectangle::new(
                    Point::new(query.loc.x + ROW_PAD_X, query.loc.y),
                    Size::new(query.size.w - 2.0 * ROW_PAD_X, query.size.h),
                );
                cx.text(
                    q_text,
                    &self.placeholder,
                    muted,
                    font,
                    TextAlign::Start,
                    TextStyle::REGULAR,
                );
            }

            // Result rows (the visible scroll window), from the one geometry helper.
            for (ri, row) in self.row_rects(&results, panel, list_top) {
                let m = &results[ri];
                let cmd = &self.commands[m.index];
                let is_sel = ri == self.selected;
                if is_sel {
                    let row_border = cx.border(accent.with_alpha(cx.theme().colors.interaction.panel_row_border));
                    cx.rect(row, accent.with_alpha(cx.theme().colors.interaction.panel_row_fill), row_border, ctrl_radius, None);
                    // Left accent bar.
                    cx.rect(
                        Rectangle::new(
                            Point::new(row.loc.x, row.loc.y + row.size.h * 0.2),
                            Size::new(2.5, row.size.h * 0.6),
                        ),
                        accent,
                        None,
                        1.0,
                        None,
                    );
                }
                // The line the label, its match highlights, the icon and the shortcut all share.
                // In a one-line list that is the whole row (unchanged); in a described list it is
                // the top line, with the description under it — so every element of the row keeps
                // one vertical rule instead of each re-deriving it.
                let first_line = Rectangle::new(
                    Point::new(row.loc.x, row.loc.y + ROW_PAD_Y),
                    Size::new(row.size.w, line),
                );
                // Icon. The column is reserved **whether or not this row has one**, so a list where
                // some commands carry a glyph and some do not still reads as one column of text —
                // indenting only the iconed rows left the others starting at the panel edge.
                let icon_x = row.loc.x + ROW_PAD_X;
                let isz = font * ICON_FONT_MUL;
                if let Some(ch) = cmd.icon.and_then(|g| g.primary_char()) {
                    let irect = Rectangle::new(
                        Point::new(icon_x, first_line.loc.y),
                        Size::new(isz as f64, first_line.size.h),
                    );
                    // The accent says "here": the selected row, and the row you are already on.
                    cx.icon(
                        irect,
                        &ch.to_string(),
                        if is_sel || cmd.current { accent } else { muted },
                        isz,
                    );
                }
                // **The row's text is a child.** A title `Label` (cut to its box, its match
                // marks a property) over an optional description `Label` (wrapped) — placed by
                // `place_rows` and painted here under the row's content colour, so both track the
                // selection without either being told what a selection is. Truncation, wrapping and
                // the marks are the label's; this widget draws no text.
                let content = if is_sel {
                    foreground
                } else if cmd.current {
                    // Where you already are, when the keyboard is somewhere else. Selection still
                    // wins on the row it is on: two accents on one row would say nothing.
                    accent
                } else {
                    muted.lerp(foreground, 0.7)
                };
                cx.with_content_color(content, |cx| self.row_child(m.index).paint(cx));
                // Bindings: one row of keycap chips per binding, stacked down the row, each
                // right-aligned to the same reserved column so every command's chips line up.
                let cap_font = self.keycap_font();
                let right_edge = row.loc.x + row.size.w - ROW_PAD_X;
                for (ci, chord) in cmd.chords.iter().enumerate() {
                    let line_top = first_line.loc.y + ci as f64 * line;
                    let mut x = right_edge - self.chord_w(chord);
                    for cap in chord {
                        let cs = cap.size(cap_font);
                        let chip = Rectangle::new(
                            Point::new(x, line_top + (line - cs.h) / 2.0),
                            cs,
                        );
                        // The bordered chip — the same primitive the context-menu quick-pick draws,
                        // never hand-rolled here.
                        cap.paint(
                            cx,
                            chip,
                            cap_font,
                            Some(if is_sel { accent } else { muted }),
                            KeycapVariant::Bordered,
                        );
                        x += cs.w + CAP_GAP;
                    }
                }
            }
        });
    }

    /// Owns its walk. While open it grabs the viewport — typing, nav and outside-click dismissal —
    /// and it paints its row children itself, in the overlay panel it positions them into.
    /// It types: the query field is its own, and every character re-filters the list.
    fn takes_text_input(&self) -> bool {
        true
    }

    fn routes_own_subtree(&self) -> bool {
        true
    }

    /// Lay the row text out at **the width it will be drawn at**, in a column.
    ///
    /// This is what makes a description wrap correctly: the engine measures each text child against
    /// this width, so the line count it reports is the line count that will be painted. The viewport
    /// is the one cached at the last paint, so the very first frame falls back to the size variant's
    /// nominal width and settles on the next — the same one-frame settle `Select` documents for its
    /// measured rows.
    fn remeasure(&mut self) {
        // Reflow the selected row **before** the children are measured. `build` calls this on a node
        // and only then descends into its children, so a wrap flipped here is the wrap they measure
        // with — flipping it after layout would show the previous selection's shape for a frame.
        self.sync_wrap();
        // The palette **fills the viewport**, like every other layer root, so `on_layout` can read
        // its own size and learn the viewport from the layout pass rather than waiting for a paint.
        self.base.style.layout.direction = Direction::Column;
        self.base.style.layout.width = Length::Pct(1.0);
        self.base.style.layout.height = Length::Pct(1.0);
        // The text children carry the width instead: the engine measures each against the width it
        // will be drawn at, so the line count it reports is the line count that gets painted.
        let w = self.text_w() as f32;
        for child in self.base.children.iter_mut() {
            child.base_mut().style.layout.width = Length::Px(w);
        }
    }

    /// Layout just reset every row to its natural position and size: re-read the measured heights
    /// from it, then place the rows into the panel again.
    fn on_layout(&mut self) {
        // The root fills the viewport, so this is the viewport — known one whole frame earlier than
        // the paint that used to be the only source. Without it the first frame after the palette
        // opens places its rows against a fallback panel and the text lands off the panel.
        let size = self.base.bounds.size;
        if size.w > 0.0 && size.h > 0.0 && size.w.is_finite() && size.h.is_finite() {
            self.viewport.set(size);
        }
        self.natural_h = self
            .base
            .children
            .iter()
            .map(|c| c.base().bounds.size.h)
            .collect();
        self.place_rows();
    }

    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        // Track modifiers even while closed; keep the query field's copy in sync
        // (it needs them for word/line delete). Observe, don't consume.
        if let Event::ModifiersChanged(m) = ev {
            self.modifiers = *m;
            crate::component::dispatch(&mut *self.query.borrow_mut(), ev);
            return Handled::No;
        }
        if !self.is_open() {
            return Handled::No;
        }
        match ev {
            // Nav is host-resolved from the configurable `[keys.widgets]` bindings and
            // arrives as a semantic `WidgetIntent` — the palette carries NO hardcoded nav
            // keys. Handled before the query field so a nav key never types. A vertical
            // list: `MenuUp`/`MenuDown` (not the horizontal `Item*`).
            Event::Widget(intent) => match intent {
                WidgetIntent::Dismiss => {
                    self.close();
                    Handled::Yes
                }
                WidgetIntent::Activate => {
                    self.run_selected();
                    Handled::Yes
                }
                WidgetIntent::MenuDown => {
                    self.select_next();
                    Handled::Yes
                }
                WidgetIntent::MenuUp => {
                    self.select_prev();
                    Handled::Yes
                }
                // **Handled before the `_` arm below**, which forwards anything unlisted to the
                // query field — an `Input` would ignore these and the key would fall through.
                // The walk itself is the model's; this only applies the answer.
                // Walks the **active mode's** history: `Ctrl+p` under `@` offers panes searched
                // for, never command queries. The stored entries carry no sigil, so the one
                // currently typed is put back in front before the field is set — otherwise a
                // recall would drop the mode the user is standing in.
                WidgetIntent::MenuHistoryUp | WidgetIntent::MenuHistoryDown => {
                    let (sigil, scope, query) = self.active();
                    let recalled = self.search.with_scope(scope).handle(*intent, &query);
                    if let SearchAction::SetQuery(text) = recalled
                        && text != query
                    {
                        self.query
                            .borrow_mut()
                            .set_value(Self::with_sigil(sigil, &text));
                        self.on_query_replaced();
                    }
                    Handled::Yes
                }
                // **Everything else goes to the query field** — `Edit*` above all
                // (`edit_select_all`, `edit_delete_back`, `edit_delete_to_line_start`), which
                // `Input` already implements. Forwarded rather than listed, so an intent added to
                // the vocabulary later reaches the field without a change here; the palette owns
                // only the four it answers above. This is the same **field-first** rule
                // [`Dialog`](super::Dialog) applies through its `FocusManager` — the palette's query
                // lives off-tree, so it forwards by hand.
                _ => {
                    let before = self.query_text();
                    let handled = crate::component::deliver(&mut *self.query.borrow_mut(), ev);
                    if self.query_text() != before {
                        self.on_query_changed();
                    }
                    handled
                }
            },
            // Typed text goes to the query field, exactly as a key does — and for the same reason:
            // the palette is a surface around a field, not a text widget of its own.
            Event::TextInput(_) => {
                let before = self.query_text();
                let handled = crate::component::deliver(&mut *self.query.borrow_mut(), ev);
                if self.query_text() != before {
                    self.on_query_changed();
                }
                handled
            }
            Event::Key { pressed: true, .. } => {
                // The query field owns editing keys (typing, selection, char/word/line delete,
                // caret moves). Return **what the field did**: a single-line `Input` ignores
                // ArrowUp/Down/Enter (returns `No`), so those fall through to the host, which
                // resolves them to a `WidgetIntent` (MenuUp/MenuDown/Activate). Modal capture is
                // the host's job — do NOT hardcode `Handled::Yes` here.
                let before = self.query_text();
                let handled = crate::component::deliver(&mut *self.query.borrow_mut(), ev);
                if self.query_text() != before {
                    self.on_query_changed();
                }
                handled
            }
            // Rows drawn from data, hit-tested inside a panel the router already put the pointer
            // over (`hit_bounds`) — a move anywhere else never reaches this widget.
            Event::PointerMove(p) => {
                // Hover-select a row.
                let results = self.results();
                let (panel, _q, list_top) = self.layout(&results);
                let mut moved = false;
                for (ri, row) in self.row_rects(&results, panel, list_top) {
                    if row.contains(p.pos) {
                        moved = self.selected != ri;
                        self.selected = ri;
                        break;
                    }
                }
                if moved {
                    self.sync_wrap();
                }
                Handled::Yes
            }
            Event::PointerDown(p) => {
                let results = self.results();
                let (panel, query_rect, list_top) = self.layout(&results);
                // A click on the query line places the caret / selects (the Input
                // needs its current bounds + font to hit-test the char position).
                // Delivered by hand, to a field the palette holds off-tree and places itself —
                // the one case where a resolved event is handed to a widget the router could not
                // have found, and the rect above is the guard that makes it honest.
                if query_rect.contains(p.pos) {
                    let font = self.base.font;
                    let mut q = self.query.borrow_mut();
                    q.base_mut().bounds = query_rect;
                    q.base_mut().font = font;
                    crate::component::deliver(&mut *q, ev);
                    return Handled::Yes;
                }
                for (ri, row) in self.row_rects(&results, panel, list_top) {
                    if row.contains(p.pos) {
                        self.selected = ri;
                        self.run_selected();
                        break;
                    }
                }
                Handled::Yes
            }
            // The press that landed somewhere else closes the palette — no panel geometry of its
            // own to keep in step with the placement.
            Event::PointerDownOutside(_) => {
                self.close();
                Handled::No
            }
            // Swallow all other input while open.
            _ => Handled::Yes,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let open = self.is_open();
        // Advance the query's caret (it marks *its own* base on a blink flip, but it
        // lives off-tree so that mark is unobserved). Promote a flip to a palette
        // repaint of just the panel — no full-frame, no pegging frames every tick.
        let flipped = {
            let mut q = self.query.borrow_mut();
            q.base_mut().focused.set(open);
            q.tick(dt);
            let f = q.base().needs_paint();
            q.base().clear_needs_paint();
            f
        };
        if open && flipped {
            self.base.mark_needs_paint(); // collect_damage reads `damage_bounds` (the panel)
        }
        false
    }

    /// While open, wake the host for the query caret's next blink (instead of
    /// redrawing every frame). Closed: nothing pending.
    fn next_redraw(&self) -> Option<f32> {
        if self.is_open() {
            self.query.borrow().next_redraw()
        } else {
            None
        }
    }

    /// The palette paints its panel on the overlay layer, not at its layout `bounds`,
    /// so a caret-blink repaint must target the cached panel rect.
    fn damage_bounds(&self) -> Rectangle {
        if self.is_open() {
            self.panel.get()
        } else {
            self.base.bounds
        }
    }

    /// The palette's **input** surface is the panel it draws — its query line and rows live
    /// there, not in the layout box it was placed in. Closed, it takes nothing.
    fn hit_bounds(&self) -> Option<Rectangle> {
        if self.is_open() {
            Some(self.panel.get())
        } else {
            None
        }
    }
}

impl LayoutExt for CommandPalette {}

#[cfg(test)]
mod tests {
}
