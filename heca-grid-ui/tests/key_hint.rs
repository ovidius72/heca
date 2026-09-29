mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, LayoutEngine, Size, Theme};

#[test]
fn key_hint_overlays_letter_only_when_set() {
    use heca_grid_ui::{FontRole, Glyph, Icon, KeyHint};

    let hint: Signal<Option<String>> = signal(None);
    let mut wrapped = KeyHint::new(Icon::new(Glyph::Terminal).size(20.0)).hint(hint);

    let paint = |w: &mut KeyHint| -> (Vec<String>, usize) {
        LayoutEngine::new().compute(w, Size::new(80.0, 80.0));
        let theme = Theme::default();
        // `paint_via_child` (which paints through `paint_child`, not `w.paint`): the framework
        // draws the hint letter for whatever it is handed, which is what lets any widget carry one
        // instead of only a `KeyHint` (F003/P082/T431). Calling `paint` directly is painting
        // *around* the framework.
        let scene = common::paint_via_child(w, &theme);
        let mut texts = Vec::new();
        let mut icons = 0usize;
        for c in scene.iter() {
            if let DrawCommand::Text(t) = c {
                match t.font {
                    // Both glyph faces count as an icon run here: this test is about how many
                    // glyphs vs words a KeyHint paints, not which font supplied them.
                    FontRole::Icon | FontRole::NerdFont => icons += 1,
                    FontRole::Text => texts.push(t.text.clone()),
                }
            }
        }
        (texts, icons)
    };

    // No hint: the child icon paints, no keycap letter.
    let (texts, icons) = paint(&mut wrapped);
    assert!(icons >= 2, "wrapped icon still paints (duotone = 2 runs)");
    assert!(
        texts.iter().all(|t| t != "a"),
        "no keycap letter while hint is None"
    );

    // Hint set: the letter overlays; the child icon still paints underneath.
    hint.set(Some("a".to_string()));
    let (texts, icons) = paint(&mut wrapped);
    assert!(icons >= 2, "child icon still paints under the keycap");
    assert!(
        texts.iter().any(|t| t == "a"),
        "keycap letter paints while hint is Some"
    );
}

/// **A widget that is not a `KeyHint` gets its letter drawn too** (F003/P082/T431).
///
/// This is the whole point of moving the drawing into `paint_child`: being pickable stopped
/// depending on being wrapped. Before it, only `KeyHint::paint` knew how to draw a cap, so any
/// widget that wanted one had to be put inside a wrapper — and a plugin's widget could not be
/// pickable at all without knowing that.
#[test]
fn any_widget_carrying_a_letter_gets_a_keycap_not_only_key_hint() {
    use heca_grid_ui::{FontRole, Label};

    let letter: Signal<Option<String>> = signal(None);
    // A plain `Label` — no `KeyHint` anywhere in this tree.
    let mut plain = Label::new("nvim");
    plain.base_mut().hint_label = letter;

    let caps = |w: &mut Label| -> Vec<String> {
        LayoutEngine::new().compute(w, Size::new(120.0, 40.0));
        let theme = Theme::default();
        let scene = common::paint_via_child(w, &theme);
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) if t.font == FontRole::Text => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    };

    assert!(
        !caps(&mut plain).iter().any(|t| t == "b"),
        "no letter while none is offered"
    );

    letter.set(Some("b".to_string()));
    let texts = caps(&mut plain);
    assert!(
        texts.iter().any(|t| t == "b"),
        "the framework draws the letter over a bare Label"
    );
    assert!(
        texts.iter().any(|t| t == "nvim"),
        "and the widget's own content still paints"
    );
}

#[test]
fn key_hint_is_transparent_to_focus_and_activation() {
    use heca_grid_ui::{FocusManager, KeyHint};
    use std::cell::Cell;
    use std::rc::Rc;

    // A focusable child wrapped in a KeyHint must stay reachable + activatable.
    let clicks = Rc::new(Cell::new(0u32));
    let sink = clicks.clone();
    let mut wrapped =
        KeyHint::new(Item::new("file.rs").on_activate(move || sink.set(sink.get() + 1)));
    LayoutEngine::new().compute(&mut wrapped, Size::new(200.0, 60.0));

    // Focus traversal recurses through the transparent wrapper to the child.
    let mut focus = FocusManager::new();
    focus.advance(&mut wrapped, true);
    assert_eq!(
        focus.focused(),
        Some(0),
        "wrapped child is reachable by Tab"
    );

    // Events route through the wrapper to the child.
    focus.deliver_key(&mut wrapped, heca_grid_ui::GridKey::Enter);
    assert_eq!(clicks.get(), 1, "Enter activates the wrapped child");
}

#[test]
fn attention_effect_plays_a_fixed_number_of_pulses() {
    use heca_grid_ui::Attention;

    let mut a = Attention::new();
    assert!(!a.is_active(), "idle until triggered");
    a.trigger(3);
    assert!(a.is_active(), "active after trigger");
    assert_eq!(a.amount(), 1.0, "starts at full strength");

    // Each tick of (≥) the pulse duration completes one pulse. After 3, it stops.
    assert!(a.tick(0.3), "pulse 1 done, pulse 2 begins");
    assert!(a.tick(0.3), "pulse 2 done, pulse 3 begins");
    assert!(!a.tick(0.3), "pulse 3 done — sequence ends");
    assert!(!a.is_active(), "inactive after the fixed pulse count");
}

#[test]
fn row_attention_request_pulses_then_settles() {
    use heca_grid_ui::Row;

    // Host-owned attention request: setting it true fires one pulse sequence and
    // is consumed back to false (so each request = one sequence).
    let req = signal(false);
    let mut row = Row::new().attention(req).on_activate(|| {});
    LayoutEngine::new().compute(&mut row, Size::new(200.0, 40.0));

    req.set(true);
    assert!(
        row.tick(0.0),
        "attention request triggers an animating pulse"
    );
    assert!(!req.get_untracked(), "the request signal is consumed");

    // The sequence is finite — ticking it out eventually settles (no animation).
    let mut settled = false;
    for _ in 0..60 {
        if !row.tick(0.05) {
            settled = true;
            break;
        }
    }
    assert!(
        settled,
        "attention pulse sequence ends and the row stops animating"
    );
}
