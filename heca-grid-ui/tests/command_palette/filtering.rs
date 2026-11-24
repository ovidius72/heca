use super::*;

#[test]
fn command_palette_is_overlay_active_only_while_open() {
    use heca_grid_ui::Component;
    let (p, _) = palette_with_markers();
    assert!(!p.overlay_active() && !p.is_shown(), "inert while closed");
    let p = p.default_open(true);
    assert!(p.overlay_active() && p.is_shown(), "in front while open");
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

/// **The keyboard moves the selection, and the list follows it** — a row moved onto by the arrows
/// or Ctrl+j/k scrolls itself into view, so the twentieth of thirty is on screen when selected.
#[test]
fn the_list_scrolls_to_keep_the_selected_row_in_view() {
    use heca_grid_ui::{
        Command, CommandPalette, Component, Event, LayoutEngine, Size, Theme, WidgetIntent,
    };
    let theme = Theme::default();
    let viewport = Size::new(1200.0, 800.0);
    let mut p = CommandPalette::new();
    for i in 0..30 {
        p = p.command(Command::new(format!("Command {i:02}"), || {}).description("does a thing"));
    }
    let mut p = p.default_open(true);
    LayoutEngine::new().compute(&mut p, viewport);
    for _ in 0..20 {
        heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuDown));
        p.tick(0.0);
        LayoutEngine::new().compute(&mut p, viewport);
    }
    let y = common::texts(&common::paint_in(&p, &theme, viewport))
        .into_iter()
        .find(|t| t.text == "Command 20")
        .expect("the selected row is drawn")
        .rect
        .loc
        .y;
    assert!(
        y > 0.0 && y < viewport.h,
        "the selected row is on screen, at y={y}"
    );
}

/// **A palette given your signal follows it both ways** — up while it is true, and writing it
/// `false` when it closes itself on its dismiss key — with the keyboard on its query field while it
/// is up, so typing filters at once.
#[test]
fn a_palette_follows_the_signal_it_was_given() {
    use heca_grid_ui::reactive::{SignalGet, SignalUpdate, signal};
    use heca_grid_ui::{
        Command, CommandPalette, Component, Event, LayoutEngine, Size, WidgetIntent,
    };
    let ran = std::rc::Rc::new(std::cell::Cell::new(false));
    let r = ran.clone();
    let open = signal(false);
    let mut p = CommandPalette::new()
        .command(Command::new("Close pane", move || r.set(true)))
        .command(Command::new("Toggle sidebar", || {}))
        .open(open);
    let viewport = Size::new(1200.0, 800.0);
    LayoutEngine::new().compute(&mut p, viewport);
    assert!(!p.open_signal().get(), "closed while the signal is false");

    open.set(true);
    p.tick(0.0);
    LayoutEngine::new().compute(&mut p, viewport);
    common::type_text(&mut p, "close");
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert!(
        ran.get(),
        "typing reached the field of the palette your signal opened"
    );

    open.set(true);
    p.tick(0.0);
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Dismiss));
    assert!(!open.get(), "closing itself wrote your signal false");
}

/// **Closing dissolves** — as a host closes a surface: it calls `close()` and asks at once whether
/// the surface is still leaving, keeping its layer drawn while it is. The palette must answer yes;
/// once the dissolve has played out it is gone, and its query was reset. Answering no at once (the
/// palette has no animation of its own) dropped the layer mid-dissolve, and the palette vanished
/// with a cut.
#[test]
fn closing_dissolves_the_panel_and_resets_after_it() {
    use heca_grid_ui::{Command, CommandPalette, Component, LayoutEngine, Size};
    let viewport = Size::new(1200.0, 800.0);
    let mut p = CommandPalette::new()
        .command(Command::new("Close pane", || {}))
        .command(Command::new("Toggle sidebar", || {}));
    let query = p.query_signal();
    p.show();
    p.tick(1.0);
    LayoutEngine::new().compute(&mut p, viewport);
    common::type_text(&mut p, "tog");
    p.tick(0.0);

    Component::close(&mut p);
    let leaving = |p: &CommandPalette| p.presence().is_some_and(|presence| presence.is_leaving());
    assert!(
        leaving(&p),
        "asked right after closing, it is still leaving"
    );
    p.tick(0.01);
    assert!(
        leaving(&p) && p.is_shown(),
        "and still on screen while it dissolves"
    );

    p.tick(1.0);
    p.tick(0.0);
    assert!(
        !leaving(&p) && !p.is_shown(),
        "gone once the dissolve has played out"
    );
    assert_eq!(query.get_untracked(), "", "and the query was reset");
}

/// **Closing a closed palette tells nobody.** A host listens to `open` and closes the palette's
/// layer when it turns false; while the panel dissolves the layer is still there, so the host closes
/// the palette again — and if that second write of `false` woke the listener, the two answered each
/// other for ever and the app froze. A write that changes nothing wakes nobody.
#[test]
fn closing_a_closed_palette_wakes_nobody() {
    use heca_grid_ui::reactive::{SignalGet, create_effect};
    use heca_grid_ui::{Command, CommandPalette, Component};
    let mut p = CommandPalette::new().command(Command::new("Close pane", || {}));
    p.show();
    p.tick(1.0);
    let open = p.open_signal();
    let woken = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = woken.clone();
    create_effect(move |_| {
        if !open.get() {
            count.set(count.get() + 1);
        }
    });
    for _ in 0..5 {
        Component::close(&mut p);
    }
    assert_eq!(woken.get(), 1, "the listener heard the palette close once");
}
