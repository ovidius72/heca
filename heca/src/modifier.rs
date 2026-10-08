//! **Which held keys a configured modifier means** — the one place a [`ModifierKey`] from settings
//! meets the modifiers the window tree reports.

use heca_config::theme::ModifierKey;
use heca_grid_ui::Modifiers;

/// Whether `key` is among what is held.
pub(crate) fn held(key: ModifierKey, held: Modifiers) -> bool {
    match key {
        ModifierKey::Super => held.meta,
        ModifierKey::Alt => held.alt,
        ModifierKey::Ctrl => held.ctrl,
        ModifierKey::Shift => held.shift,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_key_is_its_own_held_flag() {
        let m = Modifiers {
            meta: true,
            ..Modifiers::default()
        };
        assert!(held(ModifierKey::Super, m));
        for other in [ModifierKey::Alt, ModifierKey::Ctrl, ModifierKey::Shift] {
            assert!(!held(other, m));
        }
    }
}
