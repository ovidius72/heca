use heca_grid_ui::Component;
use heca_view::{PropValue, WidgetKind};

use super::modal::build_modal_root;
use super::spec::{ModalAction, ModalSpec, OverlayId};
use crate::app::interaction::{InteractionIntent, InteractionSource};
use crate::chrome::{ChromeIntentEmitter, FormBindings};
use crate::input::WmAction;

fn noop_emit() -> ChromeIntentEmitter {
    ChromeIntentEmitter::of(InteractionSource::MouseContent, |_, _| {})
}

#[test]
fn message_spec_wraps_body_in_a_label() {
    let spec = ModalSpec::message("Delete pane?", "This cannot be undone.");
    assert_eq!(spec.body.kind, WidgetKind::Label);
    assert_eq!(
        spec.body.props.get("text").and_then(PropValue::as_text),
        Some("This cannot be undone."),
    );
    assert!(spec.dismissible && !spec.danger && spec.actions.is_empty());
}

#[test]
fn a_long_confirmation_message_wraps_and_preserves_the_complete_warning() {
    use heca_grid_ui::{DrawCommand, Flex, Length, builders::{LayoutExt, Parent}};
    let message = "Trust /Users/example/projects/really-long-worktree/target/p102/harness-icons-preview/.heca/config.toml? It can bind keys to commands, so only trust a project you know.";
    let theme = heca_grid_ui::Theme::default();
    for width in [360.0, 640.0, 2000.0] {
        let spec = ModalSpec::message("Trust project settings", message)
            .action(ModalAction::new("cancel", "Cancel"))
            .action(ModalAction::new("trust", "Trust"));
        let root = build_modal_root(&spec,
            OverlayId(crate::chrome::LayerRegistry::default().reserve_id()), &theme,
            &noop_emit(), &crate::chrome::ActionShortcuts::default(), &mut FormBindings::default());
        let mut root = Flex::column().width(Length::FULL).height(Length::FULL).child(root);
        // Advance the normal arrival before inspecting what the dialog actually paints.
        root.tick(1.0);
        let scene = crate::chrome::paint_chrome_root(&mut root, width, 480.0, &theme);
        let dialog = &root.base().children[0];
        let overlay = &dialog.base().children[0];
        let panel = &overlay.base().children[0];
        // The panel is a Surface whose single child is the Grid of [title, body, action-row].
        let layout = &panel.base().children[0];
        for child in &layout.base().children {
            let bounds = child.base().bounds;
            assert!(bounds.loc.y + bounds.size.h <= panel.base().bounds.loc.y + panel.base().bounds.size.h + 1.0,
                "dialog child escapes panel at {width}px: panel {:?}; child {:?}", panel.base().bounds, bounds);
        }
        assert!(panel.base().bounds.size.w <= f64::from(theme.font_size * theme.colors.dialog.max_width_em) + 1.0);
        let body = layout.base().children[1].base().bounds;
        let lines: Vec<_> = scene.iter().filter_map(|cmd| match cmd {
            DrawCommand::Text(text) if text.rect.loc.y >= body.loc.y
                && text.rect.loc.y < body.loc.y + body.size.h => Some(text),
            _ => None,
        }).collect();
        assert!(lines.len() > 1, "a long warning was still one truncated line at {width}px; body {body:?}; lines {lines:?}");
        let compact = |text: &str| text.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        let painted = lines.iter().map(|line| compact(&line.text)).collect::<String>();
        assert_eq!(painted, compact(message), "part of the warning was lost");
        for line in lines {
            // Taffy rounds the label box; painted line metrics retain fractional pixels.
            assert!(line.rect.loc.y + line.rect.size.h <= body.loc.y + body.size.h + 1.0);
            assert!(line.rect.loc.x >= body.loc.x);
            assert!(line.rect.loc.x + line.rect.size.w <= body.loc.x + body.size.w + 0.01);
        }
    }
}

/// Every modal action button declares what a `prefix+/` pick does to it, and it is the same
/// `SubmitOverlay` its click carries — so the picker reaches a dialog's buttons with nothing
/// registered anywhere.
#[test]
fn each_action_button_declares_the_submit_its_click_would_fire() {
    let spec = ModalSpec::message("Delete pane?", "Gone forever.")
        .action(ModalAction::new("cancel", "Cancel"))
        .action(ModalAction::new("confirm", "Delete").danger(true))
        .dismissible(false);
    let id = OverlayId(crate::chrome::LayerRegistry::default().reserve_id());
    let shortcuts = crate::chrome::ActionShortcuts::default();
    let fired: std::rc::Rc<std::cell::RefCell<Vec<InteractionIntent>>> = Default::default();
    let emit: ChromeIntentEmitter = {
        let fired = fired.clone();
        ChromeIntentEmitter::of(InteractionSource::MouseContent, move |_, intent| {
            fired.borrow_mut().push(intent)
        })
    };
    let mut root = build_modal_root(
        &spec,
        id,
        &heca_grid_ui::Theme::default(),
        &emit,
        &shortcuts,
        &mut FormBindings::default(),
    );

    let targets = heca_grid_ui::collect_hints(root.as_ref());
    assert_eq!(targets.len(), 2, "two actions → two pick targets");
    for (offset, action_id) in [(0, "cancel"), (1, "confirm")] {
        assert!(heca_grid_ui::fire_hint(root.as_mut(), &targets[offset].0));
        let intent = fired.borrow().last().cloned().unwrap();
        assert!(
            matches!(
                &intent,
                InteractionIntent::ActivateAction(WmAction::SubmitOverlay { overlay, action })
                    if *overlay == Some(id) && action == action_id
            ),
            "target {offset} should submit '{action_id}' to this overlay, got {intent:?}",
        );
    }

    // Structure: the Dialog root composes a base Overlay (the blocking layer) whose single child is
    // the panel (a Surface) whose single child is the Grid holding [title, body, action-row(2)].
    let overlay = &root.base().children[0];
    let panel = &overlay.base().children[0];
    let layout = &panel.base().children[0];
    assert_eq!(layout.base().children.len(), 3, "title + body + action row");
    assert_eq!(
        layout.base().children[2].base().children.len(),
        2,
        "two buttons"
    );
}

/// **A modal's action button is one pick target, not two** (docs/hint-architecture.md § 5a).
///
/// `on_hint` is on `HintExt`, so the `Button` already answers for itself — wrapping it in
/// a `KeyHint` used to add a second pick target on top of the button's own actionability, the
/// same bug the exposé's pane card had once. This pins `build_modal_root` to the
/// fix: one action button, one target — and that the target IS the button, not a wrapper
/// standing in front of it.
///
/// The count alone can't catch a reintroduced wrapper: `collect_hints`'s own "one letter per
/// thing, not per layer" rule already collapses a transparent `KeyHint` and the single
/// actionable child it holds into one target (`heca-grid-ui/src/hint/collect.rs`), so a
/// wrapped button and a bare one both report exactly one. What differs is *which node* survives
/// — the wrapper's declaration outranks the button's mere actionability, so a reintroduced
/// `KeyHint` would make the surviving target the transparent wrapper, not the button.
#[test]
fn a_modal_button_is_one_pick_target_not_two() {
    let spec = ModalSpec::message("Delete pane?", "Gone forever.")
        .action(ModalAction::new("confirm", "Delete").danger(true))
        .dismissible(false);
    let id = OverlayId(crate::chrome::LayerRegistry::default().reserve_id());
    let shortcuts = crate::chrome::ActionShortcuts::default();
    let emit = noop_emit();
    let root = build_modal_root(
        &spec,
        id,
        &heca_grid_ui::Theme::default(),
        &emit,
        &shortcuts,
        &mut FormBindings::default(),
    );

    let targets = heca_grid_ui::collect_hints(root.as_ref());
    assert_eq!(
        targets.len(),
        1,
        "one action button must be exactly one pick target, not the button plus its wrapper",
    );

    let mut node: &dyn Component = root.as_ref();
    for &step in &targets[0].0 {
        node = node.base().children[step].as_ref();
    }
    assert!(
        !node.base().transparent,
        "the pick target must be the button itself, not a transparent KeyHint wrapped \
         around it",
    );
}

/// **An overlay's close gesture closes that overlay** — the one door every dismiss hook and every
/// finished menu goes through, so it is the id it was asked for and nothing else.
#[test]
fn the_close_gesture_targets_its_own_overlay() {
    let mut registry = crate::chrome::LayerRegistry::default();
    let id = OverlayId(registry.reserve_id());
    let other = OverlayId(registry.reserve_id());
    let fired: std::rc::Rc<std::cell::RefCell<Vec<InteractionIntent>>> = Default::default();
    let emit = {
        let fired = fired.clone();
        ChromeIntentEmitter::of(InteractionSource::MouseContent, move |_, intent| {
            fired.borrow_mut().push(intent)
        })
    };

    id.closer(&emit)();
    other.submitter("confirm", &emit)();

    let fired = fired.borrow();
    assert!(
        matches!(
            &fired[0],
            InteractionIntent::ActivateAction(WmAction::CloseOverlay { overlay }) if *overlay == Some(id)
        ),
        "closes this overlay, got {:?}",
        fired[0],
    );
    assert!(
        matches!(
            &fired[1],
            InteractionIntent::ActivateAction(WmAction::SubmitOverlay { overlay, action })
                if *overlay == Some(other) && action == "confirm"
        ),
        "answers the other one with its action, got {:?}",
        fired[1],
    );
}
