//! What a [`Base`] answers and does.

use super::*;

impl Base {
    /// A part owned by this container, without crossing into a nested component's template.
    pub(crate) fn child_in_area(&mut self, name: &str) -> Option<&mut crate::WidgetModel> {
        self.children.iter_mut().find(|child| child.base().grid_area.as_deref() == Some(name))
    }

    /// **Does this widget declare anything the pointer could mean on it?** A handler, a focus
    /// stop, a drag or drop role, a cursor, a tooltip, a menu, a hint, or something painted (fill,
    /// border, glow). A box with none of these is layout only: where no child is, nothing is under
    /// the pointer but whatever is behind it.
    pub fn answers_pointer(&self) -> bool {
        let v = &self.style.visual;
        self.handlers.is_some()
            || self.focusable
            || self.one_click_target
            || self.draggable
            || self.drop_target
            || self.cursor.is_some()
            || self.tooltip.is_some()
            || self.context_menu.is_some()
            || self.hint.is_some()
            || v.fill.is_some()
            || v.border.is_some()
            || v.glow.is_some()
    }

    /// **The name this widget declares itself by** — its [`key`](Self::key), or its
    /// [`scope_key`](Self::scope_key) when it names a region rather than a row.
    ///
    /// The two fields stay separate for the reasons given on each: one says *which row*, the other
    /// *which region containing rows*, and only the first is a steppable cursor stop. But both are
    /// a name the widget chose about itself, so **everything that names a widget asks here** and
    /// they cannot disagree.
    ///
    /// This exists because they did disagree. [`nav::identity_of`](crate::nav::identity_of) read
    /// only `key`, so a dock — which declares itself with `scope_key` — contributed nothing to its
    /// children's names. Two docks holding the same rows produced two sets of identical identities,
    /// and anything keyed on identity silently addressed the wrong one: a remembered hint letter
    /// bounced between the two copies on every opening, and each dock's own name fell back to its
    /// decorative drag grip and shifted whenever a row was added.
    pub fn identity(&self) -> Option<&str> {
        self.key.as_deref().or(self.scope_key.as_deref())
    }

    /// **Does this widget answer to `name`?** Either declaration counts, so a container can be
    /// addressed by the region name it published without also inventing a row key.
    ///
    /// The lookup twin of [`identity`](Self::identity): that one asks what a widget is *called*,
    /// this one asks whether a given name reaches it. Both live here so no caller writes the
    /// `key`-or-`scope_key` test by hand and drifts from the other.
    pub fn answers_to(&self, name: &str) -> bool {
        self.key.as_deref() == Some(name) || self.scope_key.as_deref() == Some(name)
    }

    /// A base for a widget whose whole job is **arranging children** (a row, a grid, a wrapper):
    /// where none of its children is, the pointer goes to whatever is behind it, unless the widget
    /// declares something or paints (see [`answers_pointer`](Self::answers_pointer)). Every layout
    /// container starts here, so no author of one writes anything for it.
    pub fn container() -> Self {
        let mut base = Self::new();
        base.layout_only = true;
        base
    }

    /// A layer shares its containing viewport instead of taking space from sibling layers.
    /// Composed surfaces need the same outer box as the overlay they contain.
    pub(crate) fn viewport_layer() -> Self {
        let mut base = Self::new();
        base.style.layout.width = crate::Length::Percent(1.0);
        base.style.layout.height = crate::Length::Percent(1.0);
        base.style.layout.placement = Some(crate::style::Placement {
            left: crate::Length::Px(0.0), top: crate::Length::Px(0.0),
            width: crate::Length::Auto, height: crate::Length::Auto,
        });
        base
    }

    /// A new base with default style and an empty child list.
    pub fn new() -> Self {
        Self {
            style: Style::default(),
            flash: None,
            grid_area: None,
            node: None,
            layout_tree: Cell::new(0),
            layout_cache: RefCell::new(None),
            bounds: Rectangle::from_size(Size::new(0.0, 0.0)),
            visible: signal(true),
            disabled: signal(false),
            focused: signal(false),
            focus_door: crate::focus::FocusDoor::default(),
            focus_visible: signal(false),
            focusable: false,
            focus_barrier: false,
            one_click_target: false,
            pointer_transparent: false,
            tab_index: None,
            children: Vec::new(),
            draggable: false,
            drag_gate: None,
            drag_kind: None,
            drag_chip: false,
            drop_target: false,
            accepts: Vec::new(),
            accepts_onto: true,
            key: None,
            scope_key: None,
            keeps_keyboard: false,
            keyboard_to: None,
            font: 15.0,
            root_font: 15.0,
            input_style: heca_theme::InputStyle::default(),
            dialog_style: heca_theme::DialogStyle::default(),
            viewport: Size::new(f64::MAX, f64::MAX),
            layout_only: false,
            pointer: crate::pointer::PointerState::new(),
            handlers: None,
            props: None,
            clip_children: false,
            built_from: None,
            cursor: None,
            context_menu: None,
            surface: false,
            surface_slot: None,
            presence: crate::animation::Presence::new(),
            open: crate::reactive::signal(false),
            lock: false,
            captures_keyboard: None,
            hint: None,
            hint_scopes: Vec::new(),
            picker_scope: None,
            tooltip: None,
            content_tone: None,
            hintable: true,
            activatable: false,
            transparent: false,
            hint_label: crate::reactive::signal(None),
            hint_style: crate::widgets::HintStyle::default(),
            actions: Vec::new(),
            mounted: Cell::new(false),
            needs_paint: Cell::new(true),
            // A fresh widget is laid out by the pass that mounts it; it has nothing to re-request.
            needs_layout: Cell::new(false),
        }
    }

    /// Whether the pointer is over this widget **or a descendant** — the CSS `:hover` rule.
    ///
    /// Read this instead of testing `bounds.contains(pos)`: the router already resolved which
    /// widget the pointer is over, including which one is on top and what is clipped away, and a
    /// private copy of the test cannot know either.
    pub fn hovered(&self) -> bool {
        self.pointer.is_hovered()
    }

    /// Mark this widget as needing a repaint and ask the host for a frame. Call on
    /// a visual change the host wouldn't otherwise know about — an animation step,
    /// a caret move, an imperative state edit. (Reactive state changes route here
    /// too, at their mutation site.)
    pub fn mark_needs_paint(&self) {
        self.dirty_next_paint();
        request_frame();
    }

    /// Retain damage for a frame already scheduled through `next_redraw`.
    pub(crate) fn dirty_next_paint(&self) {
        self.needs_paint.set(true);
    }

    /// Whether this widget needs repainting.
    pub fn needs_paint(&self) -> bool {
        self.needs_paint.get()
    }

    /// Clear the repaint flag — the renderer calls this once the widget is painted.
    pub fn clear_needs_paint(&self) {
        self.needs_paint.set(false);
    }

    /// **Show or hide this widget — the one way to do it.**
    ///
    /// `hidden` is the engine's `display: none`: the widget leaves the layout entirely, and
    /// everything after it moves. So flipping it is a **layout** change, not a paint one, and this
    /// asks for the pass that re-places the siblings — which is why nothing should ever write
    /// `style.layout.hidden` directly. Writing it by hand is how a dock's rows ended up painted on
    /// top of each other and a sidebar row stayed collapsed with its content already arrived: the
    /// value was right and nobody had moved anything.
    ///
    /// **It only asks when the value actually changes.** That is what makes it safe to call from
    /// `remeasure`, which runs *inside* the layout pass: re-applying the same state there marks
    /// nothing, so a widget that syncs itself every pass cannot request one every pass.
    pub fn set_hidden(&mut self, hidden: bool) {
        if self.style.layout.hidden != hidden {
            self.style.layout.hidden = hidden;
            // **Coming back, it has no box yet.** A hidden widget is `display: none`, so the layout
            // gives it a zero rect at the origin; revealing it does not undo that, and the bounds it
            // still carries say it is a nothing in the window's top-left corner. Most of a widget
            // draws from those bounds and so draws nothing — but an icon is a glyph at the font's
            // size, drawn wherever its box says it is, so a revealed button put its icon in the
            // corner of the screen for the frame before the next layout reached it. During a drag
            // that is continuous, and it is red when the button is a destructive one.
            //
            // Clearing the layout node says the truth — nothing has placed this yet — and
            // [`paint_child`](crate::paint_child) already declines to draw what has never been
            // placed. The layout this same call asks for supplies the box, and it draws from then
            // on. One rule, rather than every widget learning to distrust its own bounds.
            if !hidden {
                self.node = None;
            }
            self.mark_needs_layout();
        }
    }

    /// Whether this widget is out of the layout entirely (`display: none`).
    pub fn is_hidden(&self) -> bool {
        self.style.layout.hidden
    }

    /// **Say that this widget's tree changed and must be laid out again.**
    ///
    /// Call it when you add or remove children outside the layout pass — reconciling a host-owned
    /// list, revealing a subtree, growing a row. A repaint is not enough: the widget's siblings are
    /// still laid out around the shape the tree used to have.
    ///
    /// It also requests a frame, so a host that is otherwise idle wakes up to run the pass.
    ///
    /// **Why this exists.** A stacked notification removes its card only once the card's exit has
    /// played — several frames after the click that dismissed it. Nothing re-laid-out at that
    /// moment, so the cards below it kept their old positions until some unrelated click happened
    /// to trigger a layout, and the gap where the card had been simply sat there
    pub fn mark_needs_layout(&self) {
        self.needs_layout.set(true);
        request_frame();
    }

    /// Whether this widget's tree changed and wants a layout pass.
    pub fn needs_layout(&self) -> bool {
        self.needs_layout.get()
    }

    /// Clear the relayout flag — [`needs_layout`] does this as it walks.
    pub fn clear_needs_layout(&self) {
        self.needs_layout.set(false);
    }

    /// The widget's size-variant **padding/dimension** multiplier — what widgets
    /// multiply their intrinsic padding / fixed dims by in `remeasure`. (The font is
    /// scaled separately, by [`WidgetSize::font_scale`](crate::style::WidgetSize::font_scale),
    /// during layout.) At `Small` this is tighter than the font so controls get compact, not just
    /// smaller. See
    /// [`WidgetSize`](crate::style::WidgetSize).
    pub fn size_scale(&self) -> f32 {
        self.style.layout.size.pad_scale()
    }

    /// Whether the keyboard **focus ring** should draw: focused AND the focus is
    /// keyboard-driven ([`focus_visible`](Self::focus_visible)) — CSS
    /// `:focus-visible` semantics. A mouse click focuses a widget (so Enter/Space
    /// work, the caret shows, …) but sets `focus_visible = false`, so pointer
    /// users are not ringed; Tab/arrow navigation sets it `true` and the ring
    /// appears. Every widget gates its ring paint on this single definition
    /// (plus the theme's `show_focus_border` and its own disabled check).
    pub fn shows_focus_ring(&self) -> bool {
        self.focused_by_keyboard()
    }

    /// **Did this widget's focus arrive from the keyboard?** — `focused && focus_visible`, CSS
    /// `:focus-visible` semantics, and the one definition of that question.
    ///
    /// Two things read it, for the same reason, and neither should re-derive it:
    ///
    /// - [`shows_focus_ring`](Self::shows_focus_ring) — a click focuses without ringing.
    /// - [`wants_visible`](Component::wants_visible) — **pointing at something never scrolls it.**
    ///   A reveal exists to bring into view what the user cannot see, which is the keyboard's
    ///   case; what the mouse is on is visible by definition, and scrolling it moves it out from
    ///   under the pointer that asked. Clicking a row inside a scrolled region used to focus it,
    ///   which asked for a reveal, which centred it — so the first click only scrolled and the
    ///   second one did what you meant.
    pub fn focused_by_keyboard(&self) -> bool {
        self.focused.get_untracked() && self.focus_visible.get_untracked()
    }

    /// The rect an **inset state highlight** should cover — a hover tint, an active pill:
    /// [`bounds`](Self::bounds) pulled in by `inset` on each side, but **never further than this
    /// widget's own padding** on that side.
    ///
    /// The inset exists so a pill's rounded corners never contend with a rounded container's, and
    /// the space padding already reserves is enough for that. Past the padding it would be cutting
    /// into the content box, which is a highlight crossing the very thing it highlights: a row with
    /// no padding used to draw a pill *shorter than its own content*, leaving a badge sticking out
    /// above and below it.
    ///
    /// Every widget that draws an inset highlight asks here, so the rule holds for the next one
    /// too.
    /// **Advance this widget's presence, and finish the exit when it has played out.**
    ///
    /// An exit holds the surface on screen while it plays: [`Component::close`] leaves `visible`
    /// **true** so the frames can be drawn, and something has to take it down when the gesture
    /// ends. Nothing did. A surface with an exit animation therefore stayed `visible` forever after
    /// its first close — inert and invisible to the eye, but still up as far as every walk over the
    /// tree was concerned.
    ///
    /// What that cost: the exposé's cards kept being offered letters by `prefix+/` after it had
    /// been opened once and closed — a full-window target sitting over everything you could
    /// actually see.
    ///
    /// It lives beside `close` because it is the other half of that rule, and splitting them is
    /// how they came to disagree: two widgets had hand-written `presence.tick` and neither ended
    /// the exit, which is a rule in two call sites and therefore in the wrong place.
    pub fn tick_presence(&mut self, dt: f32) -> bool {
        // A widget that follows a host signal for focus catches up here, once a frame, for every
        // widget — see [`Base::follow_focus`].
        self.sync_focus_follow();
        let tone_changed = self.content_tone.is_some_and(|tone| {
            let next = tone.get_untracked();
            if self.style.visual.content_tone == next { return false; }
            self.style.visual.content_tone = next;
            self.mark_needs_paint();
            true
        });
        let previous_frame = self.presence.frame();
        let animating = self.presence.tick(dt);
        // **Exactly the frame an exit ends.** Asked of the presence rather than inferred from
        // "not open and not leaving", which is also true of a widget that was never up — and which
        // hid the whole tree on the first tick when it was tried that way.
        let finished_exit = self.presence.take_finished_exit();
        if previous_frame != self.presence.frame() || finished_exit {
            // A moving surface must invalidate itself even when another widget already
            // supplied a smaller damage region. Include the final frame that removes it.
            self.mark_needs_paint();
        }
        if finished_exit {
            crate::reactive::SignalUpdate::set(&self.visible, false);
        }
        animating || tone_changed
    }

    pub fn highlight_rect(&self, inset: f64) -> Rectangle {
        let b = self.bounds;
        let l = &self.style.layout;
        let font = self.font;
        let (left, right) = (
            inset.min(l.pad_left(font) as f64),
            inset.min(l.pad_right(font) as f64),
        );
        let (top, bottom) = (
            inset.min(l.pad_top(font) as f64),
            inset.min(l.pad_bottom(font) as f64),
        );
        Rectangle::new(
            Point::new(b.loc.x + left, b.loc.y + top),
            Size::new(
                (b.size.w - left - right).max(0.0),
                (b.size.h - top - bottom).max(0.0),
            ),
        )
    }
}

impl Default for Base {
    fn default() -> Self {
        Self::new()
    }
}

/// [`Event::Unmount`] fires from here, because this is the only moment the library can be sure a
/// widget is leaving: trees are values, and a rebuilt one drops the old one wholesale.
///
/// Only [`Base`] handlers see it — the widget around this base is already coming apart, so there
/// is nothing left to call `on_event` on. That is enough for what unmount is for: releasing what
/// the widget registered with the host (a drag id, a hint slot) at the moment the tree that
/// registered it goes away, instead of a range the host has to remember to prune.
impl Drop for Base {
    fn drop(&mut self) {
        if let Some(h) = self.handlers.as_mut()
            && h.has(EventKind::Unmount)
        {
            let _ = h.run(&Event::Unmount);
        }
    }
}
