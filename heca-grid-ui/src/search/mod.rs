//! Search as a capability a widget **embeds** — matching, ranking by use, and query history.
//!
//! The shape is [`ScrollRegion`](crate::widgets::ScrollRegion)'s. That widget owns the paging, the
//! clamp and the axis-decline, and anything needing to scroll *nests one* rather than reimplementing
//! it. A widget that needs to be searchable embeds a [`SearchModel`] and reimplements nothing: it
//! forwards its list to [`rank`](SearchModel::rank), forwards the history intents to
//! [`handle`](SearchModel::handle), and tells it when something ran.
//!
//! **No filesystem, no clock, and no widget type is named here.** That is what keeps it testable
//! without a window and reusable by whatever searches next — the `>` / `@` / `:` modes a palette
//! grows, a filterable `Select`, a menu. Persisting the store is the *app's* job: a UI library has
//! no business knowing where an install keeps its state, and a widget test would otherwise need a
//! temp directory to run at all.

mod frecency;
mod history;
mod matcher;
mod model;
mod store;

pub use frecency::Frecency;
pub use history::History;
pub use matcher::{fuzzy, smart_case, Match, MatchCase};
pub use model::{Ranked, SearchAction, SearchModel};
pub use store::{Caps, Scope, SearchStore, DEFAULT_HISTORY_CAP, DEFAULT_USAGE_CAP};

#[cfg(test)]
mod tests;
