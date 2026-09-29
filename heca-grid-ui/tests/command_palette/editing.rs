use super::*;

/// The query field is a real [`Input`], so the configurable `[keys.widgets]` **edit** intents reach
/// it — `edit_select_all` (Ctrl+a), `edit_delete_back` (Ctrl+h), `edit_delete_to_line_start`
/// (Ctrl+u). The palette answers four nav intents and **forwards the rest to the field**; swallowing
/// them was why typing in the palette had none of the editing every other field has.
#[test]
fn command_palette_query_takes_the_edit_intents() {
    use heca_grid_ui::{Command, CommandPalette, WidgetIntent};
    let ran = std::rc::Rc::new(std::cell::Cell::new(0u8));
    let (r1, r2) = (ran.clone(), ran.clone());
    let mut p = CommandPalette::new()
        .command(Command::new("Close pane", move || r1.set(1)))
        .command(Command::new("Toggle sidebar", move || r2.set(2)))
        .default_open(true);

    // Type a query that matches only "Close pane", then select-all + type over it: the field must
    // replace the selection, leaving "tog" → "Toggle sidebar".
    for c in "close".chars() {
        type_text(&mut p, &c.to_string());
    }
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::EditSelectAll));
    for c in "tog".chars() {
        type_text(&mut p, &c.to_string());
    }
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        2,
        "Ctrl+a selected the whole query so typing replaced it"
    );

    // Ctrl+u (delete to line start) clears a query back to everything.
    let ran = std::rc::Rc::new(std::cell::Cell::new(0u8));
    let r = ran.clone();
    let mut p = CommandPalette::new()
        .command(Command::new("Close pane", move || r.set(1)))
        .default_open(true);
    for c in "zzz".chars() {
        type_text(&mut p, &c.to_string());
    }
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::EditDeleteToLineStart));
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        1,
        "Ctrl+u emptied the query, so the only command matched again"
    );
}

#[test]
fn command_palette_query_reuses_input_word_delete() {
    use heca_grid_ui::{Modifiers, WidgetIntent};
    let (mut p, ran) = palette_with_markers();
    p = p.default_open(true);

    // "Toggle xyz" matches nothing (no command contains "...xyz").
    for c in "Toggle xyz".chars() {
        type_text(&mut p, &c.to_string());
    }
    // Ctrl+Backspace word-deletes the whole "xyz" (not one char), leaving
    // "Toggle " — which now matches "Toggle sidebar". A char-delete would leave
    // "Toggle xy" (still no match), so this proves the Input editing is wired.
    heca_grid_ui::dispatch(
        &mut p,
        &Event::ModifiersChanged(Modifiers {
            ctrl: true,
            ..Default::default()
        }),
    );
    heca_grid_ui::dispatch(
        &mut p,
        &Event::Key {
            key: GridKey::Backspace,
            pressed: true,
        },
    );
    heca_grid_ui::dispatch(&mut p, &Event::ModifiersChanged(Modifiers::default()));
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        3,
        "Ctrl+Backspace word-delete leaves 'Toggle ' → runs Toggle sidebar"
    );
}
