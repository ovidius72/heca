//! The built-in actions, one file per [`ActionCategory`] — the compile-time seed
//! [`ActionCatalog::with_builtins`](super::ActionCatalog::with_builtins) loads into the runtime
//! catalog.

use super::ActionCategory;
use crate::args::ArgDescriptor;
use heca_grid_ui::Glyph;

mod chrome;
mod layout;
mod navigation;
mod pane;
mod system;
mod workspace;

/// Static descriptor for a window-manager action. The built-in set is [`builtins`];
/// [`ActionCatalog`](super::ActionCatalog) loads it into the runtime, plugin-extensible metadata
/// surface every UI reads from.
///
/// It carries no category: the file it is listed in is its category (see `BY_CATEGORY`), so the
/// two can never disagree.
#[derive(Debug, Clone, Copy)]
pub struct ActionDescriptor {
    /// Config key name (e.g. "focus_left").
    pub name: &'static str,
    /// **Where it runs** — see [`Side`]. Required, like `args`: a struct literal has to fill every
    /// field, so an action cannot be added without saying which side of the server/client split
    /// it is on.
    pub side: super::Side,
    /// Human-readable label for the command palette.
    pub label: &'static str,
    /// Short description of what the action does.
    pub description: &'static str,
    /// Centralized action icon. The single source of an action's [`Glyph`] —
    /// every surface that renders this action (pane-action bar, context menu,
    /// command palette) reads it from here instead of inventing its own. `None`
    /// for actions without an assigned icon yet.
    pub icon: Option<Glyph>,
    /// The arguments this action takes, in the order a reader should meet them. **Empty means it
    /// takes none** — there is no "not stated": a struct literal has to fill every field, so a new
    /// action cannot be added without answering the question, exactly as it cannot skip `policy`.
    pub args: &'static [ArgDescriptor],
}

/// Every category's built-ins, in [`ActionCategory`] order — the one place a built-in gets its
/// category.
const BY_CATEGORY: &[(ActionCategory, &[ActionDescriptor])] = &[
    (ActionCategory::Navigation, navigation::ACTIONS),
    (ActionCategory::Layout, layout::ACTIONS),
    (ActionCategory::Pane, pane::ACTIONS),
    (ActionCategory::Workspace, workspace::ACTIONS),
    (ActionCategory::Chrome, chrome::ACTIONS),
    (ActionCategory::System, system::ACTIONS),
];

/// Every built-in action with its category, in [`ActionCategory`] order — the seed
/// [`ActionCatalog::with_builtins`](super::ActionCatalog::with_builtins) loads.
pub fn builtins_by_category() -> impl Iterator<Item = (ActionCategory, &'static ActionDescriptor)> {
    BY_CATEGORY
        .iter()
        .flat_map(|(category, actions)| actions.iter().map(move |d| (*category, d)))
}

/// Every built-in action, in [`builtins_by_category`] order — what a caller that needs the static
/// set before any catalog exists (config loading) reads.
pub fn builtins() -> impl Iterator<Item = &'static ActionDescriptor> {
    builtins_by_category().map(|(_, d)| d)
}

#[cfg(test)]
mod tests;
