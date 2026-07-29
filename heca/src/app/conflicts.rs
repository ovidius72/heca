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

/// Everything that collided while the app was being assembled.
#[derive(Debug, Default)]
pub(crate) struct Conflicts {
    pub(crate) keys: Vec<BindingConflict>,
    pub(crate) actions: Vec<ActionConflict>,
}

impl Conflicts {
    pub(crate) fn is_empty(&self) -> bool {
        self.keys.is_empty() && self.actions.is_empty()
    }

    /// A key bound to something else in the same layer.
    pub(crate) fn key(&mut self, conflict: BindingConflict) {
        self.keys.push(conflict);
    }

    /// An id two declarers both wanted.
    pub(crate) fn action(&mut self, conflict: ActionConflict) {
        self.actions.push(conflict);
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
            self.keys.len() + self.actions.len()
        );

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
