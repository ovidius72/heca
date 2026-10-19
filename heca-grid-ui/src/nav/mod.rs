//! **Navigation keys** — a row's own identity, declared once and read by everything that has to
//! name a row.
//!
//! A list-shaped component labels its rows with
//! [`IdentityExt::key`](crate::builders::IdentityExt::key); the keyboard cursor, the right-click
//! target and the drag identity are all read off that **one** declaration. Three readers, one thing
//! said — instead of a closed enum of row kinds that only the app can extend, which is what made a
//! plugin row impossible to point at.
//!
//! # Why a string, and not a token from a registry
//!
//! A registry slot — the widget takes an opaque id and the app keeps the map — is valid only for as
//! long as the tree that registered it. A nav key is the opposite: it must **survive a tree
//! rebuild**, because a chrome tree is rebuilt for reasons that have nothing to do with navigation
//! (a pane's git status changing is enough), and a cursor that resets every time is not a cursor.
//! An index into a tree cannot do that; an identity the row asserts about itself can. This is also
//! why a scoped [`FocusManager`](crate::focus::FocusManager) — a visit *index* — cannot be the
//! cursor.
//!
//! A row need not be named at all: one that declares no key still has an identity derived from its
//! **content**, so a capability is never gated on having been named.
//!
//! The key is **opaque to this library**: it is a string the component chose (`"pane:7"`,
//! `"container:abc123"`), and nothing here parses it.
//!
//! ```ignore
//! // The component labels its rows; that is the whole of its side.
//! Row::new().key(format!("pane:{}", pane.id)).child(Label::new(&pane.name))
//! ```
//!
//! The functions split by job: `collect` gathers the navigable rows and hit-tests a point,
//! `identity` names a widget and finds one by name, and `ambiguity` reports a collection whose
//! items were never keyed. The two walks every group shares — skipping hidden subtrees, and
//! descending through transparent wrappers — live here.

use crate::component::Component;

mod ambiguity;
mod collect;
mod identity;

pub use ambiguity::{ambiguous_identities, Ambiguity};
pub use collect::{collect_keys, key_at};
pub use identity::{child_name, identity_of, node_with_key, node_with_key_mut};

/// Should this subtree be enumerated? Hidden widgets have stale bounds and never receive input, so
/// they're skipped — the same rule paint, event routing, drag and hint resolution use.
fn skip(c: &dyn Component) -> bool {
    !c.is_shown()
}

/// **Down through transparent wrappers to the node that actually carries the identity.**
///
/// A control is rarely the node you are handed. A chrome button is
/// `Tooltip(KeyHint(IconButton))`, and the picker addresses the node that *declared the hint* —
/// the `KeyHint` — while the thing with a name and a `key` is the `IconButton` two levels down.
/// Without this, every author wrapping a control would have to remember to put the key on the
/// wrapper instead of the control, which is a rule a caller has to remember and therefore one that
/// belongs here.
///
/// A **wrapper** is a node with no key of its own, no action of its own, and exactly one visible
/// child. The descent stops at the first node that is keyed (it said who it is) or actionable (it
/// is the control), and at anything holding several children — that is a real container, not a
/// wrapper, and descending into it would speak for something that is not one thing.
fn through_wrappers(node: &dyn Component) -> &dyn Component {
    let mut at = node;
    // Bounded by the depth walked; a wrapper chain is two or three deep in practice.
    loop {
        // A node that names itself is not a wrapper, whichever way it declared that name.
        if at.base().identity().is_some() || at.base().activatable {
            return at;
        }
        match at.base().children.as_slice() {
            [only] if !skip(only.as_ref()) => at = only.as_ref(),
            _ => return at,
        }
    }
}
