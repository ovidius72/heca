//! **The one door every binding goes through.**
//!
//! Bind a key, record who wrote it in the [`BindingIndex`], and hand any clash to
//! [`Conflicts`] to be reported once; unbind a key and forget it in the index too. The report
//! itself lives in [`crate::app::conflicts`]. Owns nothing about which config table a binding came
//! from.

use crate::app::conflicts::{BindingConflict, Conflicts};
use crate::keymap::{
    ActionRef, BindingIndex, BoundKey, KeyCombo, KeymapRegistry, WrittenKey, index_binding,
};

/// What a binding was written as: which action, in which layer, under which key.
///
/// Carried together because the same three facts answer two questions — where to send the user when
/// two bindings collide, and what key an action answers to ([`BindingIndex`]).
pub(super) struct Written<'a> {
    /// The action id the binding resolves to, after any component qualification.
    pub(super) action: &'a str,
    /// The layer, named the way the config file names it.
    pub(super) layer: &'a str,
    /// The combo exactly as the user types it — `prefix+` and all.
    pub(super) key: &'a str,
    /// The arguments the binding calls the action with — empty for a plain one.
    pub(super) args: &'a std::collections::HashMap<String, String>,
    /// The key that opens the mode the binding lives in — empty for a key pressed straight away.
    pub(super) after: &'a str,
}

pub(super) fn bind_with_conflict_tracking(
    keymap: &mut KeymapRegistry,
    mode: &str,
    combo: KeyCombo,
    action: ActionRef,
    written: Written<'_>,
    conflicts: &mut Conflicts,
    index: &mut BindingIndex,
) {
    if let Some(previous_action) = keymap.resolve(mode, &combo).cloned()
        && previous_action != action
    {
        conflicts.key(BindingConflict {
            mode: mode.to_string(),
            combo: combo.clone(),
            previous_action,
            previous_source: "existing binding".to_string(),
            new_action: action.clone(),
            new_source: format!("{} {}", written.layer, written.action),
        });
    }
    keymap.bind(mode, combo, action);
    index_binding(
        index,
        written.action,
        BoundKey::new(written.layer, written.key)
            .args(written.args)
            .after(written.after),
    );
}

/// **Bind a key into the flat map, in the layer it names** — after the leader or direct — through
/// [`bind_with_conflict_tracking`]. What every flat binding table does with each key it lists.
pub(super) fn bind_flat(
    keymap: &mut KeymapRegistry,
    action: ActionRef,
    written: Written<'_>,
    conflicts: &mut Conflicts,
    index: &mut BindingIndex,
) {
    let key = WrittenKey::parse(written.key);
    if key.then.is_empty() {
        bind_with_conflict_tracking(
            keymap,
            key.layer(),
            key.combo,
            action,
            written,
            conflicts,
            index,
        );
        return;
    }
    // **A sequence** — the keys after the first are pressed one after another. Only after the
    // prefix: a sequence of direct keys would hold back every key typed into a terminal while it
    // waited to see whether the next one continued it.
    if !key.after_leader {
        eprintln!(
            "[heca] {} {}: a key sequence must start after the prefix (`prefix+…`); ignored",
            written.layer, written.key
        );
        return;
    }
    let keys = key.keys();
    if let Some(previous_action) = keymap.bind_sequence(key.layer(), keys, action.clone())
        && previous_action != action
    {
        conflicts.key(BindingConflict {
            mode: key.layer().to_string(),
            combo: key.combo.clone(),
            previous_action,
            previous_source: "existing sequence".to_string(),
            new_action: action,
            new_source: format!("{} {}", written.layer, written.action),
        });
    }
    index_binding(
        index,
        written.action,
        BoundKey::new(written.layer, written.key)
            .args(written.args)
            .after(written.after),
    );
}

/// Retire a combo from a layer, and from the index with it.
///
/// An `unbind` that left the index alone would have `--show-keys` and every tooltip reporting a key
/// that no longer does anything — the exact drift the index exists to end. Matched on the parsed
/// combo rather than the literal string, so `"Prefix+W"` retires `"prefix+w"`.
pub(super) fn unbind_and_deindex(
    keymap: &mut KeymapRegistry,
    mode: &str,
    layer: &str,
    key: &str,
    index: &mut BindingIndex,
) {
    let target = WrittenKey::parse(key);
    if target.then.is_empty() {
        keymap.unbind(mode, &target.combo);
    } else {
        keymap.unbind_sequence(mode, &target.keys());
    }
    for bound in index.values_mut() {
        bound.retain(|b| b.layer != layer || WrittenKey::parse(&b.key) != target);
    }
    index.retain(|_, bound| !bound.is_empty());
}

#[cfg(test)]
mod tests {

    use crate::app::conflicts::Conflicts;
    use crate::app::registry::build_component_keymaps;
    use crate::app::registry::testing::*;

    use crate::keymap::BindingIndex;

    /// The index is what `--show-keys` and every tooltip read, so it must carry the **qualified** id
    /// and the layer — and an `unbind` must take its entry out with it.
    #[test]
    fn the_index_records_the_qualified_id_and_forgets_what_is_unbound() {
        let mut layer = layer_of(
            "workspaces",
            None,
            &[("cursor_up", "k"), ("cursor_down", "j")],
        );
        layer.unbind.insert("j".to_string(), true);
        let mut index = BindingIndex::new();
        build_component_keymaps(&with_layer(layer), &mut Conflicts::default(), &mut index);

        let up = &index["workspaces.cursor_up"];
        assert_eq!(up.len(), 1);
        assert_eq!(up[0].key, "k");
        // Reported under the general spelling whichever way the entry was written: the label names
        // where a *surface's* keys live, and `[[keys.component]]` is now one way to spell that.
        assert_eq!(up[0].layer, "[[keys.surface]] workspaces");
        assert!(
            !index.contains_key("workspaces.cursor_down"),
            "a retired key must not be reported as still running the action: {index:?}",
        );
    }
}
