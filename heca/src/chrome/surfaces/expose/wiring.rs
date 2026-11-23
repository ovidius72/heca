//! **The map's host side** — the only part of the exposé that touches `AppState` or the app's
//! intents. [`register`] gathers the session into plain data and hands it to [`map`]; [`callbacks`]
//! turns the four acts a card can ask for into the intents and actions that already exist.

use heca_core::layout::PaneId;

use super::{DispatchAction, ExposeCallbacks, ExposeDeleteKeys, SURFACE, map, model};

/// **Wire the map's seams to the host**, once — the four acts every component in it can ask for.
///
/// Separate from [`map`] because a component test needs exactly these and nothing else: they are
/// the only things in the surface that reach the app, so a test that supplies them can build any
/// part of the map on its own.
pub(super) fn callbacks(keys: ExposeDeleteKeys) -> ExposeCallbacks {
    use crate::chrome::{Intent, PropValue};
    use heca_grid_ui::intent::send;
    // Each act names its action and `send`s it: the map is a layer, and the layer registry's seat
    // listens for it, so every action goes out as the map's own gesture with nothing handed in.
    //
    // **Deleting is the card's own key handler, dispatching the actions that already exist.** No
    // new `WidgetIntent`, no new `ActionPolicy` arm, no host-side key match: a handler on the
    // widget, exactly as a click is (AGENTS § 0c). The action carries the confirm with it —
    // `close`, `delete_column` and `delete_workspace` all declare a `ConfirmSpec`
    // (`actions::builtin_confirm_specs`), so the central destructive gate raises the same prompt
    // the sidebar and a header button raise, and this surface neither asks for it nor can skip it.
    let delete: DispatchAction = std::rc::Rc::new(|action: &str, args: &[(&str, i64)]| {
        send(Intent::new(action).args(args.iter().map(|(k, v)| (*k, PropValue::Int(*v)))));
    });
    // Moving the map's highlight is its own action — what an arrow key on the map does — so
    // handing the cursor on before a delete goes through the one door that already exists.
    let cursor_to: std::rc::Rc<dyn Fn(PaneId)> = std::rc::Rc::new(|pane_id: PaneId| {
        send(Intent::new(CURSOR_ACTION).arg("pane_id", PropValue::Int(pane_id.0 as i64)));
    });
    let choose: std::rc::Rc<dyn Fn(PaneId)> = std::rc::Rc::new(|pane_id: PaneId| {
        // **Choosing a card is activate-and-leave**, the same three-part act the sidebar
        // performs when you pick a pane from it:
        //
        //   focus the pane → hand the keyboard back → put the surface away.
        //
        // The middle step is the one that is invisible until it is missing. `focus_pane`
        // changes which pane is *active* and deliberately nothing else — it does not release
        // container focus, because whether the keyboard should follow is a property of what
        // the user asked for, not of a pane being focused (a hint focuses without leaving).
        // Without it the map focused the right pane and the keyboard stayed where it was, so
        // choosing a card looked like it had done nothing at all.
        send(Intent::new("focus_pane").arg("pane_id", PropValue::Int(pane_id.0 as i64)));
        send(Intent::new("unfocus_dock"));
        send(Intent::new("close_overlay"));
    });
    // Dismissal goes through the **action**, not a bespoke hide: `close_overlay` is in the registry,
    // so it is bindable, rebindable in config, listed in the palette and reachable over RPC.
    let dismiss: std::rc::Rc<dyn Fn()> = std::rc::Rc::new(|| send(Intent::new("close_overlay")));
    ExposeCallbacks {
        choose,
        delete,
        cursor_to,
        dismiss,
        keys,
    }
}

fn record_expose_cursor(state: &mut crate::app_state::AppState, pane_id: PaneId) {
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
///   picture of where you are, so it opens where you are. Preferring the remembered highlight
///   instead would open the map where you last browsed while the session is on another pane, so
///   its cursor and its `active` styling would sit on different cards.
/// - **Rebuilding it while it is up** — the cursor stays exactly where it is. The layer is rebuilt
///   from scratch whenever the session changes underneath it, and a rebuild that moved the
///   highlight would fight the user: reading the *active workspace's* slot here is what made
///   deleting a pane in one row jump the map to another.
///
/// Pure, and takes the `Layout` rather than the `AppState` around it, so both answers are testable
/// without a window — this resolution has now been got wrong three times.
pub(super) fn open_on(
    layout: heca_core::layout::Layout<'_>,
    already_up: bool,
    cursor_ws: Option<usize>,
    cursor_per_ws: &[Option<PaneId>],
) -> Option<PaneId> {
    if already_up {
        // The row the cursor is in, which is not necessarily the active workspace's.
        let row = cursor_ws.unwrap_or(layout.active_workspace_idx());
        let remembered = cursor_per_ws
            .get(row)
            .copied()
            .flatten()
            // A pane that has since been deleted is no answer at all — fall through to the
            // session's, which is exactly what the cursor was handed on to before it went.
            .filter(|id| crate::app::focus::find_pane_workspace(layout.session(), *id).is_some());
        if remembered.is_some() {
            return remembered;
        }
    }
    // `active_pane` answers for both domains — the focused float when one is active, the tiled
    // pane otherwise — which is the same rule the model marks a card `active` by.
    layout
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
pub(crate) fn register(state: &mut crate::app_state::AppState) -> Option<crate::chrome::LayerId> {
    let name = crate::chrome::surfaces::layers::layer_name(
        crate::chrome::surfaces::layers::HOST_OWNER,
        SURFACE,
    )?;
    let programs = state.client.programs.clone();
    // **Where each pane is, if the user asked for it** — `[appearance.expose] show_cwd`, read here
    // because this is the file that may touch `AppState`. Off ⇒ the model simply carries no folder,
    // and nothing below has a flag to pass on.
    let folders = state.settings.expose_show_cwd;
    let add_process_name = state.settings.pane_renamed_add_process_name;
    let rows = model(
        state.layout(),
        |pane| {
            crate::chrome::pane_info_view(
                &programs,
                &pane.title,
                pane.custom_name.as_deref(),
                state.chrome_state.panes.presentation(pane.id).as_ref(),
                add_process_name,
            )
            .label()
        },
        folders,
    );
    let already_up = state.layers.is_visible_named(&state.window_root, &name);
    let here = open_on(
        state.layout(),
        already_up,
        state.expose_cursor_ws,
        &state.expose_cursor_per_ws,
    );
    // **The cards' delete letters, from `[[keys.surface]] heca.expose`.** Read through
    // `ActionShortcuts`, which is built from the *resolved* keymaps at load and at every
    // `prefix+Shift+r` — so a rebind reaches the map without this path knowing anything about the
    // config file, and without a second reader of it.
    let keys = ExposeDeleteKeys {
        pane: state
            .action_shortcuts
            .in_surface(&name, "delete_pane")
            .to_vec(),
        column: state
            .action_shortcuts
            .in_surface(&name, "delete_column")
            .to_vec(),
        workspace: state
            .action_shortcuts
            .in_surface(&name, "delete_workspace")
            .to_vec(),
    };
    // The map's own "you were just here" mark, from the one field the bindings read.
    let previous = state
        .last_visited_pane_per_ws
        .get(state.layout().active_workspace_idx())
        .copied()
        .flatten()
        .filter(|id| crate::app::focus::find_pane_workspace(&state.session, *id).is_some());
    // **The map's own sink, so its intents say the map made them** — `register_named_layer` hands it
    // over, stamped with this layer's id. Stamped `Keyboard` before, which was indistinguishable from
    // `prefix+j` typed at the session behind the map, and once the active context started refusing
    // the app's bindings that sameness would have refused the map's own deletes with them.
    //
    // **The map says what it is, on itself.** It takes the keyboard while it is up, and it does
    // **not** cover the content: `lock` is what refuses actions on panes the user cannot
    // see — and in the map you can see them; that is what it is. Declaring coverage made the
    // surface refuse every act on the pane it exists to let you choose (`FocusPane` blocked in
    // `Domain::Overlay`), however the intents were arranged. A dialog covers. A map does not.
    //
    // The frost and the animation are declared on the surface too, in `map` — one builder on the
    // widget, exactly what a plugin writes.
    let options = state.session.options.clone();
    // The layer's seat listens for what the map sends (`LayerRegistry::seat`), so the map itself
    // takes no emitter.
    Some(crate::chrome::register_named_layer(state, &name, |_| {
        let mut root = map(&rows, here, &options, &keys, previous);
        root.base_mut().lock = false;
        root
    }))
}

/// The action that moves the map's cursor onto a pane — what an arrow key on the map and handing
/// the cursor on before a delete both do.
pub(crate) const CURSOR_ACTION: &str = "expose_cursor_to";

/// **Register `expose_cursor_to`**, so the map's cursor is moved by name: from a card's own widget
/// (`ev.dispatch`), a script, a plugin. It was an internal message only the map could send.
pub(crate) fn register_cursor_action(
    registry: &mut crate::actions::ActionRegistry,
    catalog: &mut crate::actions::ActionCatalog,
) -> Result<crate::actions::ActionHandle, crate::actions::DuplicateAction> {
    use crate::args::{ArgKind, ArgSpec};
    crate::actions::register_dynamic(
        registry,
        catalog,
        crate::actions::ActionMeta {
            name: CURSOR_ACTION.into(),
            label: "Exposé: Move Cursor to Pane".into(),
            description: "Move the exposé map's cursor onto a pane.".into(),
            category: crate::actions::ActionCategory::Chrome,
            owner: None,
            icon: None,
            // The map covers the panes whenever this is used, so refusing it while an overlay is up
            // would refuse it always — the same reason its message was always allowed.
            policy: crate::app::interaction::ActionPolicy::Global,
            args: vec![ArgSpec {
                name: "pane_id".into(),
                kind: ArgKind::Int,
                required: true,
                description: "The pane the cursor moves to.".into(),
                values: Vec::new(),
            }],
            confirm: None,
        },
        Some(std::rc::Rc::new(|state, intent| {
            // Declared `Int`, so it arrives as one from every door (`ActionMeta::typed_args`).
            let pane = match intent.args.get("pane_id") {
                Some(heca_view::PropValue::Int(id)) => u64::try_from(*id).ok(),
                _ => None,
            };
            if let Some(pane) = pane {
                record_expose_cursor(state, PaneId(pane));
            }
        })),
    )
}
