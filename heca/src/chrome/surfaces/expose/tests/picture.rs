//! What is drawn: only panes have a surface, the map fits the window, and every card is a pick target.

use super::*;

/// **Every card in the map is a pane, and nothing else has a surface.** The workspace name was
/// a fixed gutter column that pushed every strip inwards, so a row stopped lining up with the
/// screen it is a picture of; niri's overview labels nothing either.
#[test]
fn a_row_carries_no_workspace_label() {
    let s = session();
    let rows = model(s.l(), |p| p.title.clone(), false);
    let theme = GuiTheme::default();
    let emit: crate::chrome::ChromeIntentEmitter = crate::chrome::ChromeIntentEmitter::of(
        crate::app::interaction::InteractionSource::Keyboard,
        |_, _| {},
    );
    let mut root = map(
        &rows,
        &theme,
        emit,
        None,
        &LayoutOptions::default(),
        &shipped_keys(),
        None,
    );
    root.show(); // as the layer stack shows it — see `built`

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
        !seen
            .iter()
            .any(|t| t == "Editing" || t.starts_with("Workspace ")),
        "no workspace name is drawn; the map showed: {seen:?}",
    );
    assert!(
        seen.iter().any(|t| t == "a"),
        "the pane names are still there: {seen:?}"
    );

    // **Everything the map DRAWS lives in the scene's OVERLAY layer**, because `Overlay` paints
    // through `PaintCx::with_overlay`. A host deciding whether to flush the layer pass by
    // asking "did the base layer draw anything" gets `no` and skips it, and `prefix+Tab` shows
    // nothing at all — which is exactly what happened.
    //
    // The one thing in the base layer is the **frost request**, and it is there on purpose: the
    // host performs the requests between flushing the base and flushing the overlay bands, so a
    // backdrop recorded with the visuals would blur the frame *after* the surface it is meant
    // to sit under. Nothing is drawn there — the blur is GPU work the host does, not a shape.
    assert!(
        scene
            .base_layer()
            .iter()
            .all(|c| matches!(c, heca_grid_ui::DrawCommand::Host(_))),
        "the map draws nothing into the base layer — do not gate its flush on that",
    );
    assert!(
        scene.has_overlay(),
        "and everything it draws is in the overlay layer"
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
        let rows = model(s.l(), |p| p.title.clone(), false);
        let theme = GuiTheme::default();
        let emit: crate::chrome::ChromeIntentEmitter = crate::chrome::ChromeIntentEmitter::of(
            crate::app::interaction::InteractionSource::Keyboard,
            |_, _| {},
        );
        let mut root = map(
            &rows,
            &theme,
            emit,
            Some(PaneId(1)),
            &LayoutOptions::default(),
            &shipped_keys(),
            None,
        );
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), heca_grid_ui::Size::new(w, h));
        let drawn = crate::chrome::surfaces::expose::testing::cards_bounds(root.as_ref()).expect("the map has cards");
        assert!(
            drawn.loc.x >= 0.0
                && drawn.loc.y >= 0.0
                && drawn.loc.x + drawn.size.w <= w + 1.0
                && drawn.loc.y + drawn.size.h <= h + 1.0,
            "in {w}x{h} the cards reach {drawn:?}",
        );
        let card =
            card_of(root.as_ref(), &pane_card::pane_key(PaneId(1))).expect("the first pane's card");
        assert!(
            card.size.h > h * 0.3,
            "and a card is a pane-shaped box, not a sliver: {card:?} in {w}x{h}",
        );
    }
}

/// **The map declares its own picker, and its cards declare their own picks**.
///
/// Both halves, because either alone is silent: an action nothing declares is a no-op that
/// looks exactly like a typo, and a picker over cards that declare nothing shows no letters.
#[test]
fn the_map_owns_its_picker_and_every_card_is_a_target() {
    let s = session();
    let rows = model(s.l(), |p| p.title.clone(), false);
    let theme = GuiTheme::default();
    let emit: crate::chrome::ChromeIntentEmitter = crate::chrome::ChromeIntentEmitter::of(
        crate::app::interaction::InteractionSource::Keyboard,
        |_, _| {},
    );
    let mut root = map(
        &rows,
        &theme,
        emit,
        Some(PaneId(1)),
        &LayoutOptions::default(),
        &shipped_keys(),
        None,
    );
    // Open, as the host shows it: a closed map is gone to every walk, its picker included.
    root.show();
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
