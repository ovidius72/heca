mod common;

use common::click_at;
use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, Event, LayoutEngine, Point, Size, Theme};

#[test]
fn button_click_fires_within_bounds() {
    use std::cell::Cell;
    use std::rc::Rc;

    let clicked = Rc::new(Cell::new(false));
    let flag = clicked.clone();
    let mut button = Button::new("DEREZ").on_click(move || flag.set(true));
    LayoutEngine::new().compute(&mut button, Size::new(200.0, 80.0));

    let b = button.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    let outside = Point::new(b.loc.x + b.size.w + 100.0, b.loc.y);

    click_at(&mut button, outside, PointerButton::Left);
    assert!(!clicked.get(), "click outside bounds must not fire");
    click_at(&mut button, center, PointerButton::Left);
    assert!(clicked.get(), "click inside bounds must fire");
}

#[test]
fn button_hover_tracks_pointer() {
    let mut button = Button::new("HOVER");
    LayoutEngine::new().compute(&mut button, Size::new(200.0, 80.0));
    let hovered = button.hovered();
    let b = button.base().bounds;

    heca_grid_ui::dispatch(
        &mut button,
        &Event::pointer_moved(Point::new(b.loc.x + 2.0, b.loc.y + 2.0)),
    );
    assert!(hovered.get_untracked(), "entering bounds sets hover");
    heca_grid_ui::dispatch(
        &mut button,
        &Event::pointer_moved(Point::new(b.loc.x + b.size.w + 50.0, b.loc.y)),
    );
    assert!(!hovered.get_untracked(), "leaving bounds clears hover");
}

#[test]
fn button_variants_paint_distinct_fills() {
    let theme = Theme::default();
    let fill_of = |button: Button| {
        let scene = common::paint(&button, &theme);
        scene.iter().find_map(|c| match c {
            DrawCommand::Rect(r) => Some(r.fill),
            _ => None,
        })
    };
    // At rest: primary paints an opaque dark surface (neon border, not a bright
    // fill); ghost is invisible (alpha 0).
    let primary = fill_of(Button::primary("X"));
    let ghost = fill_of(Button::ghost("X"));
    assert_eq!(
        primary,
        Some(theme.colors.surface),
        "primary rests on a dark surface"
    );
    assert_eq!(ghost.map(|c| c.a), Some(0), "ghost is invisible at rest");
    assert_ne!(primary, ghost);
}

#[test]
fn disabled_button_ignores_clicks_and_focus() {
    use std::cell::Cell;
    use std::rc::Rc;

    let clicked = Rc::new(Cell::new(false));
    let flag = clicked.clone();
    let mut button = Button::new("X")
        .disabled(true)
        .on_click(move || flag.set(true));
    LayoutEngine::new().compute(&mut button, Size::new(200.0, 80.0));

    let b = button.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    click_at(&mut button, center, PointerButton::Left);
    assert!(!clicked.get(), "disabled button ignores clicks");
    assert!(!button.focusable(), "disabled button is unfocusable");
}

/// The label color of a button after layout + paint (its single Text run).
fn button_label_color(button: &mut Button, theme: &Theme) -> Color {
    LayoutEngine::new().compute(button, Size::new(200.0, 80.0));
    let scene = common::paint(button, theme);
    scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text(t) => Some(t.color),
            _ => None,
        })
        .expect("button paints a label text run")
}

#[test]
fn disabled_button_label_is_muted_and_faded_on_every_variant() {
    use heca_grid_ui::ButtonVariant;
    let theme = Theme::default();
    let muted = theme.colors.muted;
    for variant in [
        ButtonVariant::Primary,
        ButtonVariant::Secondary,
        ButtonVariant::Destructive,
        ButtonVariant::Outline,
        ButtonVariant::Ghost,
        ButtonVariant::Link,
    ] {
        let disabled = button_label_color(
            &mut Button::new("OK").variant(variant).disabled(true),
            &theme,
        );
        let enabled = button_label_color(&mut Button::new("OK").variant(variant), &theme);
        // Disabled label is the theme `muted` hue (not the variant's vivid color)…
        assert_eq!(
            disabled.with_alpha(255),
            muted.with_alpha(255),
            "disabled label should use the theme muted hue",
        );
        // …at a clearly reduced opacity, so the disabled state reads even on Ghost/Link
        // (whose enabled rest label is already `muted` at full alpha).
        assert!(disabled.a < 255, "disabled label should be faded");
        assert!(
            disabled.a < enabled.a,
            "disabled label must be fainter than the enabled label",
        );
    }
}

#[test]
fn icon_button_hugs_icon_by_default_and_pins_an_explicit_size() {
    use heca_grid_ui::{Glyph, Icon, IconButton};

    // Default: hugs the icon + padding (square-ish, larger than the glyph).
    let mut hug = IconButton::new(Icon::new(Glyph::Gear).size(18.0));
    LayoutEngine::new().compute(&mut hug, Size::new(200.0, 200.0));
    let b = hug.base().bounds;
    assert!(b.size.w > 18.0 && b.size.h > 18.0, "hugs icon + padding");
    assert!((b.size.w - b.size.h).abs() < 2.0, "roughly square");

    // Pinned: an exact square.
    let mut pinned = IconButton::new(Icon::new(Glyph::Gear).size(18.0)).cell(40.0);
    LayoutEngine::new().compute(&mut pinned, Size::new(200.0, 200.0));
    let pb = pinned.base().bounds;
    assert_eq!(pb.size.w, 40.0, "pinned width");
    assert_eq!(pb.size.h, 40.0, "pinned square");
}

#[test]
fn icon_button_activates_on_click_and_enter_only_when_wired() {
    use heca_grid_ui::{Glyph, Icon, IconButton};
    use std::cell::Cell;
    use std::rc::Rc;

    // No on_click → inert + unfocusable.
    let mut bare = IconButton::new(Icon::new(Glyph::Search).size(18.0));
    LayoutEngine::new().compute(&mut bare, Size::new(100.0, 100.0));
    assert!(
        !bare.focusable(),
        "an icon button without on_click is not focusable"
    );

    let clicks = Rc::new(Cell::new(0u32));
    let sink = clicks.clone();
    let mut btn = IconButton::new(Icon::new(Glyph::Search).size(18.0))
        .on_click(move || sink.set(sink.get() + 1));
    LayoutEngine::new().compute(&mut btn, Size::new(100.0, 100.0));
    assert!(btn.focusable(), "wired icon button is focusable");

    let b = btn.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    click_at(&mut btn, center, PointerButton::Left);
    // A raw key reaches only the widget that owns the keyboard — focus it, as a real surface
    // would before sending one.
    btn.base().focus(false);
    heca_grid_ui::dispatch(
        &mut btn,
        &Event::Key {
            key: heca_grid_ui::GridKey::Enter,
            pressed: true,
        },
    );
    assert_eq!(clicks.get(), 2, "click + Enter both fire on_click");
}

// ── Button: content is composed from children (viewnode-all-widgets) ────────────────────────

/// The sugar builders are exactly that: they build **children**. There is no separate "simple
/// mode" — `Button::new(..)` and `.icon(..)` produce the same child vector a caller (or the
/// `ViewNode` mapper) would compose by hand, so there is one layout and one paint path.
#[test]
fn button_sugar_desugars_into_children() {
    assert_eq!(
        Button::empty().base().children.len(),
        0,
        "empty button has no content"
    );
    assert_eq!(
        Button::new("Delete").base().children.len(),
        1,
        "label sugar → one Label child"
    );

    let with_icon = Button::new("Delete").icon(Glyph::Trash);
    assert_eq!(
        with_icon.base().children.len(),
        2,
        "icon sugar prepends → [Icon, Label]"
    );

    // Arbitrary content, any depth — the same vector, composed instead of sugared.
    let composed = Button::empty().child(
        Flex::column()
            .child(
                Flex::row()
                    .child(Icon::new(Glyph::Trash))
                    .child(Label::new("Delete")),
            )
            .child(Label::new("Ctrl+D")),
    );
    assert_eq!(composed.base().children.len(), 1, "one composed subtree");
    assert_eq!(
        composed.base().children[0].base().children.len(),
        2,
        "the subtree keeps its own structure (row + accelerator label)",
    );
}

/// The button no longer computes its own width from a character count — it **hugs its content**
/// and taffy measures it. The resulting box must still match the old hand-computed geometry
/// exactly: label (`chars × font × advance`) + one character of breathing room per side + the
/// size-scaled base padding.
#[test]
fn button_hugs_its_content_at_the_historical_size() {
    use heca_grid_ui::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
    const BASE_PAD: f32 = 10.0; // Button::BASE_PAD (private)

    let mut b = Button::new("DELETE").size(WidgetSize::Large); // Large ⇒ pad_scale == 1.0
    LayoutEngine::new().compute(&mut b, Size::new(400.0, 200.0));
    let fs = b.base().font;
    let bounds = b.base().bounds;

    let chars = "DELETE".chars().count() as f32;
    let expected_w = (chars + 2.0) * fs * MONO_ADVANCE_RATIO + BASE_PAD * 2.0;
    let expected_h = fs * MONO_LINE_RATIO + BASE_PAD * 2.0;
    assert!(
        (bounds.size.w - expected_w as f64).abs() < 0.5,
        "width hugs content at the historical size: got {}, want {expected_w}",
        bounds.size.w,
    );
    assert!(
        (bounds.size.h - expected_h as f64).abs() < 0.5,
        "height hugs content at the historical size: got {}, want {expected_h}",
        bounds.size.h,
    );
}

/// A button's content is sized by the tree, so richer content makes the button grow — the thing a
/// hand-computed, label-only width could never do.
#[test]
fn button_grows_to_fit_composed_content() {
    let measure = |mut b: Button| {
        LayoutEngine::new().compute(&mut b, Size::new(500.0, 200.0));
        b.base().bounds.size
    };
    let plain = measure(Button::new("Delete"));
    let with_icon = measure(Button::new("Delete").icon(Glyph::Trash));
    let stacked = measure(
        Button::empty().child(
            Flex::column()
                .child(Label::new("Delete"))
                .child(Label::new("Ctrl+D")),
        ),
    );

    assert!(with_icon.w > plain.w, "a leading icon widens the button");
    assert!(
        stacked.h > plain.h,
        "a two-line column makes the button taller"
    );
}

/// The size variant cascades into composed content: a `Small` button's `Label` must shrink with
/// it. Before the layout pass inherited the variant, a child kept the default and a small button
/// rendered full-size text.
#[test]
fn button_size_variant_cascades_to_composed_content() {
    let label_font = |size: WidgetSize| {
        let mut b = Button::new("SAVE").icon(Glyph::Check).size(size);
        LayoutEngine::new().compute(&mut b, Size::new(400.0, 200.0));
        // children = [Icon, Label]; read the label's resolved font.
        b.base().children[1].base().font
    };
    assert!(
        label_font(WidgetSize::Small) < label_font(WidgetSize::Large),
        "the button's size variant reaches its composed Label",
    );

    // An explicit choice on the child wins over the inherited one.
    let mut b = Button::new("SAVE").size(WidgetSize::Small);
    b.base_mut().children[0]
        .base_mut()
        .style
        .layout
        .set_size(WidgetSize::Large);
    LayoutEngine::new().compute(&mut b, Size::new(400.0, 200.0));
    let pinned = b.base().children[0].base().font;
    assert!(
        pinned > label_font(WidgetSize::Small),
        "an explicit child variant is not overwritten"
    );
}

/// Composed content inherits the button's **state color**: the button publishes one color per
/// frame and unstyled children pick it up, which is what makes them animate with the hover sweep
/// and fade when disabled — with no wiring between the two widgets.
#[test]
fn composed_content_inherits_the_buttons_state_color() {
    let theme = Theme::default();

    // Disabled ⇒ the content fades to `muted` at reduced alpha, on every variant.
    let disabled = button_label_color(&mut Button::primary("OK").disabled(true), &theme);
    assert_eq!(
        disabled.r, theme.colors.muted.r,
        "disabled content takes the muted tone"
    );
    assert!(disabled.a < 255, "disabled content is faded");

    // Enabled ⇒ the variant's own tone, not the theme foreground.
    let enabled = button_label_color(&mut Button::primary("OK"), &theme);
    assert_ne!(enabled, disabled, "enabled and disabled content differ");

    // An explicit child color opts out of inheritance entirely.
    let pinned = button_label_color(
        &mut Button::empty().child(Label::new("OK").color(theme.colors.success)),
        &theme,
    );
    assert_eq!(
        pinned, theme.colors.success,
        "an explicit child color wins over the inherited one"
    );
}

/// A control is **one** Tab stop, whatever it composes. Focus traversal must not descend into a
/// button's content — otherwise a focusable child would take its own Tab stop while being
/// click-dead (the button consumes the press and never routes it to children).
#[test]
fn a_button_is_one_tab_stop_whatever_it_contains() {
    let mut ui = Flex::row()
        .child(Button::new("A").child(Toggle::new())) // an interactive child, as decoration
        .child(Button::new("B"));

    let mut focus = FocusManager::new();
    focus.advance(&mut ui, true);
    let first = focus.focused(&mut ui);
    focus.advance(&mut ui, true);
    let second = focus.focused(&mut ui);
    focus.advance(&mut ui, true);

    assert_eq!(
        focus.focused(&mut ui),
        first,
        "exactly two focusables: focus wraps after the 2nd button"
    );
    assert_ne!(first, second, "each button is its own (single) Tab stop");
}
