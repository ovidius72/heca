//! The tree the component builds, and the names and gestures on its rows.

use super::*;

/// **Every row kind declares its own `key` on the widget.**
///
/// `key_at` — how a right-click finds out what it landed on — reads `Base::key` and
/// nothing else. The workspace header pushed its key into the cursor-signal list and never told
/// the widget, so the hit-test found nothing at that row: right-clicking a pane or a column
/// opened its menu and a workspace opened none. Pushing the key to the signals and declaring it
/// on the widget are two different acts, and the projection tests only ever checked the first.
///
/// Note what this test no longer needs: a `ChromeCtx`, and therefore a window. The dock is
/// components now, so it is built from its seams.
#[test]
fn every_row_kind_carries_a_key_the_hit_test_can_find() {
    let tree = testing::tree();
    let mut fx = testing::Fixture::default();
    let root = {
        let (seams, mut reg) = fx.split("left");
        DockView {
            tree: &tree,
            scroll: heca_grid_ui::reactive::signal(0.0),
        }
        .build(&seams, &mut reg)
    };

    let declared = testing::declared_keys(&root);
    for expected in [
        workspace_key(heca_core::layout::WorkspaceId(0)),
        column_key(heca_core::layout::ColumnId(0)),
        pane_key(PaneId(1)),
    ] {
        assert!(
            declared.contains(&expected),
            "no widget declares {expected:?}, so a right-click there finds nothing: {declared:?}",
        );
    }
}


#[test]
fn build_produces_the_real_workspace_tree_and_registers_host_ids() {
    // The seam builds the *same* body the bespoke path builds: a column with one
    // child per workspace, and — the part a placeholder could never fake — the
    // drag and hint ids the interactive rows need, allocated in the host's
    // registries.
    let p = WorkspacesContainerProvider::new("workspaces");
    let theme = GuiTheme::default();
    let emit: ChromeIntentEmitter = ChromeIntentEmitter::of(
        crate::app::interaction::InteractionSource::Keyboard,
        |_, _| {},
    );
    let store = store();
    // The model is the component's own, read off its state — not handed in by the host.
    *store.workspaces.tree_mut() = tree();

    let catalog = crate::actions::ActionCatalog::with_builtins();
    let ctx = ChromeCtx::for_build(&store, &theme, &emit, &catalog);
    let c = container(&p, &ctx);

    let mut signals = ChromeSignals::default();
    let mut drag = DragItemRegistry::default();
    let mut bx = BuildCx::new("workspaces", &mut signals, &mut drag);
    let body = (c.build)(&ctx, &mut bx);

    assert_eq!(
        body.base().children.len(),
        1,
        "one workspace in the tree ⇒ one workspace dock in the body",
    );
    // Drag ids: the workspace (drop target), its column, its pane.
    assert_eq!(drag.items().len(), 3);
    // Pick targets: the pane card and the workspace dock each declared what `prefix+/` does to
    // it, and the dock's own collapse control earns one for being a control — a thing you can
    // click is a thing you can aim at, whatever it happens to sit inside. A column
    // has none: it declares nothing and does nothing on its own, only carrying a pick-letter
    // signal stamped when it is a *destination* for a move/swap.
    assert_eq!(heca_grid_ui::collect_hints(body.as_ref()).len(), 3);
    // The active pane's card bound its `active` signal for per-frame updates.
    assert_eq!(signals.pane_active.len(), 1);
}

/// **Every letter the picker offers can be found again after a rebuild**.
///
/// The picker addresses a target by path for the frame and by *identity* across frames, so the
/// two must be inverses **on the real sidebar tree**, not only on a fixture: `identity_of` names
/// the widget, `path_of` finds it again. If any target's identity resolves to a different path,
/// its letter silently lands on another row after the next rebuild — and the chrome tree is
/// rebuilt constantly.
#[test]
fn every_pick_target_in_the_sidebar_can_be_found_again_by_its_identity() {
    let p = WorkspacesContainerProvider::new("workspaces");
    let theme = GuiTheme::default();
    let emit: ChromeIntentEmitter = ChromeIntentEmitter::of(
        crate::app::interaction::InteractionSource::Keyboard,
        move |_, _| {},
    );
    let store = store();
    *store.workspaces.tree_mut() = tree();
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let ctx = ChromeCtx::for_build(&store, &theme, &emit, &catalog);
    let c = container(&p, &ctx);
    let mut signals = ChromeSignals::default();
    let mut drag = DragItemRegistry::default();
    let mut bx = BuildCx::new("workspaces", &mut signals, &mut drag);
    let body = (c.build)(&ctx, &mut bx);

    let targets = heca_grid_ui::collect_hints(body.as_ref());
    assert!(!targets.is_empty(), "the sidebar declares pick targets");
    for (path, _) in &targets {
        let Some(identity) = heca_grid_ui::identity_of(body.as_ref(), path) else {
            continue; // nothing to name it by: the picker keeps the path, and says so
        };
        assert!(
            heca_grid_ui::hint_targets_of(body.as_ref(), &identity).contains(path),
            "{identity:?} resolves to a different widget than the one it names",
        );
    }
}

/// **No two pick targets in the sidebar answer to the same name**.
///
/// A remembered letter is looked up by identity, so two targets sharing one identity both ask
/// for the same letter. The first takes it, the second is refused and draws a fresh one — and
/// the remember step then writes the loser's letter into the map, so the next opening trades
/// them back. The letters oscillate forever and nothing fails.
///
/// A target with **no** identity is the same defect by another route: it can never be
/// remembered, so it takes a fresh letter every time.
#[test]
fn no_two_pick_targets_in_the_sidebar_share_an_identity() {
    let p = WorkspacesContainerProvider::new("workspaces");
    let theme = GuiTheme::default();
    let emit: ChromeIntentEmitter = ChromeIntentEmitter::of(
        crate::app::interaction::InteractionSource::Keyboard,
        move |_, _| {},
    );
    let store = store();
    *store.workspaces.tree_mut() = tree();
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let ctx = ChromeCtx::for_build(&store, &theme, &emit, &catalog);
    let c = container(&p, &ctx);
    let mut signals = ChromeSignals::default();
    let mut drag = DragItemRegistry::default();
    let mut bx = BuildCx::new("workspaces", &mut signals, &mut drag);
    let body = (c.build)(&ctx, &mut bx);

    let targets = heca_grid_ui::collect_hints(body.as_ref());
    assert!(!targets.is_empty(), "the sidebar declares pick targets");

    let mut seen: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    let mut anonymous = 0usize;
    for (i, (path, _)) in targets.iter().enumerate() {
        match heca_grid_ui::identity_of(body.as_ref(), path) {
            Some(identity) => seen.entry(identity).or_default().push(i),
            None => anonymous += 1,
        }
    }

    let mut collisions: Vec<_> = seen
        .iter()
        .filter(|(_, which)| which.len() > 1)
        .map(|(id, which)| format!("{id:?} claimed by {} targets", which.len()))
        .collect();
    collisions.sort();
    assert!(
        collisions.is_empty(),
        "{} sidebar targets share a name, so their letters swap on every opening:\n  {}",
        collisions.len(),
        collisions.join("\n  "),
    );
    assert_eq!(
        anonymous, 0,
        "{anonymous} sidebar targets have no identity, so they cannot keep a letter",
    );
}

/// **Our own rows stay keyed** — the test half of the identity rule.
///
/// The sidebar is heca's biggest collection: workspaces holding columns holding panes, rebuilt
/// whenever anything about a pane changes. Every level declares a `key`, so the keyboard
/// cursor, the right-click target, the drag identity and the picker's remembered letter all
/// survive a rebuild. Stop declaring one at any level and the rows fall back to a **derived**
/// identity — their name — and this fixture is built so that fallback is not enough: two panes
/// in a column are both called `zsh`, and both workspaces are built the same way, which is the
/// everyday case and not a contrived one.
///
/// Nothing fails when the keys go. The letters just move under whoever is driving, which is
/// what happened to the pane-header buttons and what nothing caught. That is
/// why this assertion exists rather than a behavioural one.
#[test]
fn every_row_of_the_real_sidebar_tree_is_keyed_even_when_two_panes_share_a_name() {
    use super::testing::pane;
    use heca_core::layout::PaneId;

    let p = WorkspacesContainerProvider::new("workspaces");
    let theme = GuiTheme::default();
    let emit: ChromeIntentEmitter = ChromeIntentEmitter::of(
        crate::app::interaction::InteractionSource::Keyboard,
        |_, _| {},
    );
    let store = store();

    // Two workspaces, each with a column of two identically-named panes — nothing here can be
    // told apart by its content, so only a declared key can tell it apart at all.
    let twins = |ws_idx: usize| WorkspaceEntry {
        ws_idx,
        ws_id: heca_core::layout::WorkspaceId(ws_idx as u64),
        name: format!("ws{ws_idx}"),
        custom_name: None,
        collapsed: false,
        columns: vec![ColumnEntry {
            col_idx: 0,
            col_id: heca_core::layout::ColumnId(0),
            panes: vec![pane(PaneId(1), "zsh"), pane(PaneId(2), "zsh")],
            collapsed: false,
        }],
        floating_panes: Vec::new(),
    };
    let mut model = WorkspaceTree::new();
    model.workspaces.push(twins(0));
    model.workspaces.push(twins(1));
    *store.workspaces.tree_mut() = model;

    let catalog = crate::actions::ActionCatalog::with_builtins();
    let ctx = ChromeCtx::for_build(&store, &theme, &emit, &catalog);
    let c = container(&p, &ctx);
    let mut signals = ChromeSignals::default();
    let mut drag = DragItemRegistry::default();
    let mut bx = BuildCx::new("workspaces", &mut signals, &mut drag);
    let body = (c.build)(&ctx, &mut bx);

    let ambiguous = heca_grid_ui::nav::ambiguous_identities(body.as_ref());
    assert!(
        ambiguous.is_empty(),
        "a collection in heca's own sidebar lost its keys: {ambiguous:#?}",
    );
}

/// **A row's gesture is a NAME** — an action id plus arguments, so a menu
/// entry, a keybinding and RPC all reach the same thing a click does. It used to be a closure
/// emitting a host-side variant, which only the click could ever run.
///
/// **And a pick is not a click**: both rows aim `prefix+/` at this component's
/// own `peek_selected`, carrying the picked row's nav key — *look at that one*, staying in the
/// dock — where a click on the pane row means *go there and leave*.
///
/// **Every gesture also names the seating it was declared in**, so nothing has to resolve the
/// call back to an instance: see
/// `a_gesture_names_the_seating_it_was_built_in_so_two_placements_answer_for_themselves`.
#[test]
fn a_rows_gesture_is_a_named_intent_and_a_pick_is_its_own_gesture() {
    use crate::app::interaction::InteractionIntent;
    let p = WorkspacesContainerProvider::new("workspaces");
    let theme = GuiTheme::default();
    let fired: Rc<std::cell::RefCell<Vec<InteractionIntent>>> = Default::default();
    let emit: ChromeIntentEmitter = {
        let fired = fired.clone();
        ChromeIntentEmitter::of(
            crate::app::interaction::InteractionSource::Keyboard,
            move |_, intent| fired.borrow_mut().push(intent),
        )
    };
    let store = store();
    *store.workspaces.tree_mut() = tree();
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let ctx = ChromeCtx::for_build(&store, &theme, &emit, &catalog);
    let c = container(&p, &ctx);

    let mut signals = ChromeSignals::default();
    let mut drag = DragItemRegistry::default();
    let mut bx = BuildCx::new("workspaces", &mut signals, &mut drag);
    let mut body = (c.build)(&ctx, &mut bx);

    // Every pick in the built body, run in document order.
    for (path, _) in heca_grid_ui::collect_hints(body.as_ref()) {
        assert!(heca_grid_ui::fire_hint(body.as_mut(), &path));
    }
    // The ROWS' picks — the subject of this test. Controls *inside* a row also earn letters
    // (see the assertion below) and fire their own actions, which are not row gestures.
    let declared: Vec<(String, Option<PropValue>)> = fired
        .borrow()
        .iter()
        .filter_map(|intent| match intent {
            InteractionIntent::View(vi) => {
                Some((vi.action.clone(), vi.args.get("key").cloned()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        declared,
        vec![
            // Document order: the workspace row, then the pane rows inside it.
            (
                "workspaces.peek_selected".to_string(),
                Some(PropValue::Text("ws:0".to_string())),
            ),
            (
                "workspaces.peek_selected".to_string(),
                Some(PropValue::Text("pane:1".to_string())),
            ),
        ],
        "each row aims the picker at its own row, by nav key",
    );

    // **A control inside a row is a thing of its own, and gets its own letter**.
    // The picker used to letter only what declared, so a row's own collapse control — a button
    // like any other — was unreachable by `prefix+/` purely because of what it had been put
    // inside. Where a widget sits does not decide what it can do.
    assert!(
        fired
            .borrow()
            .iter()
            .any(|i| !matches!(i, InteractionIntent::View(_))),
        "the row's collapse control was lettered too, and picking it ran its own action"
    );
}

/// **A gesture names the seating it was built in**, so the same container
/// mounted twice has two rows that each answer for themselves — the way two `<div>`s on a page
/// do, rather than one being resolved back to "whichever instance has focus".
///
/// Without it the host guessed the owner (`owning_mount`: the focused seating, else the last
/// focused, else the first that declares the name), so a right-sidebar row's letter was
/// performed by the left-sidebar copy — with the wrong dock taking the keyboard.
#[test]
fn a_gesture_names_the_seating_it_was_built_in_so_two_placements_answer_for_themselves() {
    use crate::app::interaction::InteractionIntent;
    use crate::providers::SEAT_ARG;

    let seats = |mount: &str| {
        let p = WorkspacesContainerProvider::new("workspaces");
        let theme = GuiTheme::default();
        let fired: Rc<std::cell::RefCell<Vec<InteractionIntent>>> = Default::default();
        let emit: ChromeIntentEmitter = {
            let fired = fired.clone();
            ChromeIntentEmitter::of(
                crate::app::interaction::InteractionSource::Keyboard,
                move |_, intent| fired.borrow_mut().push(intent),
            )
        };
        let store = store();
        *store.workspaces.tree_mut() = tree();
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let ctx = ChromeCtx::for_build(&store, &theme, &emit, &catalog);
        let c = container(&p, &ctx);
        let mut signals = ChromeSignals::default();
        let mut drag = DragItemRegistry::default();
        let mut bx = BuildCx::new(mount, &mut signals, &mut drag);
        let mut body = (c.build)(&ctx, &mut bx);
        for (path, _) in heca_grid_ui::collect_hints(body.as_ref()) {
            assert!(heca_grid_ui::fire_hint(body.as_mut(), &path));
        }
        // The ROWS' gestures. A control inside a row is lettered too and fires its own
        // action, which is not a row gesture and names no seating — it acts on what it names.
        let seats: Vec<PropValue> = fired
            .borrow()
            .iter()
            .filter_map(|intent| match intent {
                InteractionIntent::View(vi) => {
                    Some(vi.args.get(SEAT_ARG).cloned().unwrap_or_else(|| {
                        panic!("a gesture must name its seating, got {:?}", vi.args)
                    }))
                }
                _ => None,
            })
            .collect();
        assert!(!seats.is_empty(), "the body declared at least one gesture");
        seats
    };

    for (mount, expected) in [("dock.left", "dock.left"), ("dock.right", "dock.right")] {
        for seat in seats(mount) {
            assert_eq!(seat, PropValue::Text(expected.to_string()));
        }
    }
}

#[test]
fn build_outside_a_render_pass_yields_an_empty_body() {
    // An observe-only context has no tree/theme/emitter to project. Building is
    // still legal (a host may want the metadata) and must not panic.
    let p = WorkspacesContainerProvider::new("workspaces");
    let store = store();
    let ctx = ChromeCtx::new(&store);
    let c = container(&p, &ctx);

    let mut signals = ChromeSignals::default();
    let mut drag = DragItemRegistry::default();
    let mut bx = BuildCx::new("workspaces", &mut signals, &mut drag);
    let body = (c.build)(&ctx, &mut bx);

    assert!(body.base().children.is_empty());
    assert!(drag.items().is_empty());
    assert!(heca_grid_ui::collect_hints(body.as_ref()).is_empty());
}
