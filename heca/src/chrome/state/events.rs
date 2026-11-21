//! What the chrome tells its listeners: the region names, the sidebar selection, and the event bus
//! that carries chrome events to whoever subscribed.

// The event bus carries events the planned plugin bridge will subscribe to; some have no listener in
// the app yet.
#![allow(dead_code)]
#![allow(clippy::enum_variant_names)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use heca_core::layout::PaneId;
use heca_core::runtime::ProcessStatus;

/// Canonical identity of a pluggable chrome region (contract §3.1.1). The
/// vertical sidebars and horizontal bars are all addressed by this enum; the
/// grid-ui `ChromeRegion` *widget* is the oriented shell that renders one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

    /// **The name a region is written as** — the dotted one `regions("sidebar.left")` takes, so
    /// what heca prints is what an author types. The older `left-sidebar` / `left` still parse.
    pub fn as_str(self) -> &'static str {
        match self {
            RegionId::LeftSidebar => "sidebar.left",
            RegionId::RightSidebar => "sidebar.right",
            RegionId::TopBar => "bar.top",
            RegionId::BottomBar => "bar.bottom",
        }
    }
}

/// One parser for region names, shared by **every** surface that names a region: RPC commands
/// (`move-container-to-region workspaces right-sidebar`), config binding args
/// (`args = { region = "right-sidebar" }`), and plugin intents. A short alias (`right`) is accepted
/// everywhere the long form is, so the two surfaces can never drift apart into different spellings.
impl crate::args::EnumArg for RegionId {
    const VALUES: &'static [&'static str] = &[
        "sidebar.left",
        "left-sidebar",
        "left",
        "sidebar.right",
        "right-sidebar",
        "right",
        "bar.top",
        "top-bar",
        "top",
        "bar.bottom",
        "bottom-bar",
        "bottom",
    ];
}

impl std::str::FromStr for RegionId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // `sidebar.left` first: the dotted spelling `regions(..)` and context-menu paths use.
            "sidebar.left" | "left-sidebar" | "left" => Ok(RegionId::LeftSidebar),
            "sidebar.right" | "right-sidebar" | "right" => Ok(RegionId::RightSidebar),
            "bar.top" | "top-bar" | "top" => Ok(RegionId::TopBar),
            "bar.bottom" | "bottom-bar" | "bottom" => Ok(RegionId::BottomBar),
            _ => Err(()),
        }
    }
}

/// A `Copy` description of a workspaces-sidebar row — a mirror of
/// `workspaces::model::WorkspaceRow` without the sidebar-model coupling. Worked out from a
/// placement's cursor key when asked ([`StateView::sidebar_selection`](crate::host::StateView::sidebar_selection));
/// never stored.
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

/// **Something that changed in a window**, heard with `cx.on("<name>", |ev| …)`.
///
/// A listener outside heca reads what it needs through the accessors below
/// ([`pane`](Self::pane), [`agent`](Self::agent), [`feedback`](Self::feedback)) — the same shape
/// as the server's events (`ev.as_session()`, `ev.as_state()`) — rather than matching variants, so
/// heca can add an event without breaking it.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ChromeEvent {
    PaneActiveChanged {
        pane: Option<PaneId>,
    },
    /// **Which targets are wearing a pick letter**, as `(letter, target id)` — empty when the
    /// picker closed.
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
    /// A report changed, or the agent left the foreground command. Process status is separate.
    /// Both values let subscribers react to transitions without keeping another state store.
    PaneAgentChanged {
        pane: PaneId,
        previous: Option<Box<heca_core::agent::AgentState>>,
        agent: Option<Box<heca_core::agent::AgentState>>,
    },
    /// One server-granted, unobserved feedback sound, not every animation pulse.
    PaneFeedback {
        session: heca_core::layout::SessionId,
        pane: PaneId,
        revision: u64,
        status: String,
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
    /// **This window now shows another workspace** — `workspace` is its lasting key (`None` before
    /// the window knows its session). Raised once per change, from the one place the window's
    /// active workspace is mirrored; `cx.state().active_workspace()` is the same value to bind to.
    WorkspaceFocused {
        workspace: Option<crate::entry::service::WorkspaceKey>,
    },
    /// **`session.changed`** — a session was created, renamed, terminated, restarted or deleted: the
    /// same event an extension's service hears on the server, heard in a window. `session` is the
    /// session it happened to (as it was just before, for a deleted one);
    /// `cx.state().sessions()` is the whole list after it.
    SessionChanged {
        kind: crate::entry::service::SessionChange,
        session: crate::entry::service::SessionEntry,
    },
    /// A **container's** nested scroll area moved. Carries which one, because several can be
    /// mounted at once — including two of the same kind in different regions.
    ContainerScrollChanged {
        container: String,
        offset: f32,
    },
    /// A region was shown or hidden — the one visible/hidden state every region has.
    RegionVisibilityChanged {
        region: RegionId,
        visible: bool,
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
    /// focus survives it being moved between regions.
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
    /// A placement's cursor moved to another row, named by the `key` that row declared, or was
    /// cleared (`None`). Carries which placement, because each keeps its own.
    ContainerCursorChanged {
        container: String,
        key: Option<String>,
    },
}

impl ChromeEvent {
    /// **The pane this event is about**, for every `pane.*` event; `None` for the others, and for
    /// `pane.active.changed` when no pane is active.
    pub fn pane(&self) -> Option<PaneId> {
        match self {
            ChromeEvent::PaneActiveChanged { pane } => *pane,
            ChromeEvent::PaneProcessChanged { pane }
            | ChromeEvent::PaneStatusChanged { pane, .. }
            | ChromeEvent::PaneAgentChanged { pane, .. }
            | ChromeEvent::PaneFeedback { pane, .. }
            | ChromeEvent::PaneCwdChanged { pane }
            | ChromeEvent::PaneGitChanged { pane, .. }
            | ChromeEvent::PaneCustomNameChanged { pane, .. }
            | ChromeEvent::PaneExited { pane, .. }
            | ChromeEvent::TerminalViewportChanged { pane, .. } => Some(*pane),
            _ => None,
        }
    }

    /// **The agent's report now**, for `pane.agent.changed`; `None` for other events and when the
    /// agent left the foreground command.
    pub fn agent(&self) -> Option<&heca_core::agent::AgentState> {
        match self {
            ChromeEvent::PaneAgentChanged { agent, .. } => agent.as_deref(),
            _ => None,
        }
    }

    /// **The status a feedback sound was granted for**, for `pane.feedback` — the open status id
    /// (`waiting`, `done`, …) the agent configuration names.
    pub fn feedback(&self) -> Option<&str> {
        match self {
            ChromeEvent::PaneFeedback { status, .. } => Some(status),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ChromeEvent::PaneActiveChanged { .. } => "pane.active.changed",
            ChromeEvent::HintLettersChanged { .. } => "hint.changed",
            ChromeEvent::PaneProcessChanged { .. } => "pane.process.changed",
            ChromeEvent::PaneStatusChanged { .. } => "pane.status.changed",
            ChromeEvent::PaneAgentChanged { .. } => "pane.agent.changed",
            ChromeEvent::PaneFeedback { .. } => "pane.feedback",
            ChromeEvent::PaneCwdChanged { .. } => "pane.cwd.changed",
            ChromeEvent::PaneGitChanged { .. } => "pane.git.changed",
            ChromeEvent::PaneCustomNameChanged { .. } => "pane.name.changed",
            ChromeEvent::PendingPickChanged { .. } => "pick.pending.changed",
            ChromeEvent::PaneExited { .. } => "pane.exited",
            ChromeEvent::WorkspaceCollapsedChanged { .. } => "workspace.collapsed.changed",
            ChromeEvent::WorkspaceFocused { .. } => "workspace.focused",
            ChromeEvent::SessionChanged { .. } => "session.changed",
            ChromeEvent::ContainerScrollChanged { .. } => "container.scroll.changed",
            ChromeEvent::RegionVisibilityChanged { .. } => "chrome.region.visibility.changed",
            ChromeEvent::RegionSizeChanged { .. } => "chrome.region.size.changed",
            ChromeEvent::TerminalViewportChanged { .. } => "terminal.viewport.changed",
            ChromeEvent::ContainerFocusChanged { .. } => "chrome.container.focus.changed",
            ChromeEvent::ContainerPlacementChanged { .. } => "chrome.container.placement.changed",
            ChromeEvent::ContainerCursorChanged { .. } => "container.cursor.changed",
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

    /// A listener outside heca reads an event through its accessors, never by matching variants.
    #[test]
    fn a_pane_event_says_its_pane_report_and_feedback_through_accessors() {
        use heca_core::agent::{AgentState, AgentStatus};
        let report = AgentState::new("pi", "pi", AgentStatus::new("waiting"));
        let agent = ChromeEvent::PaneAgentChanged {
            pane: PaneId(3),
            previous: None,
            agent: Some(Box::new(report.clone())),
        };
        assert_eq!(agent.pane(), Some(PaneId(3)));
        assert_eq!(agent.agent(), Some(&report));
        assert_eq!(agent.feedback(), None);

        let feedback = ChromeEvent::PaneFeedback {
            session: heca_core::layout::SessionId(1),
            pane: PaneId(4),
            revision: 1,
            status: "done".into(),
        };
        assert_eq!(
            (feedback.pane(), feedback.feedback()),
            (Some(PaneId(4)), Some("done"))
        );
        assert_eq!(feedback.agent(), None);

        let left = ChromeEvent::PaneAgentChanged {
            pane: PaneId(3),
            previous: None,
            agent: None,
        };
        assert_eq!(
            left.agent(),
            None,
            "the agent left the foreground: no report"
        );
        let other = ChromeEvent::HintLettersChanged {
            letters: Vec::new(),
        };
        assert_eq!(
            (other.pane(), other.agent(), other.feedback()),
            (None, None, None)
        );
    }
}
