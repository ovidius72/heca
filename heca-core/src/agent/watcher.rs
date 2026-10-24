//! File reporting needs write events while a producer keeps its file open.
//! FSEvents delays those until close; kqueue supplies the needed write event on macOS.

use notify::{Config, Event, Result, Watcher};

#[cfg(target_os = "macos")]
pub(super) type AgentWatcher = notify::KqueueWatcher;
#[cfg(not(target_os = "macos"))]
pub(super) type AgentWatcher = notify::RecommendedWatcher;

pub(super) fn new(handler: impl FnMut(Result<Event>) + Send + 'static) -> Result<AgentWatcher> {
    AgentWatcher::new(handler, Config::default())
}
