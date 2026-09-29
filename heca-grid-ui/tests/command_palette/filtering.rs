use super::*;

#[test]
fn command_palette_is_overlay_active_only_while_open() {
    use heca_grid_ui::Component;
    let (p, _) = palette_with_markers();
    assert!(!p.overlay_active() && !p.focusable(), "inert while closed");
    let p = p.default_open(true);
    assert!(
        p.overlay_active() && p.focusable(),
        "captures input while open"
    );
}

#[test]
fn command_palette_typing_filters_then_activate_runs_top_result() {
    use heca_grid_ui::{Component, WidgetIntent};
    let (mut p, ran) = palette_with_markers();
    p = p.default_open(true);

    // Type "tog" → "Toggle sidebar" is the top (only) match.
    for c in "tog".chars() {
        type_text(&mut p, &c.to_string());
    }
    // Nav is host-resolved: `activate` arrives as WidgetIntent::Activate.
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        3,
        "activate runs the filtered top result (Toggle sidebar)"
    );
    assert!(
        !p.overlay_active(),
        "palette closes after running a command"
    );
}

#[test]
fn command_palette_navigates_via_menu_nav() {
    use heca_grid_ui::WidgetIntent;
    let (mut p, ran) = palette_with_markers();
    p = p.default_open(true);

    // No query → all three; selection starts at 0. Down ×2 → idx 2, Up → idx 1.
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuDown));
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuDown));
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuUp));
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        2,
        "MenuDown ×2 then MenuUp lands on the 2nd command (Close pane)"
    );
}

/// Filtering matches the **label**. A description explains a command the user has already found;
/// ranking on it would surface a command whose label the query never mentioned.
#[test]
fn command_palette_filters_on_the_label_not_the_description() {
    use heca_grid_ui::{Command, CommandPalette, WidgetIntent};
    let ran = std::rc::Rc::new(std::cell::Cell::new(0u8));
    let (r1, r2) = (ran.clone(), ran.clone());
    let mut p = CommandPalette::new()
        .command(Command::new("Close pane", move || r1.set(1)).description("Splits nothing."))
        .command(Command::new("Split pane", move || r2.set(2)).description("Adds a column."))
        .default_open(true);

    for c in "split".chars() {
        type_text(&mut p, &c.to_string());
    }
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        2,
        "'split' found the command called Split pane, not the one whose description says 'Splits'",
    );
}

/// **A row renamed while the palette is open follows in place** — and is then found by its new
/// name, because the widget ranks the text it shows rather than the label it was built with.
#[test]
fn a_live_label_is_both_redrawn_and_matched_by_its_new_name() {
    use heca_grid_ui::reactive::SignalUpdate;
    use heca_grid_ui::{Command, CommandPalette, WidgetIntent};
    let ran = std::rc::Rc::new(std::cell::Cell::new(""));
    let (a, b) = (ran.clone(), ran.clone());
    let mut p = CommandPalette::new()
        .command(Command::new("zsh", move || a.set("first")).id("one"))
        .command(Command::new("bash", move || b.set("second")).id("two"))
        .default_open(true);

    // The host renames the first row — a pane's process changed, or it was renamed.
    p.label_signals()[0].set("nvim".to_string());

    for c in "nvi".chars() {
        type_text(&mut p, &c.to_string());
    }
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        "first",
        "the row was matched by the name it now shows, not the one it was built with",
    );
}
