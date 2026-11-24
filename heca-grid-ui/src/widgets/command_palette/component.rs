//! **What the palette does with input** — the keyboard's verbs (move, choose, complete, recall,
//! dismiss) and what a pointer did to a row. Nothing here draws, measures or hit-tests: the
//! composed view (`view`) does all of that, and each row reports its own pointer.

use super::view::Pick;
use super::*;

impl Component for CommandPalette {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    // **What a host asks of a layer in front**, answered from whether it is open — the overlay
    // inside does the covering, the scrim and the keyboard; these only report it.
    fn overlay_active(&self) -> bool {
        self.is_open()
    }
    /// On screen while open, **and while its panel is still dissolving after it closed** — closed
    /// alone, the walk would stop drawing it and the panel's exit would never be seen.
    fn is_shown(&self) -> bool {
        self.base.visible.get_untracked() && (self.is_open() || self.panel_shown())
    }
    /// **Its arrival and exit are its panel's** — the palette itself has no animation, so a host
    /// asking whether it is still leaving (to keep its layer drawn) is answered by the panel that is.
    ///
    /// Before its panel is built (it is, the first time it is needed), the palette's own.
    fn presence(&self) -> Option<&crate::animation::Presence> {
        self.base
            .children
            .iter()
            .find_map(|panel| panel.presence())
            .or(Some(&self.base.presence))
    }

    fn presence_mut(&mut self) -> Option<&mut crate::animation::Presence> {
        if self.base.children.is_empty() {
            return Some(&mut self.base.presence);
        }
        self.base
            .children
            .iter_mut()
            .find_map(|panel| panel.presence_mut())
    }

    /// **Closing starts the panel's exit, now** — a host closes a surface and asks at once whether
    /// it is still leaving; the panel must already be on its way out by then, or the layer is
    /// dropped mid-dissolve and the palette vanishes with a cut.
    fn close(&mut self) {
        self.open.set(false);
        // Its own state too: before the panel is built, that is the palette's arrival and exit.
        self.base.presence.leave();
        for panel in self.base.children.iter_mut() {
            panel.close();
        }
    }

    fn overlay_occludes(&self, _pos: heca_core::layout::Point) -> bool {
        self.is_open()
    }

    fn remeasure(&mut self) {
        self.sync_query();
        self.sync_view();
    }

    fn tick(&mut self, dt: f32) -> bool {
        // **Closing resets it, whichever way it closed** — its dismiss key, a choice, a click
        // outside, a host writing the signal: one edge, so no path forgets the reset.
        //
        // The panel ticks first, so a dismissal written since the last frame has already started
        // its exit by the time anything here asks whether it is on screen.
        // The panel put in step first (built, the first time), as layout, events and focus do.
        crate::builders::apply_bound(&mut self.base);
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        let open = self.is_open();
        if self.was_open && !open {
            self.reset();
        }
        self.was_open = open;
        if open {
            self.sync_query();
            self.follow_pointed();
            self.take_pick();
        }
        animating
    }

    fn on_event(&mut self, ev: &Event) -> Handled {
        // A click on a row bubbles here after the row recorded it: act on it now, not a frame late.
        if self.is_open() && matches!(ev, Event::Click(_)) && self.take_pick() {
            return Handled::Yes;
        }
        Handled::No
    }

    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        self.sync_query();
        match ev {
            Event::Widget(WidgetIntent::Dismiss) => {
                self.dismiss();
                Handled::Yes
            }
            Event::Widget(WidgetIntent::Activate) => {
                self.run_selected();
                Handled::Yes
            }
            Event::Widget(WidgetIntent::MenuDown) => {
                self.select_next();
                Handled::Yes
            }
            Event::Widget(WidgetIntent::MenuUp) => {
                self.select_prev();
                Handled::Yes
            }
            Event::Widget(WidgetIntent::Complete) => self.complete_selected(),
            // **Tab completes before it traverses.** The keymap offers a plain key raw first, and
            // the overlay the palette sits in takes a raw Tab to move focus — so `complete = "Tab"`
            // would never reach the palette. A selected row with a completion takes it here;
            // anything else passes on to the overlay's traversal, as the config promises
            // ("complete a selected suggestion; otherwise normal focus traversal").
            Event::Key {
                key: crate::component::GridKey::Tab,
                pressed: true,
            } if !crate::event::modifiers().shift => self.complete_selected(),
            Event::Widget(
                intent @ (WidgetIntent::MenuHistoryUp | WidgetIntent::MenuHistoryDown),
            ) => {
                let (sigil, scope, query) = self.active();
                let recalled = self.search.with_scope(scope).handle(*intent, &query);
                if let SearchAction::SetQuery(text) = recalled
                    && text != query
                {
                    self.query.set(Self::with_sigil(sigil, &text));
                    self.on_query_replaced();
                    self.base.mark_needs_layout();
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }
}

impl CommandPalette {
    /// **A row the pointer moved onto is the selected one** — the list selected it by position.
    fn follow_pointed(&mut self) {
        if let Some(at) = self.list_selected.get_untracked()
            && at != self.selected
        {
            self.selected = at;
            self.sync_view();
        }
    }

    /// **Run the row the pointer chose** since the last look. `true` when there was something to act on.
    fn take_pick(&mut self) -> bool {
        let Some(pick) = self.picks.take() else {
            return false;
        };
        let Pick::Choose(index) = pick;
        if let Some(at) = self.results().iter().position(|r| r.index == index) {
            self.selected = at;
            self.sync_view();
            self.run_selected();
        }
        true
    }
}
