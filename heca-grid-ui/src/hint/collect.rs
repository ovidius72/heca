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
pub(crate) fn skip(c: &dyn Component) -> bool {
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
pub(crate) fn narrowed(clip: Option<Rectangle>, node: &dyn Component) -> Option<Rectangle> {
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
pub(crate) fn out_of_view(node: &dyn Component, clip: Option<Rectangle>) -> bool {
    clip.is_some_and(|c| c.intersection(node.base().bounds).is_none())
}

/// **Has this widget been squeezed to nothing?** — laid out with no width, or no height.
///
/// Such a widget draws nothing, so there is nothing to aim at. Nobody asked before, and nothing
/// downstream caught it either: the cap's own placement is computed from the target's box, so
/// [`HintPlacement::Center`](crate::widgets::HintPlacement) on a target of zero width puts the
/// keycap half a cap to the *left* of it — outside a thing with no inside. Several panes squeezed
/// to nothing by two sidebars in a narrow window stacked their letters in the gutter between them,
/// looking dragged there (F003/P082/T478). The layout no longer produces that pane, and this is why
/// no widget of any other shape can produce it again.
///
/// ⚠️ **A box with no geometry AT ALL has not been laid out yet** — no position and no size, which
/// is what every widget's bounds are before the first layout pass. That is *"no answer yet"*, never
/// *"invisible"*: a chrome tree is rebuilt with zero bounds and laid out afterwards, and judging it
/// in between calls every row hidden and takes back its letter (Antonio, driving, 2026-08-24). A
/// widget that is genuinely gone is hidden or invisible, which [`skip`] already catches.
pub(crate) fn collapsed(node: &dyn Component) -> bool {
    let b = node.base().bounds;
    let unplaced = b.loc.x == 0.0 && b.loc.y == 0.0 && b.size.w == 0.0 && b.size.h == 0.0;
    !unplaced && (b.size.w <= 0.0 || b.size.h <= 0.0)
}

/// **Can this widget show a letter at all?** The one visibility question, asked in both places a
/// letter is decided: by the collector when it spends one of the 52, and by the offer walk when it
/// hands one over. Two copies of it would be two answers, and only one of them is what you see.
pub(crate) fn unseen(node: &dyn Component, clip: Option<Rectangle>) -> bool {
    out_of_view(node, clip) || collapsed(node)
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
    is_target_of(c, None)
}

/// The same question, asked **by a picker with a scope of its own**.
///
/// One definition with the scope as a parameter, not two walks each deciding what a target is: they
/// had drifted, and the global picker lettered an ordinary button where a surface's own picker did
/// not, so a plugin owning a picker over a panel of buttons got no letters and nothing said why.
pub(crate) fn is_target_of(c: &dyn Component, scope: Option<&str>) -> bool {
    in_scope(c, scope) && is_addressable(c)
}

/// **Could a letter land on this at all** — the same question with the picker's scope left out.
///
/// A scope answers *which collected picker may letter this*. **Naming a target outright is not a
/// collection**, so it is not the scope's business: a host that asks for `col:3` or `workspaces` by
/// name has already decided what it means, and the target has no set to be excluded from.
///
/// Keeping the two apart is what lets a target be taken out of `prefix+/` without also
/// disappearing from the pick that owns it: the workspaces dock is a keyboard destination for
/// `prefix+Shift+e` and nothing the ordinary picker should spend a letter on, and it could not be
/// both until this was separated (Antonio, driving, 2026-09-14).
pub(crate) fn is_addressable(c: &dyn Component) -> bool {
    c.base().hintable && (c.base().hint.is_some() || actionable(c))
}

/// **Does this target belong to the picker asking?**
///
/// `scope` is the picker's: `None` is the ordinary one — `prefix+/` in heca — and a name is a
/// surface's own verb. A target declares the scopes it answers to with
/// [`hint_scope`](crate::builders::ComponentExt::hint_scope), and declaring none means it belongs
/// to the ordinary picker and to nothing else.
///
/// **The asymmetry is deliberate.** A scoped target does *not* also appear in the ordinary picker:
/// a ⊠ that deletes a card must not wear a letter in the picker you use to jump between them, and
/// a column that exists to receive a moved pane is not somewhere `prefix+/` should send you.
/// Belonging to both is still sayable — name the scope *and* let the ordinary picker have it by
/// declaring a second target — but it is never the default, because the failure of getting this
/// backwards is destructive.
pub(crate) fn in_scope(c: &dyn Component, scope: Option<&str>) -> bool {
    match scope {
        None => c.base().hint_scopes.is_empty(),
        Some(want) => c.base().hint_scopes.iter().any(|s| s == want),
    }
}

/// **Is this node a picker for a different scope than the one collecting?**
///
/// A walk stops here. Two pickers over one tree is the whole point of scopes, and a picker that
/// descended into another's subtree would letter targets that answer to somebody else — which is
/// the sixteen-letters-for-eight failure, arriving by a different route.
pub(crate) fn foreign_picker(c: &dyn Component, scope: Option<&str>) -> bool {
    match c.base().picker_scope.as_deref() {
        Some(theirs) => Some(theirs) != scope,
        None => false,
    }
}

/// **Every widget in this tree that says what a pick does to it**, with the rect the letter goes
/// over, in document order. The path addresses the widget so [`fire_hint`] can reach it again.
///
/// The framework's half of [`on_hint`](crate::builders::ComponentExt::on_hint): a host walks its trees,
/// lays the letters out and draws them, and hands the pick back here. Nothing is registered, and no
/// id outlives the frame it was collected in — a retained tree rebuilt between the letters
/// appearing and one being picked simply offers a fresh set.
pub fn collect_hints(root: &dyn Component) -> Vec<(Vec<usize>, Rectangle)> {
    collect_hints_scoped(root, None)
}

/// **The same walk, asked by a picker with a scope of its own.**
///
/// `None` is the ordinary picker — `prefix+/` in heca. A name is a surface's own verb, and only
/// targets declaring it answer.
///
/// It exists because [`KeyHintGroup`](crate::widgets::KeyHintGroup) had a **second walk** of its
/// own. The two had already disagreed once about what a target even *is*, which was fixed by
/// sharing the predicate — but the walk stayed duplicated, so every rule the collector learned
/// afterwards reached one picker and not the other. The wrapper rule was the next one: a surface's
/// own picker lettered both a `KeyHint` wrapper and the widget inside it, two keycaps on one card
/// (Antonio, driving the exposé, 2026-09-15).
pub fn collect_hints_scoped(
    root: &dyn Component,
    scope: Option<&str>,
) -> Vec<(Vec<usize>, Rectangle)> {
    let mut out = Vec::new();
    hints_into(root, &mut Vec::new(), None, None, scope, &mut out);
    out.into_iter().flatten().collect()
}

/// **One surface's pick targets** — everything a letter could land on inside it, and what the
/// surface itself is, so a host never has to walk back down the tree to find out.
///
/// A "surface" is a node marked [`Base::surface`]: a layer, an overlay, a menu, a plugin's panel —
/// anything seated *above* the page rather than laid out in it. The page's own targets come back in
/// a group whose [`path`](Self::path) is empty, so "is this the page or a surface" is answered by
/// structure and never by matching a name.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceHints {
    /// Path to the surface node, from the root. **Empty for the page itself.**
    pub path: Vec<usize>,
    /// The surface's own declared key, when it has one — what a host looks it up by.
    pub key: Option<String>,
    /// The surface's laid-out box, for a host that occludes by geometry.
    pub bounds: Rectangle,
    /// What it declares about standing in front of the page — [`Base::lock`].
    pub lock: bool,
    /// **Whether it holds the keyboard** — observed unless the surface overrode it.
    ///
    /// A surface that wants keys *holds focus*: `Overlay`, `ContextMenu` and `CommandPalette` all
    /// bind their open signal to [`Base::focused`], which is the whole of how an open layer takes
    /// the keyboard. So an author writes nothing and an open overlay answers `true` by being open,
    /// while an ambient stack answers `false` by holding no focus.
    /// [`Base::captures_keyboard`] overrides it for a surface the framework cannot read.
    pub holds_keyboard: bool,
    /// Its targets, each addressed **from the root** so one resolver still reaches them.
    pub targets: Vec<(Vec<usize>, Rectangle)>,
}

/// **Every pick target, grouped by the surface that owns it, front → back.**
///
/// The picker's other half. [`collect_hints`] answers *what* can be lettered; this answers *whose*
/// it is — which is what a host needs to ask each surface what it hides, and to stop at the one
/// holding the keyboard. Built **on** `collect_hints`, never beside it, so the candidacy rules stay
/// in one place.
///
/// **Front → back is lexicographic on the path, reversed.** Sibling order is paint order and a
/// child is drawn above its parent, so descending path order is exactly "nearest the viewer first"
/// — the same rule the surface tree already states about z, rather than a second ordering a caller
/// keeps in step by hand.
///
/// **Nesting is handled, so a caller never assumes a shape.** A target belongs to the *deepest*
/// surface enclosing it, so a menu inside a dialog inside an overlay groups under the menu. A host
/// that instead reads the first step of a path is assuming every surface is a direct child of the
/// root — true only for as long as whatever seats them keeps making it true.
pub fn collect_hints_by_surface(root: &dyn Component) -> Vec<SurfaceHints> {
    let mut groups: Vec<SurfaceHints> = Vec::new();
    for (path, bounds) in collect_hints(root) {
        let owner = enclosing_surface(root, &path);
        match groups.iter_mut().find(|g| g.path == owner) {
            Some(g) => g.targets.push((path, bounds)),
            None => {
                let node = node_at(root, &owner);
                groups.push(SurfaceHints {
                    key: node.base().key.clone(),
                    bounds: node.base().bounds,
                    lock: node.base().lock,
                    holds_keyboard: node
                        .base()
                        .captures_keyboard
                        .unwrap_or_else(|| crate::component::focus_path(node).is_some()),
                    path: owner,
                    targets: vec![(path, bounds)],
                });
            }
        }
    }
    // Front → back: deepest/latest first. `Reverse` on the path is the whole ordering rule.
    groups.sort_by(|a, b| b.path.cmp(&a.path));
    groups
}

/// The path of the **deepest** surface enclosing `path`, or empty for one the page owns directly.
fn enclosing_surface(root: &dyn Component, path: &[usize]) -> Vec<usize> {
    let mut node = root;
    let mut here = Vec::new();
    let mut deepest = Vec::new();
    for &step in path {
        let Some(child) = node.base().children.get(step) else {
            break;
        };
        node = child.as_ref();
        here.push(step);
        if node.base().surface {
            deepest = here.clone();
        }
    }
    deepest
}

/// The node `path` names, or `root` when it names nothing left.
fn node_at<'a>(root: &'a dyn Component, path: &[usize]) -> &'a dyn Component {
    let mut node = root;
    for &step in path {
        let Some(child) = node.base().children.get(step) else {
            return node;
        };
        node = child.as_ref();
    }
    node
}

/// **Every pick target in this tree that answers to `identity`** — a pick's address across frames.
///
/// A path is child indices: it lives one frame, and a rebuilt tree does not merely invalidate it,
/// it makes it name a *different* widget. An identity ([`identity_of`](crate::identity_of)) outlives
/// the tree, so everything holding on to a pick between frames holds the identity and asks this.
///
/// **It asks the collector, and that is the whole point.** Several nodes answer to one identity on
/// purpose: `identity_of` resolves *through wrappers*, so a `KeyHint` around a keyed `Row` — and
/// every container above it that wraps nothing else — all report the row's name. Answering "the
/// first node with that name" hands back the outermost one, which is commonly the tree's own root:
/// the letter is then offered to something that declared no pick, and nothing is lettered at all
/// (F003/P082/T438). Asking [`collect_hints`] instead can only return a node the picker itself
/// would have chosen, so offering and collecting cannot disagree.
///
/// **Every one of them, not the first.** One thing is commonly shown in several places at once — a
/// pane listed in the left sidebar *and* the right one is two views of the same pane, and both wear
/// its letter. Answering with the first match lettered one of them and left the other dark, which
/// is a bug this codebase has already had once, from an `.any()` in the by-key walk
/// (F003/P082/T431). A caller that wants one takes the first; a caller handing out a letter takes
/// them all.
pub fn hint_targets_of(root: &dyn Component, identity: &str) -> Vec<Vec<usize>> {
    collect_hints(root)
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| crate::identity_of(root, path).as_deref() == Some(identity))
        .collect()
}

/// **One letter per THING, not per layer** — the walk, and the whole of the de-duplication rule.
///
/// A widget that can be acted on gets a letter. That is all it takes, and **where it sits never
/// enters into it**: a button in a pane's bar, in a sidebar row, in a plugin's panel or on its own
/// is the same button, and an author never has to know which. Anything else makes placement a thing
/// developers must think about, which is exactly what this project refuses (AGENTS § 0d).
///
/// The one case that genuinely needs handling is a **wrapper**: a node that exists only to hold one
/// other node. The exposé's card is wrapped by a decorator that says what picking does, and the card
/// within can be activated — two nodes, one card, and two letters came out of it.
///
/// So the rule counts things:
///
/// - a **wrapper and the single node it holds are one thing**, and share one letter — the inner one,
///   which is the more precise answer and the one whose bounds the letter should sit on;
/// - a node holding **more than one** child is a real container, and what is inside it are separate
///   things — each keeps its own letter, and so does the container if it is a target itself.
///
/// A pane holds a bar and its content, so it is a container: the pane keeps its letter and every
/// button in its bar keeps one too.
///
/// ⚠️ **What this replaces, so it is not reinstated.** The previous rule said a declared hint
/// silenced mere actionability beneath it. That silenced *layers*, and could not tell a decorator
/// speaking for one card from a pane that merely contains buttons. It made a button's letter depend
/// on what it had been put inside — so every button in a pane's bar had to repeat its own click as a
/// hint to win its letter back, and the one control the bar builds for itself (the overflow `⋮`) had
/// nobody to do that for it and silently wore none (Antonio, driving, 2026-09-04).
fn hints_into(
    node: &dyn Component,
    path: &mut Vec<usize>,
    // `wrapping`: the target this node would be a mere layer of — `Some((i, declared))` when every
    // node from that target down to here holds exactly one child, so they are all one thing.
    wrapping: Option<(usize, bool)>,
    clip: Option<Rectangle>,
    scope: Option<&str>,
    out: &mut Vec<Option<(Vec<usize>, Rectangle)>>,
) {
    if skip(node) {
        return;
    }
    // **A surface that owns its own picker keeps its targets.** Walking into it would letter, in
    // the ordinary picker, things that answer to that surface's verb instead.
    if foreign_picker(node, scope) {
        return;
    }
    // **A target nobody can see is not a target** — dropped here rather than at the letter, so it
    // does not spend one of the 52 either. Hidden by an ancestor's clip, or squeezed to nothing of
    // its own: one question, asked once (see [`unseen`]).
    let mut here = wrapping;
    if is_target_of(node, scope) && !unseen(node, clip) {
        let declares = node.base().hint.is_some();
        // **Among layers of one thing, a declaration outranks mere actionability.** Saying what a
        // pick does is precisely saying it is *not* the click — a sidebar row is activated and left
        // by a click, and peeked at without leaving by a pick — so the layer that said so is the
        // one the letter must run. Letting the inner layer win regardless turned every pane row's
        // letter into "go there and leave", which is the click it was overriding.
        //
        // Otherwise the inner layer wins: it is the more precise answer, and its bounds are where
        // the letter belongs.
        let outer_speaks_for_this = matches!(wrapping, Some((_, true))) && !declares;
        if !outer_speaks_for_this {
            if let Some((outer, _)) = wrapping {
                out[outer] = None;
            }
            out.push(Some((path.clone(), node.base().bounds)));
            here = Some((out.len() - 1, declares));
        }
    }
    // **Only a transparent wrapper passes the chain on** — it is the widget inside it, so the two
    // are layers of one thing. A real container's children are things of their own, and it is a
    // thing of its own too.
    //
    // This used to count children: a node holding exactly one was *assumed* to be a wrapper. So
    // whether a container kept its own letter depended on how many things were inside it — the
    // workspaces dock wore one with two workspaces and none with one, because with one the chain
    // ran unbroken down to the workspace row, which declared a pick and replaced it. A parent
    // counting its children to work out what it is is the pattern this codebase forbids
    // everywhere else; `Base::transparent` is the node answering for itself instead.
    let pass = node.base().transparent.then_some(here).flatten();
    // Judged per node rather than by pruning the subtree: a transparent wrapper can carry bounds
    // its child does not, and pruning on one would silently take every letter beneath it.
    let clip = narrowed(clip, node);
    for (i, child) in node.base().children.iter().enumerate() {
        path.push(i);
        hints_into(child.as_ref(), path, pass, clip, scope, out);
        path.pop();
    }
}
