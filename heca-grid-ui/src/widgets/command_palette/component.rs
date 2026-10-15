//! Component lifecycle and input handling.

use super::*;

impl Component for CommandPalette {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn focusable(&self) -> bool {
        self.is_open()
    }

    fn overlay_active(&self) -> bool {
        self.is_open()
    }

    /// An open palette grabs input for the whole viewport (typing, nav, outside
    /// click = dismiss), so every point is occluded while open — a host must not
    /// synthesize a page-level action (e.g. open a context menu) under it.
    fn overlay_occludes(&self, _pos: Point) -> bool {
        self.is_open()
    }

    fn paint(&self, cx: &mut PaintCx) {
        self.paint_palette(cx);
    }

    /// **A layer is not scrolled into view.** `Base::focused` says this widget holds the keyboard
    /// while it is open, and `wants_visible` defaults to exactly that — so an enclosing
    /// `ScrollRegion` would scroll the page to wherever this widget's layout node happens to sit,
    /// every frame it is open. A layer draws over the page; the page does not come to it.
    fn wants_visible(&self) -> bool {
        false
    }

    /// Lay the row text out at **the width it will be drawn at**, in a column.
    ///
    /// This is what makes a description wrap correctly: the engine measures each text child against
    /// this width, so the line count it reports is the line count that will be painted. The viewport
    /// is the one cached at the last paint, so the very first frame falls back to the size variant's
    /// nominal width and settles on the next — the same one-frame settle `Select` documents for its
    /// measured rows.
    fn remeasure(&mut self) {
        // Reflow the selected row **before** the children are measured. `build` calls this on a node
        // and only then descends into its children, so a wrap flipped here is the wrap they measure
        // with — flipping it after layout would show the previous selection's shape for a frame.
        self.sync_wrap();
        // The palette **fills the viewport**, like every other layer root, so `on_layout` can read
        // its own size and learn the viewport from the layout pass rather than waiting for a paint.
        self.base.style.layout.direction = Direction::Column;
        self.base.style.layout.width = Length::Percent(1.0);
        self.base.style.layout.height = Length::Percent(1.0);
        // The text children carry the width instead: the engine measures each against the width it
        // will be drawn at, so the line count it reports is the line count that gets painted.
        let w = self.text_w() as f32;
        for child in self.base.children.iter_mut() {
            child.base_mut().style.layout.width = Length::Px(w);
        }
    }

    /// Layout just reset every row to its natural position and size: re-read the measured heights
    /// from it, then place the rows into the panel again.
    fn on_layout(&mut self) {
        // **The viewport the engine was told to compute against**, which it records on every node
        // (`Base::viewport`) — known one whole frame earlier than the paint that used to be the
        // only source. Without it the first frame after the palette opens places its rows against a
        // fallback panel and the text lands off the panel.
        //
        // It used to read its OWN bounds, on the assumption "the root fills the viewport". That
        // holds when the palette is mounted as a full-size layer, as the app mounts it, and not
        // when a host puts it in a column of overlays, as the showcase does — there its bounds are
        // whatever the flex gave it, the panel is computed against the wrong box, and every row's
        // text lands 76px above the row it belongs to while the icons and keycaps (painted from the
        // panel, not placed) stay put. A widget must not infer the viewport from where it happens
        // to be mounted; the framework already records it.
        let size = self.base.viewport;
        if size.w > 0.0 && size.h > 0.0 && size.w.is_finite() && size.h.is_finite() {
            self.viewport.set(size);
        }
        self.natural_h = self
            .base
            .children
            .iter()
            .map(|c| c.base().bounds.size.h)
            .collect();
        self.place_rows();
    }

    /// The palette paints its panel on the overlay layer, not at its layout `bounds`.
    fn damage_bounds(&self) -> Rectangle {
        if self.is_open() {
            self.panel.get()
        } else {
            // Closed, it damages like any other widget — its own box plus its bubble, which draws
            // on the overlay layer outside that box.
            crate::widgets::tooltip::damage(&self.base)
        }
    }

    /// Its **input** surface is the panel while open, and nothing while closed.
    fn hit_bounds(&self) -> Option<Rectangle> {
        if self.is_open() {
            Some(self.panel.get())
        } else {
            None
        }
    }

    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        match ev {
            // Nav is host-resolved from the configurable `[keys.widgets]` bindings and
            // arrives as a semantic `WidgetIntent` — the palette carries NO hardcoded nav
            // keys. Handled before the query field so a nav key never types. A vertical
            // list: `MenuUp`/`MenuDown` (not the horizontal `Item*`).
            Event::Widget(intent) => match intent {
                WidgetIntent::Dismiss => {
                    self.close();
                    Handled::Yes
                }
                WidgetIntent::Activate => {
                    self.run_selected();
                    Handled::Yes
                }
                WidgetIntent::MenuDown => {
                    self.select_next();
                    Handled::Yes
                }
                WidgetIntent::MenuUp => {
                    self.select_prev();
                    Handled::Yes
                }
                // **Handled before the `_` arm below**, which forwards anything unlisted to the
                // query field — an `Input` would ignore these and the key would fall through.
                // The walk itself is the model's; this only applies the answer.
                // Walks the **active mode's** history: `Ctrl+p` under `@` offers panes searched
                // for, never command queries. The stored entries carry no sigil, so the one
                // currently typed is put back in front before the field is set — otherwise a
                // recall would drop the mode the user is standing in.
                WidgetIntent::MenuHistoryUp | WidgetIntent::MenuHistoryDown => {
                    let (sigil, scope, query) = self.active();
                    let recalled = self.search.with_scope(scope).handle(*intent, &query);
                    if let SearchAction::SetQuery(text) = recalled
                        && text != query
                    {
                        self.query
                            .borrow_mut()
                            .set_value(Self::with_sigil(sigil, &text));
                        self.on_query_replaced();
                    }
                    Handled::Yes
                }
                // **Everything else goes to the query field** — `Edit*` above all
                // (`edit_select_all`, `edit_delete_back`, `edit_delete_to_line_start`), which
                // `Input` already implements. Forwarded rather than listed, so an intent added to
                // the vocabulary later reaches the field without a change here; the palette owns
                // only the four it answers above. This is the same **field-first** rule
                // [`Dialog`](crate::widgets::Dialog) applies through its `FocusManager` — the palette's query
                // lives off-tree, so it forwards by hand.
                _ => {
                    let before = self.query_text();
                    let handled = crate::component::deliver(&mut *self.query.borrow_mut(), ev);
                    if self.query_text() != before {
                        self.on_query_changed();
                    }
                    handled
                }
            },
            // Typed text goes to the query field, exactly as a key does — and for the same reason:
            // the palette is a surface around a field, not a text widget of its own.
            Event::TextInput(_) => {
                let before = self.query_text();
                let handled = crate::component::deliver(&mut *self.query.borrow_mut(), ev);
                if self.query_text() != before {
                    self.on_query_changed();
                }
                handled
            }
            Event::Key { pressed: true, .. } => {
                // The query field owns editing keys (typing, selection, char/word/line delete,
                // caret moves). Return **what the field did**: a single-line `Input` ignores
                // ArrowUp/Down/Enter (returns `No`), so those fall through to the host, which
                // resolves them to a `WidgetIntent` (MenuUp/MenuDown/Activate). Modal capture is
                // the host's job — do NOT hardcode `Handled::Yes` here.
                let before = self.query_text();
                let handled = crate::component::deliver(&mut *self.query.borrow_mut(), ev);
                if self.query_text() != before {
                    self.on_query_changed();
                }
                handled
            }
            // Rows drawn from data, hit-tested inside a panel the router already put the pointer
            // over (its bounds are the panel) — a move anywhere else never reaches this widget.
            Event::PointerMove(p) => {
                // Hover-select a row.
                let results = self.results();
                let (panel, _q, list_top) = self.layout(&results);
                let mut moved = false;
                for (ri, row) in self.row_rects(&results, panel, list_top) {
                    if row.contains(p.pos) {
                        moved = self.selected != ri;
                        self.selected = ri;
                        break;
                    }
                }
                if moved {
                    self.sync_wrap();
                }
                Handled::Yes
            }
            Event::PointerDown(p) => {
                let results = self.results();
                let (panel, query_rect, list_top) = self.layout(&results);
                // A click on the query line places the caret / selects (the Input
                // needs its current bounds + font to hit-test the char position).
                // Delivered by hand, to a field the palette holds off-tree and places itself —
                // the one case where a resolved event is handed to a widget the router could not
                // have found, and the rect above is the guard that makes it honest.
                if query_rect.contains(p.pos) {
                    let font = self.base.font;
                    let mut q = self.query.borrow_mut();
                    q.base_mut().bounds = query_rect;
                    q.base_mut().font = font;
                    crate::component::deliver(&mut *q, ev);
                    return Handled::Yes;
                }
                for (ri, row) in self.row_rects(&results, panel, list_top) {
                    if row.contains(p.pos) {
                        self.selected = ri;
                        self.run_selected();
                        break;
                    }
                }
                Handled::Yes
            }
            // The press that landed somewhere else closes the palette — no panel geometry of its
            // own to keep in step with the placement.
            Event::PointerDownOutside(_) => {
                self.close();
                Handled::No
            }
            // Swallow all other input while open.
            _ => Handled::Yes,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let open = self.is_open();
        // Advance the query's caret (it marks *its own* base on a blink flip, but it
        // lives off-tree so that mark is unobserved). Promote a flip to a palette
        // repaint of just the panel — no full-frame, no pegging frames every tick.
        let flipped = {
            let mut q = self.query.borrow_mut();
            q.base().sync_focus_follow();
            q.tick(dt);
            let f = q.base().needs_paint();
            q.base().clear_needs_paint();
            f
        };
        if open && flipped {
            self.base.mark_needs_paint(); // collect_damage reads the bounds, which ARE the panel
        }
        false
    }

    /// While open, wake the host for the query caret's next blink (instead of
    /// redrawing every frame). Closed: nothing pending.
    fn next_redraw(&self) -> Option<f32> {
        // Its own tooltip wakes it whether the panel is open or shut — the trigger can carry one.
        let tip = crate::widgets::tooltip::wake(&self.base);
        if self.is_open() {
            crate::component::soonest_redraw(tip, self.query.borrow().next_redraw())
        } else {
            tip
        }
    }
}
