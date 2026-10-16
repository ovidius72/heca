//! **The letter a widget wears while a pick is open**, and what picking it does.

use crate::color::Color;
use crate::component::Component;

/// **The letter a widget wears while a pick is open**: whether it gets one, what picking it does,
/// which pick it belongs to, and where and how the letter is drawn.
pub trait HintExt: Component + Sized {
    /// **Declare an action this widget answers to, by name.**
    ///
    /// ```
    /// use heca_grid_ui::prelude::*;
    /// use heca_grid_ui::widgets::{KeyHintGroup, Flex};
    /// use heca_grid_ui::reactive::{signal, SignalUpdate};
    ///
    /// let open = signal(false);
    /// let picker = KeyHintGroup::new(Flex::column())
    ///     .open_when(open)
    ///     .on_action("mypanel.pick", move || open.set(true));
    /// ```
    ///
    /// The name is namespaced by whoever declares it, exactly as a provider's actions are, and a
    /// binding names it the same way:
    ///
    /// ```toml
    /// [[keys.surface]]
    /// name = "mypanel"
    /// pick = "s"          # → mypanel.pick
    /// ```
    ///
    /// **The widget names the action; config names the key.** A key written into a widget would be
    /// unrebindable, absent from the palette and unreachable over RPC — the thing AGENTS § 2
    /// forbids.
    ///
    /// This is what lets a **layer** own a verb. A dock declares its actions through `Provider`;
    /// an overlay had no such seam, so it could only bind verbs the app had already compiled in.
    /// See [`Base::actions`](crate::component::Base::actions).
    /// **Keep this widget out of the picker**, however actionable it is.
    ///
    /// ```
    /// use heca_grid_ui::prelude::*;
    /// use heca_grid_ui::widgets::Button;
    ///
    /// // A close button on every row would eat a letter each, for the one gesture nobody picks.
    /// let b = Button::new("×").hintable(false);
    /// ```
    ///
    /// **The opt-out exists because being pickable is not opt-in.** Anything you can act on — a
    /// click, a double click, a key — wears a letter with nothing declared, so the only thing left
    /// to say is "not me". `hintable(true)` is the default and changes nothing; a widget nobody can
    /// act on has nothing for a letter to run.
    ///
    /// Letters are scarce (52 in one picker, one keystroke each), so this is how a dense surface
    /// keeps them for the targets that matter.
    #[heca_grid_ui_macros::prop]
    fn hintable(mut self, yes: bool) -> Self {
        self.base_mut().hintable = yes;
        self
    }


    /// **What a pick does to this widget, when that differs from acting on it.**
    ///
    /// Being pickable is **not** what this turns on — anything actionable already wears a letter,
    /// and picking it does what clicking it does. This is the **override**:
    ///
    /// ```
    /// use heca_grid_ui::prelude::*;
    /// use heca_grid_ui::widgets::Row;
    ///
    /// // A click activates the pane and leaves the sidebar; a pick looks at it and stays.
    /// let row = Row::new().on_activate(|| { /* focus_pane */ }).on_hint(|| { /* peek_selected */ });
    /// ```
    ///
    /// **It fires in the target phase only** — when *this* widget is the one picked, never when a
    /// pick from a child passes through on the way up. A pane that is itself pickable and holds
    /// pickable rows would otherwise fire both and land you on the pane, and every such container
    /// would hand-write the DOM's `e.target !== e.currentTarget` guard — N copies of a framework
    /// rule, which by our own rule means the API is missing. This deliberately diverges from
    /// [`on_click`](super::EventExt::on_click): clicking a child of a clickable box **is** clicking
    /// the box, because the pointer is over both, while picking a row is **not** picking the pane —
    /// a pick is nominal, not spatial.
    ///
    /// **To watch picks from your children instead**, listen for the bubbled event:
    /// `.on(EventKind::Hint, |e| …)`. It sees which widget was picked and can
    /// [`stop_propagation`](crate::event::EventCx::stop_propagation); it declares nothing and gets
    /// no letter of its own.
    ///
    /// **Say what it is, where you can.** `f` may be a plain closure, or a
    /// [`Hint`](crate::hint::Hint) carrying the [`Intent`](heca_view::Intent) the act *is* — which
    /// is what lets a host ask its own policy about a candidate before spending a letter on it. The
    /// app's `fires` and `realize` both hand over the pair; nothing else has to.
    fn on_hint(mut self, f: impl Into<crate::hint::Hint>) -> Self {
        self.base_mut().hint = Some(f.into());
        self
    }


    /// **Which pickers letter this target.** Unset — the default — means the ordinary one, which
    /// is `prefix+/` in heca and letters everything actionable.
    ///
    /// One surface can carry more than one verb over the same tree. The exposé's cards mean *go
    /// there* and their ⊠ icons mean *remove that*: a picker that letters both hands out sixteen
    /// letters where you wanted eight, and half of them delete what you meant to jump to. So a
    /// target says which sets it belongs to and a picker says which one it hands letters for.
    ///
    /// ```ignore
    /// Row::new().on_hint(go_to(id))                          // the ordinary picker
    /// IconButton::new(Glyph::X).on_hint(remove(id)).hint_scope(["close"])   // only "close"
    /// ```
    ///
    /// **Naming a scope takes the target OUT of the ordinary picker.** That is what makes this
    /// worth more than the surface it was built for: a column in the workspaces dock is a
    /// *destination* for "move a pane to a column", never somewhere `prefix+/` should send you, and
    /// saying so is one line on the widget instead of a rule the picker carries about columns.
    ///
    /// Name several and the target belongs to each of those pickers. Addressing a widget **by
    /// key** — what `prefix+q` and "move to column" do — ignores scopes entirely: those name one
    /// target outright rather than collecting a set, so there is nothing to filter.
    #[heca_grid_ui_macros::prop]
    fn hint_scope(mut self, scopes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.base_mut().hint_scopes = scopes.into_iter().map(Into::into).collect();
        self
    }


    /// **Where this widget's hint letter sits over it** (default
    /// [`TopCenter`](crate::widgets::HintPlacement::TopCenter)).
    ///
    /// ```ignore
    /// Row::new().hint_placement(HintPlacement::CenterRight)
    /// ```
    ///
    /// [`Base::hint_style`](crate::Base::hint_style) has always been universal — every widget that
    /// wears a letter has one — but the only way to *set* it was to wrap the widget in a
    /// [`KeyHint`](crate::widgets::KeyHint). That is the wrapper rule again: it puts the knowledge
    /// in every caller's discipline, and it cannot be done at all to a widget a typed container
    /// holds, because wrapping it changes what it is — the same defect that made the tooltip a
    /// property. One builder, on the widget.
    ///
    /// The wrapper writes these same four fields, so there is one slot and one rule rather than two
    /// that can drift.
    #[heca_grid_ui_macros::prop]
    fn hint_placement(mut self, placement: crate::widgets::HintPlacement) -> Self {
        self.base_mut().hint_style.placement = placement;
        self
    }


    /// The keycap's font size in logical px. Unset = derived from the widget's resolved font, which
    /// is what keeps a letter proportional to the thing it captions.
    #[heca_grid_ui_macros::prop]
    fn hint_size(mut self, px: f32) -> Self {
        self.base_mut().hint_style.size = Some(px);
        self
    }


    /// The keycap's colour, glow included. Unset = the theme's `accent`, so a letter follows a
    /// theme change with nothing rewritten.
    ///
    /// Prefer [`hint_tone`](HintExt::hint_tone) where a *meaning* fits: a literal does not
    /// follow a theme reload, and a widget has no theme at build time to take one from.
    #[heca_grid_ui_macros::prop]
    fn hint_color(mut self, color: Color) -> Self {
        self.base_mut().hint_style.color = Some(color);
        self
    }


    /// **What this widget's keycap MEANS** — the theme picks the colour.
    ///
    /// `Accent` is a place to go, `Muted` a structural control (fold this, close that), and
    /// `Warning` / `Success` / `Danger` are further classes so two kinds never read alike. A
    /// literal via [`hint_color`](HintExt::hint_color) still wins where one is set, but a
    /// widget composing itself has no theme to take a literal from — which is why a meaning is the
    /// thing it can say.
    #[heca_grid_ui_macros::prop]
    fn hint_tone(mut self, tone: crate::Tone) -> Self {
        self.base_mut().hint_style.tone = Some(tone);
        self
    }


    /// A vertical nudge applied **after** placement — positive moves the cap down. What drops a
    /// [`TopRight`](crate::widgets::HintPlacement::TopRight) cap onto a dock's header line.
    #[heca_grid_ui_macros::prop]
    fn hint_offset_y(mut self, px: f64) -> Self {
        self.base_mut().hint_style.offset_y = px;
        self
    }
}

impl<T: Component + Sized> HintExt for T {}
