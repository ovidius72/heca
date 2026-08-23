//! **Which widgets get a letter** — the candidacy walk, read top-down when a picker opens.
//!
//! Candidacy is a *property*, and it never propagates: a container declaring a hint does not make
//! its children pickable, nor itself pickable on their behalf. The *pick* is an event and lives in
//! [`super::fire`].

use crate::component::Component;
use crate::reactive::SignalGet;
use heca_core::layout::Rectangle;

/// Should this subtree be enumerated? Hidden widgets have stale bounds and never
/// receive input, so they're skipped (matching paint / event / drag resolution).
pub(super) fn skip(c: &dyn Component) -> bool {
    !c.base().visible.get_untracked() || c.base().style.layout.hidden
}

/// **What a picker can still see, walking down** — the clip an ancestor imposes on everything
/// beneath it, narrowed at each [`clips_children`](Component::clips_children) widget on the way.
///
/// A row scrolled out of a [`ScrollRegion`](crate::widgets::ScrollRegion) has **real bounds** and
/// simply is not drawn, so nothing above the tree can tell: the app asks whether a target is in the
/// **window**, and a row hidden by the sidebar's own fold is inside the window. The tree is where
/// the answer is, and the framework already has a name for it — paint honours `clips_children`,
/// input honours it (`pointer::hit_test`), and the picker was the one walk that never asked. Three
/// letters were handed to rows past the fold and their keycaps painted over the top bar and the
/// status bar (Antonio, driving, F003/P082/T438).
///
/// **The same trade input makes**: a child placed deliberately *outside* its clipping ancestor — a
/// dropdown panel extending past a scroll region — is judged by the clip like anything else. When
/// that needs to change it changes for input too, in both walks, and not by a special case here.
pub(super) fn narrowed(clip: Option<Rectangle>, node: &dyn Component) -> Option<Rectangle> {
    if !node.clips_children() {
        return clip;
    }
    let own = node.base().bounds;
    // **A box with no geometry clips nothing.** Before the first layout every widget's bounds are
    // zero, and a clip of nothing would hide everything beneath it — which took every letter off
    // the workspaces tree in the provider's own tests, where the body is built and collected
    // without being laid out. Degenerate bounds mean *"no answer yet"*, not *"nothing is visible"*;
    // a widget that is genuinely collapsed is hidden or invisible, which `skip` already catches.
    if own.size.w <= 0.0 || own.size.h <= 0.0 {
        return clip;
    }
    match clip {
        // Two clips that do not meet leave nothing visible: an empty box, which every candidate
        // then falls outside of — rather than `None`, which means "nothing clips this".
        Some(outer) => Some(outer.intersection(own).unwrap_or(Rectangle::new(
            own.loc,
            heca_core::layout::Size::new(0.0, 0.0),
        ))),
        None => Some(own),
    }
}

/// **Is this widget hidden by a clipping ancestor?**
///
/// **Any overlap at all counts as visible**, so a row half past the fold keeps its letter — you can
/// see it, so you can aim at it (Antonio, 2026-08-23: *"a half visible pane row should have the
/// letter to peek"*). Its keycap is deliberately **not** clipped to match: the letter is drawn whole
/// so it stays readable, which is the whole point of lettering a row you can only half see.
pub(super) fn out_of_view(node: &dyn Component, clip: Option<Rectangle>) -> bool {
    clip.is_some_and(|c| c.intersection(node.base().bounds).is_none())
}

/// **Can the user act on this widget?** A click, a double click, or a key — the three ways a widget
/// says "do something to me" (Antonio, 2026-08-17: *"if we have on_click, on_key_up,
/// on_double_click also maybe it needs to be hintable"*).
///
/// **It reads one flag and not the handler list**, because the answer is not in the handler list:
/// eight widgets — `Button`, `IconButton`, `BadgeButton`, `Toast`, `Choice`, `RailCell`, `Item`,
/// `Row` — keep their action in a private field of their own, so `Handlers::has(Click)` is `false`
/// for a `Button`. Two red tests said so before this was written. And the two spellings
/// (`on_click`, `on_activate`) mean the same thing, so no set of `EventKind`s names it either.
/// [`Base::activatable`](crate::component::Base::activatable) is set wherever the action is wired,
/// which is the only place that knows.
fn actionable(c: &dyn Component) -> bool {
    c.base().activatable
}

/// **Does this widget get a letter?**
///
/// Anything you can act on, plus anything that declared what a pick does — minus anything that said
/// [`hintable(false)`](crate::builders::ComponentExt::hintable). **Being pickable is not opt-in**
/// (F003/P082/T441): requiring a declaration is what made the picker show a curated handful and feel
/// not worth having, and it is the drift this closes — the declarative side has defaulted a pick to
/// the node's `press` all along (`heca-view-realize`).
///
/// [`Base::hint`] stays meaningful as the **override**: it says a pick does something *other* than
/// acting on the widget normally.
pub(crate) fn is_target(c: &dyn Component) -> bool {
    c.base().hintable && (c.base().hint.is_some() || actionable(c))
}

/// **Every widget in this tree that says what a pick does to it**, with the rect the letter goes
/// over, in document order. The path addresses the widget so [`fire_hint`] can reach it again.
///
/// The framework's half of [`on_hint`](crate::builders::ComponentExt::on_hint): a host walks its trees,
/// lays the letters out and draws them, and hands the pick back here. Nothing is registered, and no
/// id outlives the frame it was collected in — a retained tree rebuilt between the letters
/// appearing and one being picked simply offers a fresh set.
pub fn collect_hints(root: &dyn Component) -> Vec<(Vec<usize>, Rectangle)> {
    let mut out = Vec::new();
    hints_into(root, &mut Vec::new(), false, None, &mut out);
    out
}

/// `declared_above` — is some ancestor already saying what a pick of this region does? See the
/// shadowing rule below.
fn hints_into(
    node: &dyn Component,
    path: &mut Vec<usize>,
    declared_above: bool,
    clip: Option<Rectangle>,
    out: &mut Vec<(Vec<usize>, Rectangle)>,
) {
    if skip(node) {
        return;
    }
    let declares = node.base().hint.is_some();
    // **A declared hint shadows the mere actionability beneath it — but never another declaration**
    // (F003/P082/T441).
    //
    // `on_hint` says *"picking this does X"* about a whole region, so the widget it wraps must not
    // also wear a letter for the same gesture: the exposé's card declares a hint on its `KeyHint`
    // and an `on_activate` on the card within, and every card came out with two letters.
    //
    // But a declaration **inside** a declaration is a genuinely different target, and suppressing
    // that broke the sidebar instantly — a workspace row declares a pick and *contains* pane rows
    // that each declare their own, so the panes vanished from the picker.
    // **A target nobody can see is not a target** — and it is dropped here, not at the letter, so
    // it does not spend one of the 52 either (see [`narrowed`]).
    if node.base().hintable
        && (declares || (actionable(node) && !declared_above))
        && !out_of_view(node, clip)
    {
        out.push((path.clone(), node.base().bounds));
    }
    // Judged per node rather than by pruning the subtree: a transparent wrapper can carry bounds
    // its child does not, and pruning on one would silently take every letter beneath it.
    let clip = narrowed(clip, node);
    for (i, child) in node.base().children.iter().enumerate() {
        path.push(i);
        hints_into(child.as_ref(), path, declared_above || declares, clip, out);
        path.pop();
    }
}
