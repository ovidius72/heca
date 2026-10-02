//! Making the keyboard's owner true: **one holder per tree**.
//!
//! A widget asks for focus ([`Base::focus`], a followed signal turning true) but cannot take it from
//! the previous holder — it holds no tree. [`settle`] does, and it runs wherever a tree is handed an
//! event that depends on the answer (a key, a press) and once a frame from the host, so the answer
//! is never stale where anyone looks.
//!
//! The rules, the browser's:
//!
//! - **A claim takes the keyboard**, and the previous holder lets go.
//! - **A followed signal that turns true takes it *unless something inside already has it*** — a dock
//!   told it holds the keyboard does not steal it from the terminal the user just clicked in.
//! - **A modal remembers who held the keyboard when it opened and gives it back when it closes**,
//!   if the keyboard was still its own.
//! - **A region that lets go remembers what inside it held the keyboard**, so taking it again goes
//!   back there.

use super::door::Claim;
use super::{at, at_mut, blur_widget, focus_widget};
use crate::component::Component;
use crate::reactive::SignalGet;

/// A node that has something to say to the next settle.
struct Seen {
    path: Vec<usize>,
    flagged: bool,
    visible: bool,
    claim: Claim,
    released: bool,
}

fn gather(node: &dyn Component, path: &mut Vec<usize>, shown: bool, out: &mut Vec<Seen>) {
    let base = node.base();
    base.sync_focus_follow();
    // The root is the tree itself: whether *it* is shown is the host's question, not the tree's.
    let shown =
        shown && (path.is_empty() || (base.visible.get_untracked() && !base.style.layout.hidden));
    let flagged = base.focused.get_untracked();
    let (claim, released) = (base.focus_door.claim.get(), base.focus_door.released.get());
    if flagged || claim != Claim::None || released {
        out.push(Seen {
            path: path.clone(),
            flagged,
            visible: shown,
            claim,
            released,
        });
    }
    for (i, child) in base.children.iter().enumerate() {
        path.push(i);
        gather(child.as_ref(), path, shown, out);
        path.pop();
    }
}

/// The path to the widget with focus identity `id` anywhere under `scope`, if it is still shown and
/// can still take focus.
fn find_focusable(root: &dyn Component, scope: &[usize], id: u64) -> Option<Vec<usize>> {
    fn walk(
        node: &dyn Component,
        path: &mut Vec<usize>,
        scope_len: usize,
        id: u64,
    ) -> Option<Vec<usize>> {
        let base = node.base();
        let shown =
            path.len() == scope_len || (base.visible.get_untracked() && !base.style.layout.hidden);
        if !shown {
            return None;
        }
        if base.focus_door.id.get() == id {
            return node.focusable().then(|| path.clone());
        }
        for (i, child) in base.children.iter().enumerate() {
            path.push(i);
            let found = walk(child.as_ref(), path, scope_len, id);
            path.pop();
            if found.is_some() {
                return found;
            }
        }
        None
    }
    walk(at(root, scope), &mut scope.to_vec(), scope.len(), id)
}

/// Tell the widget at `path` it now has the keyboard (hook, then event), as this settle's own
/// doing — so it leaves nothing pending for the next one.
fn arrive(root: &mut dyn Component, path: &[usize]) {
    focus_widget(at_mut(root, path), false);
    at(root, path).base().focus_door.claim.set(Claim::None);
}

/// A followed signal turned true for the widget at `path`: take the keyboard, unless something
/// inside it already has it.
fn follow(root: &mut dyn Component, path: &[usize], holder: &mut Option<Vec<usize>>, prior: u64) {
    let door = &at(root, path).base().focus_door;
    let modal = door.follows.get().is_some_and(|f| f.modal);
    // Something inside already has it: this region is where the keyboard already is. (A surface
    // that opened with a control focused inside it, in the same moment, still remembers who held
    // the keyboard before.)
    if holder
        .as_ref()
        .is_some_and(|h| h.as_slice() != path && h.starts_with(path))
    {
        if modal && prior != 0 {
            door.opener.set(prior);
        }
        at(root, path).base().blur();
        return;
    }
    let remembered = door.remembered.get();
    if let Some(previous) = holder.take().filter(|h| h.as_slice() != path) {
        if modal {
            let opener = at(root, &previous).base().focus_id();
            at(root, path).base().focus_door.opener.set(opener);
        }
        blur_widget(at_mut(root, &previous));
    }
    // A region goes back to the control it held when it last let go; a modal has no such memory.
    let back = (!modal && remembered != 0)
        .then(|| find_focusable(root, path, remembered))
        .flatten();
    match back {
        Some(inner) => {
            at(root, path).base().blur();
            arrive(root, &inner);
            *holder = Some(inner);
        }
        None => {
            arrive(root, path);
            *holder = Some(path.to_vec());
        }
    }
}

/// **Make the keyboard's owner single.** Idempotent: nothing pending, nothing changes.
pub(crate) fn settle(root: &mut dyn Component) {
    let mut seen = Vec::new();
    gather(root, &mut Vec::new(), true, &mut seen);
    if seen.is_empty() {
        return;
    }

    // Hidden things cannot hold the keyboard; let go of them quietly.
    for s in seen.iter().filter(|s| s.flagged && !s.visible) {
        at(root, &s.path).base().blur();
    }
    // Who holds the keyboard now. A widget asked for it by hand (`Take`) is the newest holder and the
    // one before it lets go; with no such request the holder is the one already settled. (If widgets
    // set the flag by hand and left several, the later one in the tree wins.)
    let flagged = |claim: fn(Claim) -> bool| {
        seen.iter()
            .filter(|s| s.flagged && s.visible && claim(s.claim))
            .map(|s| s.path.clone())
            .collect::<Vec<_>>()
    };
    let mut previous = flagged(|c| c == Claim::None);
    let mut taken = flagged(|c| c == Claim::Take);
    let mut prior = 0;
    let mut holder = match taken.pop() {
        Some(new) => {
            if let Some(old) = previous.last() {
                prior = at(root, old).base().focus_id();
            }
            for stale in previous.drain(..).chain(taken) {
                blur_widget(at_mut(root, &stale));
            }
            Some(new)
        }
        None => {
            let current = previous.pop();
            for stale in previous {
                blur_widget(at_mut(root, &stale));
            }
            current
        }
    };
    for s in seen.iter().filter(|s| s.claim == Claim::Take) {
        at(root, &s.path).base().focus_door.claim.set(Claim::None);
    }

    // Letting go first, for everyone, so a region that releases while another claims still gets to
    // remember where the keyboard was inside it.
    let mut restore = None;
    for s in seen.iter().filter(|s| s.released) {
        let base = at(root, &s.path).base();
        base.focus_door.released.set(false);
        let modal = base.focus_door.follows.get().is_some_and(|f| f.modal);
        let opener = base.focus_door.opener.replace(0);
        let held_itself = base.focus_door.held_when_released.replace(false);
        let inside = holder.clone().filter(|h| h.starts_with(&s.path));
        let held_inside = inside.is_some();
        if let Some(h) = inside {
            if !modal && h != s.path {
                let inner = at(root, &h).base().focus_id();
                at(root, &s.path).base().focus_door.remembered.set(inner);
            }
            blur_widget(at_mut(root, &h));
            holder = None;
        }
        // Give the keyboard back only if it was this surface's to give: it held it itself, or
        // something inside it did.
        if modal && opener != 0 && (held_itself || held_inside) {
            restore = Some(opener);
        }
    }

    for s in seen.iter().filter(|s| s.claim == Claim::Follow) {
        at(root, &s.path).base().focus_door.claim.set(Claim::None);
        follow(root, &s.path, &mut holder, prior);
    }

    // A modal closed over the keyboard and nobody took it since: back to the opener.
    if holder.is_none()
        && let Some(id) = restore
        && let Some(path) = find_focusable(root, &[], id)
    {
        arrive(root, &path);
    }
}

/// The path to the widget that owns the keyboard in `node`, or `None` when nothing does.
///
/// Read-only, so it brings followed widgets up to date but settles nothing: with one holder per tree
/// there is one answer; between a claim and the next settle the **later-added** wins, which is the
/// hit-test order (what is drawn on top owns the input). Hidden and invisible subtrees are skipped —
/// a closed overlay must not pull the keyboard into something nobody can see.
pub(crate) fn holder_path(node: &dyn Component) -> Option<Vec<usize>> {
    node.base().sync_focus_follow();
    for (i, child) in node.base().children.iter().enumerate().rev() {
        if !child.base().visible.get_untracked() || child.base().style.layout.hidden {
            continue;
        }
        if let Some(mut sub) = holder_path(child.as_ref()) {
            sub.insert(0, i);
            return Some(sub);
        }
    }
    node.base().focused.get_untracked().then(Vec::new)
}

/// **A press gives the keyboard to the deepest focusable under it** — the browser's rule, for every
/// tree, with nothing declared. `hit` is the path the router resolved the press to.
///
/// - the target is the deepest widget on `hit` that is [`focusable`](Component::focusable); a
///   `Button` that composes an icon and a label is the target when the label is what was hit;
/// - a press on **nothing focusable changes nothing**: the keyboard stays where it was;
/// - a press **inside the widget that already holds the keyboard** (or on it) changes nothing;
/// - otherwise the previous holder lets go and the target is focused, without a ring — it was the
///   mouse.
///
/// It runs before the tree is told of the press, so a widget that consumes the press is still the
/// widget that holds the keyboard afterwards.
pub(crate) fn focus_on_press(root: &mut dyn Component, hit: &[usize]) {
    settle(root);
    let Some(target) = (0..=hit.len())
        .rev()
        .map(|depth| &hit[..depth])
        .find(|path| at(root, path).focusable())
    else {
        return;
    };
    let holder = holder_path(root);
    if holder.as_deref().is_some_and(|h| h.starts_with(target)) {
        return;
    }
    if let Some(previous) = holder {
        blur_widget(at_mut(root, &previous));
    }
    focus_widget(at_mut(root, target), false);
}
