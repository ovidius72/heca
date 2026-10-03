use super::super::component::Terminal;
use super::super::input::{Search, TerminalInput};
use super::super::testing::{laid_out_in, recording};
use super::super::viewport::Viewport;
use super::*;
use heca_config::appearance::ScrollbarVisibility;
use heca_grid_ui::component::{Event, GridKey, Handled, WidgetIntent};
use heca_grid_ui::event::PointerButton;
use heca_grid_ui::scene::{DrawCommand, Scene};
use heca_grid_ui::theme::Theme;
use heca_grid_ui::{LayoutEngine, PaintCx};

fn found(row: isize, start: usize, end: usize) -> SearchMatch {
    SearchMatch {
        stable_row: row,
        start_col: start,
        end_col: end,
    }
}

fn viewport() -> Viewport {
    Viewport {
        rows: 24,
        scrollback_rows: 124,
        offset: 0,
        scrollbar: ScrollbarVisibility::WhenNeeded,
        badge: true,
        cell: (10.0, 20.0),
        top_stable_row: 100,
        match_alpha: 64,
        current_match_alpha: 150,
        nominal_cell: (10.0, 20.0),
    }
}

/// A terminal in a 300 × 200 box whose owner is listening, and what it says.
fn placed() -> (
    Terminal,
    Box<dyn heca_grid_ui::Component>,
    std::rc::Rc<RefCell<Vec<TerminalInput>>>,
) {
    let (seams, said) = recording();
    let t = Terminal::new();
    t.bind(seams);
    t.show(&viewport());
    let root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    (t, root, said)
}

fn frame(root: &mut Box<dyn heca_grid_ui::Component>) {
    root.tick(0.0);
    LayoutEngine::new().compute(root.as_mut(), heca_grid_ui::Size::new(300.0, 200.0));
}

fn painted(root: &dyn heca_grid_ui::Component) -> Scene {
    let theme = Theme::default();
    let mut scene = Scene::new();
    heca_grid_ui::paint_child(root, &mut PaintCx::new(&mut scene, &theme));
    scene
}

fn said_search(said: &std::rc::Rc<RefCell<Vec<TerminalInput>>>) -> Vec<Search> {
    said.borrow()
        .iter()
        .filter_map(|i| match i {
            TerminalInput::Search(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

fn typed_into_the_program(said: &std::rc::Rc<RefCell<Vec<TerminalInput>>>) -> bool {
    said.borrow()
        .iter()
        .any(|i| matches!(i, TerminalInput::Text(_) | TerminalInput::Key { .. }))
}

// ── What the counter says ────────────────────────────────────────────────────────────────

#[test]
fn the_counter_is_silent_until_there_is_a_query() {
    assert_eq!(count_label(false, 0, None), None);
    assert_eq!(count_label(false, 5, Some(0)), None);
}

#[test]
fn the_counter_says_no_matches_or_where_you_are() {
    assert_eq!(count_label(true, 0, None).as_deref(), Some("no matches"));
    assert_eq!(count_label(true, 12, Some(2)).as_deref(), Some("3/12"));
    assert_eq!(count_label(true, 12, None).as_deref(), Some("0/12"));
}

// ── Where the matches are drawn ──────────────────────────────────────────────────────────

/// A match is drawn on its cells, counted from the terminal's own corner — wherever that is.
#[test]
fn a_match_is_drawn_on_its_cells_from_the_terminals_corner() {
    let slot = SearchSlot::default();
    slot.set_result(vec![found(103, 4, 9)], Some(0));
    slot.set_view(100, 24);

    let at = slot.highlights(Point::new(50.0, 70.0), (10.0, 20.0));

    assert_eq!(at.len(), 1);
    let (rect, current) = &at[0];
    assert!(*current, "the only match is the current one");
    assert_eq!(
        (rect.loc.x, rect.loc.y, rect.size.w, rect.size.h),
        (50.0 + 40.0, 70.0 + 60.0, 50.0, 20.0),
        "column 4, three rows down, five cells wide"
    );
}

/// Only the matches on screen are drawn; the rest are the scrollback's.
#[test]
fn matches_above_and_below_the_screen_are_not_drawn() {
    let slot = SearchSlot::default();
    slot.set_result(
        vec![
            found(99, 0, 3),
            found(100, 0, 3),
            found(123, 0, 3),
            found(124, 0, 3),
        ],
        None,
    );
    slot.set_view(100, 24);

    let rows: Vec<f64> = slot
        .highlights(Point::new(0.0, 0.0), (10.0, 20.0))
        .iter()
        .map(|(r, _)| r.loc.y)
        .collect();

    assert_eq!(
        rows,
        vec![0.0, 23.0 * 20.0],
        "the first and last rows of the screen"
    );
}

/// The highlights are in the picture the terminal paints, over its surface.
#[test]
fn a_terminal_paints_the_matches_it_was_shown() {
    let (t, mut root, _said) = placed();
    frame(&mut root);
    let before = painted(root.as_ref()).iter().count();

    t.show_matches(vec![found(101, 2, 6)], Some(0));
    let after = painted(root.as_ref()).iter().count();

    assert_eq!(after, before + 1, "one more rectangle: the match");
}

// ── Asking for a frame ───────────────────────────────────────────────────────────────────

/// **An open search must not redraw every frame.** The result is pushed whenever the owner has
/// one; the same one again changes nothing the user can see, so it asks for nothing.
#[test]
fn showing_the_same_result_twice_asks_for_no_frame() {
    let t = Terminal::new();

    assert!(
        t.show_matches(vec![found(1, 0, 2)], Some(0)),
        "a new result is a change"
    );
    assert!(
        !t.show_matches(vec![found(1, 0, 2)], Some(0)),
        "the same result again is not"
    );
    assert!(
        t.show_matches(vec![found(1, 0, 2)], None),
        "another current match is"
    );
    assert!(t.show_matches(vec![], None), "so is no matches at all");
}

/// With nothing highlighted, scrolling moves nothing that is drawn: no frame for it.
#[test]
fn scrolling_asks_for_a_frame_only_when_a_highlight_moves() {
    let slot = SearchSlot::default();
    assert!(!slot.set_view(10, 24), "no matches: nothing to move");
    assert!(!slot.set_view(11, 24));

    slot.set_result(vec![found(12, 0, 2)], Some(0));
    assert!(
        slot.set_view(5, 24),
        "matches shown: the screen moved under them"
    );
    assert!(!slot.set_view(5, 24), "and the same screen is quiet");
    assert!(slot.set_view(5, 30), "a taller screen moves them too");
}

// ── The bar ──────────────────────────────────────────────────────────────────────────────

fn bar_bounds(root: &dyn heca_grid_ui::Component) -> heca_core::layout::Rectangle {
    // The grid box → the terminal → its children: scrollbar, chip, then the search bar.
    root.base().children[0].base().children[2].base().bounds
}

fn terminal_bounds(root: &dyn heca_grid_ui::Component) -> heca_core::layout::Rectangle {
    root.base().children[0].base().bounds
}

/// The bar sits in the terminal it belongs to, at its bottom-right corner — opposite the chip.
#[test]
fn the_bar_sits_at_the_terminals_bottom_right() {
    let (t, mut root, _said) = placed();
    t.find();
    frame(&mut root);

    let (bar, term) = (bar_bounds(root.as_ref()), terminal_bounds(root.as_ref()));
    let (right_gap, bottom_gap) = (
        (term.loc.x + term.size.w) - (bar.loc.x + bar.size.w),
        (term.loc.y + term.size.h) - (bar.loc.y + bar.size.h),
    );
    assert!(
        right_gap >= 0.0 && bottom_gap >= 0.0,
        "the bar {bar:?} is inside its terminal {term:?}"
    );
    assert!(
        right_gap < term.size.w / 2.0 && bottom_gap < term.size.h / 2.0,
        "and in its bottom-right corner: gaps right={right_gap} bottom={bottom_gap}"
    );
}

/// It is placed by the terminal's own layout, so a terminal far from the window's origin gets its
/// bar beside it, not beside the window.
#[test]
fn a_terminal_far_from_the_origin_gets_its_bar_in_its_own_corner() {
    let (seams, _said) = recording();
    let t = Terminal::new();
    t.bind(seams);
    t.show(&viewport());
    let mut root = super::super::testing::header_above(Box::new(t.clone()), 80.0, 300.0, 300.0);
    t.find();
    root.tick(0.0);
    LayoutEngine::new().compute(root.as_mut(), heca_grid_ui::Size::new(300.0, 300.0));

    // grid → [header, terminal]; the terminal's third child is the bar.
    let term = root.base().children[1].base().bounds;
    let bar = root.base().children[1].base().children[2].base().bounds;
    assert!(
        bar.loc.y >= term.loc.y && bar.loc.y + bar.size.h <= term.loc.y + term.size.h,
        "the bar {bar:?} is within the terminal {term:?}, below the header"
    );
}

#[test]
fn the_bar_is_not_shown_until_a_search_opens() {
    let (t, mut root, _said) = placed();
    frame(&mut root);
    assert!(
        !root.base().children[0].base().children[2]
            .base()
            .visible
            .get_untracked(),
        "closed: no bar"
    );

    t.find();
    frame(&mut root);
    assert!(
        root.base().children[0].base().children[2]
            .base()
            .visible
            .get_untracked(),
        "open: the bar"
    );
}

// ── Typing into it ───────────────────────────────────────────────────────────────────────

/// Opening the search puts the keyboard in the field, so what is typed is the query — said once per
/// edit, and not typed into the program behind it.
#[test]
fn what_is_typed_after_find_is_the_query_and_not_the_programs() {
    let (t, mut root, said) = placed();
    t.find();
    frame(&mut root);

    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("a".into()));
    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("b".into()));

    assert_eq!(
        said_search(&said),
        vec![
            Search::Editing(true),
            Search::Query("a".into()),
            Search::Query("ab".into()),
        ],
        "the field took the keyboard, then one query per edit"
    );
    assert!(
        !typed_into_the_program(&said),
        "nothing reached the program"
    );
}

/// Keys the field does not take (an arrow, say) are not the program's either while it is there.
#[test]
fn a_key_the_field_does_not_take_is_not_typed_into_the_program() {
    let (t, mut root, said) = placed();
    t.find();
    frame(&mut root);

    let handled = heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::Key {
            key: GridKey::ArrowUp,
            pressed: true,
        },
    );

    assert_eq!(
        handled,
        Handled::Yes,
        "swallowed, so nothing behind it answers"
    );
    assert!(!typed_into_the_program(&said));
}

/// Enter leaves the field and keeps the matches: the keyboard is the terminal's again, and typing
/// is the program's.
#[test]
fn enter_gives_the_keyboard_back_to_the_terminal() {
    let (t, mut root, said) = placed();
    t.find();
    frame(&mut root);
    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("a".into()));

    heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::Key {
            key: GridKey::Enter,
            pressed: true,
        },
    );
    said.borrow_mut().clear();
    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("x".into()));

    assert_eq!(
        *said.borrow(),
        vec![
            TerminalInput::Search(Search::Editing(false)),
            TerminalInput::Text("x".into()),
        ],
        "the field let go, and the program heard the x"
    );
    assert!(t.search_open(), "and the search is still open for n / N");
}

/// Escape dismisses the search: it says so, the bar goes, and typing is the program's.
#[test]
fn escape_dismisses_the_search() {
    let (t, mut root, said) = placed();
    t.find();
    frame(&mut root);
    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("a".into()));

    heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::Key {
            key: GridKey::Escape,
            pressed: true,
        },
    );

    assert!(said_search(&said).contains(&Search::Close));
    assert!(!t.search_open());
    frame(&mut root);
    assert!(
        !root.base().children[0].base().children[2]
            .base()
            .visible
            .get_untracked(),
        "the bar went"
    );
}

// ── Asking for it ────────────────────────────────────────────────────────────────────────

/// A terminal that holds the keyboard opens its own search when the intent reaches it — whatever
/// key the user bound to it.
#[test]
fn the_find_intent_opens_the_search_of_the_terminal_that_holds_the_keyboard() {
    let (t, mut root, said) = placed();
    // A click gives it the keyboard, like any focusable.
    frame(&mut root);
    heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::pointer_pressed(heca_grid_ui::Point::new(45.0, 61.0), PointerButton::Left),
    );
    said.borrow_mut().clear();

    let taken = heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(WidgetIntent::Find));
    frame(&mut root);
    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("q".into()));

    assert_eq!(taken, Handled::Yes);
    assert!(t.search_open());
    assert!(said_search(&said).contains(&Search::Query("q".into())));
}

/// The step intents ask the owner for the next and previous match.
#[test]
fn the_step_intents_ask_for_the_next_and_previous_match() {
    let (_t, mut root, said) = placed();
    frame(&mut root);
    heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::pointer_pressed(heca_grid_ui::Point::new(45.0, 61.0), PointerButton::Left),
    );
    said.borrow_mut().clear();

    heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(WidgetIntent::FindNext));
    heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(WidgetIntent::FindPrevious));

    assert_eq!(
        said_search(&said),
        vec![
            Search::Step { forward: true },
            Search::Step { forward: false }
        ]
    );
}

/// The counter shows where the current match is among them.
#[test]
fn the_counter_shows_the_match_position() {
    let (t, mut root, _said) = placed();
    t.find();
    frame(&mut root);
    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("a".into()));
    t.show_matches(vec![found(101, 0, 1), found(105, 0, 1)], Some(1));
    frame(&mut root);

    let scene = painted(root.as_ref());
    assert!(
        scene
            .iter()
            .any(|c| matches!(c, DrawCommand::Text(t) if t.text == "2/2")),
        "the chip reads 2/2"
    );
}

// ── How strongly the matches are highlighted ─────────────────────────────────────────────

fn highlight_alphas(scene: &Scene) -> Vec<u8> {
    scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r) => Some(r.fill.a),
            _ => None,
        })
        .collect()
}

/// The strengths are the user's, said by the owner beside the viewport — the terminal holds no
/// number of its own: the current match is drawn at one, the others at the other.
#[test]
fn matches_are_drawn_at_the_strengths_the_owner_said() {
    let (t, mut root, _said) = placed();
    let mut look = viewport();
    look.match_alpha = 30;
    look.current_match_alpha = 200;
    t.show(&look);
    frame(&mut root);
    t.show_matches(vec![found(101, 0, 2), found(102, 0, 2)], Some(1));

    let alphas = highlight_alphas(&painted(root.as_ref()));

    assert!(alphas.contains(&30), "the other match: {alphas:?}");
    assert!(alphas.contains(&200), "the current match: {alphas:?}");
}

/// A new strength is a frame only while something is highlighted to show it.
#[test]
fn a_new_strength_asks_for_a_frame_only_while_matches_are_shown() {
    let slot = SearchSlot::default();
    assert!(
        !slot.set_look(10, 20),
        "nothing highlighted: nothing to see"
    );

    slot.set_result(vec![found(1, 0, 2)], Some(0));
    assert!(!slot.set_look(10, 20), "the same strengths again");
    assert!(
        slot.set_look(11, 20),
        "a changed one, with a match on screen"
    );
}

// ── Stepping: the terminal holds the matches, so it is the one that steps ────────────────

/// Next and previous wrap around the matches, and the terminal says which one it is on.
#[test]
fn stepping_wraps_around_the_matches() {
    let t = Terminal::new();
    t.show_matches(
        vec![found(1, 0, 1), found(2, 0, 1), found(3, 0, 1)],
        Some(2),
    );

    assert!(t.step_match(true));
    assert_eq!(
        t.current_match(),
        Some(found(1, 0, 1)),
        "past the last wraps to the first"
    );
    assert!(t.step_match(false));
    assert_eq!(
        t.current_match(),
        Some(found(3, 0, 1)),
        "before the first wraps to the last"
    );
    assert!(t.step_match(false));
    assert_eq!(t.current_match(), Some(found(2, 0, 1)));
}

/// With nothing found there is nowhere to go, and nothing to redraw.
#[test]
fn stepping_with_no_matches_goes_nowhere() {
    let t = Terminal::new();
    assert!(!t.step_match(true));
    assert_eq!(t.current_match(), None);
}
