//! The select's runtime: the geometry of the panel and its rows, opening and
//! closing the list, committing a choice, and the `Component` impl that measures,
//! paints and routes input for the trigger and the open overlay.

use super::*;
use crate::action::SignalData;
use crate::component::{Component, Event, GridKey, Handled, PaintCx, WidgetIntent};
use crate::font::MONO_LINE_RATIO;
use crate::scene::Border;
use crate::style::{Align, Length};
use crate::widgets::choice;
use heca_core::layout::{Point, Rectangle};

impl Select {
    /// The measured height of row `i` (fallback: font-derived, before the first layout).
    fn row_h(&self, i: usize) -> f64 {
        self.natural_h
            .get(i)
            .copied()
            .filter(|h| *h > 0.0)
            .unwrap_or(self.base.font as f64 * ROW_H_RATIO)
    }

    /// The tallest row — what the fit/flip decision reserves per row, so a mixed-height list is
    /// capped conservatively and no row is ever clipped by the viewport edge.
    fn max_row_h(&self) -> f64 {
        (0..self.count())
            .map(|i| self.row_h(i))
            .fold(0.0_f64, f64::max)
            .max(self.base.font as f64 * ROW_H_RATIO)
    }

    /// Padding / chevron width scaled by the size variant (the font already is), so the control and
    /// its panel grow/shrink as a unit.
    fn pad_h(&self) -> f64 {
        PAD_H * self.base.size_scale() as f64
    }
    fn pad_v(&self) -> f64 {
        PAD_V * self.base.size_scale() as f64
    }
    /// Room reserved on the right of the rows: the trigger's chevron, and the panel's scrollbar
    /// lane. Applied as a margin on the rows, so it widens the control (taffy sizes it to the
    /// widest option *plus* this) and keeps the option content clear of both.
    fn gutter(&self) -> f64 {
        CHEVRON_W * self.base.size_scale() as f64 + SCROLLBAR_W
    }

    /// The open list's rect — where the engine laid it out, moved to its panel place.
    fn panel_rect(&self) -> Rectangle {
        self.list()
            .map_or(Rectangle::from_size(Size::new(0.0, 0.0)), |list| {
                list.bounds
            })
    }

    /// The chosen option's slot **inside the trigger** — where its content is echoed. Its width
    /// stops at the [gutter](Select::gutter), so the content never runs under the chevron.
    fn trigger_slot(&self) -> Rectangle {
        let b = self.base.bounds;
        let h = self.row_h(self.selected.get_untracked());
        Rectangle::new(
            Point::new(b.loc.x, b.loc.y + (b.size.h - h) / 2.0),
            Size::new((b.size.w - self.gutter()).max(0.0), h),
        )
    }

    /// Open the list. Picks a direction (below the trigger, or flipped above when there's no room)
    /// and caps the visible rows to what fits in the viewport; longer lists scroll inside the panel.
    fn open_list(&mut self) {
        if self.count() == 0 {
            return;
        }
        self.open.set(true);
        // **Focus is not set here.** A list opens either from a click — which focused this widget
        // through the host's `FocusManager`, clearing whoever held it — or from a key, which this
        // widget only received because it was focused already. Setting the flag directly instead
        // made a *second* widget claim focus without releasing the first, and `wants_visible`
        // defaults to that flag: every enclosing `ScrollRegion` then kept scrolling to a select
        // that had been opened once and never blurred, so a click anywhere on the page jumped it to
        // the same spot.
        self.highlight = self.selected.get_untracked();
        // Opening shows the chosen row: the list scrolls to it as a keyboard move would.
        self.by_keyboard.set(true);

        // The window as the last layout saw it — the room the list has above and below.
        let vp = match crate::layout::viewport().h {
            h if h.is_finite() => h,
            _ => f64::MAX,
        };
        let b = self.base.bounds;
        let space_below = (vp - (b.loc.y + b.size.h) - 2.0 * PANEL_GAP).max(0.0);
        let space_above = (b.loc.y - 2.0 * PANEL_GAP).max(0.0);
        let row_h = self.max_row_h();
        let rows_in = |space: f64| ((space - 2.0 * PANEL_PAD) / row_h).floor().max(0.0) as usize;
        let want = self.count().min(MAX_VISIBLE);
        let fit_below = rows_in(space_below);
        let fit_above = rows_in(space_above);

        // As many rows as fit on the side with room — the layer then hangs the list there: below
        // when it fits, flipped above when it does not and does there.
        let rows = if fit_below >= want {
            want
        } else if fit_above > fit_below {
            want.min(fit_above).max(1)
        } else {
            want.min(fit_below).max(1)
        };
        if self.vis_rows.get_untracked() != rows {
            self.vis_rows.set(rows);
        }
        self.base.mark_needs_layout();
        self.sync_option_states();
    }

    /// Close the list: it is hidden, keeping its place in the flow (the control's width is its).
    fn close(&mut self) {
        self.open.set(false);
        self.sync_option_states();
        self.base.mark_needs_paint();
    }

    fn commit(&mut self, i: usize) {
        if i < self.count() {
            self.selected.set(i);
            self.sync_option_states();
            if let Some(f) = &self.on_change {
                f(Action::value("select-change", SignalData::Usize(i)));
            }
        }
    }

    /// Move the cursor to row `i`, saying who moved it.
    fn move_cursor(&mut self, i: usize, by_keyboard: bool) {
        if self.by_keyboard.get_untracked() != by_keyboard {
            self.by_keyboard.set(by_keyboard);
        }
        if self.highlight != i {
            self.highlight = i;
            self.sync_option_states();
            // A keyboard move brings the row into view, which the list does on its next layout.
            if by_keyboard {
                self.base.mark_needs_layout();
            }
            self.base.mark_needs_paint();
        }
    }

    /// **The option under the pointer**, asked of the framework: hover is marked along the
    /// target's ancestor chain before an event is delivered, so the row answering `hovered` is the
    /// row the user sees there — never row arithmetic, never a hit-test of this widget's own. A
    /// row scrolled out of the panel is under no point the panel shows.
    fn pointed_row(&self) -> Option<usize> {
        self.rows().iter().position(|row| row.base().hovered())
    }

    /// Whether the pointer is over the open list at all — a row, or the list's own scrollbar.
    fn over_list(&self) -> bool {
        self.list().is_some_and(|list| list.hovered())
    }
}

impl Component for Select {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn overlay_active(&self) -> bool {
        self.is_open() && !self.base.disabled.get_untracked()
    }

    /// An open dropdown occludes its **panel rect** (the option list drawn above
    /// the page) — a host must not synthesize a page-level action (e.g. open a
    /// context menu) on a point the panel covers. Points outside the panel are
    /// not occluded (an outside click is the light-dismiss).
    fn overlay_occludes(&self, pos: Point) -> bool {
        self.overlay_active() && self.panel_rect().contains(pos)
    }

    /// The trigger's height tracks the resolved font. Its **width is the widest option** — the
    /// engine measures the rows (they hug their content) and sizes this node to them, so the
    /// control is snug around real content, icons included, instead of a char-count estimate.
    ///
    /// The rows carry the control's horizontal insets as margins: `left` aligns their content with
    /// the trigger's label (an option's own padding is smaller than the trigger's), `right` is the
    /// gutter for the chevron and the scrollbar. Being margins, they widen the
    /// control *and* keep the option content out of them — which is exactly what a padding on this
    /// node could not do, since it would inset the rows themselves.
    fn remeasure(&mut self) {
        let fs = self.base.font;
        // Tall enough for a line of text, and for the tallest option — the trigger has to be able
        // to stand *any* of them in it (a two-line option, an icon row), and sizing to the tallest
        // rather than to the chosen one keeps the control from resizing as the selection moves. The
        // rows' measured heights come from the previous layout pass, so a first frame with content
        // taller than one line settles on the next one; for text options the two agree exactly.
        let line_h = fs * MONO_LINE_RATIO + 2.0 * self.pad_v() as f32;
        let tallest = self.natural_h.iter().copied().fold(0.0_f64, f64::max) as f32;
        self.base.style.layout.height = Length::Px(line_h.max(tallest));
        self.base.style.layout.width = if self.count() == 0 {
            Length::Px(EMPTY_W * self.base.size_scale())
        } else {
            Length::Auto
        };

        // The row's *box* spans the whole list, so its pill, its hover tint and its hit rect reach
        // the edges; its *content* starts on the trigger label's inset and stops short of the
        // chevron and the scrollbar lane — padding, not margin, for exactly that split.
        let inset = PAD_H as f32 * self.base.size_scale();
        let gutter = self.gutter() as f32 + choice::BASE_PAD * self.base.size_scale();
        let Some(list) = self.list_mut() else {
            return;
        };
        for child in list.base_mut().children.iter_mut() {
            let style = &mut child.base_mut().style.layout;
            style.align_self = Some(Align::Stretch);
            style.padding_left = Some(inset.into());
            style.padding_right = Some(gutter.into());
        }
    }

    /// Layout just laid the list out in the control's flow: re-measure the rows from it.
    fn on_layout(&mut self) {
        self.natural_h = self.rows().iter().map(|c| c.base().bounds.size.h).collect();
        self.sync_option_states();
    }

    /// **The trigger's final place, handed to the list's layer** — after every ancestor has moved
    /// its children (a scrolled page), and before the layer places the list, since a parent is told
    /// before its children.
    fn on_placed(&mut self) {
        let trigger = self.base.bounds;
        if self.trigger.get_untracked() != trigger {
            self.trigger.set(trigger);
        }
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let open = self.is_open();
        let active = open || self.base.focused.get_untracked();
        let (surface, accent, muted, foreground, radius, bw, ia) = {
            let t = cx.theme();
            (
                t.colors.surface,
                cx.accent(),
                t.colors.muted,
                t.colors.foreground,
                t.colors.control_radius(),
                t.colors.border_width,
                t.colors.interaction,
            )
        };
        let b = self.base.bounds;

        // Trigger box: border firms muted → accent when focused/open. Radius +
        // border width come from the theme so global settings scale them. A faint
        // theme rest glow (`interaction.control_rest_glow`) gives the trigger the
        // shared neon identity at rest; `glow_size` scales it.
        let p = if active { 1.0 } else { 0.0 };
        let rest_border = ia.control_rest_border as f32;
        let border_a = rest_border + (255.0 - rest_border) * p;
        let border = Border {
            color: muted.lerp(accent, p).with_alpha(border_a.round() as u8),
            width: bw,
        };
        let glow = if disabled {
            None
        } else {
            cx.rest_glow(GLOW_RADIUS)
        };
        cx.rect(b, surface, Some(border), radius, glow);

        // **One control, one highlight.** The whole trigger lights under the pointer — the chosen
        // option's text and the chevron together — in the same interaction token an `Item` and a
        // `Choice` use, so a control and a list row read as the same family. It used to be the
        // echoed option that hovered, and its pill stops at the chevron gutter, which is why the
        // caret stayed dark while the text lit up.
        if !disabled && !open && self.base.hovered() {
            cx.rect(
                b,
                foreground.with_alpha(ia.row_hover_fill),
                None,
                radius,
                None,
            );
        }

        // What the trigger shows: the **chosen option**, content and all — its icon, its badge,
        // whatever it composes — never just words about it. The option lives in the list (a
        // component is laid out in exactly one place), so the trigger draws a second **image** of
        // its content, translated from the list into the slot. Only the content is echoed, not the
        // row's chrome: the trigger's own box is its chrome, and a selection pill inside it would
        // be a box in a box.
        if let Some(chosen) = self.rows().get(self.selected.get_untracked()) {
            let row = chosen.base().bounds;
            let slot = self.trigger_slot();
            let (dx, dy) = (slot.loc.x - row.loc.x, slot.loc.y - row.loc.y);
            cx.with_translate(dx, dy, |cx| {
                cx.with_content_color(foreground, |cx| {
                    for content in &chosen.base().children {
                        content.paint(cx);
                    }
                });
            });
        }

        // Down-chevron on the right, built from stacked rects (a small triangle).
        let chev_color = muted.lerp(accent, p);
        let cw = 9.0_f64;
        let cx0 = b.loc.x + b.size.w - self.pad_h() - cw;
        let cy0 = b.loc.y + b.size.h / 2.0 - 2.0;
        for k in 0..3 {
            let inset = k as f64 * (cw / 2.0) / 3.0;
            cx.rect(
                Rectangle::new(
                    Point::new(cx0 + inset, cy0 + k as f64 * 2.0),
                    Size::new(cw - 2.0 * inset, 1.5),
                ),
                chev_color,
                None,
                0.0,
                None,
            );
        }

        if disabled {
            cx.dim(b, radius);
        }
        if !disabled && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            let ring = cx.theme().colors.effective_focus_ring();
            cx.focus_ring(b, ring, radius);
        }

        // The open list — its layer draws itself on the overlay layer, frame and rows, while open.
        for child in &self.base.children {
            crate::component::paint_child(child.as_ref(), cx);
        }
    }

    /// The select's **input** surface: the trigger, plus the list it is showing.
    fn hit_bounds(&self) -> Option<Rectangle> {
        let trigger = self.base.bounds;
        if !self.is_open() {
            return Some(trigger);
        }
        let panel = self.panel_rect();
        let x0 = trigger.loc.x.min(panel.loc.x);
        let y0 = trigger.loc.y.min(panel.loc.y);
        let x1 = (trigger.loc.x + trigger.size.w).max(panel.loc.x + panel.size.w);
        let y1 = (trigger.loc.y + trigger.size.h).max(panel.loc.y + panel.size.h);
        Some(Rectangle::new(
            Point::new(x0, y0),
            Size::new(x1 - x0, y1 - y0),
        ))
    }

    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            // The open panel is opaque by construction now: it is what the router hit-tested, so
            // a move that reaches this widget is over the trigger or the list, and the widgets the
            // panel covers are not on the path at all — they cannot light up behind it.
            Event::PointerMove(_) => {
                if self.is_open()
                    && let Some(i) = self.pointed_row()
                {
                    self.move_cursor(i, false);
                }
                Handled::No
            }
            Event::PointerDown(_) => {
                if self.is_open() {
                    // A press on the list but not on a row is the list's own (its scrollbar).
                    let pointed = self.pointed_row();
                    if self.over_list() && pointed.is_none() {
                        return Handled::No;
                    }
                    if let Some(i) = pointed {
                        self.commit(i);
                    }
                    // A press on the option or the trigger closes it; one outside arrives as
                    // `PointerDownOutside` below.
                    self.close();
                    Handled::Yes
                } else {
                    self.open_list();
                    Handled::Yes
                }
            }
            Event::PointerDownOutside(_) if self.is_open() => {
                self.close();
                Handled::No
            }
            // The wheel over the open list scrolls it: the list is a `ScrollRegion` and takes it
            // itself.
            // Open-list navigation: the widget is an overlay while open, so the host sends the
            // configurable `[keys.widgets]` keys as a semantic `WidgetIntent` (shared with the
            // context menu / command palette). A vertical list: `MenuUp`/`MenuDown` (not the
            // horizontal `Item*`). No literal nav keys live here.
            Event::Widget(intent) if self.is_open() => match intent {
                WidgetIntent::MenuUp => {
                    self.move_cursor(self.highlight.saturating_sub(1), true);
                    Handled::Yes
                }
                WidgetIntent::MenuDown => {
                    let last = self.count().saturating_sub(1);
                    self.move_cursor((self.highlight + 1).min(last), true);
                    Handled::Yes
                }
                WidgetIntent::Activate => {
                    self.commit(self.highlight);
                    self.close();
                    Handled::Yes
                }
                WidgetIntent::Dismiss => {
                    self.close();
                    Handled::Yes
                }
                _ => Handled::No,
            },
            // Closed + focused (not yet an overlay): raw activation keys open the list, like a
            // button's Enter/Space. The open list is driven by `WidgetIntent` above, not raw keys.
            Event::Key {
                key: GridKey::Enter | GridKey::Space | GridKey::ArrowDown,
                pressed: true,
            } if !self.is_open() => {
                self.open_list();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn on_blur(&mut self) {
        self.close();
        self.base.blur();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Closed, the panel is not consulted: the widget reports no overlay and the
    /// chosen option stands in the trigger.
    #[test]
    fn closed_select_is_not_an_overlay() {
        let mut s = Select::new(["low", "medium", "high"]);
        s.base.bounds = Rectangle::new(Point::new(40.0, 100.0), Size::new(120.0, 30.0));
        assert!(!s.overlay_active());
        assert!(!s.overlay_occludes(Point::new(45.0, 140.0)));
    }
}
