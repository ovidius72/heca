mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, LayoutEngine, Size, Theme};

#[test]
fn card_carries_title_label() {
    let theme = Theme::default();
    let mut card = Card::new("UPLINK").child(Label::new("ONLINE"));
    // Laid out first — see `paint_emits_background_rect_and_label_text`.
    LayoutEngine::new()
        .base_font(13.0)
        .compute(&mut card, Size::new(300.0, 120.0));

    let scene = common::paint(&card, &theme);
    let texts: Vec<&str> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"UPLINK"), "card title should render");
    assert!(texts.contains(&"ONLINE"), "card body should render");
}

// ── Phase C: display widgets ──

#[test]
fn badge_colored_has_fill_outline_has_none() {
    let theme = Theme::default();
    let fill_of = |badge: Badge| {
        let mut badge = badge;
        LayoutEngine::new().compute(&mut badge, Size::new(200.0, 80.0));
        let scene = common::paint(&badge, &theme);
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Rect(r) => Some(r.fill),
                _ => None,
            })
            .unwrap()
    };
    assert!(
        fill_of(Badge::success("OK")).a > 0,
        "colored badge has a translucent fill"
    );
    assert_eq!(
        fill_of(Badge::outline("OK")).a,
        0,
        "outline badge has no fill"
    );
}

#[test]
fn badge_renders_its_label() {
    let theme = Theme::default();
    let mut badge = Badge::new("LIVE");
    LayoutEngine::new().compute(&mut badge, Size::new(200.0, 80.0));
    let scene = common::paint(&badge, &theme);
    assert!(
        scene
            .iter()
            .any(|c| matches!(c, DrawCommand::Text(t) if t.text == "LIVE")),
        "badge renders its label"
    );
}

#[test]
fn status_dot_color_and_glow_track_status() {
    let theme = Theme::default();
    let probe = |dot: StatusDot| {
        let mut dot = dot;
        LayoutEngine::new().compute(&mut dot, Size::new(50.0, 50.0));
        let scene = common::paint(&dot, &theme);
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Rect(r) => Some((r.fill, r.glow.is_some())),
                _ => None,
            })
            .unwrap()
    };
    let (online, online_glow) = probe(StatusDot::online());
    let (offline, offline_glow) = probe(StatusDot::offline());
    assert_eq!(online, theme.colors.success);
    assert!(online_glow, "active dot glows");
    assert_eq!(offline, theme.colors.muted);
    assert!(!offline_glow, "offline dot does not glow");
}

#[test]
fn horizontal_separator_spans_container_width() {
    let mut col = Flex::column()
        .width(Length::Px(120.0))
        .height(Length::Px(40.0))
        .child(Separator::horizontal());
    LayoutEngine::new().compute(&mut col, Size::new(120.0, 40.0));
    let sep = &col.base().children[0];
    assert_eq!(
        sep.base().bounds.size.w,
        120.0,
        "horizontal separator stretches to the container width"
    );
    assert!(sep.base().bounds.size.h <= 1.0, "separator is thin");
}

/// A separator's two properties do not depend on each other, in either order.
///
/// `length` used to write straight onto `width`, assuming the rule was horizontal. Set the
/// orientation afterwards — which a described separator does, since properties arrive sorted by
/// name and `length` sorts before `orientation` — and the length landed on the axis the rule runs
/// *across*, leaving the span unset. The widget now recomputes both axes from the pair, so this
/// passes whichever way round it is written.
#[test]
fn a_separators_length_and_orientation_can_be_set_in_either_order() {
    use heca_grid_ui::{PropInput, SetProp};

    for (first, second) in [("length", "orientation"), ("orientation", "length")] {
        let apply = |sep: Separator, key: &str| match key {
            "length" => sep.set_prop("length", &PropInput::Number(60.0)),
            _ => sep.set_prop("orientation", &PropInput::Text("vertical".into())),
        };
        let sep = apply(apply(Separator::horizontal(), first), second);

        let mut row = Flex::row()
            .width(Length::Px(200.0))
            .height(Length::Px(200.0))
            .child(sep);
        LayoutEngine::new().compute(&mut row, Size::new(200.0, 200.0));
        let bounds = row.base().children[0].base().bounds;

        assert_eq!(
            bounds.size.h, 60.0,
            "setting {first} then {second}: a vertical rule runs 60px down",
        );
        assert!(
            bounds.size.w <= 1.0,
            "setting {first} then {second}: a vertical rule stays thin ({}px wide)",
            bounds.size.w,
        );
    }

    // And `vertical()` still means what it always meant, without any property being set.
    let mut row = Flex::row()
        .width(Length::Px(200.0))
        .height(Length::Px(80.0))
        .child(Separator::vertical());
    LayoutEngine::new().compute(&mut row, Size::new(200.0, 80.0));
    let bounds = row.base().children[0].base().bounds;
    assert_eq!(
        bounds.size.h, 80.0,
        "a vertical rule stretches to the container height"
    );
    assert!(bounds.size.w <= 1.0, "and stays thin");
}

/// A `Panel`'s heading is a real composed child, and setting it works **whichever side of the
/// children it happens on** — which is what a description needs, since properties are applied after
/// children are attached.
///
/// Before F003/P017/T008 there was no `Panel` widget at all: `WidgetKind::Panel` realized to a bare
/// `Surface`, so the published examples showed `Panel::new().title("…")` against something with no
/// title, and no reader could tell.
#[test]
fn a_panel_titles_itself_whichever_order_it_is_built_in() {
    use heca_grid_ui::Panel;

    // Title first, then content.
    let a = Panel::new()
        .title("Containers")
        .child(Label::new("nginx"))
        .child(Label::new("redis"));
    // Content first, then title — the order `realize` uses.
    let b = Panel::new()
        .child(Label::new("nginx"))
        .child(Label::new("redis"))
        .title("Containers");

    for (which, panel) in [("title first", &a), ("children first", &b)] {
        let kids = &panel.base().children;
        assert_eq!(
            kids.len(),
            4,
            "{which}: header + rule + two content children"
        );
        assert!(
            !kids[0].base().style.layout.hidden && !kids[1].base().style.layout.hidden,
            "{which}: the header AND its rule show once titled",
        );
    }
    assert_eq!(a.title_signal().get_untracked(), "Containers");
    assert_eq!(b.title_signal().get_untracked(), "Containers");

    // An untitled panel keeps the header out of the layout rather than leaving a blank line.
    let plain = Panel::new().child(Label::new("body"));
    assert!(
        plain.base().children[0].base().style.layout.hidden
            && plain.base().children[1].base().style.layout.hidden,
        "no title means no heading and no bare rule across the top of the content",
    );

    // And a title can be cleared back to nothing.
    let cleared = Panel::titled("Gone").title("");
    assert!(
        cleared.base().children[0].base().style.layout.hidden
            && cleared.base().children[1].base().style.layout.hidden,
    );
}

/// A separator with no length spans its container **even when the container centres its children**.
///
/// Found by looking at it: the showcase row centres, like most rows do, so the rule was laid out
/// one pixel by zero and simply did not appear. The widget's answer used to be a line in its docs
/// telling the caller to pass a `length` — a workaround repeated at every call site for something
/// the rule can say once about itself, and one that silently produces nothing when forgotten.
#[test]
fn a_separator_spans_a_container_that_centres_its_children() {
    let mut row = Flex::row()
        .align(Align::Center)
        .width(Length::Px(200.0))
        .height(Length::Px(40.0))
        .child(Separator::vertical())
        .child(Separator::vertical().length(24.0));
    LayoutEngine::new().compute(&mut row, Size::new(200.0, 40.0));

    let stretched = row.base().children[0].base().bounds;
    assert_eq!(
        stretched.size.h, 40.0,
        "with no length, the rule spans the row despite Align::Center",
    );

    let cut = row.base().children[1].base().bounds;
    assert_eq!(cut.size.h, 24.0, "an explicit length still wins");
    assert!(
        cut.loc.y > stretched.loc.y,
        "…and the container's own alignment centres the shorter one",
    );

    // Same story the other way round: a column that centres still gets a full-width rule.
    let mut col = Flex::column()
        .align(Align::Center)
        .width(Length::Px(200.0))
        .height(Length::Px(40.0))
        .child(Separator::horizontal());
    LayoutEngine::new().compute(&mut col, Size::new(200.0, 40.0));
    assert_eq!(
        col.base().children[0].base().bounds.size.w,
        200.0,
        "a horizontal rule spans a centring column too",
    );
}

#[test]
fn spinner_animates_and_paints_its_ring() {
    let theme = Theme::default();
    let mut spinner = Spinner::new();
    LayoutEngine::new().compute(&mut spinner, Size::new(40.0, 40.0));
    assert!(spinner.tick(0.016), "spinner keeps requesting frames");

    let scene = common::paint(&spinner, &theme);
    let dots = common::rects(&scene).len();
    assert_eq!(dots, 8, "the ring paints 8 dots");
}

#[test]
fn alert_renders_title_body_and_accent_bar() {
    let theme = Theme::default();
    let mut alert = Alert::success("DEPLOYED").body("grid online");
    LayoutEngine::new().compute(&mut alert, Size::new(400.0, 100.0));

    let scene = common::paint(&alert, &theme);
    let texts: Vec<&str> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"DEPLOYED"), "alert renders its title");
    assert!(texts.contains(&"grid online"), "alert renders its body");

    let rects = common::rects(&scene).len();
    assert_eq!(rects, 2, "alert paints a surface + accent bar");
}

#[test]
fn progress_bar_fill_eases_toward_value() {
    let theme = Theme::default();
    let fill_w = |bar: &ProgressBar| {
        let scene = common::paint(bar, &theme);
        // Track is the first Rect; the fill (if any) is the second.
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Rect(r) => Some(r.rect.size.w),
                _ => None,
            })
            .nth(1)
    };

    let mut bar = ProgressBar::new().width(Length::Px(200.0));
    LayoutEngine::new().compute(&mut bar, Size::new(200.0, 20.0));
    assert_eq!(fill_w(&bar), None, "no fill at zero");

    bar.set(0.5);
    for _ in 0..40 {
        bar.tick(0.016);
    }
    let w = fill_w(&bar).expect("fill present after raising value");
    assert!(
        w > 90.0 && w < 110.0,
        "fill eases to ~half the 200px track, got {w}"
    );
}

#[test]
fn gauge_lights_segments_by_value() {
    let theme = Theme::default();
    let lit = |g: &Gauge| {
        let scene = common::paint(g, &theme);
        // Lit segments carry a glow; unlit do not.
        scene
            .iter()
            .filter(|c| matches!(c, DrawCommand::Rect(r) if r.glow.is_some()))
            .count()
    };

    let mut empty = Gauge::new();
    LayoutEngine::new().compute(&mut empty, Size::new(168.0, 18.0));
    let mut full = Gauge::new().value(1.0);
    LayoutEngine::new().compute(&mut full, Size::new(168.0, 18.0));

    assert_eq!(lit(&empty), 0, "empty gauge lights nothing");
    assert_eq!(lit(&full), 12, "full gauge lights all 12 segments");
}

#[test]
fn icon_lays_out_as_a_square() {
    use heca_grid_ui::{Glyph, Icon};
    let mut icon = Icon::new(Glyph::GitBranch).size(24.0);
    // Large == the reference (un-scaled) size; the explicit px is taken verbatim.
    // `set_size` (not a raw `style.size = ..`) because the variant must be marked *explicit*,
    // or the layout pass replaces it with the one inherited from the parent — here, the root
    // default. `Icon::size` is glyph pixels, so it can't be the variant builder.
    icon.base_mut().style.layout.set_size(WidgetSize::Large);
    LayoutEngine::new().compute(&mut icon, Size::new(200.0, 200.0));
    let b = icon.base().bounds;
    assert_eq!(b.size.w, 24.0, "icon width = glyph size");
    assert_eq!(b.size.h, 24.0, "icon is square");

    // The size variant scales an explicit glyph size too (so icon-only buttons
    // resize): Small renders the same icon smaller.
    let mut small = Icon::new(Glyph::GitBranch).size(24.0);
    small.base_mut().style.layout.set_size(WidgetSize::Small);
    LayoutEngine::new().compute(&mut small, Size::new(200.0, 200.0));
    assert!(
        small.base().bounds.size.w < 24.0,
        "Small scales the explicit glyph size down"
    );
}

#[test]
fn icon_paints_duotone_layers_in_the_icon_font() {
    use heca_grid_ui::{DrawCommand, FontRole, Glyph, Icon};
    let mut icon = Icon::new(Glyph::Folder).size(24.0);
    LayoutEngine::new().compute(&mut icon, Size::new(100.0, 100.0));

    let theme = Theme::default();
    let scene = common::paint(&icon, &theme);
    let glyphs: Vec<(String, FontRole)> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some((t.text.clone(), t.font)),
            _ => None,
        })
        .collect();

    // Two stacked layers: secondary (:before) then primary (secondary+1), both
    // shaped with the icon font.
    assert_eq!(glyphs.len(), 2, "duotone icon paints two layers");
    assert!(
        glyphs.iter().all(|(_, f)| *f == FontRole::Icon),
        "both shaped with the icon font"
    );
    assert_eq!(
        glyphs[0].0,
        char::from_u32(0xe24a).unwrap().to_string(),
        "secondary layer first"
    );
    assert_eq!(
        glyphs[1].0,
        char::from_u32(0xe24b).unwrap().to_string(),
        "primary layer on top"
    );
}

#[test]
fn tag_lays_out_leading_and_label_and_hugs_content() {
    use heca_grid_ui::{Glyph, Icon, Tag};
    let mut tag = Tag::new("main 1+").leading(Icon::new(Glyph::GitBranch).size(13.0));
    LayoutEngine::new().compute(&mut tag, Size::new(300.0, 40.0));

    // The first segment holds [leading, label].
    let seg0 = tag.base().children[0].base();
    assert_eq!(
        seg0.children.len(),
        2,
        "first segment holds [leading, label]"
    );
    assert!(
        seg0.children[0].base().bounds.size.w > 0.0,
        "leading icon is laid out"
    );
    assert!(
        seg0.children[1].base().bounds.size.w > 0.0,
        "label is laid out"
    );
    assert!(
        tag.base().bounds.size.w < 300.0,
        "chip hugs its content, not the full width"
    );
}

#[test]
fn tag_with_multiple_segments_lays_them_in_a_row() {
    use heca_grid_ui::{Component, Glyph, Icon, Tag};
    let leading: Option<Box<dyn Component>> = Some(Box::new(Icon::new(Glyph::File).size(13.0)));
    let mut tag = Tag::new("main")
        .leading(Icon::new(Glyph::GitBranch).size(13.0))
        .segment_text("5 +152 -12", leading);
    LayoutEngine::new().compute(&mut tag, Size::new(400.0, 40.0));

    assert_eq!(tag.base().children.len(), 2, "two segments");
    let s0 = tag.base().children[0].base().bounds;
    let s1 = tag.base().children[1].base().bounds;
    assert!(
        s1.loc.x > s0.loc.x + s0.size.w - 1.0,
        "the second segment sits right of the first"
    );
}
