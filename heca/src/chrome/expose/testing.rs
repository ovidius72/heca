//! Shared fixtures for the exposé's component tests.
//!
//! Every component here takes plain data and no `AppState`, which is what makes a headless test
//! possible at all (§ 0b) — these are the few values each of them needs, written once so a test
//! reads as the thing it is asserting rather than as its setup.

use heca_core::layout::{LayoutOptions, Pane, PaneId, Rectangle, Session, SessionId, Size};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::Component;

use super::pane_card::{ExposeCallbacks, ExposeDeleteKeys};

/// Two panes in one workspace, plus a second, empty workspace — the smallest session with
/// something to compare.
pub(super) fn session() -> Session {
    let viewport = Size::new(800.0, 600.0);
    let mut s = Session::new(SessionId(0), viewport, 1.0, LayoutOptions::default());
    s.workspaces[0].name = Some("Editing".to_string());
    s.add_pane(Pane::new(PaneId(1), "a"), None, false);
    s.add_pane(Pane::new(PaneId(2), "b"), None, false);
    s.add_workspace(Rectangle::from_size(viewport));
    s
}

/// The delete letters **as the bundled defaults bind them**, so these tests exercise what a user
/// actually gets. Held to the real file by `the_shipped_defaults_bind_the_maps_delete_keys`.
pub(super) fn shipped_keys() -> ExposeDeleteKeys {
    ExposeDeleteKeys {
        pane: vec!["x".into()],
        column: vec!["r".into()],
        workspace: vec!["d".into()],
    }
}

/// What a component saw fit to send, in the order it sent it — one line per intent, which is
/// enough to assert against and far easier to read than the enum.
pub(super) type Sink = std::rc::Rc<std::cell::RefCell<Vec<crate::app::interaction::InteractionIntent>>>;

/// The seams, wired to a sink instead of to the app.
pub(super) fn callbacks() -> (ExposeCallbacks, Sink) {
    let sink: Sink = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let emit: crate::chrome::ChromeIntentEmitter = {
        let sink = sink.clone();
        std::rc::Rc::new(move |i| sink.borrow_mut().push(i))
    };
    (super::callbacks(emit, shipped_keys()), sink)
}

/// The actions a sink saw, as `("name", [(arg, value)…])` with the args sorted — the shape an
/// assertion can be written in.
pub(super) fn actions(sink: &Sink) -> Vec<(String, Vec<(String, i64)>)> {
    sink.borrow()
        .iter()
        .filter_map(|i| match i {
            crate::app::interaction::InteractionIntent::View(vi) => {
                let mut args: Vec<(String, i64)> = vi
                    .args
                    .iter()
                    .filter_map(|(k, v)| match v {
                        heca_view::PropValue::Int(n) => Some((k.clone(), *n)),
                        _ => None,
                    })
                    .collect();
                args.sort();
                Some((vi.action.clone(), args))
            }
            _ => None,
        })
        .collect()
}

/// Lay a component out in a box of the given size and hand back the laid-out tree.
pub(super) fn lay_out<T: Component + 'static>(node: T, w: f64, h: f64) -> Box<dyn Component> {
    let mut root: Box<dyn Component> = Box::new(node);
    heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), heca_grid_ui::Size::new(w, h));
    root
}

/// The bounds of the card a pane's `key` names, wherever it sits in the tree.
pub(super) fn card_of(n: &dyn Component, key: &str) -> Option<heca_grid_ui::Rectangle> {
    if n.base().key.as_deref() == Some(key) {
        return Some(n.base().bounds);
    }
    n.base().children.iter().find_map(|c| card_of(c.as_ref(), key))
}

/// The rectangle **every** card in a tree falls inside — what has to stay within the box the map
/// was given, however many workspaces there are and whatever shape the window is.
pub(super) fn cards_bounds(root: &dyn Component) -> Option<heca_grid_ui::Rectangle> {
    fn walk(n: &dyn Component, acc: &mut Option<heca_grid_ui::Rectangle>) {
        if n.base().key.is_some() {
            let b = n.base().bounds;
            *acc = Some(match acc.take() {
                None => b,
                Some(a) => {
                    let x0 = a.loc.x.min(b.loc.x);
                    let y0 = a.loc.y.min(b.loc.y);
                    let x1 = (a.loc.x + a.size.w).max(b.loc.x + b.size.w);
                    let y1 = (a.loc.y + a.size.h).max(b.loc.y + b.size.h);
                    heca_grid_ui::Rectangle::new(
                        heca_grid_ui::Point::new(x0, y0),
                        heca_grid_ui::Size::new(x1 - x0, y1 - y0),
                    )
                }
            });
        }
        for c in &n.base().children {
            walk(c.as_ref(), acc);
        }
    }
    let mut acc = None;
    walk(root, &mut acc);
    acc
}

/// A default theme — every component reads all of its styling from one.
pub(super) fn theme() -> GuiTheme {
    GuiTheme::default()
}
