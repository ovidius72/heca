#![allow(dead_code)]
#![allow(clippy::enum_variant_names)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use heca_core::layout::PaneId;
use heca_core::runtime::ProcessStatus;
use heca_grid_ui::widgets::RegionMode;

/// Canonical identity of a pluggable chrome region (contract §3.1.1). The
/// vertical sidebars and horizontal bars are all addressed by this enum; the
/// grid-ui `ChromeRegion` *widget* is the oriented shell that renders one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegionId {
    LeftSidebar,
    RightSidebar,
    TopBar,
    BottomBar,
}

/// **A region, as a caller says it**: `Region::LeftSidebar.child(Workspaces::new())`.
///
/// The same type as [`RegionId`] — one region vocabulary, not two — under the name that reads at a
/// call site, where `Id` said nothing.
pub use RegionId as Region;

impl RegionId {
    /// **Put a container in this region** — one, or several.
    ///
    /// ```ignore
    /// Region::LeftSidebar.child(Workspaces::new());
    /// Region::LeftSidebar.child([workspaces, docker]);
    /// ```
    ///
    /// It ADDS to the region that is already on screen; it never builds one. The region is the
    /// receiver rather than an argument, so a container stops declaring which parent it belongs to
    /// — the same inversion the grid's placement lost.
    ///
    /// A plugin writes this identical line, which is the point: no `Box`, no host object to reach,
    /// no `::placed` constructor, and no mount id unless the caller wants to name that placement.
    ///
    /// **It works before the host exists.** The containers queue here and [`ChromeHost`] takes them
    /// when it is built, so load order stops mattering — a plugin adding one at startup and one an
    /// hour later write the same call.
    ///
    /// [`ChromeHost`]: crate::chrome::ChromeHost
    pub fn child(self, containers: impl IntoProviders) -> Self {
        PENDING.with(|q| {
            q.borrow_mut()
                .extend(containers.into_providers().into_iter().map(|p| (self, p)))
        });
        self
    }

    /// **How this region arranges what is in it** — a track template, as a stylesheet writes one.
    ///
    /// ```ignore
    /// Region::LeftSidebar.template_row("1fr 1fr").gap("sm").child([workspaces, docker]);
    /// ```
    ///
    /// Without one the containers stack and divide the region by the share each asked for, which is
    /// what they did before this existed. With one, the region says the arrangement in a line a
    /// reader can check against the picture — `"auto 1fr"` for a fixed dock above one that takes
    /// the rest — and a part that is not there is a missing row rather than a number to revisit.
    pub fn template_row(self, tracks: &str) -> Self {
        LAYOUT.with(|l| l.borrow_mut().entry(self).or_default().rows = Some(tracks.to_string()));
        self
    }

    /// The column tracks, for a region arranged across rather than down.
    pub fn template_column(self, tracks: &str) -> Self {
        LAYOUT.with(|l| l.borrow_mut().entry(self).or_default().columns = Some(tracks.to_string()));
        self
    }

    /// **Air between the containers in this region** — a number of pixels, a step of the theme's
    /// rhythm (`"sm"`), or a string, exactly as everywhere else.
    pub fn gap(self, gap: impl Into<heca_grid_ui::style::Space>) -> Self {
        LAYOUT.with(|l| l.borrow_mut().entry(self).or_default().gap = Some(gap.into()));
        self
    }

    /// All four regions in canonical order.
    pub const ALL: [RegionId; 4] = [
        RegionId::LeftSidebar,
        RegionId::RightSidebar,
        RegionId::TopBar,
        RegionId::BottomBar,
    ];

    /// Dense `0..4` index for array-keyed storage (e.g. `ChromeHost`'s region array).
    pub fn index(self) -> usize {
        match self {
            RegionId::LeftSidebar => 0,
            RegionId::RightSidebar => 1,
            RegionId::TopBar => 2,
            RegionId::BottomBar => 3,
        }
    }

    /// The canonical spelling of a region, as written in config args and RPC commands.
    pub fn as_str(self) -> &'static str {
        match self {
            RegionId::LeftSidebar => "left-sidebar",
            RegionId::RightSidebar => "right-sidebar",
            RegionId::TopBar => "top-bar",
            RegionId::BottomBar => "bottom-bar",
        }
    }
}

/// One parser for region names, shared by **every** surface that names a region: RPC commands
/// (`move-container-to-region workspaces right-sidebar`), config binding args
/// (`args = { region = "right-sidebar" }`), and plugin intents. A short alias (`right`) is accepted
/// everywhere the long form is, so the two surfaces can never drift apart into different spellings.
impl crate::input::EnumArg for RegionId {
    const VALUES: &'static [&'static str] = &[
        "left-sidebar",
        "left",
        "right-sidebar",
        "right",
        "top-bar",
        "top",
        "bottom-bar",
        "bottom",
    ];
}

impl std::str::FromStr for RegionId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "left-sidebar" | "left" => Ok(RegionId::LeftSidebar),
            "right-sidebar" | "right" => Ok(RegionId::RightSidebar),
            "top-bar" | "top" => Ok(RegionId::TopBar),
            "bottom-bar" | "bottom" => Ok(RegionId::BottomBar),
            _ => Err(()),
        }
    }
}

/// A `Copy` projection of the sidebar-nav cursor selection — a mirror of
/// `workspaces::model::WorkspaceRow` without the sidebar-model coupling. Carried by
/// [`ChromeEvent::SidebarSelectionChanged`] and mirrored into the chrome store so
/// the expanded sidebar can highlight the nav cursor **distinctly** from the real
/// focused pane (`active_pane`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarSelection {
    /// `ws_idx` says where it sits and moves when a neighbour is inserted; `ws_id` is what it *is*,
    /// and is what a key is built from. Both, because an action still acts on a position.
    Workspace {
        ws_idx: usize,
        ws_id: heca_core::layout::WorkspaceId,
    },
    Column {
        ws_idx: usize,
        col_idx: usize,
        col_id: heca_core::layout::ColumnId,
    },
    Pane {
        pane_id: PaneId,
    },
    FloatingPane {
        pane_id: PaneId,
        ws_idx: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChromeEvent {
    PaneActiveChanged {
        pane: Option<PaneId>,
    },
    /// **Which targets are wearing a pick letter**, as `(letter, target id)` — empty when the
    /// picker closed (F003/P082/T427).
    ///
    /// One event for every kind of target, because there is now one mechanism: a pane, a workspace,
    /// a column and a mounted dock are all lettered through the same door, addressed by the
    /// identity each already declares (`key`, or `scope_key` for a container). It replaces
    /// `pane.pick.changed` and `dock.pick.changed`, which mirrored four per-frame projections that
    /// no longer exist.
    ///
    /// This is what a plugin subscribes to — `app.on("hint.changed", …)` — to render its own prompt
    /// or highlight beside heca's keycaps.
    HintLettersChanged {
        letters: Vec<(char, String)>,
    },
    PaneProcessChanged {
        pane: PaneId,
    },
    PaneStatusChanged {
        pane: PaneId,
        status: ProcessStatus,
    },
    PaneCwdChanged {
        pane: PaneId,
    },
    PaneGitChanged {
        pane: PaneId,
    },
    /// A pane's user-set custom display name (from rename) changed. `name` is the new
    /// override (`None` = the pane is back to tracking its process name).
    PaneCustomNameChanged {
        pane: PaneId,
        name: Option<String>,
    },
    /// A keyboard pick (move/select/swap/take) started, changed, or ended. `pick` is
    /// the new pending action (`None` = no pick active). Lets components/plugins render
    /// their own prompt UI for the pending action.
    PendingPickChanged {
        pick: Option<crate::app_state::PendingPick>,
    },
    /// A pane's terminal (PTY child) has exited. `code` is the captured exit code
    /// (from `try_wait`). Emitted once via the per-wake monitor's `take_exit_code`;
    /// the existing auto-close then fires for shell panes (§0.6 — no `Exit` status).
    PaneExited {
        pane: PaneId,
        code: Option<i32>,
    },
    WorkspaceCollapsedChanged {
        ws_idx: usize,
        collapsed: bool,
    },
    /// A **container's** nested scroll area moved. Carries which one, because several can be
    /// mounted at once — including two of the same kind in different regions (F003/P011/T021).
    ContainerScrollChanged {
        container: String,
        offset: f32,
    },
    RegionModeChanged {
        region: RegionId,
        mode: RegionMode,
    },
    RegionSizeChanged {
        region: RegionId,
        size: f32,
    },
    /// A pane's terminal viewport state (offset/bottom/scrollback-rows) changed.
    /// Only terminal-backed panes emit this; non-terminal panes never fire it.
    TerminalViewportChanged {
        pane: PaneId,
        viewport_offset: usize,
        at_bottom: bool,
        scrollback_rows: usize,
    },
    /// Chrome **keyboard focus** moved to another container, or was cleared (`None`).
    ///
    /// Carries a **container id**, not a region: a dock is focused wherever it is seated, so the
    /// focus survives it being moved between regions (F003/P011/T020).
    ContainerFocusChanged {
        container: Option<String>,
    },
    /// A mounted container's host-level placement changed — it was moved to a
    /// different region or reordered within its region by `ChromeHost`. Lets
    /// future chrome consumers re-read placement. `region` is the container's new
    /// region.
    ContainerPlacementChanged {
        container_id: String,
        region: RegionId,
    },
    /// The sidebar-nav cursor selection changed, or was cleared on leaving nav
    /// mode (`None`). Lets the expanded sidebar highlight the nav cursor distinctly
    /// from `active_pane` (the real session focus).
    SidebarSelectionChanged {
        selection: Option<SidebarSelection>,
    },
}

impl ChromeEvent {
    pub fn name(&self) -> &'static str {
        match self {
            ChromeEvent::PaneActiveChanged { .. } => "pane.active.changed",
            ChromeEvent::HintLettersChanged { .. } => "hint.changed",
            ChromeEvent::PaneProcessChanged { .. } => "pane.process.changed",
            ChromeEvent::PaneStatusChanged { .. } => "pane.status.changed",
            ChromeEvent::PaneCwdChanged { .. } => "pane.cwd.changed",
            ChromeEvent::PaneGitChanged { .. } => "pane.git.changed",
            ChromeEvent::PaneCustomNameChanged { .. } => "pane.name.changed",
            ChromeEvent::PendingPickChanged { .. } => "pick.pending.changed",
            ChromeEvent::PaneExited { .. } => "pane.exited",
            ChromeEvent::WorkspaceCollapsedChanged { .. } => "workspace.collapsed.changed",
            ChromeEvent::ContainerScrollChanged { .. } => "container.scroll.changed",
            ChromeEvent::RegionModeChanged { .. } => "chrome.region.mode.changed",
            ChromeEvent::RegionSizeChanged { .. } => "chrome.region.size.changed",
            ChromeEvent::TerminalViewportChanged { .. } => "terminal.viewport.changed",
            ChromeEvent::ContainerFocusChanged { .. } => "chrome.container.focus.changed",
            ChromeEvent::ContainerPlacementChanged { .. } => "chrome.container.placement.changed",
            ChromeEvent::SidebarSelectionChanged { .. } => "sidebar.selection.changed",
        }
    }
}

type Subscriber = Box<dyn FnMut(&ChromeEvent)>;

struct Subscription {
    id: usize,
    filter: String,
    handler: Subscriber,
}

#[derive(Default)]
struct ChromeEventBusInner {
    next_id: usize,
    subscribers: Vec<Subscription>,
}

#[derive(Clone, Default)]
pub struct ChromeEventBus {
    inner: Rc<RefCell<ChromeEventBusInner>>,
}

impl std::fmt::Debug for ChromeEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChromeEventBus").finish_non_exhaustive()
    }
}

impl ChromeEventBus {
    pub fn subscribe(
        &self,
        filter: impl Into<String>,
        handler: impl FnMut(&ChromeEvent) + 'static,
    ) -> ChromeSubscription {
        let mut inner = self.inner.borrow_mut();
        let id = inner.next_id;
        inner.next_id += 1;
        inner.subscribers.push(Subscription {
            id,
            filter: filter.into(),
            handler: Box::new(handler),
        });
        ChromeSubscription {
            bus: self.clone(),
            id: Cell::new(Some(id)),
        }
    }

    pub fn emit(&self, event: ChromeEvent) {
        let name = event.name();
        let mut inner = self.inner.borrow_mut();
        for sub in &mut inner.subscribers {
            if sub.filter == "*" || sub.filter == name {
                (sub.handler)(&event);
            }
        }
    }

    fn unsubscribe(&self, id: usize) {
        let mut inner = self.inner.borrow_mut();
        inner.subscribers.retain(|sub| sub.id != id);
    }
}

pub struct ChromeSubscription {
    bus: ChromeEventBus,
    id: Cell<Option<usize>>,
}

impl Drop for ChromeSubscription {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            self.bus.unsubscribe(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_and_catch_all_subscribers_receive_events() {
        let bus = ChromeEventBus::default();
        let typed = Rc::new(RefCell::new(Vec::new()));
        let all = Rc::new(RefCell::new(Vec::new()));

        let typed_seen = typed.clone();
        let _typed_sub = bus.subscribe("pane.active.changed", move |event| {
            typed_seen.borrow_mut().push(event.name().to_string());
        });

        let all_seen = all.clone();
        let _all_sub = bus.subscribe("*", move |event| {
            all_seen.borrow_mut().push(event.name().to_string());
        });

        bus.emit(ChromeEvent::PaneActiveChanged {
            pane: Some(PaneId(9)),
        });

        assert_eq!(typed.borrow().as_slice(), ["pane.active.changed"]);
        assert_eq!(all.borrow().as_slice(), ["pane.active.changed"]);
    }
}

/// **One container or several** — what [`RegionId::child`] takes, so a caller hands over whichever
/// shape they hold and never goes looking for a plural spelling.
pub trait IntoProviders {
    /// The containers, boxed once.
    fn into_providers(self) -> Vec<Box<dyn crate::providers::Provider>>;
}

impl<P: crate::providers::Provider + 'static> IntoProviders for P {
    fn into_providers(self) -> Vec<Box<dyn crate::providers::Provider>> {
        vec![Box::new(self)]
    }
}

impl IntoProviders for Box<dyn crate::providers::Provider> {
    fn into_providers(self) -> Vec<Box<dyn crate::providers::Provider>> {
        vec![self]
    }
}

impl<P: IntoProviders> IntoProviders for Vec<P> {
    fn into_providers(self) -> Vec<Box<dyn crate::providers::Provider>> {
        self.into_iter()
            .flat_map(IntoProviders::into_providers)
            .collect()
    }
}

impl<P: IntoProviders, const N: usize> IntoProviders for [P; N] {
    fn into_providers(self) -> Vec<Box<dyn crate::providers::Provider>> {
        self.into_iter()
            .flat_map(IntoProviders::into_providers)
            .collect()
    }
}

thread_local! {
    /// Containers named by [`RegionId::child`] before a [`ChromeHost`](crate::chrome::ChromeHost)
    /// existed to hold them.
    ///
    /// Thread-local rather than a global with a lock: the chrome is built and mutated on the UI
    /// thread only, and a lock here would be a promise the rest of the chrome does not keep.
    static PENDING: std::cell::RefCell<Vec<(RegionId, Box<dyn crate::providers::Provider>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Take everything [`RegionId::child`] has queued. The host calls this; nothing else should.
pub(crate) fn take_pending() -> Vec<(RegionId, Box<dyn crate::providers::Provider>)> {
    PENDING.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// **What a region says about arranging its own contents.** Empty unless a caller said something,
/// in which case the region's body is built as a grid rather than a stack.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegionLayout {
    /// The row tracks (`"auto 1fr"`, `"repeat(2, 1fr)"`).
    pub rows: Option<String>,
    /// The column tracks.
    pub columns: Option<String>,
    /// Air between the containers.
    pub gap: Option<heca_grid_ui::style::Space>,
}

impl RegionLayout {
    /// Did anyone say anything about this region?
    pub fn is_set(&self) -> bool {
        self.rows.is_some() || self.columns.is_some() || self.gap.is_some()
    }
}

thread_local! {
    /// What each region was told about arranging itself, before a host existed to hold it —
    /// alongside [`PENDING`], and taken by the same call.
    static LAYOUT: std::cell::RefCell<std::collections::HashMap<RegionId, RegionLayout>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Take the arrangements [`RegionId::template_row`] and friends have queued.
pub(crate) fn take_pending_layout() -> std::collections::HashMap<RegionId, RegionLayout> {
    LAYOUT.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
