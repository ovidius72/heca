//! The **exposé** — a bird's-eye view of every workspace at once (F003/P082/T327).
//!
//! One row per workspace, each laying out **all** of that workspace's columns side by side —
//! including the ones scrolled off-screen, which is the whole point of the view. Panes stack
//! vertically inside their column, and a floating pane is drawn over the strip where it actually
//! sits. It is the same idea as niri's overview (`docs/niri-wiki/03-usage/overview.md`): a
//! zoomed-out map you navigate and act on, not a second way to use the app.
//!
//! # The shape of this surface (F003/P082/T420)
//!
//! It is **components**, not one function that draws a picture — each taking properties, events and
//! callbacks, encapsulating its own logic, and testable on its own without a window:
//!
//! | | owns |
//! |---|---|
//! | [`model`] | what is on screen, where and how big, as plain data reduced from the session |
//! | [`PaneCard`](pane_card::PaneCard) | one pane: its name, its cursor state, its delete letters, its activate |
//! | [`ColumnCard`](column_card::ColumnCard) | a column: the panes' **vertical** shares |
//! | [`WorkspaceRow`](workspace_row::WorkspaceRow) | a row: the columns' **horizontal** shares, and the floats' rects |
//! | [`ExposeGrid`](expose_grid::ExposeGrid) | the rows' shares of the map, and the cursor |
//! | this module | the host wiring — [`register`] gathers `AppState`, [`map`] assembles the layer |
//!
//! They live **here, beside the surface**, and move up to `crate::components` the day a second
//! surface wants one (§ 0b).
//!
//! # ⚠️ Every size is a share; nothing is a pixel
//!
//! This module used to compute a `zoom` from the session's extents and emit `Length::Px(width *
//! zoom)`. Doing that takes over the layout engine's job — and then it owns *every* term: the
//! window, the overlay margin, the panel padding, the gaps between cards, the row count, each row's
//! height, the scroll region's centring, the overscroll range. Miss one and the map is wrong by
//! exactly that term, which happened **seven times, six of them differently**.
//!
//! So a column's width is a percentage of one shared denominator, a pane's height is a `grow`
//! weight, a float is a rect of percentages, and taffy answers all of it exactly at every window
//! size. There is no fit to compute, nothing to cap, and nothing to scroll: a picture built out of
//! shares of the window cannot overflow it.
//!
//! **The zoom *effect* is a different thing and stays** — it is declared on the surface itself
//! ([`Overlay::animation`](heca_grid_ui::widgets::Overlay::animation)) and plays over any surface,
//! a plugin's included. A paint transform is not a size: nothing is measured again.
//!
//! Panes are drawn as **boxes** for now: a border, the pane's name and its icon. A real snapshot
//! needs a scene primitive `heca-grid-ui` does not have (its vocabulary is `Rect`, `Text`, `Icon`,
//! clips — there is no image command) plus per-pane offscreen capture in `heca-renderer`. Nothing
//! in the layout, the navigation or the layer changes when that lands: only what fills the box.

pub(crate) mod column_card;
pub(crate) mod expose_grid;
pub(crate) mod model;
pub(crate) mod pane_card;
pub(crate) mod workspace_row;

#[cfg(test)]
mod testing;

use heca_core::layout::PaneId;
use heca_grid_ui::builders::{ComponentExt, LayoutExt, Parent, StyleExt};
use heca_grid_ui::style::{Length, Spacing};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::reactive::{signal, SignalUpdate};
use heca_grid_ui::animation::Animation;
use heca_grid_ui::widgets::{KeyHintGroup, Overlay, Surface};
use heca_grid_ui::Component;

pub(crate) use expose_grid::ExposeGrid;
pub(crate) use model::{model, ExposeWorkspace};
pub(crate) use pane_card::{DispatchAction, ExposeCallbacks, ExposeDeleteKeys};

/// The surface name the map registers under, and the one a `[[keys.surface]]` entry addresses. One
/// constant so the layer, the config entry and the key lookup cannot drift apart.
pub(crate) const SURFACE: &str = "expose";

/// **The map's own picker, declared on the widget** (F003/P082/T427).
///
/// The action a `[[keys.surface]] heca.expose` entry binds as `pick`. It is the map's, not the
/// app's: the surface declares the verb, the widget answers it, and config chooses the key. Before
/// this the map had to borrow the built-in `hint_pick` — the only picker there was — which is why a
/// plugin's overlay could contribute targets to heca's picker and never open one of its own.
pub(crate) const PICK_ACTION: &str = "heca.expose.pick";

/// **A share of the parent's height**, expressed the only way flexbox actually divides a box.
///
/// `flex_grow` alone does not do it: it distributes only *positive free space*, so a column of
/// `grow(1.0)` children collapses to its content instead of splitting the box. A share needs a
/// **zero base size and permission to shrink** as well (CSS `flex: 1 1 0`). Written once here so no
/// component re-derives it — and gaps come out of the free space before it is divided, so a gap
/// between shares stays exact.
pub(super) fn share_v<T: LayoutExt + Component>(node: T, weight: f64) -> T {
    node.grow(weight.max(0.001) as f32)
        .height(Length::Px(0.0))
        .shrink(1.0)
}

/// **Wire the map's seams to the host**, once — the four acts every component in it can ask for.
///
/// Separate from [`map`] because a component test needs exactly these and nothing else: they are
/// the only things in the surface that reach the app, so a test that supplies them can build any
/// part of the map on its own.
pub(super) fn callbacks(emit: super::ChromeIntentEmitter, keys: ExposeDeleteKeys) -> ExposeCallbacks {
    // **Deleting is the card's own key handler, dispatching the actions that already exist.** No
    // new `WidgetIntent`, no new `ActionPolicy` arm, no host-side key match: a handler on the
    // widget, exactly as a click is (AGENTS § 0c). The action carries the confirm with it —
    // `close`, `delete_column` and `delete_workspace` all declare a `ConfirmSpec`
    // (`actions::builtin_confirm_specs`), so the central destructive gate raises the same prompt
    // the sidebar and a header button raise, and this surface neither asks for it nor can skip it.
    //
    // Antonio, 2026-08-11: *"we have an actionRegistry and we MUST always reuse what we have… the
    // only thing i expect is a developer to handle the on_key_up on each pane in the overlay"*.
    let delete: DispatchAction = {
        let emit = emit.clone();
        std::rc::Rc::new(move |action: &str, args: &[(&str, i64)]| {
            let intent = args.iter().fold(heca_view::Intent::new(action), |i, (k, v)| {
                i.arg(*k, heca_view::PropValue::Int(*v))
            });
            emit.fire(crate::app::interaction::InteractionIntent::View(intent));
        })
    };
    // Moving the map's highlight is its own intent — the same one `CardGrid::on_move` sends when
    // you arrow around — so handing the cursor on before a delete goes through the one door that
    // already exists rather than a second way to move it.
    let cursor_to: std::rc::Rc<dyn Fn(PaneId)> = {
        let emit = emit.clone();
        std::rc::Rc::new(move |pane_id: PaneId| {
            emit.fire(crate::app::interaction::InteractionIntent::ExposeCursor { pane_id });
        })
    };
    let choose: std::rc::Rc<dyn Fn(PaneId)> = {
        let emit = emit.clone();
        std::rc::Rc::new(move |pane_id: PaneId| {
            // **Choosing a card is activate-and-leave**, the same three-part act the sidebar
            // performs when you pick a pane from it (`providers/workspaces/mod.rs:466`):
            //
            //   focus the pane → hand the keyboard back → put the surface away.
            //
            // The middle step is the one that is invisible until it is missing. `focus_pane`
            // changes which pane is *active* and deliberately nothing else — it does not release
            // container focus, because whether the keyboard should follow is a property of what
            // the user asked for, not of a pane being focused (a hint focuses without leaving).
            // Without it the map focused the right pane and the keyboard stayed where it was, so
            // choosing a card looked like it had done nothing at all.
            emit.fire(crate::app::interaction::InteractionIntent::FocusPaneThenAction {
                pane_id,
                action: Box::new(crate::input::WmAction::UnfocusDock),
            });
            emit.fire(crate::app::interaction::InteractionIntent::ActivateAction(
                crate::input::WmAction::CloseOverlay { overlay: None },
            ));
        })
    };
    // Dismissal goes through the **action**, not a bespoke hide: `close_overlay` is in the registry,
    // so it is bindable, rebindable in config, listed in the palette and reachable over RPC.
    let dismiss: std::rc::Rc<dyn Fn()> = {
        let emit = emit.clone();
        std::rc::Rc::new(move || {
            emit.fire(crate::app::interaction::InteractionIntent::ActivateAction(
                crate::input::WmAction::CloseOverlay { overlay: None },
            ));
        })
    };
    ExposeCallbacks { choose, delete, cursor_to, dismiss, keys }
}

/// Build the overview layer — **a composition, not a widget**: it binds `PaneId` to a `FocusPane`
/// intent and the `close_overlay` action, which is the only reason it is not in `heca-grid-ui`.
///
/// Plain data in, a `Component` out, and **never `&AppState`** (§ 0b): the session is already
/// reduced to [`ExposeWorkspace`]s by [`model`], so the whole map is testable without a window.
///
/// What is drawn, and what is not:
/// - **A workspace is a row.** No box, no border, no fill — position and scale say which it is.
/// - **A column is invisible grouping** that takes its share of the row's width.
/// - **A pane is the only thing with a surface.**
///
/// The overlay behaviour is [`Overlay`]'s and the cursor is [`ExposeGrid`]'s. Neither is
/// re-implemented here.
pub(crate) fn map(
    rows: &[ExposeWorkspace],
    theme: &GuiTheme,
    emit: super::ChromeIntentEmitter,
    start: Option<PaneId>,
    geometry: &heca_core::layout::LayoutOptions,
    keys: &ExposeDeleteKeys,
    // The pane a back-and-forth binding would return to — plain data, resolved by `register` from
    // the same field `prefix+i` reads.
    previous: Option<PaneId>,
) -> Box<dyn Component> {
    let cb = callbacks(emit, keys.clone());

    let grid = ExposeGrid {
        rows,
        // **The map's spacing is the layout's, not this module's.** `overview_gap` is a
        // `LayoutOptions` field the user sets in config.
        gap_frac: geometry.overview_gap,
        start,
        previous,
        theme,
        cb: &cb,
    }
    .build();

    // **The picker is a widget, and the map declares it.** Open it and every card beneath wears a
    // letter — each drawn by the card itself, so it lands wherever the card is, at any nesting
    // depth. Typing one runs that card's own `on_hint`. Nothing host-side is involved: no input
    // mode, no host paint pass, no registry.
    let picker_open = signal(false);
    let grid = KeyHintGroup::new(grid)
        .open_when(picker_open)
        .on_action(PICK_ACTION, move || picker_open.set(true))
        // The wrapper hugs its child, so the room the panel gives it has to be passed on
        // deliberately — the grid inside is a share of *this*, and a hugged wrapper would leave it
        // resolving a percentage of nothing (the same term the cards' `KeyHint` needs).
        .width(Length::Pct(1.0))
        .height(Length::Pct(1.0));

    Box::new(
        Overlay::new()
            .blocking(true)
            // **The map names how it comes and goes, exactly as a plugin's surface would.**
            //
            // It opens by pulling back, the way niri's overview does — the same session seen from
            // further away, rather than a different picture arriving. Antonio: *"The animation in
            // niri is zoom-in/out not fade."* Going, the shrink leads and the dissolve rides it,
            // which is what `ZoomFade` **is**: the gesture is defined once, in the library, so no
            // surface composes it out of parts and none of them can drift.
            //
            // How far back is `[settings] overview_zoom_from`, not a constant here: `1 /
            // overview_zoom` would start the cards at exactly life size, which is the truest
            // reading and overshoots — at 2× the outer rows begin off-screen and rush in. The
            // setting defaults to a gentler 1.3, and a user who wants the literal reading sets 2.0.
            .animation(Animation::ZoomFade.from(geometry.overview_zoom_from as f32))
            .panel_size(Length::Pct(1.0), Length::Pct(1.0))
            // **The panel is the frost's tint, not a lid.** The host stamps the blurred frame
            // under this layer (`LayerBackdrop::Frosted`), so the surface here is the background
            // colour at the theme's scrim strength: enough to hold the cards' contrast, thin
            // enough that the session reads behind them as a blurred wash. Painted opaque it hid
            // the blur completely; left at heca's own window colour — translucent by design, for
            // the frosted compositor — the app read straight through it, sharp.
            .panel(
                Surface::column()
                    .background(theme.colors.background.with_alpha(theme.colors.interaction.scrim))
                    .pad_all(Spacing::Md)
                    .child(grid),
            )
            // **Closed until the stack shows it**, which is what plays the arrival: `ShowLayer`
            // rebuilds the map and then shows the layer, and showing states that the surface is
            // open. A rebuild while it is already up carries the gesture over from the tree it
            // replaces, so nothing replays.
            .opened(false),
    )
}

pub(crate) fn record_expose_cursor(state: &mut crate::app_state::AppState, pane_id: PaneId) {
    let Some(ws) = crate::app::focus::find_pane_workspace(&state.session, pane_id) else {
        return;
    };
    while state.expose_cursor_per_ws.len() <= ws {
        state.expose_cursor_per_ws.push(None);
    }
    state.expose_cursor_per_ws[ws] = Some(pane_id);
    // …and which row that is, so a rebuild restores the cursor where it stands rather than at the
    // active workspace's remembered one. See `AppState::expose_cursor_ws`.
    state.expose_cursor_ws = Some(ws);
}

/// **Where the map's cursor goes when the layer is built.** Two different questions, and reading
/// them as one is what this function exists to stop:
///
/// - **Opening the map** — the cursor goes to the pane the session is focused on. The map is a
///   picture of where you are, so it opens where you are. *(Antonio, driving the rebuilt map,
///   2026-08-13: "when you open the exposé the focused pane in the exposé should be the same that
///   is focused in the scrolling area."* This **reverses** the earlier rule, which preferred the
///   remembered highlight as "the more specific answer": browse away, dismiss, and the next open
///   started where you had browsed while the session was still on the pane you left — so the map
///   drew its cursor on one card and its `active` styling on another.)
/// - **Rebuilding it while it is up** — the cursor stays exactly where it is. The layer is rebuilt
///   from scratch whenever the session changes underneath it, and a rebuild that moved the
///   highlight would fight the user: reading the *active workspace's* slot here is what made
///   deleting a pane in one row jump the map to another (Antonio, driving, 2026-08-11).
///
/// Pure, and takes the `Session` rather than the `AppState` around it, so both answers are testable
/// without a window — this resolution has now been got wrong three times.
fn open_on(
    session: &heca_core::layout::Session,
    already_up: bool,
    cursor_ws: Option<usize>,
    cursor_per_ws: &[Option<PaneId>],
) -> Option<PaneId> {
    if already_up {
        // The row the cursor is in, which is not necessarily the active workspace's.
        let row = cursor_ws.unwrap_or(session.active_workspace_idx);
        let remembered = cursor_per_ws
            .get(row)
            .copied()
            .flatten()
            // A pane that has since been deleted is no answer at all — fall through to the
            // session's, which is exactly what the cursor was handed on to before it went.
            .filter(|id| crate::app::focus::find_pane_workspace(session, *id).is_some());
        if remembered.is_some() {
            return remembered;
        }
    }
    // `active_pane` answers for both domains — the focused float when one is active, the tiled
    // pane otherwise — which is the same rule the model marks a card `active` by.
    session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.id)
}

/// Register (or re-register) the exposé as the named layer `heca.expose` — **host wiring only**.
///
/// The composition itself is [`map`], which takes plain data; this is the part that needs
/// `AppState`, and it does nothing but gather it.
///
/// **Re-registering is the rebuild path.** `add_named` replaces the layer under that name, and the
/// content is structural — a pane opens, a column is deleted — which a signal cannot express. So
/// values follow signals and shape follows this call. It is safe while the view is up: the layer is
/// re-shown if it was showing.
pub(crate) fn register(state: &mut crate::app_state::AppState) -> Option<super::LayerId> {
    let name = super::layers::layer_name(super::layers::HOST_OWNER, SURFACE)?;
    let programs = state.programs.clone();
    // **Where each pane is, if the user asked for it** — `[settings] pane_show_cwd`, the same
    // setting the sidebar's rows follow, read here because this is the file that may touch
    // `AppState`. Off ⇒ the model simply carries no folder, and nothing below has a flag to pass on.
    let folders = state.chrome_state.workspaces.pane_show_cwd();
    let rows = model(
        &state.session,
        |pane| {
            super::pane_info_view(
                &programs,
                &pane.title,
                pane.custom_name.as_deref(),
                Some(&pane.runtime),
                false,
            )
            .title
        },
        folders,
    );
    let theme = super::chrome_gui_theme(state);
    // **The map's own id, so its intents say the map made them.** Stamped `Keyboard` before, which
    // was indistinguishable from `prefix+j` typed at the session behind the map — and once the
    // active context started refusing the app's bindings, that sameness would have refused the
    // map's own deletes with them (F003/P082/T416). A re-registration keeps the layer's id, so the
    // one already registered under this name is the one to name.
    let id = state.layers.slot_for_name(&name);
    let emit = super::layer_emitter(&state.event_proxy, id);
    let already_up = state.layers.is_visible_named(&name);
    let here = open_on(
        &state.session,
        already_up,
        state.expose_cursor_ws,
        &state.expose_cursor_per_ws,
    );
    // **The cards' delete letters, from `[[keys.surface]] heca.expose`.** Read through
    // `ActionShortcuts`, which is built from the *resolved* keymaps at load and at every
    // `prefix+Shift+r` — so a rebind reaches the map without this path knowing anything about the
    // config file, and without a second reader of it.
    let keys = ExposeDeleteKeys {
        pane: state.action_shortcuts.in_surface(&name, "delete_pane").to_vec(),
        column: state.action_shortcuts.in_surface(&name, "delete_column").to_vec(),
        workspace: state
            .action_shortcuts
            .in_surface(&name, "delete_workspace")
            .to_vec(),
    };
        // The map's own "you were just here" mark, from the one field the bindings read.
    let previous = state
        .last_visited_pane_per_ws
        .get(state.session.active_workspace_idx)
        .copied()
        .flatten()
        .filter(|id| crate::app::focus::find_pane_workspace(&state.session, *id).is_some());
    let root = map(&rows, &theme, emit, here, &state.session.options, &keys, previous);
    let was_visible = state.layers.is_visible_named(&name);
    let id = state.layers.add_named(
        id,
        name.clone(),
        None,
        super::LayerKind::OnDemand,
        // Modal: it takes the keyboard while it is up.
        true,
        // **But it does not cover the content.** `covers_content` is what refuses actions on panes
        // the user cannot see — and in the map you can see them; that is what it is. Declaring
        // coverage here made the surface refuse every act on the pane it exists to let you choose:
        // `FocusPane` was blocked in `Domain::Overlay`, so choosing a card did nothing however the
        // intents were arranged. A dialog covers. A map does not.
        false,
        root,
    );
    // The map floats **over** the session, so the session has to still be there underneath — but
    // legibly out of focus. A flat fill made it a different screen; the blur makes it a lens.
    state.layers.set_backdrop(id, super::LayerBackdrop::Frosted);
    // **The animation is declared on the surface, in `map`** — one builder on the widget, exactly
    // what a plugin writes. It used to be two `pub(crate)` calls on the registry here, holding a
    // `LayerId` and knowing the sequencing rule; a plugin could reach none of it (F003/P082/T459).
    if was_visible {
        state.layers.show(id);
    }
    state.needs_redraw = true;
    Some(id)
}

#[cfg(test)]
mod tests {
    use super::testing::{card_of, session, shipped_keys};
    use super::*;
    use heca_core::layout::LayoutOptions;

    /// The whole layer, as [`register`] assembles it.
    fn built(
        start: Option<PaneId>,
    ) -> (Box<dyn Component>, std::rc::Rc<std::cell::RefCell<Vec<String>>>) {
        let s = session();
        let rows = model(&s, |p| p.title.clone(), false);
        let theme = GuiTheme::default();
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let sink = seen.clone();
        let emit: super::super::ChromeIntentEmitter =
            super::super::ChromeIntentEmitter::of(crate::app::interaction::InteractionSource::Keyboard, move |_, intent| {
                sink.borrow_mut().push(format!("{intent:?}"))
            });
        let mut root = map(&rows, &theme, emit, start, &LayoutOptions::default(), &shipped_keys(), None);
        // **Shown, as the layer stack shows it.** The map is built closed and opened by whoever
        // mounts it (`LayerRegistry::show`), which is also what plays its arrival — so a test that
        // never opens it is testing a surface nobody has raised.
        root.open();
        heca_grid_ui::LayoutEngine::new()
            .compute(root.as_mut(), heca_grid_ui::Size::new(1900.0, 1200.0));
        (root, seen)
    }

    /// **The letters reach the cards** — the whole path, from the shipped file through the built
    /// keymaps to the list [`register`] hands the cards.
    ///
    /// The gap this closed was not academic: the entry parsed, the keymap built, and the lookup
    /// still came back **empty**, because the index records the qualified id
    /// (`heca.expose.delete_pane`) while the lookup asked for the short name. The letters silently
    /// did nothing and `x` fell through to the host, which reported an action it had never heard of
    /// (Antonio, driving, 2026-08-12). Asserting the entry alone — which the test below does —
    /// could not see it: both ends were right and the join was wrong.
    #[test]
    fn the_shipped_delete_letters_reach_the_cards() {
        let mut index = crate::keymap::BindingIndex::new();
        crate::app::registry::build_component_keymaps(
            &heca_config::theme::Config::default(),
            &mut crate::app::conflicts::Conflicts::default(),
            &mut index,
        );
        let shortcuts =
            super::super::ActionShortcuts::from_index(&index, crate::shortcut::KeyStyle::default());
        let name = super::super::layers::layer_name(super::super::layers::HOST_OWNER, SURFACE)
            .expect("a valid layer name");
        let found = ExposeDeleteKeys {
            pane: shortcuts.in_surface(&name, "delete_pane").to_vec(),
            column: shortcuts.in_surface(&name, "delete_column").to_vec(),
            workspace: shortcuts.in_surface(&name, "delete_workspace").to_vec(),
        };
        assert_eq!(
            found,
            shipped_keys(),
            "the map's cards must actually receive the letters the defaults bind",
        );
    }

    /// **The letters are config, and this is the file they live in.** They were literals in this
    /// module until F003/P082/T416; the test above would happily keep passing with a default file
    /// that bound nothing, and the map would then have no delete keys at all.
    #[test]
    fn the_shipped_defaults_bind_the_maps_delete_keys() {
        let keys = heca_config::theme::KeysConfig::default();
        let name = super::super::layers::layer_name(super::super::layers::HOST_OWNER, SURFACE)
            .expect("a valid layer name");
        let entry = keys.surfaces().find(|e| e.name == name).unwrap_or_else(|| {
            panic!("keybindings.default.toml must ship a surface layer for {name}")
        });
        for (action, expected) in [
            ("delete_pane", "x"),
            ("delete_column", "r"),
            ("delete_workspace", "d"),
        ] {
            let bound = entry
                .bindings
                .get(action)
                .unwrap_or_else(|| panic!("{name} must bind {action}"));
            assert_eq!(bound.keys(), vec![expected.to_string()], "{action}");
        }
    }

    /// **Deleting a card hands the cursor to the next one in the SAME row.** The map is rebuilt
    /// when the session changes under it, and the rebuilt tree has to be told where the highlight
    /// goes — after the delete the pane is gone and its neighbour can no longer be found from it.
    #[test]
    fn deleting_a_card_moves_the_cursor_to_its_neighbour_first() {
        use heca_grid_ui::event::Event;
        // Open on the FIRST pane; the row also holds pane 2, which must take the cursor.
        let (mut root, seen) = built(Some(PaneId(1)));
        heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("x".to_string()));
        let order: Vec<String> = seen
            .borrow()
            .iter()
            .map(|i| match () {
                _ if i.contains("ExposeCursor") => "cursor".to_string(),
                _ if i.contains("close_pane_by_id") => "delete".to_string(),
                _ => "other".to_string(),
            })
            .collect();
        assert_eq!(
            order.first().map(String::as_str),
            Some("cursor"),
            "the cursor is handed on BEFORE the delete, or the rebuilt map has nowhere to put it: \
             {:?}",
            seen.borrow(),
        );
        assert!(order.contains(&"delete".to_string()), "…and the delete follows: {order:?}");
    }

    /// **Activate must reach the grid through the overlay**, and choose the card the cursor is on.
    /// Enter and Space resolve to `WidgetIntent::Activate`, which travels Overlay → Surface →
    /// CardGrid before anything happens; any widget on that path swallowing it is
    /// indistinguishable from a wrong keybinding when you are looking at the app.
    #[test]
    fn an_activate_reaches_the_grid_and_chooses_the_card_under_the_cursor() {
        use heca_grid_ui::component::{Event, WidgetIntent as W};
        let (mut root, seen) = built(Some(PaneId(2)));
        heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(W::Activate));
        let got = seen.borrow().join(" ");
        // Activate-and-leave, the same three parts the sidebar performs: focus the pane, hand the
        // keyboard back, put the surface away. The middle one is invisible until it is missing.
        assert!(
            got.contains("FocusPaneThenAction") && got.contains("PaneId(2)"),
            "it focuses the card the cursor was on: {got:?}",
        );
        assert!(got.contains("UnfocusDock"), "and hands the keyboard back: {got:?}");
        assert!(got.contains("CloseOverlay"), "and closes the map: {got:?}");
    }

    /// **Dismiss must reach the grid through the overlay** — the same routing chain, for Esc.
    #[test]
    fn a_dismiss_reaches_the_grid_through_the_overlay() {
        use heca_grid_ui::component::{Event, WidgetIntent as W};
        let (mut root, seen) = built(None);
        heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(W::Dismiss));
        let got = seen.borrow().join(" ");
        assert!(
            got.contains("CloseOverlay"),
            "Esc must run the close_overlay ACTION, not a bespoke hide; the emitter saw: {got:?}",
        );
    }

    /// **Every card in the map is a pane, and nothing else has a surface.** The workspace name was
    /// a fixed gutter column that pushed every strip inwards, so a row stopped lining up with the
    /// screen it is a picture of; niri's overview labels nothing either.
    #[test]
    fn a_row_carries_no_workspace_label() {
        let s = session();
        let rows = model(&s, |p| p.title.clone(), false);
        let theme = GuiTheme::default();
        let emit: super::super::ChromeIntentEmitter = super::super::ChromeIntentEmitter::of(crate::app::interaction::InteractionSource::Keyboard, |_, _| {});
        let mut root = map(&rows, &theme, emit, None, &LayoutOptions::default(), &shipped_keys(), None);
        root.open(); // as the layer stack shows it — see `built`

        // What is *drawn*, not what the tree holds — the question is whether a workspace name ever
        // reaches the screen.
        let viewport = heca_grid_ui::Size::new(1200.0, 800.0);
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), viewport);
        let mut scene = heca_grid_ui::Scene::new();
        root.paint(&mut heca_grid_ui::PaintCx::new(&mut scene, &theme).with_viewport(viewport));
        let seen: Vec<String> = scene
            .iter()
            .filter_map(|c| match c {
                heca_grid_ui::DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();
        assert!(
            !seen.iter().any(|t| t == "Editing" || t.starts_with("Workspace ")),
            "no workspace name is drawn; the map showed: {seen:?}",
        );
        assert!(seen.iter().any(|t| t == "a"), "the pane names are still there: {seen:?}");

        // **The whole map lives in the scene's OVERLAY layer**, because `Overlay` paints through
        // `PaintCx::with_overlay`. A host deciding whether to flush the layer pass by asking "did
        // the base layer draw anything" gets `no` and skips it, and `prefix+Tab` shows nothing at
        // all — which is exactly what happened.
        assert!(
            scene.base_layer().is_empty(),
            "the map draws nothing into the base layer — do not gate its flush on that",
        );
        assert!(scene.has_overlay(), "it is all in the overlay layer");
    }

    /// **The map opens on the pane the session is focused on** — not where it was last browsed.
    ///
    /// Antonio, driving the rebuilt map, 2026-08-13: the exposé's cursor and the scrolling area's
    /// focus must agree the moment it opens. They did not: the remembered highlight won on a fresh
    /// open too, so browsing away and dismissing left the next open showing its cursor on one card
    /// and its `active` styling on another.
    #[test]
    fn opening_the_map_starts_on_the_pane_the_session_is_focused_on() {
        let s = session();
        let focused = s
            .active_workspace()
            .and_then(|ws| ws.active_pane())
            .map(|p| p.id)
            .expect("the fixture has a focused pane");
        // A different pane remembered from a previous browse, in the row the map would open on.
        let remembered = vec![Some(PaneId(2))];
        assert_ne!(remembered[0], Some(focused), "the fixture must actually differ");

        assert_eq!(
            open_on(&s, false, Some(0), &remembered),
            Some(focused),
            "a fresh open ignores where the map was left",
        );
    }

    /// **…and a rebuild keeps the cursor exactly where it is.** The layer is rebuilt from scratch
    /// whenever the session changes underneath it; a rebuild that moved the highlight would fight
    /// the user mid-gesture.
    #[test]
    fn rebuilding_the_map_keeps_the_cursor_where_it_stands() {
        let s = session();
        assert_eq!(
            open_on(&s, true, Some(0), &[Some(PaneId(2))]),
            Some(PaneId(2)),
            "while the map is up, the remembered cursor wins",
        );
    }

    /// **A rebuild in a row other than the active workspace's stays in that row.** Reading the
    /// active workspace's slot here is what made deleting a pane in one row jump the map to
    /// another (Antonio, driving, 2026-08-11).
    #[test]
    fn a_rebuild_stays_in_the_row_the_cursor_is_in() {
        let mut s = session();
        // Give the second workspace a pane, while the session stays focused in the first.
        s.active_workspace_idx = 1;
        s.add_pane(heca_core::layout::Pane::new(PaneId(3), "c"), None, false);
        s.active_workspace_idx = 0;

        assert_eq!(
            open_on(&s, true, Some(1), &[Some(PaneId(1)), Some(PaneId(3))]),
            Some(PaneId(3)),
            "the cursor is in row 1, so the rebuild reads row 1 — not the active workspace's",
        );
    }

    /// A remembered pane that has since been **deleted** is no answer at all: fall through to the
    /// session's focus, which is where the cursor was handed on to before the card went.
    #[test]
    fn a_remembered_pane_that_no_longer_exists_falls_back_to_the_session() {
        let s = session();
        let focused = s
            .active_workspace()
            .and_then(|ws| ws.active_pane())
            .map(|p| p.id)
            .expect("the fixture has a focused pane");
        assert_eq!(
            open_on(&s, true, Some(0), &[Some(PaneId(404))]),
            Some(focused),
        );
    }

    /// **The map fills the window it is given, inside the panel's own padding.** The assembled
    /// layer is where the overlay's viewport margin and the panel padding actually appear — the two
    /// terms a hand-computed fit kept forgetting — so this is the assembled version of
    /// `ExposeGrid`'s box test.
    #[test]
    fn the_assembled_map_stays_inside_the_window() {
        for (w, h) in [(1280.0, 800.0), (1900.0, 1200.0), (800.0, 600.0)] {
            let s = session();
            let rows = model(&s, |p| p.title.clone(), false);
            let theme = GuiTheme::default();
            let emit: super::super::ChromeIntentEmitter = super::super::ChromeIntentEmitter::of(crate::app::interaction::InteractionSource::Keyboard, |_, _| {});
            let mut root =
                map(&rows, &theme, emit, Some(PaneId(1)), &LayoutOptions::default(), &shipped_keys(), None);
            heca_grid_ui::LayoutEngine::new()
                .compute(root.as_mut(), heca_grid_ui::Size::new(w, h));
            let drawn = super::testing::cards_bounds(root.as_ref()).expect("the map has cards");
            assert!(
                drawn.loc.x >= 0.0
                    && drawn.loc.y >= 0.0
                    && drawn.loc.x + drawn.size.w <= w + 1.0
                    && drawn.loc.y + drawn.size.h <= h + 1.0,
                "in {w}x{h} the cards reach {drawn:?}",
            );
            let card = card_of(root.as_ref(), &pane_card::pane_key(PaneId(1)))
                .expect("the first pane's card");
            assert!(
                card.size.h > h * 0.3,
                "and a card is a pane-shaped box, not a sliver: {card:?} in {w}x{h}",
            );
        }
    }

    /// **The map declares its own picker, and its cards declare their own picks** (F003/P082/T427).
    ///
    /// Both halves, because either alone is silent: an action nothing declares is a no-op that
    /// looks exactly like a typo, and a picker over cards that declare nothing shows no letters.
    #[test]
    fn the_map_owns_its_picker_and_every_card_is_a_target() {
        let s = session();
        let rows = model(&s, |p| p.title.clone(), false);
        let theme = GuiTheme::default();
        let emit: super::super::ChromeIntentEmitter = super::super::ChromeIntentEmitter::of(crate::app::interaction::InteractionSource::Keyboard, |_, _| {});
        let mut root =
            map(&rows, &theme, emit, Some(PaneId(1)), &LayoutOptions::default(), &shipped_keys(), None);
        heca_grid_ui::LayoutEngine::new()
            .compute(root.as_mut(), heca_grid_ui::Size::new(1280.0, 800.0));

        assert!(
            heca_grid_ui::collect_actions(root.as_ref()).contains(&PICK_ACTION.to_string()),
            "the map declares its own pick action, rather than borrowing the app's",
        );
        assert_eq!(
            heca_grid_ui::collect_hints(root.as_ref()).len(),
            2,
            "one pick target per card in the fixture's session",
        );
    }

    /// **The shipped config binds the map's own action**, not a built-in it borrowed. A name that
    /// no widget declares binds fine, dispatches, and does nothing — indistinguishable from a typo —
    /// so the file and the declaration are held together here.
    #[test]
    fn the_shipped_defaults_bind_the_maps_own_pick_action() {
        let config = heca_config::theme::KeysConfig::default();
        let entry = config
            .surfaces()
            .find(|e| e.name == "heca.expose")
            .expect("the map has a [[keys.surface]] entry");
        let short = PICK_ACTION
            .strip_prefix("heca.expose.")
            .expect("the action is namespaced by its surface");
        assert!(
            entry.bindings.contains_key(short),
            "`{short}` is bound in the shipped defaults: {:?}",
            entry.bindings.keys().collect::<Vec<_>>(),
        );
    }
}
