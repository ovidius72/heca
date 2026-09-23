//! **Verbs a widget answers to by name** — what a host lists to answer "what can this surface do
//! right now". The same uniform walk [`super::collect`] is, over the same retained trees.

use super::collect::skip;
use crate::component::Component;

/// **A verb a widget answers to, by name** — see [`Base::actions`](crate::component::Base::actions).
pub struct DeclaredAction {
    /// The id a binding, a menu entry or a script uses. Namespaced by whoever declared it, the same
    /// way a provider's are (`heca.expose.pick`, `mypanel.pick`).
    pub name: String,
    /// What it does. A closure, because this side of the boundary is native; the declarative path
    /// maps the same name to an `Intent`.
    pub run: Box<dyn Fn()>,
}

impl std::fmt::Debug for DeclaredAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeclaredAction")
            .field("name", &self.name)
            .finish()
    }
}

/// **Every action declared anywhere in this tree**, in document order.
///
/// What a host lists to answer "what can this surface do right now" — the same uniform walk
/// [`collect_hints`] is, over the same retained trees.
pub fn collect_actions(root: &dyn Component) -> Vec<String> {
    let mut out = Vec::new();
    actions_into(root, &mut out);
    out
}

fn actions_into(node: &dyn Component, out: &mut Vec<String>) {
    if skip(node) {
        return;
    }
    out.extend(node.base().actions.iter().map(|a| a.name.clone()));
    for child in &node.base().children {
        actions_into(child.as_ref(), out);
    }
}

/// **Run the action `name` if some widget in this tree declares it.** `false` when none does.
///
/// The name is the address — there is no path to keep, and therefore nothing that goes stale when
/// the tree is rebuilt between a binding being read and the key being pressed. The first
/// declaration in document order wins, so a nested surface's verb shadows an outer one's of the
/// same name, which is the same nearest-declaration rule menus and keys already follow.
pub fn fire_action(root: &dyn Component, name: &str) -> bool {
    if skip(root) {
        return false;
    }
    if let Some(action) = root.base().actions.iter().find(|a| a.name == name) {
        (action.run)();
        return true;
    }
    root.base()
        .children
        .iter()
        .any(|c| fire_action(c.as_ref(), name))
}
