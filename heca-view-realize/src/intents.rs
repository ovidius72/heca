//! What a described node fires: the intents behind a press, a pick and a change, and the slot a
//! child names.

use heca_view::{Intent, PropValue, ViewEvent, ViewNode};

/// The node's `"press"` (activation) intent, to fire on click. `None` when the node isn't
/// actionable.
pub(crate) fn press_intent(node: &ViewNode) -> Option<Intent> {
    Some(node.intent(ViewEvent::Press)?.clone())
}

/// **What a leader-key pick (`prefix+/`) does to this node.**
///
/// `hint` when the node declares one, otherwise `press` — so every actionable node stays reachable
/// by letter for free (the "everything is an action" rule), and a node that wants a pick to mean
/// something *else* than a click says so. That difference is the whole reason the two are separate
/// events: heca's own sidebar row activates the pane and leaves on a click, and stays in the
/// sidebar on a hint pick. Pointing one intent at both is what made `prefix+/` leave the sidebar.
pub(crate) fn hint_intent(node: &ViewNode) -> Option<Intent> {
    node.intent(ViewEvent::Hint)
        .or_else(|| node.intent(ViewEvent::Press))
        .cloned()
}

/// The node's `"change"` intent as a carrier (value widgets — input/toggle/checkbox). Not a pick
/// target: a value change isn't a gesture a letter can stand for. Carrying the new value in the intent is planned;
/// today the change fires the bound intent as written.
pub(crate) fn change_intent(node: &ViewNode) -> Option<Intent> {
    Some(node.intent(ViewEvent::Change)?.clone())
}

/// The child's **`"slot"`** prop — which named place in its parent it belongs to (`"header"`,
/// `"leading"`, `"trailing"`).
///
/// This is how a widget with **several places to put children** stays expressible without changing
/// `ViewNode`'s shape: `children` remains one flat `Vec`, and the *child* says where it goes. No
/// `slots: Map<..>` on the node, no second child vector, and the JSON stays flat. It generalizes to
/// every future slotted widget for free.
pub(crate) fn slot_of(node: &ViewNode) -> Option<&str> {
    node.props.get("slot").and_then(PropValue::as_text)
}

/// A child named a slot its parent doesn't have (or none, where the parent has no default). Debug-log
/// it and move on: `realize` is total for untrusted input, so a plugin's typo costs it a misplaced
/// child, never a panic.
pub(crate) fn warn_unknown_slot(parent: &ViewNode, child: &ViewNode, slot: Option<&str>, known: &[&str]) {
    #[cfg(debug_assertions)]
    eprintln!(
        "[heca] realize: {:?} child of a {:?} has slot {:?} — known slots: {:?}",
        child.kind, parent.kind, slot, known,
    );
    #[cfg(not(debug_assertions))]
    let _ = (parent, child, slot, known);
}

/// A node's intent for `event`, wrapped as the carrier a widget callback fires. (`press_intent` also
/// registers a hint target; this is for events that aren't pick targets — `change`, `dismiss`, a
/// toast's inline `action`.)
pub(crate) fn intent_carrier(node: &ViewNode, event: ViewEvent) -> Option<Intent> {
    node.intent(event).cloned()
}
