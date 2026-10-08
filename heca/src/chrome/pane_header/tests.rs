use super::*;
use heca_grid_ui::builders::ComponentExt as _;
use heca_grid_ui::widgets::Label;

fn facts_for(cwd: &str) -> super::super::pane_items::PaneFacts {
    let rt = PaneRuntime {
        program: Some("nvim".into()),
        cwd: Some(std::path::PathBuf::from(cwd)),
        ..PaneRuntime::default()
    };
    super::super::pane_items::PaneFacts::of(
        PaneId(1),
        &ProgramsConfig::default(),
        "shell",
        None,
        Some(&rt),
    )
}

/// **When the bar is too narrow only the path gives way.** The chip that is a path carries that
/// as its own property (`Fit::PathLeft`), so the header keeps no list of which chip is the long
/// one — and the others keep their words.
#[test]
fn a_narrow_bar_shortens_the_path_chip_and_nothing_else() {
    let chips = super::super::pane_items::PaneChips::default();
    let defs: Vec<_> = ["location", "app_name"]
        .iter()
        .map(|n| chips.get(n).expect("shipped"))
        .collect();
    let facts = facts_for("/a/very/long/path/to/some/project/directory");

    let roomy = segment_items(&chips, &defs, &facts, 10_000.0, 13.0);
    let tight = segment_items(&chips, &defs, &facts, 120.0, 13.0);

    assert_eq!(roomy[0].2, "/a/very/long/path/to/some/project/directory");
    assert!(
        tight[0].2.chars().count() < roomy[0].2.chars().count(),
        "the path shortened"
    );
    assert_eq!(tight[1].2, roomy[1].2, "the program name is untouched");
    assert_eq!(tight.len(), 2);
}

/// A pane_name chip builds a bar for a renamed pane and for one that is not renamed.
#[test]
fn a_bar_is_built_from_what_the_chips_say() {
    let chips = super::super::pane_items::PaneChips::default();
    let defs = [chips.get("pane_name").expect("shipped")];
    let facts = facts_for("/tmp");
    let items = segment_items(&chips, &defs, &facts, 400.0, 13.0);
    assert!(build_pane_info_bar(items, &GuiTheme::default()).is_some());
    assert!(
        build_pane_info_bar(Vec::new(), &GuiTheme::default()).is_none(),
        "no chips, no bar"
    );
}

/// **Which actions are destructive is the action's own declaration, not a surface's.**
///
/// It was written into this file as a `matches!` on the close button — a styling rule keyed to a
/// name, in a file that should know nothing about which acts cannot be undone. Every surface that
/// renders an action reads the same answer now, exactly as they already do for its icon and its
/// label, so a header button, a menu entry and the palette cannot disagree about what is dangerous.
#[test]
fn a_headers_danger_hue_comes_from_the_action_not_from_its_name() {
    let catalog = crate::actions::ActionCatalog::with_builtins();
    assert!(
        catalog.destructive("close"),
        "closing a pane is declared destructive by the action"
    );
    for safe in ["zoom_column", "float", "add_pane_to_column"] {
        assert!(
            !catalog.destructive(safe),
            "{safe} is not destructive, so nothing should draw it as though it were"
        );
    }
}

/// **A chrome button's pick says what it is, so the picker can refuse it** (F003/P082/T432).
///
/// A button hands over the same closure for its click and its pick — they are one gesture for a
/// button — and a closure is opaque, so the policy had nothing to ask about. `prefix+/`
/// therefore lettered the sidebar toggles while a pane was floating, even though `sidebar_left`
/// is `TiledOnly` and the click was already refused: a letter that did nothing.
///
/// It is named here because this is the one place a chrome button's action name is known — the
/// same place the tooltip and its live keybinding come from. If that ever goes, every chrome
/// button silently escapes the filter again and nothing else would notice.
#[test]
fn a_buttons_pick_is_named_from_its_action() {
    let shortcuts = ActionShortcuts::default();
    let button = Label::new("×").on_hint(|| {});
    let tip = action_tooltip(button, "close", "Close", &shortcuts);

    // Read straight off the widget: the tip is a **property** now, so what comes back is the
    // button itself rather than a wrapper around it — which is exactly what lets a pane header
    // button go into a `ButtonGroup`.
    let named = tip
        .base()
        .hint
        .as_ref()
        .expect("the button declares a pick")
        .intent
        .as_ref()
        .expect("…and it is named");
    assert_eq!(named.action, "close");
}

/// A pick that already said what it is keeps it — the caller knows more than the action name
/// alone (a pane button carries the pane it acts on).
#[test]
fn a_pick_that_already_named_itself_is_left_alone() {
    let shortcuts = ActionShortcuts::default();
    let button = Label::new("×").on_hint(heca_grid_ui::Hint::of(
        heca_view::Intent::new("close_pane_by_id").arg("pane_id", heca_view::PropValue::Int(7)),
        || {},
    ));
    let tip = action_tooltip(button, "close", "Close", &shortcuts);

    let named = tip.base().hint.as_ref().unwrap().intent.as_ref().unwrap();
    assert_eq!(
        named.action, "close_pane_by_id",
        "the caller's own naming wins"
    );
}
