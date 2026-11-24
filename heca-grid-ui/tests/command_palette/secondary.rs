use super::*;
use heca_grid_ui::{Command, CommandPalette, LayoutEngine, Size, WidgetIntent};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

type Ran = Rc<RefCell<Vec<&'static str>>>;

/// Two sessions and a command, each with a description. `describe` says which modes also match it.
fn palette(ran: Ran, describe: bool) -> CommandPalette {
    let (a, b, c) = (ran.clone(), ran.clone(), ran);
    let p = CommandPalette::new()
        .mode_with_commands(
            ':',
            "command",
            [Command::new("Close", move || a.borrow_mut().push("close"))
                .description("hide the pane")],
        )
        .mode_with_commands(
            '#',
            "session",
            [
                Command::new("api", move || b.borrow_mut().push("api"))
                    .description("~/work/billing"),
                Command::new("billing", move || c.borrow_mut().push("billing"))
                    .description("~/work/other"),
            ],
        );
    (if describe {
        p.match_description_in("session")
    } else {
        p
    })
    .default_open(true)
}

fn typed(p: &mut CommandPalette, text: &str) {
    for c in text.chars() {
        type_text(p, &c.to_string());
    }
}

#[test]
fn a_mode_that_matches_its_description_finds_a_row_by_a_word_only_the_description_has() {
    let ran: Ran = Default::default();
    let mut p = palette(ran.clone(), true);
    typed(&mut p, "#work/bil");
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(
        *ran.borrow(),
        ["api"],
        "found by its folder, which its name does not contain"
    );
}

#[test]
fn a_title_match_ranks_above_a_description_only_match() {
    let ran: Ran = Default::default();
    let mut p = palette(ran.clone(), true);
    // `billing` is the title of one row and the folder of the other; the title wins however it scores.
    typed(&mut p, "#billing");
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(*ran.borrow(), ["billing"]);
}

#[test]
fn a_mode_that_did_not_opt_in_is_still_matched_by_title_alone() {
    // The sessions mode, not opted in: the folder finds nothing.
    let ran: Ran = Default::default();
    let mut p = palette(ran.clone(), false);
    typed(&mut p, "#work/bil");
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert!(ran.borrow().is_empty(), "no row, so nothing ran");

    // And the command mode, beside an opted-in one, keeps title-only: its description is not searched.
    let ran: Ran = Default::default();
    let mut p = palette(ran.clone(), true);
    typed(&mut p, "hide");
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert!(
        ran.borrow().is_empty(),
        "`hide` is only in the description of a command"
    );
}

#[test]
fn entering_a_mode_is_announced_once_by_typing_and_again_after_leaving_it() {
    let heard = Rc::new(Cell::new(0));
    let hear = heard.clone();
    let mut p = CommandPalette::new()
        .mode_with_commands(':', "command", [Command::new("Close", || {})])
        .mode_with_commands('!', "notification", [Command::new("Saved", || {})])
        .on_mode_enter("notification", move || hear.set(hear.get() + 1))
        .default_open(true);
    // A typed edit is picked up by the next layout, as in a running window.
    let settle = |p: &mut CommandPalette| LayoutEngine::new().compute(p, Size::new(1200.0, 800.0));
    settle(&mut p);
    assert_eq!(heard.get(), 0, "opened in the default mode");
    type_text(&mut p, "!");
    settle(&mut p);
    assert_eq!(heard.get(), 1, "typing the sigil enters it");
    type_text(&mut p, "s");
    settle(&mut p);
    assert_eq!(heard.get(), 1, "typing inside it does not enter it again");
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::EditDeleteToLineStart));
    settle(&mut p);
    assert_eq!(heard.get(), 1, "left the mode");
    type_text(&mut p, "!");
    settle(&mut p);
    assert_eq!(heard.get(), 2, "and entered it again");
}

#[test]
fn opening_in_the_mode_announces_it_whichever_order_the_builders_ran_in() {
    for callback_first in [true, false] {
        let heard = Rc::new(Cell::new(0));
        let hear = heard.clone();
        let base = CommandPalette::new().mode_with_commands(
            '!',
            "notification",
            [Command::new("Saved", || {})],
        );
        let p = if callback_first {
            base.on_mode_enter("notification", move || hear.set(hear.get() + 1))
                .query_in(Some("notification"), "")
                .default_open(true)
        } else {
            base.query_in(Some("notification"), "")
                .default_open(true)
                .on_mode_enter("notification", move || hear.set(hear.get() + 1))
        };
        assert_eq!(heard.get(), 1, "callback_first={callback_first}");
        drop(p);
    }
}

/// **A row's second line is muted, selected or not** — the description and the detail are drawn in
/// the theme's `muted` on every row; the selection is the row's fill and outline, not a change of
/// ink. (The hand-drawn palette once brightened the selected row's second line.)
#[test]
fn every_rows_secondary_text_stays_muted_selected_or_not() {
    let mut theme = Theme::default();
    // Distinct, so a colour taken from the wrong token cannot pass by coincidence.
    theme.colors.muted = heca_grid_ui::Color::rgb(10, 20, 30);
    theme.colors.foreground = heca_grid_ui::Color::rgb(200, 210, 220);
    let mut p = CommandPalette::new()
        .command(
            Command::new("First", || {})
                .description("first description")
                .detail("first detail"),
        )
        .command(
            Command::new("Second", || {})
                .description("second description")
                .detail("second detail"),
        )
        .default_open(true);
    let viewport = Size::new(1200.0, 800.0);
    let colours = |p: &mut CommandPalette| {
        LayoutEngine::new().compute(p, viewport);
        let scene = common::paint_in(p, &theme, viewport);
        [
            "first description",
            "first detail",
            "second description",
            "second detail",
        ]
        .map(|text| {
            common::texts(&scene)
                .into_iter()
                .find(|t| t.text == text)
                .unwrap_or_else(|| panic!("{text} is painted"))
                .color
        })
    };
    assert_eq!(
        colours(&mut p),
        [theme.colors.muted; 4],
        "the first row selected"
    );
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuDown));
    assert_eq!(
        colours(&mut p),
        [theme.colors.muted; 4],
        "the second row selected"
    );
}

/// **A row can carry something after its title** — drawn on the title's line, to its right, inside
/// the row, and the title gives way to it rather than running under it.
#[test]
fn a_rows_trailing_content_sits_after_its_title_on_the_same_line() {
    use heca_grid_ui::{Command, CommandPalette, Label};
    let theme = Theme::default();
    let mut p = CommandPalette::new()
        .command(
            Command::new("work session with a long name that needs room", || {})
                .trailing(|| Label::new("pro")),
        )
        .command(Command::new("plain", || {}))
        .default_open(true);
    for width in [1200.0, 420.0] {
        // A new window width settles on the next frame (the rows are measured at the width they
        // were drawn at), so it is laid out and drawn twice, as two frames are.
        for _ in 0..2 {
            LayoutEngine::new().compute(&mut p, Size::new(width, 800.0));
            let _ = common::paint_in(&p, &theme, Size::new(width, 800.0));
        }
        LayoutEngine::new().compute(&mut p, Size::new(width, 800.0));
        let scene = common::paint_in(&p, &theme, Size::new(width, 800.0));
        let text = |name: &str| {
            scene.iter().find_map(|c| match c {
                DrawCommand::Text(t) if t.text.starts_with(name) => Some(t.rect),
                _ => None,
            })
        };
        let title = text("work").expect("the title paints");
        let trailing =
            text("pro").unwrap_or_else(|| panic!("the trailing content paints at {width}px"));
        assert!(
            trailing.loc.x >= title.loc.x + title.size.w - 0.5,
            "after the title, not over it, at {width}px"
        );
        assert!(
            (trailing.loc.y + trailing.size.h / 2.0 - (title.loc.y + title.size.h / 2.0)).abs()
                <= 2.0,
            "on the title's line at {width}px"
        );
        assert!(
            trailing.loc.x + trailing.size.w <= width,
            "inside the window at {width}px"
        );
    }
}
