mod hit_test;
mod model;

#[cfg(test)]
use hit_test::{BTN_ROW_HEIGHT, ITEM_HEIGHT};
pub use hit_test::{sidebar_button_hit_test, sidebar_hit_test};
#[allow(unused_imports)] // Re-exports reserved for sidebar consumers and RPC introspection
pub use model::{
    SidebarButtonHitbox, SidebarColEntry, SidebarItem, SidebarItemKind, SidebarPaneEntry,
    SidebarTree, SidebarWsEntry,
};

#[cfg(test)]
mod tests;
