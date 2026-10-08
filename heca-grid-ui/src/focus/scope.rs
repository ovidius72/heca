//! **Focusing a named region** — the host's way to say "the keyboard goes to that dock" — and the
//! reads that go with it.
//!
//! A region names itself with [`scope_key`](crate::builders::ComponentExt::scope_key). These calls
//! find it by that name and move the keyboard through the same door everything else uses, so the
//! tree stays the only record of where the keyboard is: there is no host flag to keep in step.
//!
//! **For the host only.** They take a name because the host is the one that knows regions by name
//! (an action, an RPC line). A plugin that wants its own region focused calls a method on the widget
//! it built — it never passes a name string.

use super::door::Claim;
use super::settle::{follow, holder_path, settle};
use super::{at, at_mut, blur_widget};
use crate::component::Component;
use crate::reactive::SignalGet;

/// The path to the first shown region that named itself `name`, or `None`.
fn scope_path(root: &dyn Component, name: &str) -> Option<Vec<usize>> {
    fn walk(node: &dyn Component, path: &mut Vec<usize>, name: &str) -> Option<Vec<usize>> {
        let base = node.base();
        let shown = path.is_empty() || (base.visible.get_untracked() && !base.style.layout.hidden);
        if !shown {
            return None;
        }
        if base.scope_key.as_deref() == Some(name) {
            return Some(path.clone());
        }
        for (i, child) in base.children.iter().enumerate() {
            path.push(i);
            let found = walk(child.as_ref(), path, name);
            path.pop();
            if found.is_some() {
                return found;
            }
        }
        None
    }
    walk(root, &mut Vec::new(), name)
}

/// **Give the keyboard to the region called `name`.** Returns whether a shown region has that name.
///
/// The region takes it **unless something inside it already holds it** — a dock told to take the
/// keyboard does not steal it from the control the user just clicked in — and otherwise goes back to
/// the control inside that held it when the region last let go (the terminal in the dock), or to the
/// region itself. Taking it again while it is there changes nothing.
///
/// **For the host.** It finds the region by a name string, which only the host knows; a plugin
/// focuses its own region with a method on its own widget.
pub fn focus_scope(root: &mut dyn Component, name: &str) -> bool {
    settle(root);
    let Some(path) = scope_path(root, name) else {
        return false;
    };
    let mut holder = holder_path(root);
    if holder.as_ref().is_some_and(|h| h.starts_with(&path)) {
        return true;
    }
    // A request through the door, settled now: a read straight after it is already true.
    at(root, &path).base().focus_door.claim.set(Claim::None);
    follow(root, &path, &mut holder, 0);
    true
}

/// **Take the keyboard out of the region called `name`**, leaving it with nobody. Returns whether a
/// shown region has that name. The region remembers which control inside held it, so
/// [`focus_scope`] goes back there.
///
/// Does nothing when the keyboard is not inside the region. **For the host**, like
/// [`focus_scope`].
pub fn release_scope(root: &mut dyn Component, name: &str) -> bool {
    settle(root);
    let Some(path) = scope_path(root, name) else {
        return false;
    };
    let Some(holder) = holder_path(root).filter(|h| h.starts_with(&path)) else {
        return true;
    };
    if holder != path {
        let inner = at(root, &holder).base().focus_id();
        at(root, &path).base().focus_door.remembered.set(inner);
    }
    blur_widget(at_mut(root, &holder));
    true
}

/// **Does the keyboard sit inside this subtree?** CSS `:focus-within`: true for the widget that
/// holds it and for everything that contains that widget. A region asks it of itself to draw its
/// ring, so what is outlined is what receives the keys.
pub fn contains_keyboard(node: &dyn Component) -> bool {
    holder_path(node).is_some()
}

/// **The named region the keyboard is in on the page** — looking through a surface above the page
/// to where the keyboard goes back when it closes.
///
/// A dialog or the palette takes the keyboard, but it does not take the dock from under it: closing
/// it hands the keyboard back to the control that opened it. So while a surface holds it, the answer
/// is the region of that opener. `None` when the keyboard is on a page widget outside every named
/// region, on nothing, or in a surface that took it from nothing.
pub fn page_scope(root: &dyn Component) -> Option<String> {
    let path = holder_path(root)?;
    let mut scope = root.base().scope_key.clone();
    let mut in_surface = root.base().surface_slot.is_some();
    let mut opener = root.base().focus_door.opener.get();
    let mut node = root;
    for &i in &path {
        node = node.base().children[i].as_ref();
        let base = node.base();
        scope = base.scope_key.clone().or(scope);
        in_surface |= base.surface_slot.is_some();
        if opener == 0 {
            opener = base.focus_door.opener.get();
        }
    }
    if !in_surface {
        return scope;
    }
    if opener == 0 {
        return None;
    }
    scope_of_focus_id(root, opener)
}

/// The region enclosing the widget whose focus identity is `id`, wherever it is in the tree.
fn scope_of_focus_id(root: &dyn Component, id: u64) -> Option<String> {
    fn walk<'a>(
        node: &'a dyn Component,
        id: u64,
        scope: Option<&'a str>,
    ) -> Option<Option<&'a str>> {
        let scope = node.base().scope_key.as_deref().or(scope);
        if node.base().focus_door.id.get() == id {
            return Some(scope);
        }
        node.base()
            .children
            .iter()
            .find_map(|child| walk(child.as_ref(), id, scope))
    }
    walk(root, id, None).flatten().map(str::to_string)
}

#[cfg(test)]
mod tests;
