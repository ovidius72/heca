use super::*;

#[test]
fn select_opens_and_paints_options_in_overlay_layer() {
    let theme = Theme::default();
    let mut sel = Select::new(["LOW", "MEDIUM", "HIGH"]);
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 200.0));

    let texts = |s: &Select| -> Vec<String> {
        let scene = common::paint(s, &theme);
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    };

    // Closed: only the selected label shows; not overlay-active.
    assert!(!sel.overlay_active(), "closed select is not overlay-active");
    assert_eq!(
        texts(&sel),
        vec!["LOW".to_string()],
        "closed shows only the trigger label"
    );

    // Open via click on the trigger.
    let b = sel.base().bounds;
    click_at(
        &mut sel,
        Point::new(b.loc.x + 5.0, b.loc.y + 5.0),
        PointerButton::Left,
    );
    assert!(
        sel.overlay_active(),
        "clicking the trigger opens + grabs input"
    );
    let open_texts = texts(&sel);
    assert!(open_texts.contains(&"MEDIUM".to_string()) && open_texts.contains(&"HIGH".to_string()));
}

#[test]
fn select_click_row_commits_and_closes() {
    use heca_grid_ui::{Action, SignalData};
    use std::cell::RefCell;
    use std::rc::Rc;

    let log: Rc<RefCell<Vec<Action>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    let mut sel =
        Select::new(["LOW", "MEDIUM", "HIGH"]).on_change(move |a| sink.borrow_mut().push(a));
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 200.0));

    let b = sel.base().bounds;
    click_at(
        &mut sel,
        Point::new(b.loc.x + 5.0, b.loc.y + 5.0),
        PointerButton::Left,
    ); // open

    // Click the third row (HIGH) **where it actually is**: the options are child components, and
    // opening the list placed them in the panel, so their bounds are the rows on screen. No row
    // arithmetic — what is drawn is what is clicked.
    let row2 = sel.base().children[2].base().bounds;
    assert!(
        row2.loc.y > b.loc.y + b.size.h,
        "the rows are placed in the panel, below the trigger"
    );
    click_at(
        &mut sel,
        Point::new(row2.loc.x + 10.0, row2.loc.y + row2.size.h / 2.0),
        PointerButton::Left,
    );
    assert_eq!(sel.index(), 2, "clicking a row selects it");
    assert!(!sel.overlay_active(), "selection closes the dropdown");
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("select-change", SignalData::Usize(2))),
    );
}

#[test]
fn select_sugar_builds_choice_children_and_composed_options_carry_their_content() {
    let theme = Theme::default();

    // The string constructor is sugar: every option is a `Choice` child whose value is the text.
    let sugar = Select::new(["LOW", "HIGH"]);
    assert_eq!(sugar.base().children.len(), 2, "one child per option");
    assert_eq!(
        sugar.base().children[1].text_summary().as_deref(),
        Some("HIGH"),
        "the option's content is a Label the widget can name",
    );

    // A composed option: any content, plus a value that is not the text.
    let mut sel = Select::empty()
        .option(Choice::new("low").child(Flex::row().child(Label::new("LOW"))))
        .option(
            Choice::new("high").child(
                Flex::row()
                    .child(Icon::new(Glyph::Warning))
                    .child(Label::new("HIGH")),
            ),
        )
        .selected(1);
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 200.0));

    // The closed trigger shows the chosen option **itself** — it stands the child inside the
    // trigger box, so the icon comes with it. (Its text alone is still available as the option's
    // accessible name, which is what `selected_label` reports.)
    assert_eq!(sel.selected_label(), "HIGH");
    let scene = common::paint(&sel, &theme);
    let closed: Vec<String> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        closed.contains(&"HIGH".to_string()),
        "the closed trigger shows the chosen option's label",
    );
    assert!(
        closed.len() > 1,
        "…and its Icon child, painted with it: {closed:?}",
    );
    let trigger = sel.base().bounds;
    let chosen = sel.base().children[1].base().bounds;
    let slack = 0.5; // the trigger hugs the tallest option, so they agree to within rounding
    assert!(
        chosen.loc.y >= trigger.loc.y - slack
            && chosen.loc.y + chosen.size.h <= trigger.loc.y + trigger.size.h + slack,
        "the chosen option is placed inside the trigger while closed",
    );
    assert_eq!(
        sel.base().children[0].base().bounds.size,
        Size::new(0.0, 0.0),
        "the options not chosen are collapsed",
    );

    // Opening draws the options' own content — including the icon, which no `Vec<String>` of
    // options could ever have carried.
    let press = Point::new(sel.base().bounds.loc.x + 5.0, sel.base().bounds.loc.y + 5.0);
    click_at(&mut sel, press, PointerButton::Left);
    let scene = common::paint(&sel, &theme);
    let runs: Vec<(String, Color)> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some((t.text.clone(), t.color)),
            _ => None,
        })
        .collect();
    let texts: Vec<&str> = runs.iter().map(|(t, _)| t.as_str()).collect();
    assert!(
        texts.contains(&"LOW") && texts.contains(&"HIGH"),
        "the open list paints each option's Label child",
    );
    assert!(
        runs.len() > 3,
        "beyond the trigger + the two labels, the option's Icon child draws its glyph too",
    );
    // The chosen row tints its whole content: the `Choice` publishes the accent as the inherited
    // content color, and its unstyled Label picks it up — with no wiring from the Select.
    let high_row = runs
        .iter()
        .rposition(|(t, _)| t == "HIGH")
        .expect("the HIGH row is painted");
    assert_eq!(
        runs[high_row].1, theme.colors.accent,
        "the selected option's content is accent-tinted",
    );

    // …and the trigger keeps showing the chosen option — icon *and* label — while the list is open.
    // The option itself is in the list now, so the trigger draws a second image of its content,
    // translated back into the trigger. Two runs land inside the trigger: the glyph and the word.
    let trigger = sel.base().bounds;
    let in_trigger: Vec<String> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some((t.rect, t.text.clone())),
            _ => None,
        })
        .filter(|(r, _)| {
            trigger.contains(Point::new(
                r.loc.x + r.size.w / 2.0,
                r.loc.y + r.size.h / 2.0,
            ))
        })
        .map(|(_, t)| t)
        .collect();
    assert!(
        in_trigger.contains(&"HIGH".to_string()),
        "the open trigger still names the chosen option: {in_trigger:?}",
    );
    assert!(
        in_trigger.len() > 1,
        "…and still shows its icon, not just the word: {in_trigger:?}",
    );
}
