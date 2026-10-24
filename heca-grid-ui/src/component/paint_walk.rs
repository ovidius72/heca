//! **The one walk that paints a tree**: each child cut to its parent's shape, under the hue its
//! parent publishes, then what the framework draws over it — its letter, its tooltip, its drag.

use super::*;

/// Paint a child, unless it is hidden via `style.hidden` (taffy `display: none`).
/// A `display: none` subtree is collapsed to zero size at the top-left by layout,
/// so painting it would stamp its (stale, overlapping) contents there — every
/// container skips hidden children instead, matching the web. Containers with
/// bespoke paint loops (e.g. [`Pane`](crate::widgets::Pane),
/// [`DockFrame`](crate::widgets::DockFrame)) reuse this so a collapsed body/group
/// never bleeds onto the rest of the tree.
///
/// **It also draws the child's hint letter**, if it is carrying one. That is here,
/// and not in each widget, for the reason the hidden check is: it is the one place every container
/// already funnels its children through, so a letter appears over *any* widget — a plugin's
/// included — with nothing to opt into and no wrapper to remember. Before this, only
/// [`KeyHint`](crate::widgets::KeyHint) could draw one, so being pickable meant being wrapped.
///
/// **No host pass may paint a keycap.** One that walks the trees itself has to guess which scene,
/// and which half of it, the declaring widget ended up in — and it guesses wrong invisibly. See
/// `key_hint::paint_hint_label`.
pub fn paint_child(c: &dyn Component, cx: &mut PaintCx) {
    if c.base().style.layout.hidden {
        return;
    }
    // **Nothing is painted before it has a box.** `Base::node` is written by the layout engine as
    // it walks, so a widget the walk has never reached still holds `None` — it has no bounds, and
    // its default ones put it at the window's origin. Painting it draws it there for one frame: a
    // small thing flickering in the top-left corner of the scrolling area, which a resize produces
    // continuously.
    //
    // A widget added *while* its tree is being laid out is exactly that case, and it happened three
    // separate times in one session — the ⋮ built inside the decision, the ⋮ rebuilt on each
    // arrangement, and a button's words put back when the room returned. Each was patched where it
    // arose. This is the one rule underneath all three, in the one place every widget's paint
    // passes through, so no widget opts in and a fourth cannot be written: the next layout reaches
    // the new child, gives it a box, and it is drawn from then on.
    if c.base().node.is_none() {
        return;
    }
    // **A published hue covers the whole subtree, and this is the only place that can apply it.**
    // Every widget's paint passes through here, so a container states `tone` once and nothing
    // inside opts in — the alternative is each publisher wrapping its own `paint`, which is how
    // the tone ended up with consumers and no publishers at all.
    //
    // The same for the colour of the text inside (`content_tone`, CSS `color`): named by meaning,
    // resolved against this frame's theme, and published for the whole subtree.
    // **Cut to what its parent cuts its children to**, whichever way the parent draws them.
    match cx.children_clip {
        Some(clip) => cx.with_rounded_clip(clip.rect, clip.radius, |cx| paint_inherited(c, cx)),
        None => paint_inherited(c, cx),
    }
}

/// [`paint_child`] once the parent's cut is in place: the hue the widget publishes, then the paint.
fn paint_inherited(c: &dyn Component, cx: &mut PaintCx) {
    let visual = &c.base().style.visual;
    let tone = c.base().content_tone.map_or(visual.content_tone, |tone| tone.get());
    let content = tone.map(|tone| tone.resolve(cx.theme()));
    match (visual.accent, content) {
        (Some(hue), Some(text)) => cx.with_accent(hue, |cx| {
            cx.with_content_color(text, |cx| paint_subtree(c, cx))
        }),
        (Some(hue), None) => cx.with_accent(hue, |cx| paint_subtree(c, cx)),
        (None, Some(text)) => cx.with_content_color(text, |cx| paint_subtree(c, cx)),
        (None, None) => paint_subtree(c, cx),
    }
}

/// The paint itself, once the inherited hue is in place — see [`paint_child`].
fn paint_subtree(c: &dyn Component, cx: &mut PaintCx) {
    // What the widget asks to have drawn over its children (a frame) is landed when its subtree is
    // done, so it covers its own children and not what is painted after it.
    let outline = cx.scene.outline_mark();
    paint_holding(c, cx);
    cx.scene.settle_outline(outline);
    crate::flash::paint(c.base(), cx);
    crate::widgets::key_hint::paint_hint_label(c, cx);
    crate::widgets::tooltip::paint_tooltip(c, cx);
    paint_drag_feedback(c, cx);
}

/// **The widget's own paint, with its cut set for the children it draws** — see
/// [`Component::children_clip`]. What the framework draws over the widget afterwards (its letter,
/// its tooltip, its drag feedback) is not something it holds, so the cut ends here.
fn paint_holding(c: &dyn Component, cx: &mut PaintCx) {
    let clip = c.children_clip(cx.theme());
    let parent = std::mem::replace(&mut cx.children_clip, clip);
    c.paint(cx);
    cx.children_clip = parent;
}

/// **What a drag looks like, drawn by the widgets taking part in it** — the target marks where the
/// thing would land, and the source carries a picture of itself under the cursor.
///
/// It is here, beside the hint letter, because both are the same kind of thing: something the
/// framework draws *over* any widget from state the framework already keeps, so no widget opts in
/// and no host paints on their behalf. That is what every other drag-and-drop library settled on —
/// Flutter's target rebuilds with what is over it, SwiftUI and the browser show a picture of the
/// dragged view by default, and the thing that follows the cursor is drawn in a layer above
/// everything so no scroll area clips it.
///
/// heca drew all of this centrally instead: the app read its own drag phases and painted the
/// indicator and the chip for the whole window, so a new surface — a plugin's especially — got a
/// drag that looked like nothing at all until the app was taught about it.
///
/// **What a widget may still decide** is what it *is*: `Onto` versus an insertion line comes from
/// where the pointer sits in the target. Whether the drop moves or swaps is not the widget's to
/// work out — it asks [`DropAction::held`](crate::drag::DropAction::held), the same answer the drop
/// itself carries, so the outline never promises something the drop will not do.
fn paint_drag_feedback(c: &dyn Component, cx: &mut PaintCx) {
    let b = c.base();
    if b.pointer.is_drag_over() && !c.paints_own_drag_feedback() {
        let bounds = b.bounds;
        match crate::drag::DropAction::held() {
            crate::drag::DropAction::Swap => cx.swap_indicator(bounds),
            crate::drag::DropAction::Move => cx.drop_indicator(bounds, b.pointer.drag_side()),
        }
    }
    if b.pointer.is_dragging() {
        // **What you are carrying is faded where it still sits.** Otherwise a container's highlight
        // is drawn around a source that looks untouched, and the two read as both being
        // highlighted — which is exactly the case of dropping a column into the workspace that
        // holds it. Faded here, outlined there, and what is under the cursor says which is which.
        let bg = cx.theme().colors.background;
        cx.rect(
            b.bounds,
            bg.with_alpha(160),
            None,
            cx.theme().colors.border_radius,
            None,
        );
        if !b.drag_chip {
            paint_drag_image(c, cx);
            return;
        }
        // The chip: the title of what was picked up, offset off the cursor and vertically centred on it, in
        // the overlay band so nothing this widget sits inside can clip it.
        let at = b.pointer.drag_pos();
        let text = c.text_summary().unwrap_or_default();
        let h = b.bounds.size.h.clamp(18.0, 28.0);
        let w = (text.chars().count() as f64 * 7.5 + 20.0).clamp(48.0, 220.0);
        let rect = Rectangle::new(
            Point::new(at.x + 10.0, at.y - h / 2.0),
            heca_core::layout::Size::new(w, h),
        );
        cx.drag_ghost(rect, &text, crate::drag::DropAction::held().is_swap());
    }
}

/// **The picture of a carried widget**: the widget painted as it is — terminal surface and all — in
/// the top layer, faded and shrunk by the theme's tokens, with the point that was grabbed kept
/// under the pointer. Painted as an echo, so the host does not take its surfaces for where they
/// live.
fn paint_drag_image(c: &dyn Component, cx: &mut PaintCx) {
    let b = c.base();
    let (alpha, scale) = {
        let colors = &cx.theme().colors;
        (colors.drag_image_alpha, colors.drag_image_scale)
    };
    let (pos, grab) = (b.pointer.drag_pos(), b.pointer.drag_origin());
    let (dx, dy) = (pos.x - grab.x, pos.y - grab.y);
    cx.with_overlay(|cx| {
        cx.with_opacity(alpha, |cx| {
            cx.with_translate(dx, dy, |cx| {
                cx.with_scale(scale, pos, |cx| {
                    cx.with_echo(|cx| {
                        let outline = cx.scene.outline_mark();
                        paint_holding(c, cx);
                        cx.scene.settle_outline(outline);
                    });
                });
            });
        });
        if crate::drag::DropAction::held().is_swap() {
            // The same double frame the chip wears: this is an exchange, not a move.
            let k = f64::from(scale);
            let at = |p: Point| {
                Point::new(
                    pos.x + (p.x + dx - pos.x) * k,
                    pos.y + (p.y + dy - pos.y) * k,
                )
            };
            let image = Rectangle::new(
                at(b.bounds.loc),
                heca_core::layout::Size::new(b.bounds.size.w * k, b.bounds.size.h * k),
            );
            cx.swap_indicator(image);
        }
    });
}
