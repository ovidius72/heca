//! **What every widget can do**: the [`Component`] trait.

use super::*;

/// Behavior shared by all components. Implementors provide access to their
/// [`Base`]; `paint`/`event` have sensible container defaults.
pub trait Component {
    /// Borrow this component's base.
    fn base(&self) -> &Base;
    /// Mutably borrow this component's base.
    fn base_mut(&mut self) -> &mut Base;

    /// **Hand this widget new props**: the facts its owner holds, for it to show without being
    /// rebuilt. `true` when it took them; `false` when it declares no
    /// [`on_props`](crate::builders::IdentityExt::on_props) or declares one for another type.
    fn set_props(&mut self, props: &dyn std::any::Any) -> bool {
        let base = self.base_mut();
        // The slot is lifted out while it runs: its handler is given the base it lives in.
        let Some(mut slot) = base.props.take() else {
            return false;
        };
        let took = slot.take(props, base);
        base.props.get_or_insert(slot);
        took
    }

    /// Whether this component participates in keyboard focus traversal
    /// (Tab/Shift+Tab). The default reads the declared [`Base::focusable`] flag and
    /// excludes disabled widgets — so an interactive widget just sets
    /// `base.focusable = true` (in its constructor, or when a callback is wired)
    /// rather than re-implementing this. Override only for genuinely dynamic
    /// focusability (e.g. an overlay focusable only while open).
    fn focusable(&self) -> bool {
        self.base().focusable && !self.base().disabled.get_untracked()
    }

    /// A single-letter **accelerator** for this component, if any (e.g. a confirm button's
    /// `y` / `n`). The widget renders it (`Label (x)`); a host that owns the keypress activates
    /// the matching component — a confirm [`Dialog`](crate::widgets::Dialog) fires the button
    /// whose `shortcut()` matches a typed letter, but only when it is a confirm (no text field to
    /// steal the key). Default `None`.
    fn shortcut(&self) -> Option<char> {
        None
    }

    /// **This surface's arrival and exit, if it has one** — the seam a host drives.
    ///
    /// A widget that can appear and disappear embeds a [`Presence`](crate::animation::Presence)
    /// and returns it here; everything else keeps the default `None` and is a *cut*, which costs
    /// nothing and needs no special case anywhere.
    ///
    /// It exists beside [`base`](Self::base) because a host that **mounts** surfaces — the layer
    /// stack, a plugin panel host — has to do three things to a surface without knowing which
    /// widget it is: keep it mounted while its exit is still playing
    /// ([`Presence::is_leaving`](crate::animation::Presence::is_leaving)), paint it as its
    /// animation says ([`Presence::frame`](crate::animation::Presence::frame)), and — when the
    /// surface is **rebuilt** with fresh content — carry the gesture across to the new tree by
    /// swapping this value, so an arrival already played does not play again.
    ///
    /// It is deliberately **not** recursive: a surface is the root of what was mounted, not
    /// something to be hunted for in a subtree.
    ///
    /// **Every component answers**, because it lives on [`Base`](Base::presence) — so nothing
    /// forwards this, and a widget that composes another is not a special case.
    fn presence(&self) -> Option<&crate::animation::Presence> {
        Some(&self.base().presence)
    }

    /// The mutable half of [`presence`](Self::presence) — see there. Both, for the same reason
    /// [`base`](Self::base) and [`base_mut`](Self::base_mut) are both there.
    fn presence_mut(&mut self) -> Option<&mut crate::animation::Presence> {
        Some(&mut self.base_mut().presence)
    }

    /// **Put this surface on screen.**
    ///
    /// Safe to call at any time: a surface already up is not re-arrived, and one already on its way
    /// out is not resurrected. Both rules live in [`Presence`](crate::animation::Presence), so no
    /// caller repeats them. With no animation declared it is simply up.
    ///
    /// **Works on any component**, because the gesture lives on [`Base`](Base::presence): a
    /// `Select`, a `Button`, a pane and a dock are shown the same way a dialog is. A widget with
    /// something extra to do on the way up — an `Overlay` places the keyboard where the surface
    /// said — overrides this and does that as well, never instead.
    fn show(&mut self) {
        let base = self.base_mut();
        // Still going away: refuse, rather than resurrect something mid-exit. The rule lives in
        // `Presence`, so no caller repeats it.
        if base.presence.is_leaving() {
            return;
        }
        base.presence.enter();
        crate::reactive::SignalUpdate::set(&base.open, true);
        crate::reactive::SignalUpdate::set(&base.visible, true);
    }

    /// **Put this surface's keyboard on the control named `key`**, if it holds one.
    ///
    /// Default: nothing. [`Overlay`](crate::widgets::Overlay) implements it, so anything composed
    /// from one — a `Dialog`, a plugin's described surface — places the keyboard by the same rule
    /// instead of each carrying its own. A trait method rather than a downcast, exactly as
    /// [`show`](Component::show) already is.
    fn set_default_focus(&mut self, _key: &str) {}

    /// **Move this surface's keyboard to the next or previous control.**
    ///
    /// Default: nothing. [`Overlay`](crate::widgets::Overlay) implements it, and anything composed
    /// from one calls it rather than keeping a focus manager of its own — two managers over one
    /// panel is two positions that drift, so Tab and the arrow keys end up disagreeing about where
    /// the keyboard is.
    fn advance_focus(&mut self, _forward: bool) {}

    /// **Follow this signal for whether the surface is up.** Default: nothing.
    ///
    /// [`Overlay`](crate::widgets::Overlay) implements it, so anything composed from one hands the
    /// caller's own state down to the widget that actually holds it, rather than keeping a second
    /// copy that has to be kept in step.
    fn follow_open(&mut self, _open: crate::reactive::Signal<bool>) {}

    /// **Put the keyboard on this surface's first control, without showing a ring.**
    ///
    /// What a surface does as it opens so Enter works immediately, while the ring still waits for
    /// the user to actually navigate. Default: nothing.
    fn focus_first_quiet(&mut self) {}

    /// **Dismiss this surface.**
    ///
    /// With an animation declared this *begins* the exit — the surface is gone when
    /// [`Presence::is_leaving`](crate::animation::Presence::is_leaving) says the gesture has played
    /// out, which is what lets a host keep painting it while it goes. With none, it is simply gone.
    ///
    /// **Works on any component**, for the reason [`show`](Self::show) does.
    ///
    /// A component with a declared animation stays on screen, inert, until its exit has played
    /// out; one with none is gone now. The two are the same call — which is the point, because a
    /// caller must never have to know which kind it is holding.
    fn close(&mut self) {
        let base = self.base_mut();
        base.presence.leave();
        crate::reactive::SignalUpdate::set(&base.open, false);
        if !base.presence.is_leaving() {
            crate::reactive::SignalUpdate::set(&base.visible, false);
        }
    }

    /// Open it if it is closed, dismiss it if it is open. The default reads
    /// [`presence`](Self::presence), so a surface gets it for free.
    fn toggle(&mut self) {
        match self
            .presence()
            .is_some_and(crate::animation::Presence::is_open)
        {
            true => self.close(),
            false => self.show(),
        }
    }

    /// Whether this component currently has an **open overlay** (e.g. a `Select`
    /// dropdown). The host routes pointer/key events to an overlay-active widget
    /// first, so it can capture clicks/keys outside its layout bounds. Default
    /// `false`; see [`FocusManager`](crate::focus::FocusManager).
    fn overlay_active(&self) -> bool {
        false
    }

    /// **Does this widget draw its own mark for a drag over it?** Then the framework does not wash
    /// the whole box. A line along a border is one: the wash would cover the panes beside it.
    fn paints_own_drag_feedback(&self) -> bool {
        false
    }

    /// **The cursor over `point`, when it depends on where the pointer is.** A widget with one
    /// cursor for its whole box declares it ([`cursor`](crate::builders::PresenceExt::cursor));
    /// one that changes across its box — a link inside a terminal, which only counts while a
    /// modifier is held — answers here. It wins over the declared one, and nearer widgets win over
    /// farther. `None` leaves it to the declaration and to what holds this widget.
    fn cursor_over(&self, _point: Point) -> Option<crate::cursor::Cursor> {
        None
    }

    /// Whether this widget's **own box** is something the pointer lands on, where none of its
    /// children is. A leaf is what it draws, so it does. A layout-only widget ([`Base::container`])
    /// does only where it declares something or paints, so a full-size layout box laid over other
    /// things does not take every press that no child wants.
    fn claims_pointer(&self) -> bool {
        let base = self.base();
        !base.layout_only || base.answers_pointer()
    }

    /// Whether this component's **overlay surface geometrically occludes** `pos`
    /// (logical px). A host asks this before synthesizing a page-level action from
    /// a raw input — e.g. right-click → "open the context menu": if the point is
    /// covered by an overlay drawn above the page, the action must not fire
    /// underneath it. Distinct from [`overlay_active`](Self::overlay_active)
    /// (input **grab**): a non-grabbing overlay like a toast card still occludes
    /// the points it covers, while a **modal** overlay (an open `Dialog` scrim)
    /// occludes the whole viewport. A widget whose open overlay deliberately
    /// yields to a fresh trigger (a `ContextMenu`, where a second right-click
    /// re-anchors) keeps the default. Scan a tree with
    /// [`overlay_occluded_at`]. Default `false` (plain widgets never occlude).
    fn overlay_occludes(&self, pos: Point) -> bool {
        let _ = pos;
        false
    }

    /// The **plain-text form of this component's content** — its accessible name.
    ///
    /// A control whose content is *composed* cannot read that content's text: children are
    /// `impl Component`, so their types are erased. But a container sometimes needs that text — to
    /// **report** its value as a string, the way
    /// [`Select::selected_label`](crate::widgets::Select::selected_label) reports the chosen
    /// option. This is the one value it can pull out of an otherwise opaque subtree.
    ///
    /// The default computes the name **from the contents**, like the web's accessible-name
    /// algorithm: the first child that has one wins. So a
    /// [`Choice`](crate::widgets::Choice) composed of an `Icon` + `Label("HIGH")` summarizes to
    /// `"HIGH"` with no wiring — the `Icon` has no text, the `Label` does.
    /// [`Label`](crate::widgets::Label) is the leaf that supplies it; widgets that render text
    /// they own (not via a child) should override this too.
    fn text_summary(&self) -> Option<String> {
        self.base().children.iter().find_map(|c| c.text_summary())
    }

    /// Emit draw commands. Default: paint the base chrome, then children.
    fn paint(&self, cx: &mut PaintCx) {
        if !self.base().visible.get_untracked() {
            return;
        }
        cx.paint_base(self.base());
        for child in &self.base().children {
            paint_child(child.as_ref(), cx);
        }
    }

    /// Handle an event **before** this widget's children see it. Default: ignore it.
    ///
    /// This is where a widget takes something away from its subtree: a modal swallowing input
    /// aimed at the page behind it, a control that owns a whole gesture (a scrollbar thumb grab
    /// beats whatever is under the cursor), or state that must be current before anything below is
    /// hit-tested. Returning [`Handled::Yes`] stops the walk — the children never see it.
    ///
    /// Use it only for those. Anything a widget does with an event its children *declined* belongs
    /// in [`on_event`](Component::on_event).
    fn on_event_capture(&mut self, _ev: &Event) -> Handled {
        Handled::No
    }

    /// Handle an event this widget's children did not take. Default: ignore it.
    ///
    /// **A widget never routes to its children.** [`dispatch`] does that, always, for every event
    /// — which is the point: forwarding is not a line anyone writes, so it is not a line anyone can
    /// forget. A container that only cared about presses used to be free to handle the press and
    /// return, quietly stranding every child that needed the wheel or the release. That is how a
    /// scroll region ends up mounted, painted, and dead.
    fn on_event(&mut self, _ev: &Event) -> Handled {
        Handled::No
    }

    /// Called on this widget **after its subtree has seen `ev`**, whether or not something in
    /// there consumed it, with `handled` reporting which.
    ///
    /// The hook for a container that **watches what its own subtree did** rather than acting on
    /// the event itself: an [`ItemGroup`](crate::widgets::ItemGroup) reports a toggle when the
    /// header row inside it flips `expanded`, and the header consuming the click is the normal
    /// case, not the exception. [`on_event`](Self::on_event) cannot express that — it runs only
    /// when nothing below wanted the event — and taking over the walk to get it (which is what
    /// these containers used to do) means every event kind now depends on that container
    /// forwarding it correctly forever.
    ///
    /// Observation only: it returns nothing, so it can neither consume the event nor let a
    /// consumed one continue.
    fn after_subtree(&mut self, ev: &Event, handled: Handled) {
        let _ = (ev, handled);
    }

    /// `true` when this widget wants an enclosing scroll region to **keep it in view**.
    ///
    /// The default is keyboard focus, which is what browsers do: focus something off-screen and the
    /// view comes to it. A widget that is "current" by some other measure — a list's navigation
    /// cursor, a search hit — overrides this to say so, and then every
    /// [`ScrollRegion`](crate::widgets::ScrollRegion) it is ever placed inside follows it, with
    /// nothing wired at the call site.
    /// **Run this widget's primary action** — what a click or Enter on it would do — and report
    /// whether it had one. `false` by default: most widgets do nothing on their own.
    ///
    /// The equivalent of `button.click()` in the DOM, and it exists because there was no way to say
    /// "activate this child" through `dyn Component`. Without it a caller has only one lever: build
    /// a fake `Event::Key { Enter }` and dispatch it, hoping the target claims raw keys — which is
    /// what [`Dialog::submit`](crate::widgets::Dialog) did to fire its primary button. That worked
    /// only because seven widgets claimed `Enter` without checking whether they owned the keyboard,
    /// and it broke the moment they stopped. Synthesising input to reach behaviour is a symptom of
    /// a missing call, not a technique.
    fn activate(&mut self) -> bool {
        false
    }

    /// **Show only your icon, keeping your words** — or go back to showing both.
    ///
    /// A question a container asks a child it is short of room for, answered by whatever the child
    /// happens to be. `false` by default: a widget with nothing to shorten simply ignores it, and a
    /// widget with no icon must ignore it, since hiding its words would leave an empty box.
    ///
    /// It is a trait method for the same reason [`activate`](Self::activate) is: a container holds
    /// `dyn Component`, so without one the only way to ask would be to know the child's concrete
    /// type — which is exactly the coupling a container must not have. The container decides *what*
    /// is shown; the widget owns *how it looks* when it is, including that a button with no words
    /// is square and one with words is not.
    fn set_icon_only(&mut self, _on: bool) {}

    /// **Take this variant**, if you are the kind of widget that has one.
    ///
    /// A container asks a child it is styling as a group — a
    /// [`ButtonGroup`](crate::widgets::ButtonGroup) does, so its own affordance reads the same as
    /// the buttons beside it. `false` by default: a widget with no variant ignores it, and one that
    /// was given its own keeps it (that is the widget's judgement, not the container's).
    fn set_variant(&mut self, _variant: crate::widgets::ButtonVariant) {}

    /// **The cursor is on you** — if you are the kind of widget that shows that.
    ///
    /// `false` by default: a widget with no such state ignores it. A container that moves a cursor
    /// over its children uses this to light the one it is on, addressed by the child's own key —
    /// so a caller wires nothing and the container's answer cannot disagree with the child's look.
    ///
    /// Takes `&self` because the state is a signal: this is a write to reactive state, not a
    /// structural change, so nothing is rebuilt, re-laid-out or re-keyed.
    fn set_selected(&self, _on: bool) -> bool {
        false
    }

    /// **Show these words instead**, if you are the kind of widget that shows words.
    ///
    /// `false` by default: a widget with no text ignores it. A host uses this to change what a
    /// retained tree *says* without rebuilding it — see
    /// [`set_text_by_key`](crate::set_text_by_key).
    ///
    /// Takes `&self` because the text is a signal, so this is a write to reactive state and not a
    /// structural change: nothing above it has to be rebuilt, re-laid-out or re-keyed.
    ///
    /// It is a trait method for the same reason [`set_icon_only`](Self::set_icon_only) is: a
    /// container holds `dyn Component`, so without one the only way to ask would be to know the
    /// child's concrete type.
    fn set_text(&self, _text: String) -> bool {
        false
    }

    fn wants_visible(&self) -> bool {
        self.base().focused_by_keyboard()
    }

    /// The rect this widget occupies **for input**, or `None` when it takes none at all.
    ///
    /// The default is [`bounds`](Base::bounds), which is right for every widget drawn where it is
    /// laid out. Two kinds are not: one that paints a **floating panel** (a menu, a palette)
    /// reports the panel, and one that is **inert right now** (a closed
    /// [`Overlay`](crate::widgets::Overlay)) reports `None`, which takes its whole subtree out of
    /// the pointer's reach while leaving it laid out.
    fn hit_bounds(&self) -> Option<Rectangle> {
        Some(self.base().bounds)
    }

    /// The rect (logical px) to repaint when this widget is flagged
    /// [`needs_paint`](Base::needs_paint) — used by [`collect_damage`] in place of `bounds`.
    /// Overlay widgets that paint **outside** their own bounds (a tooltip bubble, a command-palette
    /// panel) override it so a redraw covers what they actually drew.
    fn damage_bounds(&self) -> Rectangle {
        crate::widgets::tooltip::damage(self.base())
    }

    /// `true` when this widget **clips** its children to its own bounds, so a press or a move
    /// outside those bounds must not reach them.
    ///
    /// Without it, content scrolled out of a [`ScrollRegion`](crate::widgets::ScrollRegion) stays
    /// clickable where it *would* have been: its bounds are real, it simply isn't drawn. Paint
    /// already honours the clip; this is the same rule for input, in the one place that walks the
    /// tree rather than in each clipping widget's own gate.
    ///
    /// **Three walks ask it now**, and each one that forgets shows the same defect from a different
    /// angle: paint (the clip itself), input ([`hit_test`](crate::pointer::hit_test) — a row past
    /// the fold is not clickable) and the **picker** ([`hint`](crate::hint) — a row past the fold
    /// gets no letter). The picker was the one that did not, and its symptom was keycaps for
    /// scrolled-away sidebar rows painted over the top bar and the status bar, because a cap goes
    /// into the overlay band and an overlay segment starts unclipped on purpose (F003/P082/T438).
    ///
    /// If you are adding a fourth walk over the tree, it asks this too.
    fn clips_children(&self) -> bool {
        self.base().clip_children
    }

    /// **The shape what this widget holds is cut to** when it [clips](Self::clips_children) — its
    /// box and its corner radius by default (CSS `overflow: hidden` with `border-radius`).
    ///
    /// The framework applies it to each child as it draws it ([`paint_child`]), so it holds
    /// however the widget draws its children — under a colour, translated, in the top layer, only
    /// some of them — and no widget writes the cut itself. A widget whose cut is not its box (a
    /// scroll area keeping its scrollbar lane clear) or whose corners come from elsewhere says so
    /// here.
    fn children_clip(&self, _theme: &Theme) -> Option<crate::scene::Rounded> {
        let b = self.base();
        self.clips_children().then_some(crate::scene::Rounded {
            rect: b.bounds,
            radius: b.style.visual.radius,
        })
    }

    /// **This component arranges its children as a grid** — its column and row tracks, and its
    /// named areas as written.
    ///
    /// The layout pass asks each parent this before building its children, because a child says
    /// where it goes in terms only its parent can settle: `1 / -1` is "every column there are", and
    /// `grid-area: title` is a name from this template. Answering here rather than reading a style
    /// field keeps the template with the widget that owns it, and keeps [`Style`] `Copy`.
    ///
    /// `None` — the default — means "not a grid", and a child's grid placement means nothing.
    fn grid_template(&self) -> Option<crate::style::GridTemplate<'_>> {
        None
    }

    /// Called when this component gains keyboard focus. Default: set the focus
    /// flag (which drives the focus ring). Override to add behaviour.
    /// `visible` is true for keyboard focus (show the ring), false for mouse.
    fn on_focus(&mut self, visible: bool) {
        self.base().focus(visible);
    }

    /// Called when this component loses keyboard focus. Default: clear it.
    fn on_blur(&mut self) {
        self.base().blur();
    }

    /// The taffy style for this component's layout node. Default: the base
    /// style mapped via `Style::to_taffy`. Layout containers that need extra
    /// taffy config (e.g. [`Grid`](crate::widgets::Grid) injecting `display:
    /// grid` + track templates) override this.
    fn taffy_style(&self) -> taffy::Style {
        self.base().style.layout.to_taffy(self.base().font)
    }

    /// Recompute size from the resolved font ([`Base::font`]). Widgets whose
    /// dimensions depend on font size override this; the layout pass calls it on
    /// every node after resolving the font, so a global font reflows the tree
    /// without per-widget wiring. Default: no-op.
    fn remeasure(&mut self) {}

    /// Text whose **height depends on the width the engine resolves** — return `Some` and the
    /// layout pass hands this node to taffy's measure path instead of a fixed size.
    ///
    /// [`remeasure`](Self::remeasure) cannot express this: it runs *before* layout, so it has no
    /// width to wrap into and can only report a size it already knows. A wrapping
    /// [`Label`](crate::widgets::Label) is the one widget that needs the difference; everything
    /// else measures itself and returns `None` (the default).
    ///
    /// Only consulted for **leaves** — a widget with children is sized by them.
    fn measure_text(&self) -> Option<crate::layout::TextMeasure> {
        None
    }

    /// Called by the layout engine **after** this node's bounds (and all its
    /// descendants' bounds) have been (re)computed and written to `Base::bounds`.
    /// Post-order: children fire before the parent. Default: no-op. Override to
    /// react to a fresh layout — e.g. a scroll viewport resets any shift it had
    /// baked into its children's bounds (they are now back at their natural
    /// positions), so the next paint re-applies the shift from scratch instead
    /// of compounding. This is what lets an embeddable scroll viewport reuse the
    /// whole-page scroll pattern (shift subtree bounds + clip) without owning the
    /// layout/scroll cycle.
    fn on_layout(&mut self) {}

    /// Advance time-based animations by `dt` seconds. Returns `true` if a
    /// **continuous** animation is still running (eases, slides, spinners), so the
    /// host schedules another frame at its frame cap. Default: recurse.
    ///
    /// A widget whose visual changes only at sparse, known moments (e.g. a blinking
    /// caret toggling ~twice a second) should return `false` here and instead report
    /// the time to its next change via [`next_redraw`](Self::next_redraw), so the
    /// host can sleep until then rather than redrawing every frame.
    fn tick(&mut self, dt: f32) -> bool {
        // **Presence is advanced here, for every widget.** It used to be advanced only by the two
        // widgets that override this, so a surface shown through `Component::show`/`close` without
        // being one of those two never finished its exit — see [`Base::tick_presence`].
        let mut animating = self.base_mut().tick_presence(dt);
        for child in self.base_mut().children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }

    /// Seconds until this subtree next needs a **timed** redraw (independent of the
    /// continuous-animation signal from [`tick`](Self::tick)) — e.g. a focused
    /// `Input`'s caret returns the time to its next blink toggle. The host wakes at
    /// the soonest such time across the tree instead of redrawing continuously.
    /// `None` = no timed redraw pending. Default: the soonest across children.
    fn next_redraw(&self) -> Option<f32> {
        let mut soonest = crate::widgets::tooltip::wake(self.base());
        for child in self.base().children.iter() {
            soonest = soonest_redraw(soonest, child.next_redraw());
        }
        soonest
    }

    /// **Can this widget be dragged?** Default reads [`draggable`](Base::draggable); a widget whose
    /// draggability changes may override.
    ///
    /// It answers only *whether*, never *what*: what gets dragged is the widget's identity, and an
    /// identity is [`key`](Base::key) **when it declared one and derived from its content when it
    /// did not** — the same string the keyboard cursor, the right-click target and a remembered
    /// hint letter are filed under ([`nav::identity_of`](crate::nav::identity_of)). Asking for the
    /// key here instead would make `.draggable()` silently do nothing on every widget that never
    /// needed a name, and put an internal rule in front of the author.
    fn is_drag_source(&self) -> bool {
        self.base().draggable
    }

    /// **Would a press now pick this widget up?** It is a drag source, and its rule (if it set one)
    /// holds for what is held down right now.
    fn may_start_drag(&self) -> bool {
        self.is_drag_source()
            && self
                .base()
                .drag_gate
                .as_ref()
                .is_none_or(|rule| rule(crate::event::modifiers()))
    }

    /// **Does this widget accept drops?** The same shape, reading
    /// [`drop_target`](Base::drop_target). Walked by
    /// [`drag::resolve_at`](crate::drag::resolve_at).
    fn is_drop_target(&self) -> bool {
        self.base().drop_target
    }

    /// **Would this widget take what is being dragged?** A target that declared no
    /// [`accepts`](Base::accepts) takes anything; one that did takes only what it named, and a
    /// dragged widget that named nothing is taken by anyone.
    ///
    /// Asked during resolution, so a target that would refuse is never offered and never drawn on.
    fn accepts_drag(&self, kind: Option<&str>) -> bool {
        let want = &self.base().accepts;
        match kind {
            _ if want.is_empty() => true,
            Some(k) => want.iter().any(|w| w == k),
            None => true,
        }
    }
}
