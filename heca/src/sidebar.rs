mod hit_test;
mod model;
mod render;

#[cfg(test)]
use hit_test::BTN_ROW_HEIGHT;
#[cfg(test)]
use render::ITEM_HEIGHT;
pub use hit_test::{sidebar_button_hit_test, sidebar_hit_test};
#[allow(unused_imports)]
pub use model::{
    SidebarButtonHitbox, SidebarColEntry, SidebarItem, SidebarPaneEntry, SidebarTree,
    SidebarWsEntry,
};
pub use render::{render_sidebar_collapsed, render_sidebar_expanded};

#[cfg(test)]
mod tests;
