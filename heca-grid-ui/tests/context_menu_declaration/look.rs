//! What an open menu looks like and how the pointer drives it — the panel is a light layer and its
//! entries are real rows, drawn as the menu always drew them.

use super::*;
use heca_grid_ui::{DrawCommand, PaintCx, Scene, Theme};

/// A menu of three entries — the last one destructive — open at (100, 100) in an 800×600 window.
fn open_menu(ran: Rc<RefCell<Vec<&'static str>>>) -> ContextMenu {
    let entry = |name: &'static str| {
        let ran = ran.clone();
        MenuItem::new()
            .label(name)
            .on_click(move || ran.borrow_mut().push(name))
    };
    let mut menu = MenuAnchor::At(Point::new(100.0, 100.0)).open(
        ContextMenu::new("look").child(
            Menu::new("Pane", "what you can do")
                .child(entry("Rename"))
                .child(entry("Split"))
                .child(entry("Close").danger(true)),
        ),
    );
    LayoutEngine::new().compute(&mut menu, Size::new(800.0, 600.0));
    menu
}

fn paint(menu: &ContextMenu) -> Scene {
    let theme = Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(Size::new(800.0, 600.0));
        heca_grid_ui::paint_child(menu, &mut cx);
    }
    scene
}

/// **The menu keeps its look**: the panel's accent edge with its 12 px, 0.3 halo, and the chosen
/// entry as a pill — the accent at the theme's row fill over the whole row, with a short bar at its
/// leading edge — on the first entry when it opens.
#[test]
fn an_open_menu_draws_its_frame_and_the_chosen_entry_as_a_pill() {
    let menu = open_menu(Rc::default());
    let scene = paint(&menu);
    let theme = Theme::default();
    let accent = theme.colors.accent;
    let ia = theme.colors.interaction;
    let rects: Vec<_> = scene
        .overlay_layer()
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r) => Some(*r),
            _ => None,
        })
        .collect();
    assert!(
        rects.iter().any(|r| r
            .glow
            .is_some_and(|g| (g.radius - 12.0).abs() < 0.01 && (g.intensity - 0.3).abs() < 0.01)
            && r.border
                .is_some_and(|b| b.color == accent.with_alpha(ia.panel_border))),
        "the panel wears the accent edge and its 12 px, 0.3 halo"
    );
    let first = menu.rows()[0].base().bounds;
    assert!(
        rects
            .iter()
            .any(|r| r.rect == first && r.fill == accent.with_alpha(ia.panel_row_fill)),
        "the first entry is the chosen one, filled with the accent"
    );
    assert!(
        rects.iter().any(|r| r.fill == accent
            && r.rect.loc.x == first.loc.x
            && r.rect.size.w < 4.0
            && r.rect.size.h < first.size.h),
        "with a short accent bar at its leading edge"
    );
}

/// **The pointer moves the choice, and a press runs it** — the row under the pointer is asked of
/// the framework (its hover), not hit-tested by the menu.
#[test]
fn the_pointer_moves_the_choice_and_a_press_runs_it() {
    let ran: Rc<RefCell<Vec<&'static str>>> = Rc::default();
    let mut menu = open_menu(ran.clone());
    let second = menu.rows()[1].base().bounds;
    let at = Point::new(second.loc.x + 10.0, second.loc.y + second.size.h / 2.0);
    heca_grid_ui::dispatch(&mut menu, &Event::pointer_moved(at));
    let accent = Theme::default().colors.accent;
    let fill = accent.with_alpha(Theme::default().colors.interaction.panel_row_fill);
    let lit: Vec<_> = paint(&menu)
        .overlay_layer()
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r) if r.fill == fill => Some(r.rect),
            _ => None,
        })
        .collect();
    assert_eq!(
        lit,
        vec![second],
        "the entry under the pointer is the chosen one"
    );
    click_at(&mut menu, at, PointerButton::Left);
    assert_eq!(
        *ran.borrow(),
        vec!["Split"],
        "a press runs the entry under it"
    );
}

/// **Entries read quiet until chosen**: the chosen one in the theme's text, the others muted
/// towards it, and a destructive one in the danger colour.
#[test]
fn entries_read_quiet_until_chosen_and_a_destructive_one_in_danger() {
    let menu = open_menu(Rc::default());
    let theme = Theme::default();
    let (fg, muted, danger) = (
        theme.colors.foreground,
        theme.colors.muted,
        theme.colors.danger,
    );
    let words: Vec<(String, heca_grid_ui::Color)> = paint(&menu)
        .overlay_layer()
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some((t.text.clone(), t.color)),
            _ => None,
        })
        .collect();
    let colour = |name: &str| words.iter().find(|(t, _)| t == name).map(|(_, c)| *c);
    assert_eq!(colour("Rename"), Some(fg), "the chosen entry reads in full");
    assert_eq!(
        colour("Split"),
        Some(muted.lerp(fg, 0.75)),
        "the others read quiet"
    );
    assert_eq!(
        colour("Close"),
        Some(muted.lerp(danger, 0.75)),
        "a destructive one reads in the danger colour"
    );
}
