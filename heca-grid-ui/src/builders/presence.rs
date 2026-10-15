//! **Whether and how a widget is there**: shown, drawn, in front of the page, hit, clipping.

use crate::component::Component;

/// **Whether and how a widget is there** — in the layout, drawn, standing in front of the page,
/// holding the keyboard, hit by the pointer, cutting what it holds, and the cursor over it.
pub trait PresenceExt: Component + Sized {
    /// Label this row with the identity its component knows it by (`"pane:7"`, `"ws:0"`).
    ///
    /// One declaration, three readers: the keyboard cursor, the right-click target, and later the
    /// drag identity. The string is **opaque to the library** — only the component that wrote it
    /// and the host routing back to that component ever interpret it — and it must be stable across
    /// tree rebuilds, which is what lets a cursor survive one. See [`crate::nav`].
    /// **Let the pointer pass through this widget** — it draws, but it is not a target, and nor is
    /// anything inside it (CSS `pointer-events: none`).
    ///
    /// For decoration that stands in for something else: an echo of a value shown inside the
    /// control that owns it. Without it the echo hovers and clicks in its own right, and the
    /// control ends up wearing two highlights.
    fn pointer_transparent(mut self, transparent: bool) -> Self {
        self.base_mut().pointer_transparent = transparent;
        self
    }


    /// **What the cursor looks like over this widget** (CSS `cursor`): the nearest widget that
    /// declared one, on the way up from what the pointer is over, decides — see
    /// [`cursor_at`](crate::cursor_at).
    fn cursor(mut self, cursor: crate::cursor::Cursor) -> Self {
        self.base_mut().cursor = Some(cursor);
        self
    }


    /// **Clip what this widget holds to its own box** (CSS `overflow: hidden`): a child that is
    /// placed or grows past the edge is not drawn there and cannot be hit there.
    fn clip_children(mut self, clip: bool) -> Self {
        self.base_mut().clip_children = clip;
        self
    }


    /// **Lock what is behind this surface** — refuse actions on it while this is up.
    ///
    /// ```ignore
    /// Overlay::new().lock(true).child(my_panel)
    /// ```
    ///
    /// A dialog locks; a map of the working area does not, because picking one of the things behind
    /// it is the point. **Not about pixels** — the map covers every pixel it draws over and still
    /// passes `false` — and not [`overlay_occludes`](crate::Component::overlay_occludes), which is
    /// the geometric question the pointer asks.
    ///
    /// Widgets that know what they are set it themselves; this is for a raw
    /// [`Overlay`](crate::widgets::Overlay). See [`Base::lock`](crate::Base::lock).
    fn lock(mut self, yes: bool) -> Self {
        self.base_mut().lock = yes;
        self
    }


    /// **Override whether this surface takes the keyboard while it is up.**
    ///
    /// ```ignore
    /// Overlay::new().child(my_panel)                      // derived: open == holds focus == takes keys
    /// my_surface.captures_keyboard(true)                  // say so explicitly
    /// ```
    ///
    /// **Optional, and you almost never want it.** Left alone, the answer is read from the tree
    /// with the same rule the event walk uses — a surface that wants keys holds focus, and every
    /// layer widget binds its open signal to it. Reach for this only when a surface takes the
    /// keyboard in a way the framework cannot see. See
    /// [`Base::captures_keyboard`](crate::Base::captures_keyboard).
    fn captures_keyboard(mut self, yes: bool) -> Self {
        self.base_mut().captures_keyboard = Some(yes);
        self
    }


    /// **Take this widget out of the layout entirely** — CSS `display: none`. Its neighbours close
    /// up, and it is neither painted nor Tab-focused.
    ///
    /// ```ignore
    /// Row::new().hidden(collapsed)
    /// ```
    ///
    /// The field was always here and every widget that folds a subtree writes it
    /// (`ItemGroup`, `DockFrame`, `ChromeRegion`) — but only from *inside itself*, because there
    /// was no builder, so a caller wanting the same thing had to reach for the
    /// [`Visibility`](crate::widgets::Visibility) wrapper. Its described twin `"hidden"` has been
    /// carried by the generic style merge all along, which left the two authoring paths unequal.
    ///
    /// **Its twin is [`visible`](PresenceExt::visible)** — that one keeps the box.
    #[heca_grid_ui_macros::prop]
    fn hidden(mut self, hidden: bool) -> Self {
        self.base_mut().set_hidden(hidden);
        self
    }


    /// **Whether this widget's ink is drawn**, keeping its box either way — CSS `visibility`.
    ///
    /// ```ignore
    /// StatusDot::error().visible(false)   // the row's layout does not move when it appears
    /// ```
    ///
    /// **The other one is [`hidden`](crate::style::Layout::hidden)** — CSS `display: none`, which
    /// takes the widget out of the layout so its neighbours close up. Reach for that when the space
    /// should collapse, and for this when it must not: a row of four status slots showing one at a
    /// time stays still only if the three quiet ones keep their boxes.
    ///
    /// `Base::visible` has always been on every widget; this is the builder that was missing, so
    /// setting it meant wrapping in a [`Visibility`](crate::widgets::Visibility) — which cannot be
    /// done to a widget a typed container holds, and which a description cannot express at all.
    /// The wrapper stays for a region that is not a widget you can put a builder on.
    #[heca_grid_ui_macros::prop]
    fn visible(self, visible: bool) -> Self {
        crate::reactive::SignalUpdate::set(&self.base().visible, visible);
        self
    }
}

impl<T: Component + Sized> PresenceExt for T {}
