//! What `prefix+/` letters in the scrolling area, and what pressing a pane's letter does.

use super::*;
use crate::chrome::panes::pane::shell::PANE_CONTENT_AREA;
use heca_grid_ui::builders::Parent;
use heca_grid_ui::reactive::SignalGet;

/// The path from `root` to the node keyed `key`.
fn path_of(root: &dyn Component, key: &str) -> Option<Vec<usize>> {
    if root.base().key.as_deref() == Some(key) {
        return Some(Vec::new());
    }
    root.base().children.iter().enumerate().find_map(|(i, child)| {
        let mut path = path_of(child.as_ref(), key)?;
        path.insert(0, i);
        Some(path)
    })
}

/// The node at `path` below `root`.
fn node_at<'a>(root: &'a dyn Component, path: &[usize]) -> &'a dyn Component {
    path.iter()
        .fold(root, |node, step| node.base().children[*step].as_ref())
}

/// The pane `id`'s path, and the ordinary picker's targets inside it.
fn letters_in_pane(ws: &dyn Component, id: u64) -> (Vec<usize>, Vec<Vec<usize>>) {
    let pane = path_of(ws, &crate::chrome::pane_key(PaneId(id))).expect("the pane is built");
    let inside = heca_grid_ui::collect_hints(ws)
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| path.starts_with(&pane))
        .collect();
    (pane, inside)
}

/// Whose turn the keyboard is in `root`: the focus identity of what runs in the pane `id`.
fn content_of(root: &dyn Component, id: u64) -> u64 {
    let pane = heca_grid_ui::node_with_key(root, &crate::chrome::pane_key(PaneId(id)))
        .expect("the pane is built");
    heca_grid_ui::area(pane, PANE_CONTENT_AREA)
        .expect("the pane holds its content")
        .base()
        .focus_id()
}

/// **One letter per pane, on the pane, shared with its sidebar row.**
///
/// The terminal inside wore a second letter of its own, because it takes the keyboard and anything
/// that does is lettered. The pane hands its keyboard to what it holds, so that letter is the
/// pane's — and it stays on the pane, under the pane's name, which is what its sidebar row answers
/// to: one pane, one letter, wherever it is shown.
#[test]
fn each_pane_wears_one_letter_shared_with_its_sidebar_row() {
    let (_window, ws) = laid_out(&two_columns_and_a_float());
    for id in [10, 11, 20, 30] {
        let (pane, inside) = letters_in_pane(ws.as_ref(), id);
        assert_eq!(inside, vec![pane.clone()], "pane {id}: one letter, on the pane");
        assert_eq!(
            node_at(ws.as_ref(), &pane).base().key,
            Some(crate::chrome::pane_key(PaneId(id))),
            "under the name its sidebar row answers to"
        );
        let intent = heca_grid_ui::hint_intent(ws.as_ref(), &pane)
            .expect("the letter says what it does, so a policy can refuse it");
        assert_eq!(intent.action, "focus_pane");
        assert_eq!(
            intent.args.get("pane_id"),
            Some(&heca_view::PropValue::Int(id as i64))
        );
    }
}

/// **A column is somewhere a pane is moved to, not somewhere `prefix+/` sends you** — the same rule
/// its sidebar view already follows. Lettered in the ordinary picker, it was a second letter for its
/// first pane.
#[test]
fn a_column_is_not_lettered_by_the_ordinary_picker() {
    let (_window, ws) = laid_out(&two_columns());
    let targets: Vec<Vec<usize>> = heca_grid_ui::collect_hints(ws.as_ref())
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    for col in [1, 2] {
        let column = path_of(ws.as_ref(), &crate::chrome::column_key(ColumnId(col)))
            .expect("the column is built");
        assert!(
            !targets.contains(&column),
            "column {col} wears a letter of its own"
        );
    }
}

/// **Pressing a pane's letter focuses the pane and puts the keyboard in what runs there.**
#[test]
fn picking_a_pane_gives_what_runs_in_it_the_keyboard() {
    let (pane_cb, picked) = recording_callbacks();
    let (_window, mut ws) = laid_out_with(
        &two_columns(),
        WorkspaceSeams {
            pane: pane_cb,
            ..seams()
        },
    );
    let pane = path_of(ws.as_ref(), &crate::chrome::pane_key(PaneId(11))).expect("built");
    assert!(heca_grid_ui::fire_hint(ws.as_mut(), &pane));
    assert_eq!(*picked.borrow(), vec![PaneId(11)], "the pane was focused");
    assert_eq!(
        heca_grid_ui::keyboard_holder(ws.as_ref()),
        Some(content_of(ws.as_ref(), 11)),
        "the keyboard is in what runs in the pane"
    );
}

/// **From a dock too.** The keyboard is in a terminal placed in a dock — which keeps a letter of
/// its own, since nothing hands it a keyboard — and the pane's letter, picked the way the app picks
/// a pane (on the pane's own tree), still brings the keyboard into the pane.
#[test]
fn a_panes_letter_takes_the_keyboard_out_of_a_dock() {
    let (_w, ws) = laid_out(&two_columns());
    let mut window = Flex::row();
    window
        .base_mut()
        .children
        .push(heca_grid_ui::WidgetModel::new(Flex::column().child(Terminal::unbound())));
    window.base_mut().children.push(ws);
    let docked = vec![0, 0];
    assert!(
        heca_grid_ui::collect_hints(&window)
            .iter()
            .any(|(path, _)| *path == docked),
        "a terminal in a dock keeps its letter"
    );
    assert!(heca_grid_ui::fire_hint(&mut window, &docked));
    let dock_terminal = node_at(&window, &docked).base().focus_id();
    assert_eq!(heca_grid_ui::keyboard_holder(&window), Some(dock_terminal));

    let key = crate::chrome::pane_key(PaneId(20));
    let pane = heca_grid_ui::node_with_key_mut(&mut window, &key).expect("built");
    assert!(heca_grid_ui::fire_hint(pane, &[]));
    heca_grid_ui::settle_focus(&mut window);
    assert_eq!(
        heca_grid_ui::keyboard_holder(&window),
        Some(content_of(&window, 20)),
        "the keyboard left the dock for the pane"
    );
}

/// **Picking a pane by name letters the pane** — `prefix+q` asks for `pane:<id>`.
#[test]
fn a_pane_asked_for_by_name_wears_the_letter() {
    let (_window, ws) = laid_out(&two_columns());
    let key = crate::chrome::pane_key(PaneId(20));
    assert!(heca_grid_ui::offer_hint_by_key(
        ws.as_ref(),
        &key,
        Some("q".into())
    ));
    let pane = path_of(ws.as_ref(), &key).expect("built");
    assert_eq!(
        node_at(ws.as_ref(), &pane).base().hint_label.get(),
        Some("q".into())
    );
}

/// **A terminal is named "terminal", not after the button drawn over it** — its derived name is
/// what a letter is remembered by, and its scroll button's text ("12 lines above") changes as you
/// scroll, which moved the terminal's letter with it.
#[test]
fn a_terminal_is_named_for_what_it_is() {
    use heca_grid_ui::Component as _;
    assert_eq!(Terminal::unbound().text_summary().as_deref(), Some("terminal"));
}
