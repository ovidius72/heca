//! The pane shell — the frame around whatever app is running inside, and the one thing that
//! carries the pane's identity and its pick letter.
//!
//! The pane's only jobs are its hint letter, zoom, float, split and remove. A terminal draws no
//! letters: it is a program running inside the pane, and the pane is the container for every
//! future app.
//!
//! So the shell owns **no content**. A terminal today, a browser or a Neovim GUI later, or a
//! plugin's own tree — each is a child, and none of them has anything to do with the letter.

use heca_core::layout::PaneId;
use heca_grid_ui::widgets::{HintPlacement, Pane as UiPane};
use heca_grid_ui::{
    DragExt, EventExt, HintExt, IdentityExt, LayoutExt, MenuExt, Parent, PlaceExt, PresenceExt,
    StyleExt,
};

use super::model::PaneShellModel;

/// The app seams the shell binds, travelling as **one group** rather than one argument at a time
/// (AGENTS.md § 0b-bis rule 3) — so a test can hand it the app's edges and nothing else.
#[derive(Clone)]
pub(crate) struct PaneCallbacks {
    /// What picking this pane does: focus it. Emitted as an `InteractionIntent`, never performed
    /// here — the pane asks, the core does.
    pub(crate) pick: std::rc::Rc<dyn Fn(PaneId)>,
}

/// The pane shell component. Properties are struct fields and the constructor is a struct literal
/// (AGENTS.md § 0b-bis rule 2), so a call site reads without counting arguments.
pub(crate) struct PaneShell<'a, C> {
    pub(crate) model: &'a PaneShellModel,
    pub(crate) cb: &'a PaneCallbacks,
    /// An arbitrary component subtree. Any header belongs inside that content.
    pub(crate) content: C,
}

/// The frame's content, independent of the layout or component it contains.
pub(crate) const PANE_CONTENT_AREA: &str = "content";

impl<C: heca_grid_ui::builders::IntoChildren> PaneShell<'_, C> {
    /// Build the retained tree for one pane.
    ///
    /// Returns the `Pane` itself: it says what picking it does, where its letter goes and what
    /// colour that letter is, the same way it says what a click does. There is no wrapper to see
    /// past — which matters twice over. A wrapper round an actionable widget is a SECOND pick
    /// target, so a surface picker offered two letters where the author wrote one; and the pane's
    /// `key` sat inside it, so anything looking for the pane by name found an anonymous wrapper
    /// instead, treated every pane as unbuilt, and rebuilt all of them every frame.
    ///
    /// **The letter is drawn by the framework**, inside `heca_grid_ui::paint_child`, from
    /// `Base::hint_label`. Anything that paints this tree with a direct `.paint(cx)` will show no
    /// letter at all — which is exactly the defect this guards against.
    pub(crate) fn build(self) -> UiPane {
        let PaneShellModel {
            pane_id,
            active,
            frame,
            border_color,
            border_width,
            border_radius,
            content_inset,
            accent,
            active_glow,
            ..
        } = *self.model;

        // **The pane fills whatever rect it is given — it never carries one.** A share, not a
        // measure: the WM layout engine owns a pane's geometry, and `sync_panes` writes that rect
        // onto this tree's root every frame. Baking `Px(w)` in here instead meant the tree kept
        // the width it was first built at, so zooming or resizing left the frame at the old size
        // while the content moved (found by tracing: asked 648 wide, got 380, forever).
        let mut pane = UiPane::new()
            .border_style(frame.into())
            .width(heca_grid_ui::Length::Percent(1.0))
            .height(heca_grid_ui::Length::Percent(1.0))
            .padding(content_inset)
            .border(to_gui_color(border_color), border_width)
            .radius(border_radius)
            // What it holds is cut to its round corners, wherever it is drawn — in its column, or
            // as the picture carried under the pointer.
            .clip_children(true);
        if active {
            pane = pane.glow_with(to_gui_color(border_color), active_glow.0, active_glow.1);
        }
        // A floating pane covers what is under it, in its own paint: a fill, or a blur of what lies
        // beneath — no host step has to draw either between the panes.
        if let Some(float) = self.model.float {
            pane = if float.frost > 0.0 {
                pane.frosted(float.frost)
            } else {
                pane.background(to_gui_color(float.background))
            };
        }

        // The pane's own identity, from the data — never a counter, never a position. A pane id is
        // stable across every rebuild, which is what lets a letter stay with the same pane between
        // openings of the picker.
        // **One pane, seen in several places.** The sidebar row and the exposé card show this
        // same pane and declare the same key, so the three of them wear one letter between them.
        //
        // **Pressing the pane — or picking its letter — types into what runs in it**, whatever that
        // is: the pane hands its keyboard to its content part. So that content needs no letter of
        // its own, and has none; a terminal placed in a dock, handed nothing, keeps its own.
        let mut pane = pane
            .key(crate::chrome::pane_key(pane_id))
            .focus_fallback(active)
            .keyboard_to(PANE_CONTENT_AREA);
        // **A tiled pane is carried by the window's key and lands beside another** — the framework
        // runs the drag and paints what follows the pointer; a float is neither picked up nor
        // dropped on, since it is over everything and moves where it was put.
        if self.model.float.is_none() {
            let key = self.model.move_modifier;
            pane = pane
                .draggable_as(crate::chrome::PANE_DRAG_KIND)
                .draggable_when(move |held| crate::modifier::held(key, held))
                .accepts_beside([crate::chrome::PANE_DRAG_KIND]);
        }

        // **What a click on a pane means, said in one place, in the order it happens.**
        //
        // It was two separate declarations twenty lines apart: a press handler that asked for
        // focus, and a menu handed to the framework to open on right-click. Nothing ordered them —
        // it worked only because a press arrives before a release — and reading either one told
        // you nothing about the gesture. A sidebar row, a dock or a plugin's panel says exactly
        // this about itself, and nothing outside knows any of them exist.
        // **What a click on a pane means, and nothing host-private in sight.**
        //
        // Clicking focuses it: the action named, not a function handed in. Right-clicking focuses
        // it too and ends the gesture there, so nothing behind also acts on it — the menu is a
        // *declaration* below, opened and closed by the framework, because a pane has no business
        // knowing what a menu is.
        //
        // This carried a struct of host callbacks until now, cloned twice at the call site. A
        // plugin could build none of that, so a plugin's widget could not act at all.
        // **Clicking a pane focuses it, whichever button.** The action named, not a function
        // handed in — a plugin's widget writes the identical line.
        //
        // It does not claim the click: a declared menu opens only for a right-click nobody took
        // (guarded by `a_widget_that_claims_its_own_right_click_beats_the_declaration`), so
        // claiming it here would silently switch this pane's own menu off.
        let mut pane = pane
            .on_click(move |ev| ev.dispatch(focus_pane(pane_id)))
            .on_right_click(move |ev| ev.dispatch(focus_pane(pane_id)))
            // **It has a menu; it does not own one.** The framework opens it on a right-click at
            // the pointer and takes it down when a press lands outside. Everything in it comes
            // from whatever is registered for this name — heca's entries and any plugin's alike.
            .context_menu(|_at| {
                heca_grid_ui::widgets::ContextMenu::new("pane").child(
                    heca_grid_ui::widgets::Menu::new("Pane", "What you can do with this pane")
                        .name(crate::chrome::ContextPath::PANE),
                )
            });

        let body = heca_grid_ui::widgets::Flex::column()
            .area(PANE_CONTENT_AREA)
            .width(heca_grid_ui::Length::Percent(1.0))
            .height(heca_grid_ui::Length::Percent(1.0))
            .child(self.content);
        pane = pane.child(body);

        let pick = self.cb.pick.clone();
        // **Say what the pick IS, not only what it runs**. The closure emits
        // `InteractionIntent::FocusPane`, which is host-private and opaque to a policy; the intent
        // beside it is the same act as data, so `chrome::active_hint_targets` can ask
        // `route_interaction` whether focusing *this* pane is permitted before spending a letter on
        // it. With a floating pane active it is not, and the pane now gets no letter instead of one
        // that does nothing.
        //
        // It names the built-in `focus_pane` with the argument that action declares, so the letter,
        // a keybinding, a menu entry and RPC all resolve through one `build_action`.
        let picked = focus_pane(pane_id);
        pane
            // Centred over the pane, which is where the letter is today and what the maintainer
            // expects. `TopCenter` is for compact square targets; a pane is the large-target case.
            .hint_placement(HintPlacement::Center)
            // The theme accent, not the ambient one: see `PaneShellModel::accent`.
            .hint_color(to_gui_color(accent))
            .on_hint(heca_grid_ui::Hint::of(picked, move || pick(pane_id)))
    }
}

/// **Focus this pane** — the catalogued action, named rather than performed here.
fn focus_pane(pane_id: PaneId) -> heca_view::Intent {
    heca_view::Intent::new("focus_pane").arg("pane_id", heca_view::PropValue::Int(pane_id.0 as i64))
}

/// **Give the shell what focus changed about it.** A per-frame input, exactly like the rect above
/// and the header's words — never part of the built tree.
///
/// Whether a pane is active used to be part of its **identity**, so focusing one threw its tree
/// away and built a new one. That lost any gesture in flight: a right-click is made from a press
/// and a release on the *same* widget, and the press had been recorded on the tree that focusing
/// discarded. So the first right-click on an unfocused pane focused it and opened nothing, and only
/// a second one — with nothing left to rebuild — showed the menu.
///
/// This is the third thing in this file to move out of the rebuild key for the same reason; the
/// other two are the rect and the header's words, each with the same story.
pub(crate) fn focus_state_to(root: &mut dyn heca_grid_ui::Component, model: &PaneShellModel) {
    let color = to_gui_color(model.border_color);
    let base = root.base_mut();
    base.focus_fallback = model.active;
    if let Some(border) = base.style.visual.border.as_mut() {
        border.color = color;
    }
    base.style.visual.glow = model.active.then_some(heca_grid_ui::scene::Glow {
        color,
        radius: model.active_glow.0,
        intensity: model.active_glow.1,
    });
    base.hint_style.color = Some(to_gui_color(model.accent));
    // **A pane re-tints what it holds by publishing its hue, not by swapping the theme.**
    //
    // An active pane really does mean to re-accent its contents — that is its identity. It used to
    // be done by painting the pane under a theme whose accent had been replaced, which meant the
    // host had to make a separate paint call per pane, and therefore that nothing else could ever
    // paint one. Published as a tone, the hue reaches the same widgets through the ordinary walk,
    // so who paints the pane stops mattering.
    base.style.visual.accent = Some(color);
}

fn to_gui_color(color: [f32; 4]) -> heca_grid_ui::Color {
    heca_grid_ui::Color::new(
        (color[0] * 255.0) as u8,
        (color[1] * 255.0) as u8,
        (color[2] * 255.0) as u8,
        (color[3] * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests;
