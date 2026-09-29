use super::*;

/// A sigil switches which commands the palette lists, and **is not itself matched**: typing `@`
/// shows every pane, not the panes whose names contain an `@`.
#[test]
fn a_sigil_switches_the_mode_without_filtering_by_itself() {
    use heca_grid_ui::{Command, CommandPalette, WidgetIntent};
    let ran = std::rc::Rc::new(std::cell::Cell::new(""));
    let open = || {
        let (a, b, c) = (ran.clone(), ran.clone(), ran.clone());
        CommandPalette::new()
            .mode(':', "command")
            .mode('@', "pane")
            .command(Command::new("Close pane", move || a.set("close")).id("close"))
            .command(
                Command::new("nvim", move || b.set("nvim"))
                    .id("nvim")
                    .mode("pane"),
            )
            .command(
                Command::new("zsh", move || c.set("zsh"))
                    .id("zsh")
                    .mode("pane"),
            )
            .default_open(true)
    };

    // No sigil: the default mode, exactly as before modes existed.
    let mut p = open();
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        "close",
        "an unsigiled query is in the default mode"
    );

    // A lone sigil lists that mode unfiltered — the first pane runs, and `@` matched nothing.
    let mut p = open();
    type_text(&mut p, "@");
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(ran.get(), "nvim", "a lone sigil lists its mode whole");

    // And it filters on the rest of the query, not the sigil.
    let mut p = open();
    for c in "@zs".chars() {
        type_text(&mut p, &c.to_string());
    }
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        "zsh",
        "the effective query is what matched, the sigil is not"
    );

    // A leading character that is not a sigil is the first letter of a search, not a mode.
    let mut p = open();
    for c in "clo".chars() {
        type_text(&mut p, &c.to_string());
    }
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        "close",
        "an unknown leading char is matched literally"
    );
}

/// **Each mode remembers separately.** A pane searched for is recalled under `@` and is invisible
/// to the command history — that separation is the whole reason a mode names a search scope.
#[test]
fn every_mode_keeps_its_own_history_and_ranking() {
    use heca_grid_ui::search::{SearchModel, SearchStore};
    use heca_grid_ui::{Command, CommandPalette, WidgetIntent};
    let theme = Theme::default();
    let store = std::rc::Rc::new(std::cell::RefCell::new(SearchStore::new()));

    let open = || {
        CommandPalette::new()
            .search(SearchModel::new("command", store.clone()))
            .mode(':', "command")
            .mode('@', "pane")
            .command(Command::new("Close pane", || {}).id("close"))
            .command(Command::new("nvim", || {}).id("nvim").mode("pane"))
            .default_open(true)
    };
    let query_text = |p: &CommandPalette| {
        let scene = common::paint_in(p, &theme, Size::new(1200.0, 800.0));
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .expect("the query line paints")
    };

    // Search for a pane and run it, so the `pane` scope has a history.
    let mut p = open();
    LayoutEngine::new().compute(&mut p, Size::new(1200.0, 800.0));
    for c in "@nvi".chars() {
        type_text(&mut p, &c.to_string());
    }
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));

    // The default mode's history never saw it.
    let mut p = open();
    LayoutEngine::new().compute(&mut p, Size::new(1200.0, 800.0));
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuHistoryUp));
    assert_eq!(
        query_text(&p),
        "Type a command…",
        "nothing was recalled, so the field is still empty and paints its placeholder — a pane \
         search is not in the command history",
    );

    // Under the sigil it comes back — and comes back **with the sigil**, so the recall does not
    // drop the mode the user is standing in.
    let mut p = open();
    LayoutEngine::new().compute(&mut p, Size::new(1200.0, 800.0));
    type_text(&mut p, "@");
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuHistoryUp));
    assert_eq!(
        query_text(&p),
        "@nvi",
        "the mode's own history, sigil restored"
    );

    // The stored query itself carries no sigil: it is filed *inside* the pane memory, and a sigil
    // kept there would come back doubled on every recall.
    let recalled = store
        .borrow()
        .scope("pane")
        .expect("the pane scope exists")
        .history
        .entries()
        .to_vec();
    assert_eq!(
        recalled,
        vec!["nvi".to_string()],
        "the history stores the effective query"
    );
}

/// **The marks follow the effective query.** Under `@zs` the `z` and `s` of `zsh` are highlighted;
/// the sigil marks nothing, because it was never part of what matched.
#[test]
fn the_marks_land_on_the_effective_query_not_the_sigil() {
    use heca_grid_ui::{Command, CommandPalette};
    let theme = Theme::default();
    let open = || {
        CommandPalette::new()
            .mode(':', "command")
            .mode('@', "pane")
            .command(Command::new("Close pane", || {}).id("close"))
            .command(Command::new("zsh", || {}).id("zsh").mode("pane"))
            .default_open(true)
    };
    // Marks are **over-drawn per character**, so a marked label leaves single-character runs on top
    // of the whole line (see `a_label_marks_the_characters_it_was_given`).
    let marked_chars = |p: &CommandPalette| -> Vec<String> {
        let scene = common::paint_in(p, &theme, Size::new(1200.0, 800.0));
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) if t.text.chars().count() == 1 => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    };

    let mut p = open();
    LayoutEngine::new().compute(&mut p, Size::new(1200.0, 800.0));
    for c in "@zs".chars() {
        type_text(&mut p, &c.to_string());
    }
    let marks = marked_chars(&p);
    assert!(
        marks.contains(&"z".to_string()),
        "the effective query marked 'z': {marks:?}"
    );
    assert!(
        marks.contains(&"s".to_string()),
        "the effective query marked 's': {marks:?}"
    );
    assert!(
        !marks.contains(&"@".to_string()),
        "the sigil is not part of the match, so it marks nothing: {marks:?}",
    );
}

/// **The selection starts where the caller asked** — the pane you were last in, so Enter takes you
/// back. It is an untyped-list rule only: the first keystroke hands the lead back to the best match.
#[test]
fn the_selection_starts_on_the_preselected_row_until_something_is_typed() {
    use heca_grid_ui::{Command, CommandPalette, WidgetIntent};
    let ran = std::rc::Rc::new(std::cell::Cell::new(""));
    let open = || {
        let (a, b, c) = (ran.clone(), ran.clone(), ran.clone());
        CommandPalette::new()
            .command(Command::new("alpha", move || a.set("alpha")).id("a"))
            // Declared second, and the row the caller wants Enter to land on.
            .command(
                Command::new("beta", move || b.set("beta"))
                    .id("b")
                    .preselect(true),
            )
            .command(Command::new("gamma", move || c.set("gamma")).id("c"))
            .default_open(true)
    };

    let mut p = open();
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        "beta",
        "an untyped list opens on the preselected row"
    );

    // Navigation still moves from there, rather than from the top.
    let mut p = open();
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuDown));
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        "gamma",
        "the selection moved from the preselected row, not from row 0"
    );

    // Typing overrules it: the best match leads, exactly as `group` is dissolved by a query.
    let mut p = open();
    for c in "alp".chars() {
        type_text(&mut p, &c.to_string());
    }
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        "alpha",
        "a query hands the lead back to the match"
    );
}

/// A palette that declares no modes is the palette that existed before modes did: one scope, no
/// sigil, and a `:` in the query is just a character to match.
#[test]
fn a_palette_without_modes_is_unchanged() {
    use heca_grid_ui::{Command, CommandPalette, WidgetIntent};
    let ran = std::rc::Rc::new(std::cell::Cell::new(""));
    let (a, b) = (ran.clone(), ran.clone());
    let mut p = CommandPalette::new()
        .command(Command::new("Close pane", move || a.set("close")).id("close"))
        .command(Command::new(":wq", move || b.set("wq")).id("wq"))
        .default_open(true);

    for c in ":w".chars() {
        type_text(&mut p, &c.to_string());
    }
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        "wq",
        "with no modes declared, a leading ':' is ordinary text"
    );
}
