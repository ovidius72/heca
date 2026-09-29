mod common;

use common::{click_at, give_keyboard, type_text};
use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, Event, LayoutEngine, Point, Size, Theme};

#[test]
fn input_typing_emits_change_and_builds_text() {
    use heca_grid_ui::{Action, SignalData};
    use std::cell::RefCell;
    use std::rc::Rc;

    let log: Rc<RefCell<Vec<Action>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    let mut input = Input::new().on_change(move |a| sink.borrow_mut().push(a));
    give_keyboard(&mut input);
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));

    type_text(&mut input, "H");
    type_text(&mut input, "i");
    type_text(&mut input, " ");
    type_text(&mut input, "5");

    assert_eq!(input.value_str(), "Hi 5");
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value(
            "input-change",
            SignalData::String("Hi 5".into())
        )),
    );
}

#[test]
fn input_backspace_and_midword_insert_respect_cursor() {
    let mut input = Input::new().value("abc");
    give_keyboard(&mut input);
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));

    // Caret starts at end (after 'c'). Move left → between 'b' and 'c'.
    heca_grid_ui::dispatch(
        &mut input,
        &Event::Key {
            key: GridKey::ArrowLeft,
            pressed: true,
        },
    );
    heca_grid_ui::dispatch(
        &mut input,
        &Event::Key {
            key: GridKey::Backspace,
            pressed: true,
        },
    );
    assert_eq!(
        input.value_str(),
        "ac",
        "backspace removes char before caret"
    );

    type_text(&mut input, "X");
    assert_eq!(input.value_str(), "aXc", "insert lands at the caret");
}

#[test]
fn input_placeholder_shows_only_when_empty_and_unfocused() {
    let theme = Theme::default();
    let texts = |input: &Input| -> Vec<String> {
        let scene = common::paint(input, &theme);
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    };

    let mut empty = Input::new().placeholder("CALLSIGN");
    LayoutEngine::new().compute(&mut empty, Size::new(300.0, 60.0));
    assert_eq!(texts(&empty), vec!["CALLSIGN".to_string()]);

    let mut filled = Input::new().value("ZED");
    LayoutEngine::new().compute(&mut filled, Size::new(300.0, 60.0));
    assert_eq!(texts(&filled), vec!["ZED".to_string()]);
}

#[test]
fn input_click_cycle_selects_word_then_all_then_clears() {
    let mut input = Input::new().value("alpha beta gamma");
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));
    let y = input.base().bounds.loc.y + 5.0;
    // x inside the word "beta" (chars 6..10) — ~char 7 at advance 8.4, PAD 10.
    let x = input.base().bounds.loc.x + 10.0 + 60.0;
    // Consecutive clicks with no tick share the clock → counted as one run. Full clicks: the run
    // is press-and-release pairs, which is what the framework counts.
    let press = |i: &mut Input| click_at(i, Point::new(x, y), PointerButton::Left);

    press(&mut input); // 1: caret
    assert_eq!(input.selection(), None, "single click places a caret");
    press(&mut input); // 2: word
    assert_eq!(
        input.selected_text().as_deref(),
        Some("beta"),
        "double-click selects the word"
    );
    press(&mut input); // 3: all
    assert_eq!(
        input.selected_text().as_deref(),
        Some("alpha beta gamma"),
        "triple-click selects all"
    );
    press(&mut input); // 4: clear
    assert_eq!(input.selection(), None, "fourth click clears the selection");
}

#[test]
fn input_typing_replaces_selection() {
    let mut input = Input::new().value("hello");
    give_keyboard(&mut input);
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));
    let pos = Point::new(
        input.base().bounds.loc.x + 14.0,
        input.base().bounds.loc.y + 5.0,
    );

    click_at(&mut input, pos, PointerButton::Left); // caret
    click_at(&mut input, pos, PointerButton::Left); // word = whole "hello"
    assert_eq!(input.selected_text().as_deref(), Some("hello"));

    type_text(&mut input, "X");
    assert_eq!(input.value_str(), "X", "typing replaces the selection");
    assert_eq!(input.selection(), None, "selection cleared after replace");
}

#[test]
fn input_ctrl_backspace_deletes_previous_word() {
    use heca_grid_ui::Modifiers;
    let mut input = Input::new().value("alpha beta");
    give_keyboard(&mut input);
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));

    heca_grid_ui::dispatch(
        &mut input,
        &Event::ModifiersChanged(Modifiers {
            ctrl: true,
            ..Default::default()
        }),
    );
    heca_grid_ui::dispatch(
        &mut input,
        &Event::Key {
            key: GridKey::Backspace,
            pressed: true,
        },
    );
    assert_eq!(input.value_str(), "alpha ", "deletes the word at the caret");
    heca_grid_ui::dispatch(
        &mut input,
        &Event::Key {
            key: GridKey::Backspace,
            pressed: true,
        },
    );
    assert_eq!(
        input.value_str(),
        "",
        "again removes the word + preceding space"
    );
}

#[test]
fn input_alt_delete_removes_next_word() {
    use heca_grid_ui::Modifiers;
    let mut input = Input::new().value("alpha beta");
    give_keyboard(&mut input);
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));
    for _ in 0..20 {
        heca_grid_ui::dispatch(
            &mut input,
            &Event::Key {
                key: GridKey::ArrowLeft,
                pressed: true,
            },
        );
    }
    // On macOS the word modifier is Alt/Option — accepted cross-platform.
    heca_grid_ui::dispatch(
        &mut input,
        &Event::ModifiersChanged(Modifiers {
            alt: true,
            ..Default::default()
        }),
    );
    heca_grid_ui::dispatch(
        &mut input,
        &Event::Key {
            key: GridKey::Delete,
            pressed: true,
        },
    );
    assert_eq!(
        input.value_str(),
        " beta",
        "forward word delete from the start"
    );
}

#[test]
fn meta_backspace_and_delete_clear_to_boundary() {
    use heca_grid_ui::Modifiers;
    let arrow_left = |i: &mut Input| {
        heca_grid_ui::dispatch(
            &mut *i,
            &Event::Key {
                key: GridKey::ArrowLeft,
                pressed: true,
            },
        );
    };

    // Meta+Backspace deletes from the caret to the start.
    let mut a = Input::new().value("alpha beta");
    give_keyboard(&mut a);
    LayoutEngine::new().compute(&mut a, Size::new(400.0, 60.0));
    for _ in 0..4 {
        arrow_left(&mut a); // caret 10 → 6 (start of "beta")
    }
    heca_grid_ui::dispatch(
        &mut a,
        &Event::ModifiersChanged(Modifiers {
            meta: true,
            ..Default::default()
        }),
    );
    heca_grid_ui::dispatch(
        &mut a,
        &Event::Key {
            key: GridKey::Backspace,
            pressed: true,
        },
    );
    assert_eq!(a.value_str(), "beta", "meta+backspace deletes to start");
    // **Meta comes back up.** Modifiers are device state the framework keeps once, not a copy each
    // widget owns, so a second field built here still sees what is held — exactly as a second field
    // in the real app does. Releasing is the host's next event, so the test sends it.
    heca_grid_ui::dispatch(&mut a, &Event::ModifiersChanged(Modifiers::default()));

    // Meta+Delete deletes from the caret to the end.
    let mut b = Input::new().value("alpha beta");
    give_keyboard(&mut b);
    LayoutEngine::new().compute(&mut b, Size::new(400.0, 60.0));
    for _ in 0..5 {
        arrow_left(&mut b); // caret 10 → 5 (after "alpha")
    }
    heca_grid_ui::dispatch(
        &mut b,
        &Event::ModifiersChanged(Modifiers {
            meta: true,
            ..Default::default()
        }),
    );
    heca_grid_ui::dispatch(
        &mut b,
        &Event::Key {
            key: GridKey::Delete,
            pressed: true,
        },
    );
    assert_eq!(b.value_str(), "alpha", "meta+delete deletes to end");
    heca_grid_ui::dispatch(&mut b, &Event::ModifiersChanged(Modifiers::default()));
}

#[test]
fn shift_arrow_extends_and_shrinks_char_selection() {
    use heca_grid_ui::Modifiers;
    let mut input = Input::new().value("hello");
    give_keyboard(&mut input);
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));
    let left = |i: &mut Input| {
        heca_grid_ui::dispatch(
            &mut *i,
            &Event::Key {
                key: GridKey::ArrowLeft,
                pressed: true,
            },
        )
    };
    let right = |i: &mut Input| {
        heca_grid_ui::dispatch(
            &mut *i,
            &Event::Key {
                key: GridKey::ArrowRight,
                pressed: true,
            },
        )
    };

    heca_grid_ui::dispatch(
        &mut input,
        &Event::ModifiersChanged(Modifiers {
            shift: true,
            ..Default::default()
        }),
    );
    left(&mut input);
    assert_eq!(input.selected_text().as_deref(), Some("o"));
    left(&mut input);
    assert_eq!(input.selected_text().as_deref(), Some("lo"));
    right(&mut input);
    assert_eq!(input.selected_text().as_deref(), Some("o"));
    right(&mut input);
    assert_eq!(
        input.selection(),
        None,
        "shrinking onto the anchor deselects"
    );
}

#[test]
fn shift_ctrl_arrow_selects_to_boundary() {
    use heca_grid_ui::Modifiers;
    let mut input = Input::new().value("alpha beta");
    give_keyboard(&mut input);
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));

    heca_grid_ui::dispatch(
        &mut input,
        &Event::ModifiersChanged(Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        }),
    );
    heca_grid_ui::dispatch(
        &mut input,
        &Event::Key {
            key: GridKey::ArrowLeft,
            pressed: true,
        },
    );
    assert_eq!(
        input.selected_text().as_deref(),
        Some("alpha beta"),
        "shift+ctrl+left selects to the start"
    );
    heca_grid_ui::dispatch(
        &mut input,
        &Event::Key {
            key: GridKey::ArrowRight,
            pressed: true,
        },
    );
    assert_eq!(
        input.selection(),
        None,
        "extending back to the end deselects"
    );
}

#[test]
fn shift_alt_arrow_selects_by_word() {
    use heca_grid_ui::Modifiers;
    let mut input = Input::new().value("alpha beta gamma");
    give_keyboard(&mut input);
    LayoutEngine::new().compute(&mut input, Size::new(400.0, 60.0));

    heca_grid_ui::dispatch(
        &mut input,
        &Event::ModifiersChanged(Modifiers {
            alt: true,
            shift: true,
            ..Default::default()
        }),
    );
    heca_grid_ui::dispatch(
        &mut input,
        &Event::Key {
            key: GridKey::ArrowLeft,
            pressed: true,
        },
    );
    assert_eq!(
        input.selected_text().as_deref(),
        Some("gamma"),
        "first word back"
    );
    heca_grid_ui::dispatch(
        &mut input,
        &Event::Key {
            key: GridKey::ArrowLeft,
            pressed: true,
        },
    );
    assert_eq!(
        input.selected_text().as_deref(),
        Some("beta gamma"),
        "extends by another word"
    );
}

#[test]
fn input_edit_select_all_selects_without_typing() {
    use heca_grid_ui::WidgetIntent;
    // Select-all is host-configured (`edit_select_all`, default Ctrl+a / Cmd+a) and arrives as
    // the semantic `WidgetIntent::EditSelectAll`; a raw modified 'a' is never typed (separately).
    let mut input = Input::new().value("hello world");
    give_keyboard(&mut input);
    LayoutEngine::new().compute(&mut input, Size::new(400.0, 60.0));

    heca_grid_ui::dispatch(&mut input, &Event::Widget(WidgetIntent::EditSelectAll));
    assert_eq!(
        input.selected_text().as_deref(),
        Some("hello world"),
        "SelectAll selects the whole field"
    );
    assert_eq!(input.value_str(), "hello world", "nothing is typed");
}

#[test]
fn home_end_move_caret_to_bounds() {
    let mut input = Input::new().value("hello");
    give_keyboard(&mut input);
    LayoutEngine::new().compute(&mut input, Size::new(400.0, 60.0));

    heca_grid_ui::dispatch(
        &mut input,
        &Event::Key {
            key: GridKey::Home,
            pressed: true,
        },
    );
    type_text(&mut input, "X");
    assert_eq!(
        input.value_str(),
        "Xhello",
        "Home moves the caret to the start"
    );

    heca_grid_ui::dispatch(
        &mut input,
        &Event::Key {
            key: GridKey::End,
            pressed: true,
        },
    );
    type_text(&mut input, "Y");
    assert_eq!(
        input.value_str(),
        "XhelloY",
        "End moves the caret to the end"
    );
}

#[test]
fn shift_home_end_select_to_bounds() {
    use heca_grid_ui::Modifiers;
    let mut input = Input::new().value("hello");
    give_keyboard(&mut input);
    LayoutEngine::new().compute(&mut input, Size::new(400.0, 60.0));

    heca_grid_ui::dispatch(
        &mut input,
        &Event::ModifiersChanged(Modifiers {
            shift: true,
            ..Default::default()
        }),
    );
    heca_grid_ui::dispatch(
        &mut input,
        &Event::Key {
            key: GridKey::Home,
            pressed: true,
        },
    );
    assert_eq!(
        input.selected_text().as_deref(),
        Some("hello"),
        "Shift+Home selects to start"
    );
    heca_grid_ui::dispatch(
        &mut input,
        &Event::Key {
            key: GridKey::End,
            pressed: true,
        },
    );
    assert_eq!(
        input.selection(),
        None,
        "Shift+End back to the end deselects"
    );
}

#[test]
fn disabled_input_ignores_typing() {
    let mut input = Input::new().disabled(true);
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));
    type_text(&mut input, "x");
    assert!(input.value_str().is_empty(), "disabled input ignores keys");
    assert!(!input.focusable(), "disabled input is unfocusable");
}

#[test]
fn input_edit_deletes_char_and_deletes_to_line_start() {
    use heca_grid_ui::{Input, WidgetIntent};
    // The readline shortcuts are host-configured (`edit_delete_back` / `edit_delete_to_line_start`,
    // default Ctrl+h / Ctrl+u) and arrive as semantic `Edit*` intents, not a raw key.
    let mut inp = Input::new().value("hello world");
    give_keyboard(&mut inp);

    heca_grid_ui::dispatch(&mut inp, &Event::Widget(WidgetIntent::EditDeleteBack));
    assert_eq!(
        inp.value_str(),
        "hello worl",
        "EditDeleteBack removes one char back"
    );
    heca_grid_ui::dispatch(
        &mut inp,
        &Event::Widget(WidgetIntent::EditDeleteToLineStart),
    );
    assert_eq!(
        inp.value_str(),
        "",
        "EditDeleteToLineStart clears to the start of the line"
    );
}
