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

/// **A field bound to an outside text shows what is typed, writes it out, and shows what is written
/// in** — the drawn text, not only the value: the field once wrote the outside signal while its own
/// label kept showing an empty one.
#[test]
fn a_field_bound_to_an_outside_text_draws_and_shares_it_both_ways() {
    let drawn = |input: &mut Input| -> Vec<String> {
        LayoutEngine::new().compute(input, Size::new(300.0, 60.0));
        let theme = Theme::default();
        let mut scene = heca_grid_ui::Scene::new();
        {
            let mut cx = heca_grid_ui::PaintCx::new(&mut scene, &theme);
            heca_grid_ui::paint_child(input, &mut cx);
        }
        scene
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCommand::Text(text) => Some(text.text.clone()),
                _ => None,
            })
            .collect()
    };
    let query = heca_grid_ui::reactive::signal(String::new());
    let mut input = Input::new().value(query);
    give_keyboard(&mut input);
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));
    type_text(&mut input, "Lib");
    assert_eq!(
        query.get_untracked(),
        "Lib",
        "what is typed reaches the outside text"
    );
    assert!(
        drawn(&mut input).iter().any(|t| t == "Lib"),
        "and the field draws it"
    );
    query.set("Movies".into());
    assert!(
        drawn(&mut input).iter().any(|t| t == "Movies"),
        "what is written in is drawn"
    );
}

/// **A clearable field shows an × while it holds text, and a click on it empties the field** —
/// reported through `on_change` like typing, with the keyboard still in the field, and the × gone
/// once the field is empty. Found by what is drawn.
#[test]
fn a_clearable_fields_cross_empties_it_and_keeps_the_keyboard() {
    use heca_grid_ui::{Action, SignalData, widgets::Glyph};
    use std::cell::RefCell;
    use std::rc::Rc;

    let said: Rc<RefCell<Vec<String>>> = Rc::default();
    let sink = said.clone();
    let mut input = Input::new().clearable(true).on_change(move |a: Action| {
        if let SignalData::String(text) = a.data {
            sink.borrow_mut().push(text);
        }
    });
    give_keyboard(&mut input);
    let size = Size::new(300.0, 60.0);
    LayoutEngine::new().compute(&mut input, size);
    let cross = Glyph::XCircle
        .primary_char()
        .expect("the × has a glyph")
        .to_string();
    let cross_at = |input: &mut Input| {
        LayoutEngine::new().compute(input, size);
        let theme = Theme::default();
        let mut scene = heca_grid_ui::Scene::new();
        heca_grid_ui::paint_child(input, &mut heca_grid_ui::PaintCx::new(&mut scene, &theme));
        scene.iter().find_map(|c| match c {
            DrawCommand::Text(t) if t.text.starts_with(&cross) => Some(t.rect),
            _ => None,
        })
    };
    assert!(cross_at(&mut input).is_none(), "an empty field shows no ×");

    type_text(&mut input, "lib");
    let at = cross_at(&mut input).expect("a field with text shows the ×");
    let field = input.base().bounds;
    assert!(
        at.loc.x > field.loc.x + field.size.w / 2.0,
        "the × {at:?} sits at the field's right end {field:?}"
    );
    click_at(
        &mut input,
        Point::new(at.loc.x + at.size.w / 2.0, at.loc.y + at.size.h / 2.0),
        PointerButton::Left,
    );
    assert_eq!(
        input.text_signal().get_untracked(),
        "",
        "the click emptied the field"
    );
    assert_eq!(
        said.borrow().last().map(String::as_str),
        Some(""),
        "reported like typing"
    );
    assert!(cross_at(&mut input).is_none(), "and the × is gone");
    type_text(&mut input, "x");
    assert_eq!(
        input.text_signal().get_untracked(),
        "x",
        "the keyboard stayed in the field"
    );
}

/// **`clear_input` empties any field, wherever the caret is** — the keyboard twin of the ×, on the
/// default keys (Cmd/Super+Shift+Backspace, Ctrl+Shift+Backspace).
#[test]
fn clear_input_empties_the_field_wherever_the_caret_is() {
    use heca_grid_ui::{GridKey, KeyPress, Keymap, Modifiers};
    for mods in [
        Modifiers {
            meta: true,
            shift: true,
            ..Default::default()
        },
        Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        },
    ] {
        let mut input = Input::new();
        give_keyboard(&mut input);
        LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));
        type_text(&mut input, "library");
        heca_grid_ui::dispatch(
            &mut input,
            &Event::Key {
                key: GridKey::ArrowLeft,
                pressed: true,
            },
        );
        Keymap::with_defaults().deliver_press(
            &KeyPress {
                key: GridKey::Backspace,
                text: None,
                mods,
            },
            |event| heca_grid_ui::dispatch(&mut input, event),
        );
        assert_eq!(
            input.text_signal().get_untracked(),
            "",
            "{mods:?} emptied the field"
        );
    }
}

/// **Cmd+Backspace stays the field's own "delete to the line start"** — `clear_input` is on the
/// Shift chords, so the plain Cmd chord never empties the field.
#[test]
fn cmd_backspace_is_not_clear_input() {
    use heca_grid_ui::{GridKey, Keymap, Modifiers, WidgetIntent};
    let meta = Modifiers {
        meta: true,
        ..Default::default()
    };
    assert!(
        !Keymap::with_defaults()
            .resolve(GridKey::Backspace, meta)
            .contains(&WidgetIntent::ClearInput),
        "Cmd+Backspace is not bound to clear_input"
    );
}

/// **The × answers the pointer**: hovered, it paints more than at rest — the hover frame of an
/// `IconButton` — so it reads as something to press.
#[test]
fn a_clearable_fields_cross_lights_up_under_the_pointer() {
    use heca_grid_ui::widgets::Glyph;
    let mut input = Input::new().clearable(true);
    give_keyboard(&mut input);
    let size = Size::new(300.0, 60.0);
    LayoutEngine::new().compute(&mut input, size);
    type_text(&mut input, "lib");
    LayoutEngine::new().compute(&mut input, size);
    let theme = Theme::default();
    let cross = Glyph::XCircle
        .primary_char()
        .expect("the × has a glyph")
        .to_string();
    let frame = |input: &Input| {
        let mut scene = heca_grid_ui::Scene::new();
        heca_grid_ui::paint_child(input, &mut heca_grid_ui::PaintCx::new(&mut scene, &theme));
        scene
    };
    let at = frame(&input)
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text(t) if t.text.starts_with(&cross) => Some(t.rect),
            _ => None,
        })
        .expect("the × is drawn");
    let field_width = input.base().bounds.size.w;
    // Painted boxes around the × that are smaller than the field: the ×'s own frame.
    let rects_near = |scene: &heca_grid_ui::Scene| {
        scene
            .iter()
            .filter(|c| {
                matches!(c, DrawCommand::Rect(r)
                    if r.rect.loc.x <= at.loc.x
                        && r.rect.loc.x + r.rect.size.w >= at.loc.x + at.size.w
                        && r.rect.size.w < field_width
                        && r.fill.a > 0)
            })
            .count()
    };
    let rest = rects_near(&frame(&input));
    heca_grid_ui::dispatch(
        &mut input,
        &Event::pointer_moved(Point::new(
            at.loc.x + at.size.w / 2.0,
            at.loc.y + at.size.h / 2.0,
        )),
    );
    for _ in 0..30 {
        input.tick(0.05);
    }
    let hovered = rects_near(&frame(&input));
    assert!(
        hovered > rest,
        "hovered the × paints a frame: {rest} at rest, {hovered} hovered"
    );
}
