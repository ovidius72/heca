//! **Questions asked of a whole tree, and moves made to one**: what needs drawing or laying out,
//! where a named area is, and a subtree shifted or wrapped without a new layout.

use super::*;

/// Logical-pixel margin added around each damaged widget so glow/shadow halos —
/// which paint outside the widget's rect — are included in the redrawn region.
const DAMAGE_PAD: f64 = 64.0;

/// Walk the tree and union the bounds of every widget flagged
/// [`needs_paint`](Base::needs_paint) (padded for glow/shadow reach), **clearing
/// the flags**. Returns the damage rect to repaint, or `None` if nothing changed.
///
/// The host calls this each frame: on an animation/timed frame the result scissors
/// the render to just the changed pixels; on an input frame the host repaints in
/// full (an event can change unknown things) but still calls this to clear flags.
/// Hidden subtrees are skipped — their bounds are stale.
pub fn collect_damage(root: &dyn Component) -> Option<Rectangle> {
    fn union(a: Rectangle, b: Rectangle) -> Rectangle {
        let x0 = a.loc.x.min(b.loc.x);
        let y0 = a.loc.y.min(b.loc.y);
        let x1 = (a.loc.x + a.size.w).max(b.loc.x + b.size.w);
        let y1 = (a.loc.y + a.size.h).max(b.loc.y + b.size.h);
        Rectangle::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0))
    }
    fn walk(c: &dyn Component, acc: &mut Option<Rectangle>) {
        let b = c.base();
        if !b.visible.get_untracked() || b.style.layout.hidden {
            return;
        }
        if b.needs_paint() {
            b.clear_needs_paint();
            // Overlay widgets (tooltip bubble, command palette) paint outside their
            // own `bounds`; `damage_bounds` lets them report the rect that actually
            // changed. Default is `bounds`, so ordinary widgets are unaffected.
            let r = c.damage_bounds();
            let padded = Rectangle::new(
                Point::new(r.loc.x - DAMAGE_PAD, r.loc.y - DAMAGE_PAD),
                Size::new(r.size.w + 2.0 * DAMAGE_PAD, r.size.h + 2.0 * DAMAGE_PAD),
            );
            *acc = Some(acc.map_or(padded, |a| union(a, padded)));
        }
        for ch in &b.children {
            walk(ch.as_ref(), acc);
        }
    }
    let mut acc = None;
    walk(root, &mut acc);
    acc
}

/// Walk the tree and report whether any widget's **overlay surface occludes**
/// `pos` (see [`Component::overlay_occludes`]). The host's gate for synthesizing
/// a page-level action from raw input (e.g. right-click → context menu): if an
/// overlay above the page owns that point — an open modal's scrim, a palette, a
/// toast card — the action must not fire underneath it. Hidden subtrees are
/// skipped (their bounds are stale).
pub fn overlay_occluded_at(root: &dyn Component, pos: Point) -> bool {
    let b = root.base();
    if !b.visible.get_untracked() || b.style.layout.hidden {
        return false;
    }
    if root.overlay_occludes(pos) {
        return true;
    }
    b.children
        .iter()
        .any(|c| overlay_occluded_at(c.as_ref(), pos))
}

/// **Does any widget in this tree need laying out again?** Clears the flags as it walks.
///
/// The layout twin of [`collect_damage`], and the host calls it the same way — once a frame,
/// before deciding whether to run [`LayoutEngine::compute`](crate::LayoutEngine::compute). It
/// answers a question a repaint cannot: a widget that changed the *shape* of the tree has moved
/// its siblings, and only a layout pass can put them right.
///
/// Unlike damage there is nothing to union — layout is a whole-tree pass, so the answer is a bool.
/// Hidden subtrees are **not** skipped: a subtree that just became hidden is exactly the case that
/// needs the pass, and its stale bounds are what the pass is about to fix.
///
/// ```ignore
/// // In the host's frame, beside the existing damage call:
/// if heca_grid_ui::needs_layout(&ui) {
///     self.layout_dirty = true;
/// }
/// ```
/// **The part of a tree that named itself `name`** — its grid area, read back.
///
/// A widget with several parts asks for each one by the name it gave it, rather than by its
/// position in `children`. An index is right only for the arrangement it was written for: insert a
/// part and every index after it is wrong, and nothing fails, because the indices are still valid
/// indices. A name is wrong only if nothing carries it, which is exactly the question "is there a
/// header?" should be asking.
///
/// Depth-first, so a part nested inside a wrapper is still found. `None` means no part claims that
/// name — which is how a missing part answers.
pub fn area<'a>(root: &'a dyn Component, name: &str) -> Option<&'a dyn Component> {
    if root.base().grid_area.as_deref() == Some(name) {
        return Some(root);
    }
    root.base()
        .children
        .iter()
        .find_map(|c| area(c.as_ref(), name))
}

/// [`area`], for a widget that has to CHANGE the part it finds — hide it, hand it a signal, or
/// swap its contents outright.
///
/// It hands back the **slot** rather than the component, because replacing a part means replacing
/// the box: `*slot = Box::new(new_thing)`. Searching from a [`Base`] rather than a `Component` is
/// what lets a widget ask about its own parts while it is still being built.
pub fn area_slot<'a>(base: &'a mut Base, name: &str) -> Option<&'a mut Box<dyn Component>> {
    for child in base.children.iter_mut() {
        if child.base().grid_area.as_deref() == Some(name) {
            return Some(child);
        }
        // Take the borrow apart so the recursive call can have it: the child either answers or its
        // own children do, and only one of those borrows is live at a time.
        let found = area_slot(child.base_mut(), name).is_some();
        if found {
            return area_slot(child.base_mut(), name);
        }
    }
    None
}

pub fn needs_layout(root: &dyn Component) -> bool {
    fn walk(c: &dyn Component, found: &mut bool) {
        let b = c.base();
        if b.needs_layout() {
            b.clear_needs_layout();
            *found = true;
        }
        for child in &b.children {
            walk(child.as_ref(), found);
        }
    }
    let mut found = false;
    walk(root, &mut found);
    found
}

/// Translate a component's whole subtree by `(dx, dy)` — bounds only, no re-layout.
///
/// The primitive behind every widget that **places** children the layout engine could not: layout
/// computes each node's natural rect, and the widget then bakes an offset into it so that
/// **bounds === what is drawn === what is clickable**. A
/// [`ScrollRegion`](crate::widgets::ScrollRegion) bakes `-scroll_offset` this way; a
/// [`Select`](crate::widgets::Select) bakes the offset from the trigger's flow to its overlay
/// panel. Because bounds stay truthful, pointer routing, hit-testing and drag resolution — which
/// all read bounds — keep working with no special cases.
///
/// Shifting is destructive, so the widget must re-derive it after every layout pass (the engine
/// calls [`Component::on_layout`] post-order once bounds are natural again).
/// **This is the element's own displacement** — CSS `transform` on a box the layout already
/// placed. A host placing a retained tree at an absolute point is the same operation from the other
/// end, which is why the app kept a private copy of this until F003/P082/T474.
pub fn shift_subtree(c: &mut dyn Component, dx: f64, dy: f64) {
    c.base_mut().bounds.loc.x += dx;
    c.base_mut().bounds.loc.y += dy;
    let n = c.base().children.len();
    for i in 0..n {
        let child = &mut c.base_mut().children[i];
        shift_subtree(child.as_mut(), dx, dy);
    }
}

/// **Make a wrapper transparent to layout** — the sizing half of "transparent".
///
/// A wrapper that decorates without changing the picture ([`KeyHint`](crate::widgets::KeyHint),
/// [`Visibility`](crate::widgets::Visibility), [`FocusScope`](crate::widgets::FocusScope),
/// [`KeyHintGroup`](crate::widgets::KeyHintGroup)) hugs its child, so its bounds are the child's —
/// which is what the decoration is positioned off. Hugging alone is not transparency:
///
/// - a child sized as a **share** (`Length::Percent`) resolves that percentage against its parent,
///   and its parent is now the wrapper. A hugged wrapper is `Auto`, so the share resolves against
///   nothing and silently falls back to the child's **content** size — the widget stops being a
///   share and becomes as wide as its text;
/// - the same for a `max_width` a child sets to keep itself inside its container.
///
/// That is why the exposé's cards would not shrink with the window: each card asked for 100% of a
/// wrapper that asked for 100% of nothing, so a card stayed as wide as the path inside it and every
/// card's text ran across its neighbours. `expose/mod.rs` already carried a hand-written workaround
/// — "the room the panel gives it has to be passed on deliberately" — which is one call site fixing
/// a rule that belongs here.
///
/// Adopting whatever the child declares keeps the chain unbroken, and a child that hugs still hugs,
/// because then there is nothing to adopt.
pub fn wrap_transparently(base: &mut Base, child: &dyn Component) {
    // **Saying it once, here.** A wrapper is transparent to layout *and* to the picker — it is the
    // widget inside it, so the two must not disagree about what it is.
    base.transparent = true;
    let child = child.base().style.layout;
    if !matches!(child.width, crate::style::Length::Auto) {
        base.style.layout.width = child.width;
    }
    if !matches!(child.height, crate::style::Length::Auto) {
        base.style.layout.height = child.height;
    }
    base.style.layout.max_width = base.style.layout.max_width.or(child.max_width);
    base.style.layout.max_height = base.style.layout.max_height.or(child.max_height);
    // **Whether it may be squeezed is the content's answer too.** Everything gives way by default,
    // so a wrapper around something that refuses — a `StatusDot`, whose circle has no narrower
    // version — gave way in its place, and the dot was drawn as a 3px sliver inside a box that had
    // shrunk around it. The wrapper has no opinion of its own; it *is* the widget inside it
    // (F003/P096/T483).
    base.style.layout.flex_shrink = base.style.layout.flex_shrink.or(child.flex_shrink);
}
