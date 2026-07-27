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

impl RegionId {
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
/// `sidebar::model::SidebarItem` without the sidebar-model coupling. Carried by
/// [`ChromeEvent::SidebarSelectionChanged`] and mirrored into the chrome store so
/// the expanded sidebar can highlight the nav cursor **distinctly** from the real
/// focused pane (`active_pane`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarSelection {
    Workspace { ws_idx: usize },
    Column { ws_idx: usize, col_idx: usize },
    Pane { pane_id: PaneId },
    FloatingPane { pane_id: PaneId, ws_idx: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChromeEvent {
    PaneActiveChanged {
        pane: Option<PaneId>,
    },
    PanePickCandidatesChanged {
        candidates: Vec<(char, PaneId)>,
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
    WorkspacesScrollChanged {
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
            ChromeEvent::PanePickCandidatesChanged { .. } => "pane.pick.changed",
            ChromeEvent::PaneProcessChanged { .. } => "pane.process.changed",
            ChromeEvent::PaneStatusChanged { .. } => "pane.status.changed",
            ChromeEvent::PaneCwdChanged { .. } => "pane.cwd.changed",
            ChromeEvent::PaneGitChanged { .. } => "pane.git.changed",
            ChromeEvent::PaneCustomNameChanged { .. } => "pane.name.changed",
            ChromeEvent::PendingPickChanged { .. } => "pick.pending.changed",
            ChromeEvent::PaneExited { .. } => "pane.exited",
            ChromeEvent::WorkspaceCollapsedChanged { .. } => "workspace.collapsed.changed",
            ChromeEvent::WorkspacesScrollChanged { .. } => "workspaces.scroll.changed",
            ChromeEvent::RegionModeChanged { .. } => "chrome.region.mode.changed",
            ChromeEvent::RegionSizeChanged { .. } => "chrome.region.size.changed",
            ChromeEvent::TerminalViewportChanged { .. } => "terminal.viewport.changed",
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
