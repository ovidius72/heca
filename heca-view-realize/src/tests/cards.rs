//! Card grids and hints written into any kind.

use heca_view::WidgetId;

use crate::props::glyph_from_name;
use super::*;

/// **A card never has to be named**.
///
/// `key` is optional everywhere in this library, so it is optional on a card. A grid whose
/// cards declare none still has to report *which* card was chosen — otherwise a plugin gets a
/// picker that lights up, moves, activates, and hands back nothing, with no error anywhere.
/// That is the shape `.draggable()` shipped in: a capability gated on `Base::key`, silently
/// dead on every widget nobody had reason to name (AGENTS.md § 0a).
///
/// The name is the card's **own content**, through the accessible-name algorithm the library
/// already uses for exactly this — [`Component::text_summary`], which is what
/// `nav::identity_of` reads at its second level. So a card is named the way it reads on
/// screen, and a caller writes nothing.
///
/// ⚠️ It asks a card whose text is **nested**, because that is the documented shape: a card is
/// a `Surface` holding an icon and a label, and the card node itself carries no `text` prop of
/// its own. Reading only the node's own `text` prop passes a one-`Label` card and leaves every
/// real one anonymous.
#[test]
fn an_unnamed_card_is_still_reported_by_the_grid() {
    let (emit, fired) = recording_emitter();

    let card = |name: &str| {
        ViewNode::new(WidgetId::Surface)
            .child(ViewNode::new(WidgetId::Icon).prop("icon", PropValue::Text("terminal".into())))
            .child(ViewNode::new(WidgetId::Label).text(name))
    };
    let node = ViewNode::new(WidgetId::CardGrid)
        .on("activate", Intent::new("docker.open"))
        .child(card("nginx"))
        .child(card("redis"));

    use heca_grid_ui::event::{Event, WidgetIntent};

    let mut grid = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    // Keys reach the focus owner and nowhere else, so a test that activates says who is
    // holding the keyboard — exactly as a real surface has to.
    grid.base().focus(false);
    heca_grid_ui::dispatch(grid.as_mut(), &Event::Widget(WidgetIntent::Activate));

    assert_eq!(
        fired.borrow().first().and_then(|i| i.args.get("key")),
        Some(&PropValue::Text("nginx".into())),
        "an unnamed card must still come back by name, or a described grid reports nothing",
    );
}

/// **The cursor must walk the cards the way the eye does**.
///
/// [`CardGrid::row`] takes *columns* of cards and says so in its own contract: "`cards` must be
/// in the same left-to-right order the layout draws them, or the cursor and the picture
/// disagree." A described grid draws its children with `Flex::row` — left to right — so each
/// child is its own column. Handing the grid one column holding every card instead makes
/// arrow-right do nothing at all while arrow-down walks a row: the picture says one thing and
/// the keyboard another, and nothing anywhere fails.
///
/// It also pins the repeat spelling: two cards that read the same are still two cards, indexed
/// the way [`nav::identity_of`] indexes identically-named widgets (`nginx`, `nginx[1]`) rather
/// than collapsing onto whichever the grid met first.
#[test]
fn a_described_grid_walks_its_cards_the_way_it_draws_them() {
    use heca_grid_ui::event::{Event, WidgetIntent};

    let card = |name: &str| {
        ViewNode::new(WidgetId::Surface).child(ViewNode::new(WidgetId::Label).text(name))
    };
    let node = ViewNode::new(WidgetId::CardGrid)
        .on("activate", Intent::new("docker.open"))
        .child(card("nginx"))
        .child(card("redis"))
        .child(card("nginx"));

    let chosen_after = |steps: usize| {
        let (emit, fired) = recording_emitter();
        let mut grid = realize(
            &node,
            &Theme::default(),
            &emit,
            &mut FormBindings::default(),
        ).expect("valid described test widget");
        grid.base().focus(false);
        for _ in 0..steps {
            heca_grid_ui::dispatch(grid.as_mut(), &Event::Widget(WidgetIntent::ItemNext));
        }
        heca_grid_ui::dispatch(grid.as_mut(), &Event::Widget(WidgetIntent::Activate));
        fired
            .borrow()
            .last()
            .and_then(|i| i.args.get("key"))
            .cloned()
    };

    assert_eq!(
        chosen_after(1),
        Some(PropValue::Text("redis".into())),
        "one card right",
    );
    assert_eq!(
        chosen_after(2),
        Some(PropValue::Text("nginx[1]".into())),
        "and the second card reading `nginx` is its own card, not the first one again",
    );
}

/// **Anything a widget can be given in Rust, a description must be able to ask for.**
///
/// The runtime half: a `hint` written into a node of **any** kind survives `realize` and is
/// found by the picker. `on_hint` is on `HintExt`, so natively *every*
/// widget can be told what a pick does to it; this holds the described side to the same reach.
///
#[test]
fn a_hint_written_into_any_kind_is_found_by_the_picker() {
    let mut unreachable: Vec<String> = Vec::new();
    for kind in registered_ids() {
        let (emit, fired) = recording_emitter();
        let node = sample_node(kind.clone()).on_hint(Intent::new("picked"));
        let mut widget = realize(
            &node,
            &Theme::default(),
            &emit,
            &mut FormBindings::default(),
        ).expect("valid described test widget");
        if !heca_grid_ui::fire_hint(widget.as_mut(), &[]) {
            unreachable.push(format!(
                "{kind:?} (no hint declaration on the realized widget)"
            ));
            continue;
        }
        if fired.borrow().iter().all(|i| i.action != "picked") {
            unreachable.push(format!(
                "{kind:?} (declared a hint that fired something else)"
            ));
        }
    }
    assert!(
        unreachable.is_empty(),
        "a `hint` event written into these kinds does not reach the picker, so a plugin can \
         draw them but never make them pickable: {unreachable:#?}",
    );
}

/// Glyph names resolve to their `Glyph`; unknown names are `None` (no icon), never a panic.
#[test]
fn glyph_from_name_resolves_known_and_rejects_unknown() {
    assert_eq!(glyph_from_name("terminal"), Some(Glyph::Terminal));
    assert_eq!(glyph_from_name("git_branch"), Some(Glyph::GitBranch));
    assert_eq!(glyph_from_name("x_square"), Some(Glyph::XSquare));
    assert_eq!(glyph_from_name("not_a_real_icon"), None);
}
