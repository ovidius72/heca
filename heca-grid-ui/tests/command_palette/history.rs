use super::*;

/// **The palette recalls past queries — and implements none of it.** The walk, the draft and the
/// stop at the oldest all live in `search::SearchModel`; this asserts the *drawn* query line, which
/// is the only thing that proves the recall reached the field.
#[test]
fn command_palette_recalls_past_queries_from_its_history() {
    use heca_grid_ui::search::{SearchModel, SearchStore};
    use heca_grid_ui::{Command, CommandPalette, WidgetIntent};
    let theme = Theme::default();
    let store = std::rc::Rc::new(std::cell::RefCell::new(SearchStore::new()));

    // A palette is rebuilt every time it opens, which is exactly why the memory is the host's.
    let open = || {
        CommandPalette::new()
            .search(SearchModel::new("command", store.clone()))
            .command(Command::new("Close pane", || {}).id("close"))
            .command(Command::new("Split pane", || {}).id("split"))
            .default_open(true)
    };
    // The drawn query line: the first text run inside the panel is the field's.
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

    // Search and run twice, so there is a history to walk.
    for query in ["close", "split"] {
        let mut p = open();
        LayoutEngine::new().compute(&mut p, Size::new(1200.0, 800.0));
        for c in query.chars() {
            type_text(&mut p, &c.to_string());
        }
        heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    }

    let mut p = open();
    LayoutEngine::new().compute(&mut p, Size::new(1200.0, 800.0));
    // Type a draft, then walk back through the history.
    for c in "dra".chars() {
        type_text(&mut p, &c.to_string());
    }
    assert_eq!(query_text(&p), "dra");
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuHistoryUp));
    assert_eq!(
        query_text(&p),
        "split",
        "one step back is the newest past query"
    );
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuHistoryUp));
    assert_eq!(
        query_text(&p),
        "close",
        "a second step keeps walking — the cursor is not reset"
    );
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuHistoryDown));
    assert_eq!(query_text(&p), "split");
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuHistoryDown));
    assert_eq!(
        query_text(&p),
        "dra",
        "past the newest, the draft comes back"
    );
}

/// **Past choices order the list, and typing overrules them.** With nothing typed the palette shows
/// what the user actually uses; the moment a query spells another command better, that one leads.
#[test]
fn command_palette_ranks_by_past_use_until_something_is_typed() {
    use heca_grid_ui::search::{SearchModel, SearchStore};
    use heca_grid_ui::{Command, CommandPalette, WidgetIntent};
    let store = std::rc::Rc::new(std::cell::RefCell::new(SearchStore::new()));
    let ran = std::rc::Rc::new(std::cell::Cell::new(""));

    let open = || {
        let (a, b) = (ran.clone(), ran.clone());
        CommandPalette::new()
            .search(SearchModel::new("command", store.clone()))
            .command(Command::new("Close pane", move || a.set("close")).id("close"))
            .command(Command::new("Split pane", move || b.set("split")).id("split"))
            .default_open(true)
    };

    // Use "Split pane" — it is second in the caller's order.
    let mut p = open();
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuDown));
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(ran.get(), "split");

    // Nothing typed: the used one leads, so activating the top row runs it again.
    let mut p = open();
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(ran.get(), "split", "past use orders an untyped list");

    // Typed: the query spells the other command, and it wins despite the other's history.
    let mut p = open();
    for c in "close".chars() {
        type_text(&mut p, &c.to_string());
    }
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(ran.get(), "close", "typing overrules the boost");
}

/// A group orders the list **only while nothing is typed**. The command palette's host uses it for
/// the focused component's actions: with an empty query that focus is the only context there is,
/// and once a query exists it is better context than the block.
#[test]
fn a_group_leads_an_empty_query_and_dissolves_once_typing_starts() {
    use heca_grid_ui::{Command, CommandPalette, WidgetIntent};
    let ran = std::rc::Rc::new(std::cell::Cell::new(""));
    let open = || {
        let (a, b) = (ran.clone(), ran.clone());
        CommandPalette::new()
            // Declared second, but in the leading block.
            .command(
                Command::new("Zoom out", move || a.set("zoom"))
                    .id("zoom")
                    .group(1),
            )
            .command(
                Command::new("Workspaces › Delete row", move || b.set("del"))
                    .id("del")
                    .group(0),
            )
            .default_open(true)
    };

    let mut p = open();
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        ran.get(),
        "del",
        "the leading block comes first while nothing is typed"
    );

    let mut p = open();
    for c in "zoom".chars() {
        type_text(&mut p, &c.to_string());
    }
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(ran.get(), "zoom", "a typed query dissolves the block");
}

/// An **abandoned** search is not one anyone wants back: dismissing records nothing.
#[test]
fn command_palette_remembers_only_what_was_run() {
    use heca_grid_ui::search::{SearchModel, SearchStore};
    use heca_grid_ui::{Command, CommandPalette, WidgetIntent};
    let theme = Theme::default();
    let store = std::rc::Rc::new(std::cell::RefCell::new(SearchStore::new()));
    let open = || {
        CommandPalette::new()
            .search(SearchModel::new("command", store.clone()))
            .command(Command::new("Close pane", || {}).id("close"))
            .default_open(true)
    };

    let mut p = open();
    for c in "abandoned".chars() {
        type_text(&mut p, &c.to_string());
    }
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Dismiss));

    let mut p = open();
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuHistoryUp));
    let scene = common::paint_in(&p, &theme, Size::new(1200.0, 800.0));
    let query = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .expect("the query line paints");
    assert_ne!(query, "abandoned", "a dismissed search was not remembered");
}
