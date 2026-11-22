//! What a widget inside a layer dispatches goes out as that layer's gesture.

use super::*;
use heca_grid_ui::builders::{EventExt, IdentityExt, Parent};
use heca_grid_ui::widgets::Row;
use heca_grid_ui::{LayoutEngine, Size};
use heca_view::Intent;

/// **A row in a layer names its action, and it arrives as that layer's** — the source the picker
/// reads for the same surface (`pick_source`). The row carries no emitter; the registry's seat
/// listens.
#[test]
fn a_named_action_inside_a_layer_goes_out_as_that_layers_gesture() {
    use crate::app::interaction::{InteractionIntent, InteractionSource};
    let sent = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = sent.clone();
    let mut reg = LayerRegistry::default();
    reg.emit_through(move |surface| {
        let seen = seen.clone();
        crate::chrome::ChromeIntentEmitter::of(
            InteractionSource::Surface(surface),
            move |source, intent| {
                seen.borrow_mut().push((source, intent));
            },
        )
    });
    let mut window = crate::chrome::new_window_root();
    let root = heca_grid_ui::WidgetModel::new(
        Flex::column().child(Row::new().key("open").on_press(Intent::new("open_it"))),
    );
    let id = LayerId(9);
    reg.insert(id, None, LayerKind::OnDemand, root, &mut window);
    LayoutEngine::new().compute(&mut window, Size::new(400.0, 300.0));

    let path = heca_grid_ui::path_with_key(&window, "open").expect("the row is in the layer");
    assert_eq!(
        heca_grid_ui::click(&mut window, &path),
        heca_grid_ui::Handled::Yes
    );
    let sent = sent.borrow();
    assert_eq!(sent.len(), 1, "one action went out: {sent:?}");
    let (source, intent) = &sent[0];
    assert_eq!(*source, InteractionSource::Surface(reg.surface_key(id)));
    match intent {
        InteractionIntent::View(intent) => assert_eq!(intent.action, "open_it"),
        other => panic!("the row's own action went out: {other:?}"),
    }
}

/// **A layer is found again from the key its widgets' calls carry** — what lets a dialog's
/// `submit_overlay` answer that dialog rather than whichever modal is in front.
#[test]
fn a_layer_is_found_from_its_surface_key() {
    let mut reg = LayerRegistry::default();
    let mut window = crate::chrome::new_window_root();
    for raw in [3, 4] {
        let root = heca_grid_ui::WidgetModel::new(Flex::column());
        reg.insert(LayerId(raw), None, LayerKind::OnDemand, root, &mut window);
    }
    for raw in [3, 4] {
        let key = reg.surface_key(LayerId(raw));
        assert_eq!(reg.layer_of_surface(key), Some(LayerId(raw)));
    }
}
