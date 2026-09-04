//! **Everything that collided while assembling the app, in one place.**
//!
//! Two things can be said twice, and until now neither was answered well:
//!
//! - **A key bound twice.** Detected per keymap build and `eprintln`'d from four separate places, so
//!   a user saw four unrelated lines, or — because each build only reported its own — none at all
//!   for a collision that spanned two layers.
//! - **An action id declared twice.** Not detected at all: `ActionCatalog::insert` overwrote, so a
//!   component declaring `close` silently replaced the built-in's label, icon, policy and confirm,
//!   and the only symptom was a menu entry that looked wrong.
//!
//! The second is the one that matters now: before components declared their own actions there was
//! exactly one author of ids, and a duplicate was impossible by construction. It isn't any more.
//!
//! So collisions are **collected** rather than logged where they are found, and reported once, after
//! everything that can register has registered — config, built-ins, and every mounted component.

use crate::keymap::{ActionRef, KeyCombo};

/// A key that means two different things in one layer. The later binding wins; this says so.
#[derive(Debug)]
pub(crate) struct BindingConflict {
    pub(crate) mode: String,
    pub(crate) combo: KeyCombo,
    pub(crate) previous_action: ActionRef,
    pub(crate) previous_source: String,
    pub(crate) new_action: ActionRef,
    pub(crate) new_source: String,
}

/// An action id claimed by more than one declarer.
#[derive(Debug)]
pub(crate) struct ActionConflict {
    /// The contested id.
    pub(crate) id: String,
    /// Who ends up owning it.
    pub(crate) kept: String,
    /// Who asked for it and did not get it (or overwrote the other, for two dynamics).
    pub(crate) rejected: String,
    /// Whether the id belongs to a compiled-in action, which no component may take over.
    pub(crate) shadows_builtin: bool,
}

/// **One modifier asked to mean two things at once.**
///
/// A drag has a modifier that *starts* it and a modifier that decides what it *does* (move or
/// swap). Both are configurable, and nothing stops a config naming the same key for both — at
/// which point every content-area drag is a swap, with no way left to express a plain move.
///
/// Same family as a key bound twice, so it is reported the same way rather than rejected: a config
/// that parses should start the app.
#[derive(Debug)]
pub(crate) struct ModifierConflict {
    /// The key both settings named.
    pub(crate) modifier: String,
    /// The setting that keeps it.
    pub(crate) kept: String,
    /// The setting that loses it, and what it falls back to.
    pub(crate) rejected: String,
    /// What the losing setting does instead.
    pub(crate) fallback: String,
}

/// Everything that collided while the app was being assembled.
#[derive(Debug, Default)]
pub(crate) struct Conflicts {
    pub(crate) keys: Vec<BindingConflict>,
    pub(crate) actions: Vec<ActionConflict>,
    pub(crate) modifiers: Vec<ModifierConflict>,
}

impl Conflicts {
    pub(crate) fn is_empty(&self) -> bool {
        self.keys.is_empty() && self.actions.is_empty() && self.modifiers.is_empty()
    }

    /// A key bound to something else in the same layer.
    pub(crate) fn key(&mut self, conflict: BindingConflict) {
        self.keys.push(conflict);
    }

    /// An id two declarers both wanted.
    pub(crate) fn action(&mut self, conflict: ActionConflict) {
        self.actions.push(conflict);
    }

    /// One modifier two settings both claimed.
    pub(crate) fn modifier(&mut self, conflict: ModifierConflict) {
        self.modifiers.push(conflict);
    }

    /// Print the one report — at startup, and again after a config reload.
    ///
    /// **Reload re-answers only half of it.** Rebuilding the keymaps from the file makes the key
    /// collisions fresh, but components do not re-register, so their action-id collisions are
    /// carried over rather than recollected: they are still exactly as true as they were, and
    /// dropping them would mean pressing reload made a real problem disappear from the report.
    ///
    /// Silent when nothing collided, and in tests — a test that builds a deliberately conflicting
    /// keymap should not spray the harness. Goes to stderr because there is no place in the UI for
    /// a startup diagnostic yet; when there is, this is the single call site to change.
    pub(crate) fn report(&self) {
        if self.is_empty() || cfg!(test) {
            return;
        }
        eprintln!(
            "[heca] {} keybinding/action conflict(s) — the last declaration wins unless stated \
             otherwise:",
            self.keys.len() + self.actions.len() + self.modifiers.len()
        );

        for c in &self.modifiers {
            eprintln!(
                "[heca]   modifier '{}' is set for both {} and {} — {} keeps it, {} falls back to \
                 {}. Give one of them a different key.",
                c.modifier, c.kept, c.rejected, c.kept, c.rejected, c.fallback,
            );
        }

        for c in &self.actions {
            if c.shadows_builtin {
                eprintln!(
                    "[heca]   action '{}' is built in and cannot be replaced — {} was REJECTED, {} \
                     still owns it. Give the component's action its own namespaced id \
                     (e.g. '<kind>.{}').",
                    c.id, c.rejected, c.kept, c.id,
                );
            } else {
                eprintln!(
                    "[heca]   action '{}' declared twice — {} replaced {}. Namespace one of them by \
                     its component kind.",
                    c.id, c.kept, c.rejected,
                );
            }
        }

        for c in &self.keys {
            eprintln!(
                "[heca]   key '{}' in layer '{}': {:?} ({}) overwritten by {:?} ({})",
                format_combo(&c.combo),
                c.mode,
                c.previous_action,
                c.previous_source,
                c.new_action,
                c.new_source,
            );
        }
    }
}

/// **Settle the two drag modifiers and tell the drag API which one means swap.**
///
/// A drag has a modifier that starts it and a modifier that says what it does. Both are user
/// settings, so both can name the same key — and then every content-area drag is a swap, with no
/// way left to express a plain move, while Shift+drag also stops selecting text in a terminal.
///
/// **The start modifier wins.** A gesture that cannot be started is useless; losing swap costs one
/// of two drop meanings, and the report says so. Called at startup and again after a reload, so a
/// config edit is answered the same way both times.
pub(crate) fn settle_drag_modifiers(
    settings: &heca_config::settings::SettingsConfig,
    conflicts: &mut Conflicts,
) {
    use heca_config::settings::ModifierKey;

    let start = settings.interactive_move_modifier;
    let swap = settings.swap_modifier;
    if start == swap {
        conflicts.modifier(ModifierConflict {
            modifier: format!("{swap:?}"),
            kept: "settings.interactive_move_modifier".to_string(),
            rejected: "settings.swap_modifier".to_string(),
            fallback: "no swap — every drag is a plain move".to_string(),
        });
        // Nothing means swap, rather than everything meaning it.
        heca_grid_ui::drag::set_swap_rule(|_| false);
        return;
    }
    heca_grid_ui::drag::set_swap_rule(move |m| match swap {
        ModifierKey::Super => m.meta,
        ModifierKey::Alt => m.alt,
        ModifierKey::Ctrl => m.ctrl,
        ModifierKey::Shift => m.shift,
    });
}

/// A combo as a user would write it in config — the form they have to search for to fix it.
pub(crate) fn format_combo(combo: &KeyCombo) -> String {
    let mut parts = Vec::new();
    if combo.ctrl {
        parts.push("Ctrl".to_string());
    }
    if combo.alt {
        parts.push("Alt".to_string());
    }
    if combo.shift {
        parts.push("Shift".to_string());
    }
    if combo.super_ {
        parts.push("Super".to_string());
    }
    parts.push(combo.key.clone());
    parts.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The two drag modifiers must not be the same key.**
    ///
    /// `interactive_move_modifier` accepts `Shift`, and swap means Shift by default — so a config
    /// naming Shift for the start of a drag makes every content-area drag a swap, with no way left
    /// to express a plain move, and Shift+drag stops selecting text in a terminal as well. It
    /// parses, so it must start; it is wrong, so it must be said out loud.
    #[test]
    fn one_modifier_cannot_both_start_a_drag_and_change_what_it_means() {
        use heca_config::settings::{ModifierKey, SettingsConfig};

        let mut agreeing = Conflicts::default();
        let settings = SettingsConfig {
            interactive_move_modifier: ModifierKey::Super,
            ..Default::default()
        };
        settle_drag_modifiers(&settings, &mut agreeing);
        assert!(
            agreeing.modifiers.is_empty(),
            "Super to start and Shift to swap are two different keys",
        );

        let mut colliding = Conflicts::default();
        let settings = SettingsConfig {
            interactive_move_modifier: ModifierKey::Shift,
            ..Default::default()
        };
        settle_drag_modifiers(&settings, &mut colliding);
        assert_eq!(
            colliding.modifiers.len(),
            1,
            "Shift for both is a collision and has to be reported, not silently obeyed",
        );
        assert!(
            !colliding.is_empty(),
            "a modifier collision is a conflict like any other"
        );
    }

    /// **The start modifier wins.** A drag that cannot be started is useless; a drag that cannot
    /// swap still moves. So when they collide, swap is what gives way — and it gives way to
    /// *nothing meaning swap*, not to everything meaning it.
    #[test]
    fn when_the_modifiers_collide_the_gesture_survives_and_swap_gives_way() {
        use heca_config::settings::{ModifierKey, SettingsConfig};
        use heca_grid_ui::drag::DropAction;
        use heca_grid_ui::event::Modifiers;

        let mut conflicts = Conflicts::default();
        let settings = SettingsConfig {
            interactive_move_modifier: ModifierKey::Shift,
            ..Default::default()
        };
        settle_drag_modifiers(&settings, &mut conflicts);

        let mut root = heca_grid_ui::widgets::Flex::column();
        heca_grid_ui::dispatch(
            &mut root,
            &heca_grid_ui::Event::ModifiersChanged(Modifiers {
                shift: true,
                ..Default::default()
            }),
        );
        assert_eq!(
            DropAction::held(),
            DropAction::Move,
            "Shift starts the drag here, so it cannot also mean swap — a plain move must stay \
             reachable",
        );
    }

    #[test]
    fn a_combo_reads_back_the_way_a_user_writes_it() {
        assert_eq!(format_combo(&KeyCombo::parse("Ctrl+Shift+k")), "Ctrl+Shift+k");
        assert_eq!(format_combo(&KeyCombo::parse("Alt+PageUp")), "Alt+pageup");
    }

    #[test]
    fn nothing_collided_is_the_ordinary_case() {
        assert!(Conflicts::default().is_empty());
    }

    #[test]
    fn a_builtin_cannot_be_taken_over() {
        let mut c = Conflicts::default();
        c.action(ActionConflict {
            id: "close".into(),
            kept: "built-in".into(),
            rejected: "component 'docker'".into(),
            shadows_builtin: true,
        });
        assert!(!c.is_empty());
        assert!(c.actions[0].shadows_builtin);
    }
}
