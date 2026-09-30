//! **Sidebar pane-row lines, by name** — the lines under a pane's name in the workspaces dock (its
//! working directory, its git state, and any a program built on heca adds).
//!
//! A line is a **self-contained widget** that follows its pane: it is built once, subscribes to the
//! pane's facts, and rewrites its own words in place — the way the row already works ("the row
//! follows its pane; nothing writes to it"), so a `cd` moves a path without the sidebar being
//! rebuilt. What builds each line lives with the dock it belongs to
//! (`providers/workspaces/row_lines.rs`); this file is the registry and the rules, shared with
//! buttons and chips (`listing.rs`).
//!
//! *Client side*: a line is drawn. Its facts are plain data ([`PaneFacts`]).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use heca_grid_ui::Component;
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::Glyph;

use super::PaneFacts;
use crate::chrome::warn_author;

/// **What an added line shows**: an icon and some text.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PaneLine {
    pub icon: Glyph,
    pub text: String,
}

impl PaneLine {
    /// A line showing `text` beside `icon`.
    pub fn new(icon: Glyph, text: impl Into<String>) -> Self {
        Self {
            icon,
            text: text.into(),
        }
    }
}

/// **What a line is built from**: the theme, and the pane's facts (a function, because it is read
/// inside the line's own subscription — reading it is what makes the line follow the pane).
pub(crate) struct LineCx<'a> {
    pub(crate) theme: &'a GuiTheme,
    pub(crate) facts: Rc<dyn Fn() -> PaneFacts>,
}

/// How a line is built for one pane.
pub(crate) type BuildLine = dyn Fn(&LineCx<'_>) -> Box<dyn Component>;

/// One line, by name.
pub(crate) struct PaneLineDef {
    /// What config lists: `cwd`, `git`, `pro.status`.
    pub(crate) name: String,
    pub(crate) build: Rc<BuildLine>,
    /// Added by another crate rather than shipped with heca.
    pub(crate) from_extension: bool,
}

impl super::listing::Named for PaneLineDef {
    fn name(&self) -> &str {
        &self.name
    }
    fn is_added(&self) -> bool {
        self.from_extension
    }
}

impl std::fmt::Debug for PaneLineDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneLineDef")
            .field("name", &self.name)
            .finish()
    }
}

/// **The lines that show, in order** — what the dock builds each row from. Equal when it names the
/// same lines, which is how the store knows a reload changed nothing.
#[derive(Clone, Debug, Default)]
pub struct PaneLineSet(pub(crate) Vec<Rc<PaneLineDef>>);

impl PartialEq for PaneLineSet {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && self.0.iter().zip(&other.0).all(|(a, b)| a.name == b.name)
    }
}

impl PaneLineSet {
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Rc<PaneLineDef>> {
        self.0.iter()
    }

    /// The names, in order — what the sidebar's rebuild check hashes.
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|d| d.name.as_str())
    }
}

/// What an added line says about a pane.
pub(crate) type ProduceLine = dyn Fn(&PaneFacts) -> Option<PaneLine>;

thread_local! {
    /// Lines added before the app took them. Thread-local like the queues beside it.
    static QUEUE: RefCell<Vec<(String, Rc<ProduceLine>)>> = const { RefCell::new(Vec::new()) };
    static STARTED: Cell<bool> = const { Cell::new(false) };
}

/// Queue the line `name` (`<extension>.<short>`) — called by `Extension::pane_line`. The app takes
/// the queue once, as it starts; a call after that is said and dropped.
pub(crate) fn add_extension_line(
    name: String,
    produce: impl Fn(&PaneFacts) -> Option<PaneLine> + 'static,
) {
    if STARTED.with(Cell::get) {
        warn_author(format!(
            "[heca] pane line '{name}' was added after the app started, so it does nothing — add it \
             before `heca::run()`"
        ));
        return;
    }
    QUEUE.with(|q| q.borrow_mut().push((name, Rc::new(produce))));
}

/// **Every pane-row line there is**, by name: heca's own and the ones other crates added.
pub(crate) struct PaneRowLines {
    defs: Vec<Rc<PaneLineDef>>,
}

impl Default for PaneRowLines {
    /// heca's own lines only — what a test wants.
    fn default() -> Self {
        Self {
            defs: crate::providers::workspaces::builtin_row_lines()
                .into_iter()
                .map(Rc::new)
                .collect(),
        }
    }
}

impl PaneRowLines {
    /// heca's own, then everything queued. Takes the queue and closes it: the app calls this once.
    pub(crate) fn from_startup() -> Self {
        STARTED.with(|s| s.set(true));
        let mut all = Self::default();
        for (name, produce) in QUEUE.with(|q| std::mem::take(&mut *q.borrow_mut())) {
            all.add(PaneLineDef {
                name,
                build: crate::providers::workspaces::text_line(produce),
                from_extension: true,
            });
        }
        all
    }

    /// Add one. A name already taken is refused and said — the first keeps it.
    fn add(&mut self, def: PaneLineDef) {
        if self.defs.iter().any(|d| d.name == def.name) {
            warn_author(format!(
                "[heca] a pane line called '{}' already exists, so this one was not added",
                def.name
            ));
            return;
        }
        self.defs.push(Rc::new(def));
    }

    /// **The lines to show, in order** — the user's list by name, with what other crates added after
    /// it while that list is still the default (see [`listing`](super::listing)).
    pub(crate) fn shown(&self, config: &[String]) -> PaneLineSet {
        PaneLineSet(
            super::listing::shown(
                &self.defs,
                config,
                &heca_config::appearance::default_sidebar_pane_lines(),
            )
            .into_iter()
            .cloned()
            .collect(),
        )
    }

    /// Say what the user's list gets wrong or misses — the shared rules of
    /// [`listing::report`](super::listing::report).
    pub(crate) fn report(&self, config: &[String]) {
        super::listing::report(
            "pane line",
            "pane_lines",
            &self.defs,
            config,
            &heca_config::appearance::default_sidebar_pane_lines(),
        );
    }
}
