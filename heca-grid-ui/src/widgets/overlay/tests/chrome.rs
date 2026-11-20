use super::*;
use crate::component::PaintCx;
use crate::scene::Glow;
use crate::theme::Theme;

/// Every rect the open overlay draws, in its own overlay segment.
fn rects(o: &Overlay, theme: &Theme) -> Vec<crate::scene::RectCmd> {
    let mut scene = Scene::new();
    o.paint(&mut PaintCx::new(&mut scene, theme).with_viewport(Size::new(800.0, 600.0)));
    scene
        .overlay_segments()
        .flat_map(|segment| segment.iter().cloned().collect::<Vec<_>>())
        .filter_map(|c| match c {
            DrawCommand::Rect(r) => Some(r),
            _ => None,
        })
        .collect()
}

#[test]
fn an_overlay_wears_the_plain_chrome_until_its_owner_chooses_another() {
    let theme = Theme::default();
    let plain = open_overlay();
    assert!(
        rects(&plain, &theme).iter().all(|r| r.glow.is_none()),
        "no halo by default"
    );
}

#[test]
fn the_owner_chooses_the_panel_chrome_from_the_theme_in_force() {
    let theme = Theme::default();
    let mut o = Overlay::new()
        .panel(Flex::column().child(Label::new("hi")))
        .chrome(PanelChrome::accented)
        .default_open(true);
    crate::layout::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));
    let halo: Vec<Glow> = rects(&o, &theme).iter().filter_map(|r| r.glow).collect();
    assert_eq!(
        halo.len(),
        1,
        "the panel carries the accented halo: {halo:?}"
    );
    assert_eq!(halo[0].color, theme.colors.glow);

    // A different theme answers differently: nothing was captured at build time.
    let mut other = Theme::default();
    other.colors.glow = crate::Color::rgb(1, 2, 3);
    let halo: Vec<Glow> = rects(&o, &other).iter().filter_map(|r| r.glow).collect();
    assert_eq!(halo[0].color, other.colors.glow);
}

#[test]
fn the_accented_chrome_is_the_themes_panel_edge_in_the_accent() {
    let theme = Theme::default();
    let mut scene = Scene::new();
    let cx = PaintCx::new(&mut scene, &theme);
    let chrome = PanelChrome::accented(&cx);
    let edge = chrome.border.expect("an edge");
    assert_eq!(
        edge.color,
        cx.accent()
            .with_alpha(theme.colors.interaction.panel_border)
    );
    assert!(chrome.glow.is_some());
}
