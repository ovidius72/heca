//! The pane shell — the frame around whatever app is running inside, and the one thing that
//! carries the pane's identity and its pick letter.
//!
//! Antonio, 2026-08-18: *"the only responsibility of the pane is to draw the key hint letter, zoom
//! in/out, float/unfloat, split/unsplit, remove"*, and *"terminal should not be responsible to
//! render letters. They are programs that run inside pane. So pane is the container for every
//! future app."*
//!
//! So the shell owns **no content**. A terminal today, a browser or a Neovim GUI later, or a
//! plugin's own tree — each is a child, and none of them has anything to do with the letter.

use heca_core::layout::PaneId;
use heca_grid_ui::widgets::{HintPlacement, Pane as UiPane};
use heca_grid_ui::{ComponentExt, LayoutExt, Parent, PlaceExt, StyleExt};

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
pub(crate) struct PaneShell<'a> {
    pub(crate) model: &'a PaneShellModel,
    pub(crate) cb: &'a PaneCallbacks,
    /// **The header slot — whatever the thing running in this pane wants along its top.**
    ///
    /// The shell does not know what a terminal is, so it does not know what a title bar is either.
    /// A terminal fills this with its info bar (a `Tag` of title segments and a row of action
    /// `IconButton`s); an editor, a browser or a plugin's pane fills it with something else, or
    /// with nothing. Same slot, same rules, no special case — exactly what
    /// [`DockFrame::header`](heca_grid_ui::widgets::DockFrame::header) already does for a dock.
    ///
    /// It is a **child**, which is the whole point: the layout places it, it sizes itself, and one
    /// walk delivers its input. Before this it was a second retained tree the app laid out,
    /// positioned by hand and routed events to separately (F003/P097/T497).
    ///
    /// The pane keeps the header it is given as its own: it seats the node as a child and answers
    /// the facts it is handed by showing them there.
    pub(crate) header: Option<crate::chrome::PaneHeader>,
    /// **The content slot — what actually runs in this pane**, under the header.
    ///
    /// Empty today: a terminal paints itself into the pane's rect rather than being a child
    /// (P094/T449 is what makes it one). The slot exists anyway, because it is what gives the
    /// column its second row — the header takes its natural height and this takes everything
    /// left, which is what puts the header in a strip at the top instead of in the middle of the
    /// pane.
    pub(crate) content: Option<Box<dyn heca_grid_ui::Component>>,
}

/// **The pane's two rows, named.** The parts say which one they are and the host asks by name —
/// which used to count children instead.
pub(crate) const PANE_HEADER_AREA: &str = "header";
/// The row that takes whatever the header does not.
pub(crate) const PANE_CONTENT_AREA: &str = "content";

impl PaneShell<'_> {
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
    /// letter at all — which is exactly the defect this task exists to end.
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
            .radius(border_radius);
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
        // openings of the picker (F003/P082/T445).
        // **One pane, seen in several places.** The sidebar row and the exposé card show this
        // same pane and declare the same key, so the three of them wear one letter between them.
        let pane = pane.key(crate::chrome::pane_key(pane_id));

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

        // **The pane is a FRAME; the arrangement is a template.** The header takes its own
        // height, the content takes the rest, and each part says which row it is by name.
        //
        // A `header` slot would answer exactly this one shape — and then Header | Content | Footer
        // would need a second slot, and the next part a third. A track template answers all of
        // them in one line, and it is the same line a dock, a palette or a plugin's panel writes
        // (docs/layout.md). It also ends the question the host used to ask: "does this pane have
        // two children, so is the first one a header?" — put a second thing in the body and the
        // first was mistaken for one. A part is in the area it named, and a missing part is a
        // missing row.
        let content: Box<dyn heca_grid_ui::Component> = match self.content {
            Some(content) => content,
            None => Box::new(heca_grid_ui::widgets::Flex::column().grow(1.0)),
        };
        let body = match &self.header {
            Some(header) => heca_grid_ui::widgets::Grid::new()
                .template_row("auto 1fr")
                .template_area([PANE_HEADER_AREA, PANE_CONTENT_AREA])
                .child([
                    header.seat().area(PANE_HEADER_AREA),
                    content.area(PANE_CONTENT_AREA),
                ]),
            // **A missing part is a missing row**, not a flag and not a zero-height placeholder.
            None => heca_grid_ui::widgets::Grid::new()
                .template_row("1fr")
                .template_area([PANE_CONTENT_AREA])
                .child(content.area(PANE_CONTENT_AREA)),
        };
        if let Some(header) = self.header {
            pane = header.answer_props(pane);
        }
        pane = pane.child(
            body.width(heca_grid_ui::Length::Percent(1.0))
                .height(heca_grid_ui::Length::Percent(1.0)),
        );

        let pick = self.cb.pick.clone();
        // **Say what the pick IS, not only what it runs** (F003/P082/T432). The closure emits
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
