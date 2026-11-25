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

use std::rc::Rc;

use heca_grid_ui::widgets::Glyph;

use super::PaneFacts;
use crate::chrome::{StartupQueue, warn_author};

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

/// **What the title line shows** — the pane's name, its program's icon, its status and the dimmed
/// `(program)` suffix of a renamed pane (`None` when there is none to show).
#[derive(Clone, PartialEq)]
pub(crate) struct HeadView {
    pub(crate) title: String,
    pub(crate) icon: Glyph,
    pub(crate) status: heca_grid_ui::DotStatus,
    pub(crate) hint: Option<String>,
}

impl Default for HeadView {
    /// No name, a terminal's icon, online, no suffix — what a test of another line passes.
    fn default() -> Self {
        Self {
            title: String::new(),
            icon: Glyph::Terminal,
            status: heca_grid_ui::DotStatus::default(),
            hint: None,
        }
    }
}

/// **The title line's parts, worked out from the pane** — one value every part reads through its
/// own builder, so a `title` line built again (a view change) shows the same thing with nothing
/// copied and nothing written into it.
#[derive(Clone, Copy)]
pub(crate) struct RowHead(heca_grid_ui::reactive::Memo<HeadView>);

impl RowHead {
    /// The parts `view` works out, followed as the signals it reads change.
    pub(crate) fn new(view: impl Fn() -> HeadView + 'static) -> Self {
        Self(heca_grid_ui::reactive::create_memo(move |_| view()))
    }

    /// What the title line shows now — tracked, so reading it is what makes a part follow it.
    pub(crate) fn get(&self) -> HeadView {
        heca_grid_ui::reactive::SignalGet::get(&self.0)
    }
}

impl Default for RowHead {
    /// An empty head ([`HeadView::default`]) — what a test of another line passes.
    fn default() -> Self {
        Self::new(HeadView::default)
    }
}

/// **What a line is built from**: the theme, and the pane's facts (a function, because it is read
/// inside the line's own subscription — reading it is what makes the line follow the pane).
pub(crate) struct LineCx {
    /// The `title` line's parts.
    pub(crate) head: RowHead,
    pub(crate) facts: Rc<dyn Fn() -> PaneFacts>,
    pub(crate) agent_statuses:
        heca_grid_ui::reactive::Signal<Rc<heca_config::settings::AgentStatuses>>,
    pub(crate) agent_session_line:
        heca_grid_ui::reactive::Signal<heca_config::appearance::AgentSessionLine>,
}

/// How a line is built for one pane.
pub(crate) type BuildLine = dyn Fn(&LineCx) -> heca_grid_ui::WidgetModel;

/// One line, by name.
pub(crate) struct PaneLineDef {
    /// What config lists: `cwd`, `git`, `demo.status`.
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

/// **The lines of a pane card in each view, and the current pane's** — `[appearance.sidebar]
/// pane_<view>_view` and `pane_current_view` resolved by name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaneViews {
    /// Compact, normal, extended — in [`PaneView::ALL`](heca_config::appearance::PaneView::ALL) order.
    pub(crate) views: [PaneLineSet; 3],
    /// The current pane's lines; `None` follows the view.
    pub(crate) current: Option<PaneLineSet>,
}

impl PaneViews {
    /// **The lines a card shows**: the current pane's list when it is the current pane and one is
    /// set, else the view's.
    pub(crate) fn lines(
        &self,
        view: heca_config::appearance::PaneView,
        current: bool,
    ) -> &PaneLineSet {
        let at = heca_config::appearance::PaneView::ALL
            .iter()
            .position(|v| *v == view)
            .unwrap_or(0);
        match (&self.current, current) {
            (Some(lines), true) => lines,
            _ => &self.views[at],
        }
    }

    /// Every name in every list — what the sidebar's rebuild check hashes.
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.views
            .iter()
            .chain(self.current.iter())
            .flat_map(PaneLineSet::names)
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
    /// Lines added before the app took them.
    static QUEUE: StartupQueue<(String, Rc<ProduceLine>)> = const { StartupQueue::new() };
}

/// Queue the line `name` (`<extension>.<short>`) — called by `Extension::pane_line`. The app takes
/// the queue once, as it starts; a call after that is said and dropped.
pub(crate) fn add_extension_line(
    name: String,
    produce: impl Fn(&PaneFacts) -> Option<PaneLine> + 'static,
) {
    if let Err((name, _)) = QUEUE.with(|q| q.add((name, Rc::new(produce)))) {
        warn_author(format!(
            "[heca] pane line '{name}' was added after the app started, so it does nothing — add it \
             before `heca::run(..)`"
        ));
    }
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
        let mut all = Self::default();
        for (name, produce) in QUEUE.with(StartupQueue::take) {
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
        super::listing::add_unique(&mut self.defs, "pane line", Rc::new(def));
    }

    /// **The lines to show, in order** — the user's list by name, with what other crates added after
    /// it while that list is still `default` (see [`listing`](super::listing)).
    fn shown(&self, config: &[String], default: &[String]) -> PaneLineSet {
        PaneLineSet(
            super::listing::shown(&self.defs, config, default)
                .into_iter()
                .cloned()
                .collect(),
        )
    }

    /// **Every view's lines, and the current pane's**, from `[appearance.sidebar]`.
    pub(crate) fn views(&self, sidebar: &heca_config::appearance::SidebarAppearance) -> PaneViews {
        use heca_config::appearance::{PaneView, default_view_lines};
        PaneViews {
            views: PaneView::ALL
                .map(|view| self.shown(sidebar.view_lines(view), &default_view_lines(view))),
            // The current pane's list has no default of its own: what other crates add is not
            // added to it — the user who wrote it said exactly what the current pane shows.
            current: sidebar
                .pane_current_view
                .as_deref()
                .map(|lines| self.shown(lines, &[])),
        }
    }

    /// Say what the user's lists get wrong or miss — the shared rules of
    /// [`listing::report`](super::listing::report), once per list.
    pub(crate) fn report(&self, sidebar: &heca_config::appearance::SidebarAppearance) {
        use heca_config::appearance::{PaneView, default_view_lines};
        for view in PaneView::ALL {
            super::listing::report(
                "pane line",
                &format!("pane_{}_view", view.name()),
                &self.defs,
                sidebar.view_lines(view),
                &default_view_lines(view),
            );
        }
        if let Some(lines) = &sidebar.pane_current_view {
            super::listing::report("pane line", "pane_current_view", &self.defs, lines, &[]);
        }
    }
}
