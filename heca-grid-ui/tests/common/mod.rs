#![allow(dead_code)]

use heca_grid_ui::prelude::*;
use heca_grid_ui::scene::{RectCmd, TextCmd};
use heca_grid_ui::{DrawCommand, Event, PaintCx, Point, Scene, Size, Theme};

/// A leaf box with a fixed size, for deterministic layout assertions.
pub fn fixed_box(w: f32, h: f32) -> Flex {
    Flex::column().width(Length::Px(w)).height(Length::Px(h))
}

/// A press, on its own — for a test that needs to look at the world between the press and the
/// release rather than after a whole click.
pub fn press_at(root: &mut dyn Component, pos: Point, button: PointerButton) {
    let _ = heca_grid_ui::dispatch(root, &Event::pointer_pressed(pos, button));
}

/// A release, on its own. Pairs with [`press_at`] for a gesture that presses one place and
/// releases another.
pub fn release_at(root: &mut dyn Component, pos: Point, button: PointerButton) {
    let _ = heca_grid_ui::dispatch(root, &Event::pointer_released(pos, button));
}

/// A click: a press **and** the release that completes it, on the same spot, with the given
/// button.
///
/// Since F004/P084/T394 a control fires on the click, not on the press — so pressing and dragging
/// off cancels, the way every other control on the machine behaves. A test that only presses is
/// testing half a gesture.
pub fn click_at(root: &mut dyn Component, pos: Point, button: PointerButton) {
    press_at(root, pos, button);
    release_at(root, pos, button);
}

/// Type `text` into whatever holds the keyboard. Text is not a key: it arrives as
/// [`Event::TextInput`], which is what an IME commit and a paste look like too.
pub fn type_text(root: &mut dyn Component, text: &str) -> Handled {
    heca_grid_ui::dispatch(root, &Event::TextInput(text.to_string()))
}

/// **Hand this widget the keyboard**, the way a host does when you click a field or Tab to it.
///
/// Keys and typed text are delivered to the focus owner and nowhere else (F004/P084/T400), so a
/// test that types has to say who is typing — exactly as a real surface has to. It replaces
/// nothing: a widget used to be offered every key in the tree and decide for itself, which is why
/// an unfocused row could answer an Enter meant for the cursor.
pub fn give_keyboard(c: &mut dyn Component) {
    c.base_mut().focused.set(true);
}

/// Paint `widget` fresh into its own scene, the way a host paints one frame — assumes `widget` is
/// already laid out. This is the plain case: no viewport, no scale, no clip. A test that needs one
/// of those builds its own `Scene`/`PaintCx` so that setup stays visible where it is read.
pub fn paint(widget: &dyn Component, theme: &Theme) -> Scene {
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, theme);
        widget.paint(&mut cx);
    }
    scene
}

/// Paint `widget` into a scene with a fixed viewport, the way a host paints one frame inside a
/// window of a known size — assumes `widget` is already laid out. A test that needs one of these
/// (a flip against the window edge, a scrim sized to the window, on/off-screen culling) builds its
/// own `Scene`/`PaintCx` only when it also does something [`paint_in`] does not: sets a scale,
/// pushes a clip, or draws straight into the `PaintCx` instead of through the widget.
pub fn paint_in(widget: &dyn Component, theme: &Theme, viewport: Size) -> Scene {
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, theme).with_viewport(viewport);
        widget.paint(&mut cx);
    }
    scene
}

/// Paint `widget` the way the framework actually draws a tree — through `paint_child`, the one
/// chokepoint that applies what a widget declared on its base (a tooltip, a key hint, a published
/// tone) beside its own `paint`. A test asserting on any of that paints through this, never through
/// `widget.paint` directly, or it is testing a path no widget is ever drawn by.
pub fn paint_via_child(widget: &dyn Component, theme: &Theme) -> Scene {
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, theme);
        heca_grid_ui::paint_child(widget, &mut cx);
    }
    scene
}

/// Every rect the scene drew, in paint order.
pub fn rects(scene: &Scene) -> Vec<RectCmd> {
    scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r) => Some(*r),
            _ => None,
        })
        .collect()
}

/// Every text run the scene drew, in paint order.
pub fn texts(scene: &Scene) -> Vec<TextCmd> {
    scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}
