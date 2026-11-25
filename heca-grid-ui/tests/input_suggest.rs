//! `Input::suggest`: a word starting with the trigger opens the matching items under the caret;
//! the arrows choose, Enter / Tab / a click put the choice in place of the word, Escape closes.

mod common;

use heca_grid_ui::component::{Component, PaintCx, paint_child};
use heca_grid_ui::prelude::*;
use heca_grid_ui::scene::{DrawCommand, Scene};
use heca_grid_ui::{KeyPress, Keymap, LayoutEngine, Size};

const NAMES: [&str; 4] = ["bob", "carol", "bobby", "all"];

fn field() -> Input {
    let mut input = Input::new().multiline(true).suggest(
        '@',
        NAMES.iter().map(|n| n.to_string()).collect::<Vec<_>>(),
        |name: &String| Label::new(name.clone()),
        |name: &String| format!("@{name} "),
    );
    common::give_keyboard(&mut input);
    lay(&mut input);
    input
}

fn lay(input: &mut Input) {
    for _ in 0..2 {
        LayoutEngine::new().compute(input, Size::new(600.0, 400.0));
    }
}

fn press(input: &mut Input, key: GridKey) {
    chord(input, key, Modifiers::default());
}

/// A key with modifiers, through the default keymap — the same funnel a host uses.
fn chord(input: &mut Input, key: GridKey, mods: Modifiers) {
    heca_grid_ui::dispatch(input, &Event::ModifiersChanged(mods));
    Keymap::with_defaults().deliver_press(
        &KeyPress {
            key,
            text: None,
            mods,
        },
        |event| heca_grid_ui::dispatch(input, event),
    );
    heca_grid_ui::dispatch(input, &Event::ModifiersChanged(Modifiers::default()));
    lay(input);
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        ..Default::default()
    }
}

fn type_in(input: &mut Input, text: &str) {
    common::type_text(input, text);
    lay(input);
}

/// The words the open list shows, in order; empty when it is closed.
fn shown(input: &Input) -> Vec<String> {
    let theme = Theme::default();
    let mut scene = Scene::new();
    paint_child(input, &mut PaintCx::new(&mut scene, &theme));
    // The list is drawn on the overlay layer; the field's own text is not.
    scene
        .overlay_layer()
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Text(text) if NAMES.contains(&text.text.as_str()) => {
                Some(text.text.clone())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn the_trigger_opens_the_matching_items_and_typing_narrows_them() {
    let mut input = field();
    assert!(shown(&input).is_empty(), "closed before the trigger");
    type_in(&mut input, "hi @");
    assert_eq!(shown(&input), NAMES, "all items, in the order given");
    type_in(&mut input, "bo");
    assert_eq!(shown(&input), ["bob", "bobby"]);
    type_in(&mut input, " ");
    assert!(shown(&input).is_empty(), "a space ends the word");
}

#[test]
fn enter_puts_the_chosen_item_in_place_of_the_word() {
    let mut input = field();
    type_in(&mut input, "ask @bo");
    press(&mut input, GridKey::ArrowDown);
    press(&mut input, GridKey::Enter);
    assert_eq!(
        input.value_str(),
        "ask @bobby ",
        "the second match, not a new line"
    );
    assert!(shown(&input).is_empty(), "the list closed");
}

#[test]
fn escape_closes_until_the_word_changes() {
    let mut input = field();
    type_in(&mut input, "@ca");
    press(&mut input, GridKey::Escape);
    assert!(shown(&input).is_empty());
    press(&mut input, GridKey::Enter);
    assert_eq!(
        input.value_str(),
        "@ca\n",
        "closed: Enter is a new line again"
    );
    type_in(&mut input, "@b");
    assert_eq!(shown(&input), ["bob", "bobby"], "a new word opens it again");
}

#[test]
fn a_click_on_an_item_chooses_it() {
    let mut input = field();
    type_in(&mut input, "@");
    let carol = {
        let theme = Theme::default();
        let mut scene = Scene::new();
        paint_child(&input, &mut PaintCx::new(&mut scene, &theme));
        scene
            .overlay_layer()
            .iter()
            .find_map(|command| match command {
                DrawCommand::Text(text) if text.text == "carol" => Some(text.rect),
                _ => None,
            })
            .expect("carol is shown")
    };
    let at = heca_grid_ui::Point::new(
        carol.loc.x + carol.size.w / 2.0,
        carol.loc.y + carol.size.h / 2.0,
    );
    common::click_at(&mut input, at, heca_grid_ui::event::PointerButton::Left);
    assert_eq!(input.value_str(), "@carol ");
}

#[test]
fn the_list_hangs_under_the_caret() {
    let mut input = field();
    type_in(&mut input, "one\n@");
    let field_box = input.base().bounds;
    let theme = Theme::default();
    let mut scene = Scene::new();
    paint_child(&input, &mut PaintCx::new(&mut scene, &theme));
    let first = scene
        .overlay_layer()
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text(text) if text.text == "bob" => Some(text.rect),
            _ => None,
        })
        .expect("the list is drawn");
    let line = theme.font_size as f64 * heca_grid_ui::font::MONO_LINE_RATIO as f64;
    assert!(
        first.loc.y > field_box.loc.y + line,
        "below the second line, under the caret: {first:?} in {field_box:?}"
    );
}

/// The chosen row is lit by a fill under it, with no outline round it.
#[test]
fn the_chosen_row_is_lit_by_a_fill_not_an_outline() {
    let mut input = field();
    type_in(&mut input, "@");
    let theme = Theme::default();
    let mut scene = Scene::new();
    paint_child(&input, &mut PaintCx::new(&mut scene, &theme));
    let overlay = scene.overlay_layer();
    let chosen = overlay
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text(text) if text.text == "bob" => Some(text.rect),
            _ => None,
        })
        .expect("the first item is shown");
    let middle = heca_grid_ui::Point::new(
        chosen.loc.x + chosen.size.w / 2.0,
        chosen.loc.y + chosen.size.h / 2.0,
    );
    // The rects under the chosen row, the list's own panel left out (it covers every row).
    let panel_height = overlay
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Rect(rect) => Some(rect.rect.size.h),
            _ => None,
        })
        .fold(0.0, f64::max);
    let under: Vec<_> = overlay
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Rect(rect)
                if rect.rect.contains(middle) && rect.rect.size.h < panel_height =>
            {
                Some(rect)
            }
            _ => None,
        })
        .collect();
    assert!(
        under
            .iter()
            .any(|rect| rect.fill.a == theme.colors.interaction.nav_fill),
        "the theme's nav_fill lights it, strong enough to read alone: {under:?}"
    );
    assert!(
        under
            .iter()
            .all(|rect| rect.border.is_none_or(|b| b.width == 0.0 || b.color.a == 0)),
        "no outline: {under:?}"
    );
}

#[test]
fn ctrl_j_and_ctrl_k_move_the_choice_like_the_arrows() {
    let mut input = field();
    type_in(&mut input, "@");
    chord(&mut input, GridKey::Char('j'), ctrl());
    chord(&mut input, GridKey::Char('j'), ctrl());
    chord(&mut input, GridKey::Char('k'), ctrl());
    press(&mut input, GridKey::Enter);
    assert_eq!(
        input.value_str(),
        "@carol ",
        "down, down, up: the second item"
    );
}

#[test]
fn tab_completes_the_chosen_item() {
    let mut input = field();
    type_in(&mut input, "@bobb");
    press(&mut input, GridKey::Tab);
    assert_eq!(input.value_str(), "@bobby ");
}

#[test]
fn the_list_is_at_least_the_themes_width() {
    let mut input = field();
    type_in(&mut input, "@");
    let theme = Theme::default();
    let mut scene = Scene::new();
    paint_child(&input, &mut PaintCx::new(&mut scene, &theme));
    let widest = scene
        .overlay_layer()
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Rect(rect) => Some(rect.rect.size.w),
            _ => None,
        })
        .fold(0.0, f64::max);
    assert!(
        widest >= f64::from(theme.colors.input.suggestions_width),
        "the panel is {widest} wide"
    );
}

#[test]
fn the_field_keeps_the_keyboard_while_the_list_is_open() {
    let mut input = field();
    type_in(&mut input, "@");
    assert_eq!(shown(&input), NAMES);
    assert!(
        input.base().is_focused(),
        "the list leaves the keyboard in the field"
    );
    type_in(&mut input, "car");
    assert_eq!(
        input.value_str(),
        "@car",
        "typing still goes into the field"
    );
}

/// Near the window's bottom edge there is no room under the caret: the list opens above it.
#[test]
fn near_the_bottom_of_the_window_the_list_opens_above_the_caret() {
    let window = Size::new(600.0, 400.0);
    let theme = Theme::default();
    let mut page = Flex::column()
        .child(common::fixed_box(600.0, 330.0))
        .child(field());
    let lay_page = |page: &mut Flex| LayoutEngine::new().compute(page, window);
    lay_page(&mut page);
    common::type_text(&mut page, "@");
    // One layout, as a host runs after a key: the list is placed right the first time it opens,
    // with no paint before it to tell it the window's size.
    lay_page(&mut page);
    let scene = common::paint_in(&page, &theme, window);
    let rows: Vec<_> = scene
        .overlay_layer()
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Text(text) if NAMES.contains(&text.text.as_str()) => Some(text.rect),
            _ => None,
        })
        .collect();
    assert_eq!(rows.len(), NAMES.len(), "the whole list is drawn");
    // The caret's line is where the typed `@` is drawn.
    let caret_top = scene
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text(text) if text.text == "@" => Some(text.rect.loc.y),
            _ => None,
        })
        .expect("the typed @ is drawn");
    assert!(
        rows.iter().all(|row| row.loc.y + row.size.h <= caret_top),
        "every row above the caret at {caret_top}: {rows:?}"
    );
    assert!(
        rows.iter().all(|row| row.loc.y >= 0.0),
        "and inside the window: {rows:?}"
    );
}

/// Deep in a page that scrolls, the list still hangs at the caret — the page's scroll moves the
/// field and its list together, never the list twice.
#[test]
fn in_a_scrolled_page_the_list_stays_at_the_caret() {
    let window = Size::new(600.0, 400.0);
    let theme = Theme::default();
    let page = heca_grid_ui::ScrollRegion::new().height(400.0);
    let scrolled = page.scroll_offset();
    let mut page = page.child(
        Flex::column()
            .child(common::fixed_box(600.0, 700.0))
            .child(field())
            .child(common::fixed_box(600.0, 300.0)),
    );
    let lay_page =
        |page: &mut heca_grid_ui::ScrollRegion| LayoutEngine::new().compute(page, window);
    lay_page(&mut page);
    scrolled.set(500.0);
    lay_page(&mut page);
    common::type_text(&mut page, "@");
    lay_page(&mut page);
    let scene = common::paint_in(&page, &theme, window);
    let caret = scene
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text(text) if text.text == "@" => Some(text.rect),
            _ => None,
        })
        .expect("the typed @ is drawn");
    let first = scene
        .overlay_layer()
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text(text) if text.text == "bob" => Some(text.rect),
            _ => None,
        })
        .expect("the list is drawn on screen");
    let line = caret.size.h.max(first.size.h) * 2.0;
    assert!(
        (first.loc.y - (caret.loc.y + caret.size.h)).abs() <= line,
        "the list is next to the caret: caret {caret:?}, first row {first:?}"
    );
}
