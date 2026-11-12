use super::*;
use crate::{DrawCommand, LayoutEngine, Parent, Scene, Size, Theme, paint_child};

fn scene(widget: &dyn Component, theme: &Theme) -> Vec<DrawCommand> {
    let mut scene = Scene::new();
    paint_child(widget, &mut PaintCx::new(&mut scene, theme));
    scene.iter().cloned().collect()
}

#[test]
fn the_default_caption_matches_the_approved_label_and_rule_composition() {
    let mut caption = Caption::new("Build");
    let mut previous = Flex::row().width("100%").size(WidgetSize::Small)
        .align("center").gap(Spacing::Xs).padding_x(Spacing::Xs)
        .child(Label::new("Build").muted(true))
        .child(Separator::horizontal().align_self("center").grow(1.0));
    let mut layout = LayoutEngine::new();
    layout.compute(&mut caption, Size::new(300.0, 100.0));
    layout.compute(&mut previous, Size::new(300.0, 100.0));
    let theme = Theme::default();
    assert_eq!(scene(&caption, &theme), scene(&previous, &theme));
}

#[test]
fn blank_text_removes_padding_rule_and_parent_gap_without_caller_visibility() {
    let text = signal("Build".to_owned());
    let mut root = Flex::column().gap(Spacing::Sm)
        .child(Caption::new("").with_text_signal(text).padding(Spacing::Lg))
        .child(Label::new("Body"));
    let mut layout = LayoutEngine::new();
    for value in ["Build", "", "  \n ", "Logs"] {
        text.set(value.to_owned());
        root.tick(0.0);
        layout.compute(&mut root, Size::new(300.0, 200.0));
        let body = root.base().children[1].base().bounds;
        if value.trim().is_empty() {
            assert_eq!(body.loc.y, 0.0);
            assert!(scene(root.base().children[0].as_ref(), &Theme::default()).is_empty());
        } else { assert!(body.loc.y > 0.0); }
    }
}

#[test]
fn text_updates_keep_the_same_label_and_rule() {
    let mut caption = Caption::new("Build");
    let text = caption.text_signal();
    let nodes: Vec<_> = caption.base().children.iter().map(|child| child.base().focus_id()).collect();
    for value in ["Logs", "", "Tests"] {
        text.set(value.to_owned());
        caption.tick(0.0);
        LayoutEngine::new().compute(&mut caption, Size::new(300.0, 100.0));
        assert_eq!(caption.base().children.iter().map(|child| child.base().focus_id()).collect::<Vec<_>>(), nodes);
        assert_eq!(caption.text_summary().as_deref(), (!value.is_empty()).then_some(value));
    }
}

#[test]
fn constructor_and_signal_builders_preserve_common_properties_in_either_order() {
    let text = signal("Release".to_owned());
    let mut first = Caption::new("Build").size(WidgetSize::Normal).gap("sm").padding_x("md").with_text_signal(text);
    let mut last = Caption::new("Build").with_text_signal(text).size(WidgetSize::Normal).gap("sm").padding_x("md");
    let mut layout = LayoutEngine::new();
    layout.compute(&mut first, Size::new(400.0, 100.0));
    layout.compute(&mut last, Size::new(400.0, 100.0));
    assert_eq!(scene(&first, &Theme::default()), scene(&last, &Theme::default()));
    let renamed = first.text("Deploy");
    assert_eq!(text.get_untracked(), "Deploy");
    assert_eq!(renamed.text_summary().as_deref(), Some("Deploy"));
}

#[test]
fn live_text_never_overrides_an_authors_hidden_setting() {
    use crate::PresenceExt;
    let mut caption = Caption::new("").hidden(true);
    for value in ["Build", "", "Logs"] {
        caption.text_signal().set(value.to_owned());
        caption.tick(0.0);
        LayoutEngine::new().compute(&mut caption, Size::new(300.0, 100.0));
        assert!(!caption.is_shown());
        assert!(scene(&caption, &Theme::default()).is_empty());
    }
}

#[test]
fn revealing_a_collapsed_caption_waits_for_its_new_layout() {
    let mut caption = Caption::new("");
    LayoutEngine::new().compute(&mut caption, Size::new(300.0, 100.0));
    caption.text_signal().set("Build".to_owned());
    caption.tick(0.0);
    assert!(scene(&caption, &Theme::default()).is_empty());
    LayoutEngine::new().compute(&mut caption, Size::new(300.0, 100.0));
    assert!(!scene(&caption, &Theme::default()).is_empty());
}

#[test]
fn theme_changes_and_content_tone_are_read_live() {
    let mut caption = Caption::new("Build");
    LayoutEngine::new().compute(&mut caption, Size::new(300.0, 100.0));
    let mut theme = Theme::default();
    theme.colors.muted = theme.colors.warning;
    theme.colors.border = theme.colors.success;
    for command in scene(&caption, &theme) {
        match command {
            DrawCommand::Text(text) => assert_eq!(text.color, theme.colors.muted),
            DrawCommand::Rect(rule) => assert_eq!(rule.fill, theme.colors.border),
            _ => panic!("a caption contains only text and its rule"),
        }
    }
    let caption = caption.content_tone(Tone::Danger);
    assert!(scene(&caption, &theme).iter().any(|command| matches!(command,
        DrawCommand::Text(text) if text.color == theme.colors.danger)));
}

#[test]
fn text_and_rule_fit_narrow_layouts_at_different_font_sizes() {
    for font in [12.0, 20.0, 30.0] {
        for width in [90.0, 240.0, 560.0] {
            let mut caption = Caption::new("A long deployment caption that must fit");
            LayoutEngine::new().base_font(font).compute(&mut caption, Size::new(width, 100.0));
            let theme = Theme { font_size: font, ..Default::default() };
            for command in scene(&caption, &theme) {
                let rect = match command {
                    DrawCommand::Text(text) => {
                        assert_eq!(text.color, theme.colors.muted);
                        assert_eq!(text.size, font * WidgetSize::Small.font_scale());
                        text.rect
                    }
                    DrawCommand::Rect(rule) => rule.rect,
                    _ => panic!("a caption must not add a field or another frame"),
                };
                assert!(rect.loc.x >= 0.0 && rect.loc.x + rect.size.w <= width + 0.01, "{rect:?}");
            }
        }
    }
}
