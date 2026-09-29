//! Registry and keymap construction.
//!
//! Owns turning config into every keymap layer, and registering the handler for every action, so
//! `main.rs` can stay focused on application lifecycle and event dispatch. Each job lives in its own
//! file; this one only wires them together and re-exports what the rest of the app calls, so no
//! call site outside the folder names a file inside it.
//!
//! | file | owns |
//! |---|---|
//! | [`names`] | a written name → the action it means, and argument mistakes |
//! | [`binding`] | the one door: bind, record who wrote it, report a clash; unbind |
//! | [`global`] | `[keys]`, `[[keys.bind]]`, `[[keys.command]]`, `[keys.unbind]` |
//! | [`surfaces`] | `[[keys.surface]]`, `global_focus`, a plugin's own keys |
//! | [`modes`] | `[[keys.mode]]` |
//! | [`floors`] | `Escape`, which no config can remove |
//! | [`widgets`] | `[keys.widgets]` |
//! | [`actions`] | action → handler |

mod actions;
mod binding;
mod floors;
mod global;
mod modes;
mod names;
mod surfaces;
#[cfg(test)]
mod testing;
mod widgets;

pub use actions::build_registry;
pub use global::build_keymap;
pub use modes::build_modes;
pub(crate) use names::surface_action_id;
pub use surfaces::{build_component_keymaps, register_component_keybinding};
pub use widgets::build_widget_keymap;
pub(crate) use widgets::combo_to_grid;

use crate::app::conflicts::Conflicts;
use crate::keymap::BindingIndex;
use surfaces::bind_global_focus;

/// Build **every** keymap layer, and the reverse index over all of them, from one config.
///
/// The single entry point, so startup and `prefix+Shift+r` cannot build a different set — and so
/// [`Keymaps::by_action`] is filled by the same pass that binds, and can never describe a layer that
/// was not built (F003/P086/T366).
pub fn build_keymaps(
    config: &heca_config::theme::Config,
    conflicts: &mut Conflicts,
) -> crate::keymap::Keymaps {
    let mut by_action = BindingIndex::new();
    let (modes, triggers) = build_modes(config, conflicts, &mut by_action);
    let mut flat = build_keymap(config, conflicts, &mut by_action);
    let (components, global_focus) = build_component_keymaps(config, conflicts, &mut by_action);
    bind_global_focus(&mut flat, &global_focus, conflicts, &mut by_action);
    crate::keymap::Keymaps {
        flat,
        modes,
        components,
        triggers,
        by_action,
    }
}
