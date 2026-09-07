//! **A confirm surface, described end to end** — the acceptance test for the plugin path
//! (F003/P097/T502).
//!
//! Every other overlay heca shows is built in Rust. The described path
//! ([`open_view_layer`](super::open_view_layer)) is fully built, documented, and **had no caller at
//! all** — so "a plugin can add an overlay" was a claim about a road nobody had driven down.
//!
//! This is that road, driven. The whole surface — the scrim, the panel, the title, the message and
//! both buttons — is one [`ViewNode`], and its behaviour crosses as [`Intent`] only. It is written
//! the way a plugin *must* write it: no closures, no host-private type, no registry, nothing that
//! needs `AppState`. If it needs a nudge the host has to know about, that nudge is a gap in the
//! model, and the finding is the point of the task.
//!
//! # Why a confirm, and not the exposé
//!
//! The exposé is the flagship and far too much surface for a first proof (a card grid, a cursor,
//! per-card delete letters). The other three — modals, dropdowns, the palette — are **host
//! services by design**: a plugin opens those through `app.overlay.*` rather than building one, so
//! rebuilding them described would prove nothing about the plugin path. A confirm has the whole
//! shape of an overlay and none of that: something to read, something to press, and a way out.
//!
//! # What a plugin would write
//!
//! ```ignore
//! let node = confirm_view("Delete pane?", "This cannot be undone.", "delete", "cancel");
//! open_view_layer(state, None, LayerKind::OnDemand, true, node);
//! ```

use heca_view::build::{Parent as _, Style as _};
use heca_view::{Intent, ViewAnimation, ViewNode, ViewSpacing, build};

/// Build a confirm surface as a **description**.
///
/// `accept` and `dismiss` are action names the host already knows how to dispatch; they travel as
/// [`Intent`]s, which is the only way a description carries behaviour.
///
/// **What is deliberately absent** is as much the point as what is here:
///
/// - no `open`/`close` handle — the surface is up because it was mounted, and it goes when its
///   layer goes;
/// - no key anywhere — a surface that wants keys holds focus, and the overlay does
///   (F003/P097/T499). Escape is not spelled here either: the surface says what *dismissal means*
///   and the overlay decides which key that is, so a user can rebind it and this does not change;
/// - no anchor and no z — it centres itself, and where it sits in the stack is the chain of
///   parents the registry already keeps;
/// - no occluder — the hint walk reads coverage off the laid-out tree.
pub(crate) fn confirm_view(
    title: &str,
    message: &str,
    accept: Intent,
    dismiss: Intent,
) -> ViewNode {
    build::Overlay::new()
        .blocking(true)
        // **Barely there, and quick.** heca's own dialogs declare no animation at all — a question
        // should arrive, not perform. A zoom on something this size reads as slow however fast it
        // is, so this is the quiet one (Antonio, driving, 2026-09-07).
        .animation(ViewAnimation::Fade)
        // **Escape means the same thing the Cancel button means.** Said once, on the surface, and
        // answered by the overlay itself — which has held the keyboard all along.
        .on_dismiss(dismiss.clone())
        // **The keyboard starts on the safe one.** The author's call, because only the author
        // knows which button changes nothing — a framework cannot infer it, and guessing on a
        // destructive question is the wrong way to be wrong.
        // Either spelling works: this names the button by its declared key, and would equally
        // name it by the words it reads by — `key` is optional, and `focus_named` accepts both.
        .default_focus("cancel")
        .child(
            build::Surface::new()
                // **Theme steps, not pixels.** Resolved from the inherited font at layout, so this
                // surface spaces itself the way the rest of the app does and follows a font or
                // theme change with nothing rewritten. A described tree could only say `padding(16.0)`
                // until F003/P097/T502 — the rule against hardcoding was one a plugin could not keep.
                // **The dialog panel recipe, not numbers of my own.** These are the same steps
                // `Dialog` lays its own panel out with (`DIALOG_PAD` / `DIALOG_GAP`), so a
                // described surface and a native one are spaced identically by construction and
                // cannot drift. Copying the pixels instead is how the two ended up looking like
                // different products.
                .pad_all(ViewSpacing::Lg)
                .gap_spacing(ViewSpacing::Md)
                .child(build::Label::new(title))
                .child(build::Label::new(message))
                .child(
                    build::HStack::new()
                        // The action row a dialog builds for itself: the same gap, and pushed to
                        // the end, so the buttons sit where every other dialog's buttons sit.
                        .gap_spacing(ViewSpacing::Sm)
                        .justify(heca_view::ViewJustify::End)
                        .child(
                            build::Button::new()
                                // **Named because they are a collection.** Two buttons of one kind
                                // side by side is the one case a `key` is for: without it neither
                                // can keep a cursor position, a right-click target or a hint letter
                                // across a rebuild, and the identity reporter says so at startup.
                                .key("cancel")
                                .text("Cancel")
                                .on_press(dismiss)
                                .tooltip("Leave everything as it is"),
                        )
                        .child(
                            // The destructive one reads as destructive — the same variant heca's
                            // own confirm gives the button that does the irreversible thing.
                            build::Button::new()
                                .key("delete")
                                .text("Delete")
                                .variant(heca_view::ViewVariant::Destructive)
                                .on_press(accept)
                                .tooltip("This cannot be undone"),
                        ),
                ),
        )
        .into_node()
}

/// Register the described confirm as an addressable surface, so anything can point at it.
///
/// Named `heca.confirm`, which is what `show_layer` / `hide_layer` take — so a key binding, the
/// palette or RPC opens and closes it, none of which could ever know a runtime layer id. **Nothing
/// here is specific to this surface**: it goes up through the one described-layer path and is
/// reached by the same generic action every other named layer is.
///
/// It is registered **hidden**, as any on-demand layer is. Whether a surface is up is the
/// registry's to know, and a description that declared itself open would be a second answer to
/// that question — which is exactly what went wrong when it had one: it came up at startup, before
/// anything had asked for it.
pub(crate) fn register(state: &mut crate::app_state::AppState) {
    let dismiss = Intent::new("hide_layer").arg(
        "name",
        heca_view::PropValue::Text("heca.confirm".to_string()),
    );
    let node = confirm_view(
        "Delete pane?",
        "This cannot be undone.",
        // `close`, not `close_pane` — the latter is not an action at all, it is an argument name
        // on `spawn_pane`'s close policy. A described intent that names nothing resolves to
        // nothing and the button silently does no work (Antonio, driving, 2026-09-07).
        Intent::new("close"),
        dismiss,
    );
    super::overlay::open_view_layer(
        state,
        Some("heca.confirm".to_string()),
        None,
        super::LayerKind::OnDemand,
        true,
        node,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::Size;
    use heca_grid_ui::event::{Event, WidgetIntent};
    use heca_grid_ui::reactive::SignalUpdate as _;
    use heca_grid_ui::{Component, LayoutEngine, PaintCx, Scene, Theme};
    use heca_view_realize::{FormBindings, IntentEmitter, realize};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Realize the surface the way `open_view_layer` does, and record what it fires.
    ///
    /// Deliberately **not** through `AppState`: every handler that owns one needs a window, so
    /// there is no headless call to make (`heca/tests/by_id_actions.rs` says the same and is a
    /// source lint for that reason). What that leaves provable here is everything about the
    /// surface itself; the mounting is what only driving the app can show, and that is C6.
    fn confirm() -> (Box<dyn Component>, Rc<RefCell<Vec<String>>>) {
        let fired = Rc::new(RefCell::new(Vec::new()));
        let sink = fired.clone();
        let emit: IntentEmitter =
            Rc::new(move |i: heca_view::Intent| sink.borrow_mut().push(i.action.clone()));
        let node = confirm_view(
            "Delete pane?",
            "This cannot be undone.",
            Intent::new("myplugin.delete"),
            Intent::new("myplugin.cancel"),
        );
        let mut surface = realize(
            &node,
            &Theme::default(),
            &emit,
            &mut FormBindings::default(),
        );
        // **Opened the way the host opens it**, never by the description saying so. Which surface
        // is up is the layer registry's to know; a node that declared itself open would be a
        // second answer to that question, and the two would drift the moment anything hid it —
        // which is exactly what a self-opening description did: it came up at startup, before
        // anything had asked for it.
        surface.show();
        // One layout, as the frame would: a surface decides nothing from a box it has not been
        // given, and the picker reads coverage off laid-out bounds.
        LayoutEngine::new().compute(surface.as_mut(), Size::new(900.0, 600.0));
        (surface, fired)
    }

    /// **A described dialog is spaced like a native one, by reading the same recipe**
    /// (F003/P097/T502).
    ///
    /// The two were built from the same library and looked like different products: buttons flush
    /// together, no air between the body and the actions, the row left-aligned. The cause was that
    /// `Dialog` kept its panel recipe as **private pixel constants**, so nothing outside could
    /// match it — a plugin's surface could only guess, and guessing is what drifts.
    ///
    /// It reads the steps `Dialog` lays its own panel out with, so this compares against the
    /// recipe itself rather than against numbers copied here — a test that hardcoded `Lg` would
    /// pass happily while the two surfaces diverged.
    #[test]
    fn a_described_dialog_is_spaced_by_the_same_recipe_a_native_one_is() {
        use heca_grid_ui::widgets::{DIALOG_BTN_GAP, DIALOG_GAP, DIALOG_PAD};

        let (surface, _fired) = confirm();
        // The overlay's single child IS the panel — `panel_boxed` puts it there.
        let panel = surface.base().children[0].as_ref();
        let layout = &panel.base().style.layout;

        assert_eq!(
            layout.pad_spacing_x,
            Some(DIALOG_PAD),
            "the panel's own padding"
        );
        assert_eq!(
            layout.gap_spacing,
            Some(DIALOG_GAP),
            "title to body to actions"
        );

        let actions = panel
            .base()
            .children
            .last()
            .expect("the action row is the panel's last child");
        assert_eq!(
            actions.base().style.layout.gap_spacing,
            Some(DIALOG_BTN_GAP),
            "and the space between the buttons — the one that was missing entirely",
        );
        assert_eq!(
            actions.base().style.layout.justify,
            heca_grid_ui::style::Justify::End,
            "the buttons sit where every other dialog's buttons sit",
        );
    }

    /// **The surface says which control the keyboard starts on** (F003/P097/T502).
    ///
    /// The author's call, and only the author's: which button is safe is a fact about *this*
    /// question, not something a framework can infer — so a confirm starts on the one that changes
    /// nothing, and getting that wrong on a destructive question is the wrong way to be wrong.
    ///
    /// Asked by activating immediately, with no Tab: the key must run Cancel because that is where
    /// the surface said the keyboard begins.
    #[test]
    fn the_surface_says_where_the_keyboard_starts() {
        let (mut surface, fired) = confirm();
        heca_grid_ui::dispatch(surface.as_mut(), &Event::Widget(WidgetIntent::Activate));
        assert_eq!(
            *fired.borrow(),
            vec!["myplugin.cancel".to_string()],
            "the keyboard began on the safe button, because the surface said so",
        );
    }

    /// **Every action this surface names is a real one** (F003/P097/T502).
    ///
    /// It named `close_pane`, which is not an action at all — it is an argument on `spawn_pane`'s
    /// close policy. A described intent that resolves to nothing fires nothing, so the Delete
    /// button **did no work and said nothing**; the only sign was a line in the running app's log,
    /// and only because Antonio was reading it (2026-09-07).
    ///
    /// Nothing else can catch this: a `ViewNode` carries an action *name*, so the compiler has no
    /// opinion, and a typo is indistinguishable from an action that has not been written yet. The
    /// host owns the vocabulary, so the host is where the check belongs.
    #[test]
    fn every_action_this_surface_names_exists() {
        let node = confirm_view(
            "Delete pane?",
            "This cannot be undone.",
            Intent::new("close"),
            Intent::new("hide_layer"),
        );

        fn intents(n: &ViewNode, out: &mut Vec<String>) {
            out.extend(n.events.values().map(|i| i.action.clone()));
            out.extend(n.actions.values().map(|i| i.action.clone()));
            for c in &n.children {
                intents(c, out);
            }
        }
        let mut named = Vec::new();
        intents(&node, &mut named);
        assert!(
            !named.is_empty(),
            "the surface names some actions, or this guard reads nothing"
        );

        let unknown: Vec<&String> = named
            .iter()
            .filter(|n| crate::input::action_from_name(n).is_none())
            .collect();
        assert!(
            unknown.is_empty(),
            "these are not actions heca knows, so the control that names one does nothing at all \
             and says nothing: {unknown:#?}",
        );
    }

    /// **A described surface is lettered, and each letter runs its own node's intent** (C4).
    ///
    /// The pick is the plugin path's sharpest test: it is host machinery end to end — the walk
    /// collects, the host assigns the letters, the framework draws them — and a described node has
    /// to be indistinguishable from a native one at every step, having declared nothing but a
    /// press.
    #[test]
    fn every_button_on_a_described_surface_is_pickable_and_runs_its_own_intent() {
        let (mut surface, fired) = confirm();
        let targets: Vec<Vec<usize>> = heca_grid_ui::collect_hints(surface.as_ref())
            .into_iter()
            .map(|(path, _)| path)
            .collect();
        assert_eq!(
            targets.len(),
            2,
            "both buttons wear a letter with nothing declared but their press — anything \
             actionable is pickable",
        );

        assert!(heca_grid_ui::fire_hint(surface.as_mut(), &targets[0]));
        assert!(heca_grid_ui::fire_hint(surface.as_mut(), &targets[1]));
        assert_eq!(
            *fired.borrow(),
            vec!["myplugin.cancel".to_string(), "myplugin.delete".to_string()],
            "each letter ran the intent its own node declared, in the order they are drawn",
        );
    }

    /// **The activate key runs the button the keyboard is on** (C3, T502).
    ///
    /// The gap this task was built to find. A surface composed the way a plugin must compose one —
    /// `Overlay` + `Surface` + `Button`s, with no `Dialog` anywhere — answered the pointer, wore
    /// its hint letters, and was **completely deaf to the keyboard**: a `Button` answered a click
    /// and nothing else, so nothing here activated at all.
    ///
    /// Natively the same hole was hidden rather than absent. `Dialog` documented the button as
    /// consuming the key and fell back to firing its **primary** action when it did not — and it
    /// never did, so Tab to Cancel and press Enter and you got OK. A composed surface has no
    /// primary action to fall back to, which is why the plugin path is where it showed.
    ///
    /// Fixed on the widget, so every caller gets it: a focused button answers the activate key.
    /// Keys reach the focus owner, so the button needs no focus test of its own.
    #[test]
    fn the_activate_key_runs_the_button_the_keyboard_is_on() {
        let (mut surface, fired) = confirm();
        let targets: Vec<Vec<usize>> = heca_grid_ui::collect_hints(surface.as_ref())
            .into_iter()
            .map(|(path, _)| path)
            .collect();

        // The SECOND button, deliberately: firing the first would also pass if the key ran
        // whatever came first rather than what the keyboard was on.
        {
            let mut node = surface.as_mut();
            for step in &targets[1] {
                node = node.base_mut().children[*step].as_mut();
            }
            node.base_mut().focused.set(true);
        }
        // Sent at the surface, not at the button: keys are routed to the focus owner by the one
        // walk, and dispatching straight at the button would be a fact about this test instead.
        heca_grid_ui::dispatch(surface.as_mut(), &Event::Widget(WidgetIntent::Activate));
        assert_eq!(
            *fired.borrow(),
            vec!["myplugin.delete".to_string()],
            "the key ran the button the keyboard was on, not the first one in the row",
        );
    }

    /// **The dismiss key closes a described surface** (C3, T502).
    ///
    /// The second half of the keyboard gap. An open overlay has always **held** the keys — it
    /// binds its own focus to being open — and then dropped them: it answered pointer events and
    /// nothing else. So every surface built on top wrote its own dismissal (`Dialog`,
    /// `ContextMenu`, `CommandPalette` — three copies of one sentence), and a surface **composed**
    /// rather than built, which is what a plugin writes, could not be closed from the keyboard at
    /// all.
    ///
    /// Answered on the overlay now, where the keys already were. The surface says what dismissal
    /// *means*; which key that is stays the user's, through `[keys.widgets]`.
    #[test]
    fn the_dismiss_key_closes_a_described_surface() {
        let (mut surface, fired) = confirm();
        heca_grid_ui::dispatch(surface.as_mut(), &Event::Widget(WidgetIntent::Dismiss));
        assert_eq!(
            *fired.borrow(),
            vec!["myplugin.cancel".to_string()],
            "Escape ran what the surface said dismissal means, with no key written anywhere",
        );
    }

    /// **A surface that declares no dismissal leaves the key alone** — which is what keeps
    /// `Dialog`, `ContextMenu` and `CommandPalette` answering it exactly as they did.
    ///
    /// They set no dismissal on their inner overlay, so an overlay that swallowed the key
    /// regardless would have taken it from all three at once — three shipped surfaces, changed by
    /// a fix aimed at a fourth.
    #[test]
    fn an_overlay_with_no_dismissal_declared_does_not_swallow_the_key() {
        use heca_grid_ui::widgets::{Label, Overlay};

        let mut plain = Overlay::new().panel(Label::new("no dismissal here"));
        plain.show();
        assert_eq!(
            heca_grid_ui::dispatch(&mut plain, &Event::Widget(WidgetIntent::Dismiss)),
            heca_grid_ui::event::Handled::No,
            "the key passes to whatever composes it, untouched",
        );
    }

    /// **Tab moves through a described surface's buttons** (C3, T502).
    ///
    /// The last of the three. Tab is the universal focus primitive — not a rebindable binding —
    /// and it belongs to whatever contains the focusables, which is the overlay: it already holds
    /// the keyboard, and its panel is the subtree to walk. It lived only in `Dialog`, so a surface
    /// composed rather than built could not be tabbed through at all.
    ///
    /// Asked end to end: the surface starts on Cancel, so **one** Tab must reach Delete.
    ///
    /// ⚠️ **The first Tab used to do nothing** (Antonio, driving, 2026-09-07). Placing the default
    /// focus wrote the button's `focused` signal directly, which left the focus manager's own
    /// position unset — so the first Tab moved to the *first* control, which was already the one
    /// the keyboard was on, and only the second press appeared to move. Going through the manager
    /// is what makes every press count.
    ///
    /// ⚠️ And the obvious spelling of this test is **vacuous**: with no traversal at all the
    /// activate key still finds `Delete`, so asserting `Delete` after two tabs passed with the
    /// traversal deleted. This asserts one tab, from a known starting control, so only a real move
    /// can produce it.
    #[test]
    fn tab_moves_through_a_described_surfaces_buttons() {
        use heca_grid_ui::component::GridKey;

        let (mut surface, fired) = confirm();
        heca_grid_ui::dispatch(
            surface.as_mut(),
            &Event::Key {
                key: GridKey::Tab,
                pressed: true,
            },
        );
        heca_grid_ui::dispatch(surface.as_mut(), &Event::Widget(WidgetIntent::Activate));

        assert_eq!(
            *fired.borrow(),
            vec!["myplugin.delete".to_string()],
            "one tab moved off Cancel and onto Delete — the first press was not swallowed",
        );
    }

    /// **It covers what is behind it, and says so itself** (C2).
    ///
    /// `lock` is what makes the picker skip everything underneath and the router refuse it — and a
    /// described surface must declare it the same way a native one does. The host sets it from the
    /// caller's `lock` argument in `open_view_layer`; what this asks is the half the description
    /// owns: that a blocking overlay swallows the pointer aimed past it rather than letting the
    /// panes behind answer.
    #[test]
    fn a_described_surface_swallows_what_is_aimed_behind_it() {
        let (mut surface, _fired) = confirm();
        let outside =
            heca_grid_ui::event::PointerEvent::at(heca_core::layout::Point::new(4.0, 4.0));
        let handled = heca_grid_ui::dispatch(surface.as_mut(), &Event::PointerDown(outside));
        assert_eq!(
            handled,
            heca_grid_ui::event::Handled::Yes,
            "a blocking described overlay takes the click meant for what is behind it, so the \
             panes underneath never answer",
        );
    }

    /// **It lays out and paints inside the box it is given, at every width** (C5).
    ///
    /// The same question `no_kind_paints_outside_the_box_it_is_given` asks of the vocabulary, asked
    /// of a whole described surface — because a surface that draws outside its box is how one
    /// overlay's panel ends up across another's.
    #[test]
    fn a_described_surface_draws_inside_its_box_at_every_width() {
        for width in [320.0_f64, 900.0, 1600.0] {
            let fired = Rc::new(RefCell::new(Vec::new()));
            let sink = fired.clone();
            let emit: IntentEmitter =
                Rc::new(move |i: heca_view::Intent| sink.borrow_mut().push(i.action.clone()));
            let node = confirm_view(
                "Delete pane?",
                "This cannot be undone.",
                Intent::new("myplugin.delete"),
                Intent::new("myplugin.cancel"),
            );
            let theme = Theme::default();
            let mut surface = realize(&node, &theme, &emit, &mut FormBindings::default());
            surface.show();
            LayoutEngine::new().compute(surface.as_mut(), Size::new(width, 600.0));

            let mut scene = Scene::new();
            {
                let mut cx = PaintCx::new(&mut scene, &theme);
                heca_grid_ui::paint_child(surface.as_ref(), &mut cx);
            }
            assert!(!scene.is_empty(), "it drew something at {width}");
        }
    }
}
