#![allow(dead_code)]
#![allow(clippy::enum_variant_names)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use heca_core::layout::PaneId;
use heca_core::runtime::ProcessStatus;
use heca_grid_ui::widgets::RegionMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromeRegion {
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChromeEvent {
    PaneActiveChanged {
        pane: Option<PaneId>,
    },
    PaneHoveredChanged {
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
    WorkspaceCollapsedChanged {
        ws_idx: usize,
        collapsed: bool,
    },
    WorkspacesScrollChanged {
        offset: f32,
    },
    RegionModeChanged {
        region: ChromeRegion,
        mode: RegionMode,
    },
    RegionSizeChanged {
        region: ChromeRegion,
        size: f32,
    },
}

impl ChromeEvent {
    pub fn name(&self) -> &'static str {
        match self {
            ChromeEvent::PaneActiveChanged { .. } => "pane.active.changed",
            ChromeEvent::PaneHoveredChanged { .. } => "pane.hovered.changed",
            ChromeEvent::PanePickCandidatesChanged { .. } => "pane.pick.changed",
            ChromeEvent::PaneProcessChanged { .. } => "pane.process.changed",
            ChromeEvent::PaneStatusChanged { .. } => "pane.status.changed",
            ChromeEvent::PaneCwdChanged { .. } => "pane.cwd.changed",
            ChromeEvent::PaneGitChanged { .. } => "pane.git.changed",
            ChromeEvent::WorkspaceCollapsedChanged { .. } => "workspace.collapsed.changed",
            ChromeEvent::WorkspacesScrollChanged { .. } => "workspaces.scroll.changed",
            ChromeEvent::RegionModeChanged { .. } => "chrome.region.mode.changed",
            ChromeEvent::RegionSizeChanged { .. } => "chrome.region.size.changed",
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
        ChromeSubscription { bus: self.clone(), id: Cell::new(Some(id)) }
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
