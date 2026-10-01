use super::*;

/// Lay a toast out as the root at its fixed width so `bounds` are set for
/// hit-testing, returning its resolved height.
fn layout_toast(t: &mut heca_grid_ui::Toast) -> f64 {
    LayoutEngine::new().compute(t, Size::new(400.0, 300.0));
    t.base().bounds.size.h
}

#[test]
fn toast_height_grows_with_body_then_action() {
    use heca_grid_ui::Toast;
    let bare = layout_toast(&mut Toast::info("Saved"));
    let with_body = layout_toast(&mut Toast::info("Saved").body_text("All files written"));
    let with_action = layout_toast(
        &mut Toast::info("Saved")
            .body_text("All files written")
            .action(heca_grid_ui::Button::outline("Undo").on_click(|| {})),
    );
    assert!(with_body > bare, "a body line adds height");
    assert!(with_action > with_body, "an action row adds further height");
}

/// **The text is never cut.** A body longer than the card is wide wraps onto more lines — the whole
/// path is on screen, in pieces — and the card grows to hold them, up to the line cap.
#[test]
fn toast_body_wraps_instead_of_cutting_and_stops_at_the_line_cap() {
    use heca_grid_ui::Toast;
    let path = "/Users/antonio/projects/gleam/tutorial/gleam.toml is ignored until you trust it";
    let short = layout_toast(&mut Toast::info("Saved").body_text("ok"));

    let mut whole = Toast::info("Project settings not trusted").body_text(path);
    let tall = layout_toast(&mut whole);
    assert!(
        tall > short,
        "the long body takes more lines: {tall} vs {short}"
    );
    let drawn: String = common::paint(&whole, &Theme::default())
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");
    assert!(
        drawn.contains("/Users/") && drawn.contains("gleam.toml") && !drawn.contains('…'),
        "the path is shown whole, none of it cut: {drawn:?}"
    );

    // With a cap the card stops growing and marks the cut.
    let mut capped = Toast::info("Project settings not trusted")
        .max_lines(2)
        .body_text(path);
    let capped_h = layout_toast(&mut capped);
    assert!(
        capped_h < tall,
        "the cap stops the growth: {capped_h} vs {tall}"
    );
    let marked = common::paint(&capped, &Theme::default())
        .iter()
        .any(|c| matches!(c, DrawCommand::Text(t) if t.text.ends_with('…')));
    assert!(marked, "text over the cap ends with an ellipsis");
}

#[test]
fn toast_dismiss_button_fires_on_dismiss_and_consumes() {
    use heca_grid_ui::Toast;
    use std::cell::Cell;
    use std::rc::Rc;

    let dismissed = Rc::new(Cell::new(0u32));
    let d = dismissed.clone();
    let mut t = Toast::warning("Disk almost full").on_dismiss(move || d.set(d.get() + 1));
    layout_toast(&mut t);

    // The × is a real `IconButton` now, so it takes a **click** — press and release — like every
    // other control, and it is wherever the engine placed it rather than at a remembered pixel.
    let cross = t.base().children[2].base().bounds;
    let centre = Point::new(
        cross.loc.x + cross.size.w / 2.0,
        cross.loc.y + cross.size.h / 2.0,
    );
    let _ = heca_grid_ui::dispatch(&mut t, &Event::pointer_pressed(centre, PointerButton::Left));
    let hit = heca_grid_ui::dispatch(
        &mut t,
        &Event::pointer_released(centre, PointerButton::Left),
    );
    assert_eq!(dismissed.get(), 1, "clicking × fires on_dismiss");
    assert!(matches!(hit, Handled::Yes), "the × consumes the click");
}

#[test]
fn toast_action_button_fires_on_action() {
    use heca_grid_ui::Toast;
    use std::cell::Cell;
    use std::rc::Rc;

    let acted = Rc::new(Cell::new(0u32));
    let a = acted.clone();
    let mut t = Toast::info("File deleted")
        .action(heca_grid_ui::Button::outline("Undo").on_click(move || a.set(a.get() + 1)));
    layout_toast(&mut t);

    // Action row sits below the title, left-aligned in the text column.
    click_at(&mut t, Point::new(60.0, 50.0), PointerButton::Left);
    assert_eq!(acted.get(), 1, "clicking the action button fires on_action");
}

/// **The action button is as wide as what it says.** A column stretches its children across the
/// full width — right for the title and the body, wrong for a button: it filled the card edge to
/// edge and read as a banner rather than something to press (F003/P082/T481).
#[test]
fn toast_action_button_hugs_its_label() {
    use heca_grid_ui::{DrawCommand, Toast};

    let mut t =
        Toast::info("File deleted").action(heca_grid_ui::Button::outline("Undo").on_click(|| {}));
    layout_toast(&mut t);
    let theme = Theme::default();
    let scene = common::paint(&t, &theme);
    // The action's face is the card's only bordered rect.
    let card = t.base().bounds;
    // The card's own bracket frame is drawn as bordered rects at the card's size; the action's
    // face is the bordered rect that sits strictly inside it.
    let face = scene
        .base_layer()
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r) if r.border.is_some() && r.rect.size.w < card.size.w => {
                Some(r.rect)
            }
            _ => None,
        })
        .next()
        .expect("the action button paints a face");
    assert!(
        face.size.w < card.size.w / 2.0,
        "the action button spans {:.0} of the card's {:.0} — it should hug its label",
        face.size.w,
        card.size.w,
    );
}

#[test]
fn toast_body_click_fires_on_click_only_when_set() {
    use heca_grid_ui::Toast;
    use std::cell::Cell;
    use std::rc::Rc;

    // Without on_click, a body click is not consumed (it can fall through).
    let mut inert = Toast::info("Build finished").dismissible(false);
    layout_toast(&mut inert);
    let hit = heca_grid_ui::dispatch(
        &mut inert,
        &Event::pointer_pressed(Point::new(160.0, 20.0), PointerButton::Left),
    );
    assert!(
        matches!(hit, Handled::No),
        "a non-clickable toast doesn't eat body clicks"
    );

    // With on_click, the same click activates + consumes.
    let clicked = Rc::new(Cell::new(0u32));
    let c = clicked.clone();
    let mut t = Toast::info("Build finished")
        .dismissible(false)
        .on_click(move || c.set(c.get() + 1));
    layout_toast(&mut t);
    let hit = heca_grid_ui::dispatch(
        &mut t,
        &Event::pointer_pressed(Point::new(160.0, 20.0), PointerButton::Left),
    );
    assert_eq!(clicked.get(), 1, "body click fires on_click");
    assert!(
        matches!(hit, Handled::Yes),
        "a clickable toast consumes the body click"
    );
}

/// **The body is whatever you composed** — the card gives it the column and the engine lays it out.
/// It used to be a `String` the card turned into one `Label`, which is why a notification could
/// never carry a row of stats or a small grid (F003/P096/T483).
#[test]
fn toast_body_takes_a_component_and_lays_it_out() {
    use heca_grid_ui::{Component, Flex, Label, LayoutExt, Length, Parent, Toast};

    let mut t = Toast::info("Backup finished").body(
        Flex::row()
            .gap(8.0)
            .child(Label::new("142 files"))
            .child(Label::new("3.2 GB")),
    );
    LayoutEngine::new().compute(&mut t, Size::new(400.0, 300.0));

    let body = t.base().children[1].base().children[1].base();
    assert_eq!(
        body.children.len(),
        2,
        "the composed body is the card's body slot"
    );
    assert!(
        body.bounds.size.w > 0.0 && body.bounds.size.h > 0.0,
        "and it was laid out"
    );
    let _ = Length::Px(0.0); // keep the import honest
}

/// **Actions repeat, and they stay inside the card at any width.** One action was a widget limit
/// that had been written into the app's notification model; a real notification offers a retry
/// *and* a way to look at what failed.
#[test]
fn toast_actions_repeat_and_stay_inside_the_card() {
    use heca_grid_ui::{Button, Component, Toast};

    for width in [320.0f64, 120.0] {
        let mut t = Toast::danger("Connection lost")
            .action(Button::outline("Retry").on_click(|| {}))
            .action(Button::ghost("Details").on_click(|| {}))
            .action(Button::ghost("Ignore").on_click(|| {}));
        t.base_mut().style.layout.width = heca_grid_ui::Length::Px(width as f32);
        LayoutEngine::new().compute(&mut t, Size::new(width, 300.0));

        let card = t.base().bounds;
        let row = t.base().children[1].base().children[2].base();
        assert_eq!(row.children.len(), 3, "three actions, three controls");
        for action in &row.children {
            let b = action.base().bounds;
            assert!(
                b.loc.x >= card.loc.x - 0.5 && b.loc.x + b.size.w <= card.loc.x + card.size.w + 0.5,
                "an action spans {}..{} in a {width}px card",
                b.loc.x,
                b.loc.x + b.size.w,
            );
        }
    }
}

/// **An empty slot costs nothing — not even the row's gap.** A zero-sized placeholder still takes
/// the gap beside it, which is why a card with no leading icon began its text a gap further in
/// than a card with one.
#[test]
fn a_toast_without_an_icon_starts_its_text_where_the_padding_ends() {
    use heca_grid_ui::{Component, Toast};

    let mut with_icon = Toast::info("Saved");
    LayoutEngine::new().compute(&mut with_icon, Size::new(400.0, 300.0));
    let icon_left = with_icon.base().children[0].base().bounds.loc.x;

    let mut bare = Toast::info("Saved").no_icon();
    LayoutEngine::new().compute(&mut bare, Size::new(400.0, 300.0));
    let text_left = bare.base().children[1].base().bounds.loc.x;

    assert!(
        (text_left - icon_left).abs() < 0.5,
        "the text should start where the icon would have ({icon_left}), not at {text_left}",
    );
}

#[test]
fn toast_focusable_only_when_clickable_and_enter_activates() {
    use heca_grid_ui::{Component, Toast};
    use std::cell::Cell;
    use std::rc::Rc;

    let plain = Toast::info("Just an FYI");
    assert!(!plain.focusable(), "a non-clickable toast is not focusable");

    let clicked = Rc::new(Cell::new(0u32));
    let c = clicked.clone();
    let mut t = Toast::info("Open log?").on_click(move || c.set(c.get() + 1));
    assert!(t.focusable(), "a clickable toast is focusable");
    // A raw key reaches only the widget that owns the keyboard — the assertion below already
    // says "focused", so make it so rather than relying on an unfocused widget taking Enter.
    t.base_mut().focused.set(true);
    heca_grid_ui::dispatch(
        &mut t,
        &Event::Key {
            key: GridKey::Enter,
            pressed: true,
        },
    );
    assert_eq!(
        clicked.get(),
        1,
        "Enter activates a focused clickable toast"
    );
}

#[test]
fn toast_action_press_flashes_only_the_action_not_the_whole_card() {
    use heca_grid_ui::Toast;
    let theme = Theme::default();

    // Press the Retry action, then paint: the press flash must cover only the
    // action button, not the whole card (no "whole widget clicked" feedback).
    let mut t =
        Toast::info("File deleted").action(heca_grid_ui::Button::outline("Retry").on_click(|| {}));
    layout_toast(&mut t);
    let card_w = t.base().bounds.size.w;
    click_at(&mut t, Point::new(60.0, 50.0), PointerButton::Left);

    let scene = common::paint(&t, &theme);
    // The flash is drawn in the theme's foreground color (see PaintCx::flash).
    let fg = theme.colors.foreground;
    let flash = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Rect(r)
                if r.fill.r == fg.r && r.fill.g == fg.g && r.fill.b == fg.b && r.fill.a > 0 =>
            {
                Some(*r)
            }
            _ => None,
        })
        .expect("an action press emits a press-flash rect");
    assert!(
        flash.rect.size.w < card_w - 1.0,
        "action flash ({}) must be narrower than the whole card ({card_w})",
        flash.rect.size.w,
    );
}
