use super::*;
use heca_grid_ui::{Command, CommandPalette, WidgetIntent};
use std::cell::Cell;
use std::rc::Rc;

fn palette(ran: Rc<Cell<&'static str>>) -> CommandPalette {
    let (command, pane, workspace) = (ran.clone(), ran.clone(), ran);
    CommandPalette::new()
        .mode_with_commands(':', "command", [
            Command::new("Close", move || command.set("command")),
        ])
        .mode_with_commands('@', "pane", [
            Command::new("nvim", move || pane.set("pane")),
        ])
        .mode_with_commands('$', "workspace", [
            Command::new("Editing", move || workspace.set("workspace")),
        ])
}

#[test]
fn named_prefill_uses_the_declared_modes_and_preserves_fallbacks() {
    for (mode, query, expected) in [
        (None, "", "command"),
        (Some("command"), "cl", "command"),
        (Some("pane"), "", "pane"),
        (Some("pane"), "nv", "pane"),
        (Some("workspace"), "ed", "workspace"),
        (Some("unknown"), "cl", "command"),
        (None, "@nv", "pane"),
        (Some("unknown"), "$ed", "workspace"),
    ] {
        let ran = Rc::new(Cell::new(""));
        let mut p = palette(ran.clone()).query_in(mode, query).default_open(true);
        heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
        assert_eq!(ran.get(), expected, "mode={mode:?}, query={query:?}");
    }
}

#[test]
fn one_custom_mode_declaration_serves_typed_and_named_queries() {
    for named in [false, true] {
        let ran = Rc::new(Cell::new(""));
        let chosen = ran.clone();
        let p = palette(ran.clone()).mode_with_commands('!', "custom", [
            // An existing row annotation is replaced by the containing mode.
            Command::new("Report", move || chosen.set("custom")).mode("pane"),
        ]);
        let mut p = if named {
            p.query_in(Some("custom"), "rep").default_open(true)
        } else {
            let mut p = p.default_open(true);
            type_text(&mut p, "!rep");
            p
        };
        heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
        assert_eq!(ran.get(), "custom", "named={named}");
    }
}

#[test]
fn named_queries_keep_the_existing_search_scope_names() {
    use heca_grid_ui::search::{SearchModel, SearchStore};
    let store = Rc::new(std::cell::RefCell::new(SearchStore::new()));
    for (scope, query, id) in [
        ("command", "cl", "close"),
        ("pane", "nv", "nvim"),
        ("workspace", "ed", "editing"),
    ] {
        let mut p = CommandPalette::new()
            .search(SearchModel::new("command", store.clone()))
            .mode_with_commands(':', "command", [Command::new("Close", || {}).id("close")])
            .mode_with_commands('@', "pane", [Command::new("nvim", || {}).id("nvim")])
            .mode_with_commands('$', "workspace", [Command::new("Editing", || {}).id("editing")])
            .query_in(Some(scope), query)
            .default_open(true);
        heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
        let memory = store.borrow();
        let memory = memory.scope(scope).expect("the mode recorded its search");
        assert_eq!(memory.history.entries(), &[query.to_string()]);
        assert!(memory.frecency.boost(id) > 0);
    }
}

#[test]
fn named_prefill_can_precede_its_mode_declaration() {
    let ran = Rc::new(Cell::new(""));
    let chosen = ran.clone();
    let mut p = palette(ran.clone())
        .query_in(Some("custom"), "rep")
        .default_open(true)
        .mode_with_commands('!', "custom", [Command::new("Report", move || chosen.set("custom"))]);
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(ran.get(), "custom");
}

#[test]
fn a_plain_query_replaces_a_pending_named_prefill() {
    let ran = Rc::new(Cell::new(""));
    let mut p = palette(ran.clone())
        .query_in(Some("custom"), "rep")
        .query("cl")
        .mode_with_commands('!', "custom", [Command::new("Report", || {})])
        .default_open(true);
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(ran.get(), "command");
}
