//! **A carry settles**: the layout the places open is the same on every frame of it, whether the
//! pointer is still or moving, so nothing can loop.

use super::*;
use heca_core::layout::types::{LayoutOptions, Rectangle as Rect, Size as Sz};
use heca_core::layout::{Session, SessionId};

/// Two columns, the first holding two panes (ids 1, 2), the second one (id 3).
fn session() -> Session {
    let mut s = Session::new(SessionId(1), Sz::new(1000.0, 800.0), 1.0, LayoutOptions::default());
    s.add_pane(heca_core::layout::Pane::new(PaneId(1), ""), None, true);
    s.add_pane(heca_core::layout::Pane::new(PaneId(3), ""), None, true);
    let ws = s.active_workspace_mut().unwrap();
    ws.scrolling
        .add_pane_to_column(0, Some(1), heca_core::layout::Pane::new(PaneId(2), ""), false);
    s
}

/// The model the host would hand the workspace for this layout.
fn model_of(session: &Session, open: bool) -> WorkspaceModel {
    let ws = session.active_workspace().unwrap();
    let columns: Vec<ColumnShellModel> = ws
        .scrolling
        .columns_with_positions()
        .into_iter()
        .map(|c| ColumnShellModel {
            col_id: c.id,
            x: c.rect.loc.x as f32,
            y: c.rect.loc.y as f32,
            w: c.rect.size.w as f32,
            h: c.rect.size.h as f32,
            focus_pane: c.panes.first().map(|p| p.id),
            panes: c
                .panes
                .iter()
                .map(|p| {
                    model_at(p.id, p.rect.loc.x as f32, p.rect.loc.y as f32, p.rect.size.w as f32, p.rect.size.h as f32)
                })
                .collect(),
        })
        .collect();
    let mut model = model(columns, vec![]);
    model.area = Rect::new(Point::new(0.0, 0.0), Sz::new(1000.0, 800.0));
    model.places = if open { ws.scrolling.places() } else { Vec::new() };
    model
}

/// What a frame looks like: where every pane is.
fn frame(ws: &dyn Component) -> Vec<(f64, f64, f64, f64)> {
    let mut out = Vec::new();
    fn walk(c: &dyn Component, out: &mut Vec<(f64, f64, f64, f64)>) {
        if c.base().key.as_deref().is_some_and(|k| k.starts_with("pane:")) {
            let b = c.base().bounds;
            out.push((b.loc.x, b.loc.y, b.size.w, b.size.h));
        }
        for child in &c.base().children {
            walk(child.as_ref(), out);
        }
    }
    walk(ws, &mut out);
    out
}

#[test]
fn a_carry_has_one_layout_whatever_the_pointer_does() {
    use heca_grid_ui::component::dispatch;
    use heca_grid_ui::event::{Event, PointerButton};

    let session = session();
    let mut window = crate::chrome::new_window_root();
    window.base_mut().children.push(Box::new(workspace(seams())));
    let mut frames = Vec::new();
    let render = |window: &mut Flex, open: bool| {
        let model = model_of(&session, open);
        assert!(window.base_mut().children[0].set_props(&model));
        LayoutEngine::new().compute(window, heca_grid_ui::Size::new(1000.0, 800.0));
        frame(window.base().children[0].as_ref())
    };

    render(&mut window, false);
    let cmd = heca_grid_ui::Modifiers { meta: true, ..heca_grid_ui::Modifiers::default() };
    dispatch(&mut window, &Event::ModifiersChanged(cmd));
    dispatch(&mut window, &Event::pointer_pressed(Point::new(100.0, 120.0), PointerButton::Left));
    // The pointer moves to where it will rest, then is held still, a move event sent every frame.
    let rest = Point::new(160.0, 130.0);
    dispatch(&mut window, &Event::pointer_moved(rest));
    for _ in 0..12 {
        let carried = heca_grid_ui::dragging(&window);
        let open = carried;
        frames.push(render(&mut window, open));
        // Still, then across the other column and back: where the pointer is never decides it.
        let at = [rest, Point::new(600.0, 130.0), rest][frames.len() % 3];
        dispatch(&mut window, &Event::pointer_moved(at));
    }
    assert!(
        frames.windows(2).all(|pair| pair[0] == pair[1]),
        "the layout changes while a pane is carried and nothing about the carry changed:\n{:#?}",
        &frames[..3]
    );
}
