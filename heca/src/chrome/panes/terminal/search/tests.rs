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
        open_link_modifier: heca_config::theme::ModifierKey::Super,
    }
}

/// A terminal in a 300 × 200 box whose owner is listening, and what it says.
fn placed() -> (
    Terminal,
    heca_grid_ui::WidgetModel,
    std::rc::Rc<RefCell<Vec<TerminalInput>>>,
) {
    let (seams, said) = recording();
    let t = Terminal::unbound();
    t.bind(seams);
    t.show(&viewport());
    let root = laid_out_in(heca_grid_ui::WidgetModel::new(t.clone()), 300.0, 200.0);
    (t, root, said)
}

fn frame(root: &mut heca_grid_ui::WidgetModel) {
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
    let t = Terminal::unbound();

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
    let t = Terminal::unbound();
    t.bind(seams);
    t.show(&viewport());
    let mut root = super::super::testing::header_above(
        heca_grid_ui::WidgetModel::new(t.clone()),
        80.0,
        300.0,
        300.0,
    );
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
        !root.base().children[0].base().children[2].is_shown(),
        "closed: no bar"
    );

    t.find();
    frame(&mut root);
    assert!(
        root.base().children[0].base().children[2].is_shown(),
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

/// Press `key` the way a window does — its text, the key, then the `[keys.widgets]` intents it is
/// bound to — with heca's default bindings.
fn press(root: &mut heca_grid_ui::WidgetModel, key: GridKey) -> Handled {
    heca_grid_ui::Keymap::with_defaults().deliver_press(
        &heca_grid_ui::KeyPress {
            key,
            text: None,
            mods: Default::default(),
        },
        |event| heca_grid_ui::dispatch(root.as_mut(), event),
    )
}

/// Keys the field does not take are not the program's either while it is there — they go on to
/// the widget bindings, and nothing is typed into the process behind the bar.
#[test]
fn a_key_the_field_does_not_take_is_not_typed_into_the_program() {
    let (t, mut root, said) = placed();
    t.find();
    frame(&mut root);
    press(&mut root, GridKey::Tab);
    assert!(!typed_into_the_program(&said));
}

/// **Enter, ↓ and Ctrl+j go to the next match, ↑ and Ctrl+k to the previous one** — `activate`,
/// `menu_down` and `menu_up` of `[keys.widgets]` — and the keyboard stays in the field.
#[test]
fn enter_and_the_vertical_keys_step_through_the_matches_from_the_field() {
    let (t, mut root, said) = placed();
    t.find();
    frame(&mut root);
    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("a".into()));
    said.borrow_mut().clear();

    for key in [GridKey::Enter, GridKey::ArrowDown] {
        assert_eq!(press(&mut root, key), Handled::Yes, "{key:?} is answered");
    }
    assert_eq!(press(&mut root, GridKey::ArrowUp), Handled::Yes);
    assert_eq!(
        said_search(&said),
        vec![
            Search::Step { forward: true },
            Search::Step { forward: true },
            Search::Step { forward: false },
        ],
    );
    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("b".into()));
    assert!(
        !typed_into_the_program(&said),
        "the keyboard is still in the field"
    );
    assert!(t.search_open());
}

/// Escape dismisses the search: it says so, the bar goes, and typing is the program's.
#[test]
fn escape_dismisses_the_search() {
    let (t, mut root, said) = placed();
    t.find();
    frame(&mut root);
    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("a".into()));

    press(&mut root, GridKey::Escape);

    assert!(said_search(&said).contains(&Search::Close));
    assert!(!t.search_open());
    frame(&mut root);
    assert!(
        !root.base().children[0].base().children[2].is_shown(),
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
        "the chip reads 2/2 in full, even in a narrow terminal"
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
    let t = Terminal::unbound();
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
    let t = Terminal::unbound();
    assert!(!t.step_match(true));
    assert_eq!(t.current_match(), None);
}

/// **The field does not move when the counter appears** — the counter goes above it — and the
/// bar's arrows step through the matches: up to the previous one, down to the next. Found by what
/// is drawn, the way the user sees it.
#[test]
fn the_field_does_not_move_when_the_counter_appears_and_the_arrows_step() {
    let (t, mut root, said) = placed();
    t.find();
    frame(&mut root);
    let drawn = |root: &heca_grid_ui::WidgetModel| -> Vec<(String, heca_grid_ui::Rectangle)> {
        painted(root.as_ref())
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) => Some((t.text.clone(), t.rect)),
                _ => None,
            })
            .collect()
    };
    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("a".into()));
    frame(&mut root);
    let query_at = |root: &heca_grid_ui::WidgetModel| {
        drawn(root)
            .into_iter()
            .find(|(text, _)| text == "a")
            .expect("the field draws the query")
            .1
    };
    let before = query_at(&root);
    t.show_matches(vec![found(101, 0, 1), found(105, 0, 1)], Some(0));
    frame(&mut root);
    let after = query_at(&root);
    assert_eq!(
        before.loc, after.loc,
        "the counter appeared without moving the field"
    );
    let counter = drawn(&root)
        .into_iter()
        .find(|(text, _)| text == "1/2")
        .expect("the counter is drawn")
        .1;
    assert!(
        counter.loc.y + counter.size.h <= after.loc.y,
        "the counter {counter:?} is above the field {after:?}"
    );

    // The field's × closes the field on the right; the arrows are the icons right of it, up first.
    let cross = heca_grid_ui::widgets::Glyph::XCircle
        .primary_char()
        .expect("the × has a glyph")
        .to_string();
    let field_end = drawn(&root)
        .into_iter()
        .filter(|(text, _)| text.starts_with(&cross))
        .map(|(_, r)| r.loc.x + r.size.w)
        .fold(after.loc.x + after.size.w, f64::max);
    let mut arrows: Vec<f64> = drawn(&root)
        .into_iter()
        .filter(|(_, r)| r.loc.x > field_end && r.loc.y >= counter.loc.y + counter.size.h)
        .map(|(_, r)| r.loc.x + r.size.w / 2.0)
        .collect();
    arrows.dedup();
    assert_eq!(arrows.len(), 2, "an up and a down arrow beside the field");
    let y = after.loc.y + after.size.h / 2.0;
    said.borrow_mut().clear();
    for x in [arrows[1], arrows[0]] {
        let at = heca_grid_ui::Point::new(x, y);
        heca_grid_ui::dispatch(
            root.as_mut(),
            &Event::pointer_pressed(at, PointerButton::Left),
        );
        heca_grid_ui::dispatch(
            root.as_mut(),
            &Event::pointer_released(at, PointerButton::Left),
        );
    }
    let steps: Vec<Search> = said_search(&said)
        .into_iter()
        .filter(|s| matches!(s, Search::Step { .. }))
        .collect();
    assert_eq!(
        steps,
        vec![
            Search::Step { forward: true },
            Search::Step { forward: false }
        ],
    );
    frame(&mut root);
    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("b".into()));
    assert!(
        !typed_into_the_program(&said),
        "the keyboard is back in the field"
    );
}

/// **The search box is one opaque panel** — the theme's `surface` colour behind the field, so the
/// terminal's text never shows through — and in a terminal too narrow for its full width it stays
/// inside, off the terminal's left edge, rather than leaning on it.
#[test]
fn the_search_box_is_an_opaque_panel_inside_the_terminal() {
    let narrow = 160.0;
    let (seams, _said) = recording();
    let t = Terminal::unbound();
    t.bind(seams);
    t.show(&viewport());
    let mut root = laid_out_in(heca_grid_ui::WidgetModel::new(t.clone()), narrow, 200.0);
    t.find();
    root.tick(0.0);
    LayoutEngine::new().compute(root.as_mut(), heca_grid_ui::Size::new(narrow, 200.0));
    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("a".into()));
    root.tick(0.0);
    LayoutEngine::new().compute(root.as_mut(), heca_grid_ui::Size::new(narrow, 200.0));

    let theme = Theme::default();
    let scene = painted(root.as_ref());
    let query = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text(t) if t.text == "a" => Some(t.rect),
            _ => None,
        })
        .expect("the field draws the query");
    let panel = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Rect(r)
                if r.fill == theme.colors.surface
                    && r.rect.loc.x <= query.loc.x
                    && r.rect.loc.x + r.rect.size.w >= query.loc.x + query.size.w =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .expect("an opaque panel in the theme's surface colour behind the field");
    assert!(
        panel.loc.x > 0.0 && panel.loc.x + panel.size.w < narrow,
        "the panel {panel:?} stays inside a {narrow}-wide terminal, off both edges"
    );
}

/// **The row holding the search box takes no clicks of its own**: a press beside the panel, on the
/// terminal's last lines, still reaches the terminal (to select, or for its program's mouse).
#[test]
fn a_press_beside_the_search_box_reaches_the_terminal() {
    let narrow = 300.0;
    let (seams, said) = recording();
    let t = Terminal::unbound();
    t.bind(seams);
    t.show(&viewport());
    let mut root = laid_out_in(heca_grid_ui::WidgetModel::new(t.clone()), narrow, 200.0);
    t.find();
    root.tick(0.0);
    LayoutEngine::new().compute(root.as_mut(), heca_grid_ui::Size::new(narrow, 200.0));
    let theme = Theme::default();
    let panel = painted(root.as_ref())
        .iter()
        .find_map(|c| match c {
            DrawCommand::Rect(r) if r.fill == theme.colors.surface => Some(r.rect),
            _ => None,
        })
        .expect("the search panel is drawn");
    let beside = heca_grid_ui::Point::new(panel.loc.x / 2.0, panel.loc.y + panel.size.h / 2.0);
    said.borrow_mut().clear();
    heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::pointer_pressed(beside, PointerButton::Left),
    );
    assert!(
        said.borrow()
            .iter()
            .any(|i| matches!(i, TerminalInput::Press { .. })),
        "the terminal heard the press beside the panel: {:?}",
        said.borrow()
    );
}
