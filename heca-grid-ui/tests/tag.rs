mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, Ellipsis, Glyph, Icon, LayoutEngine, Size, Tag, Theme};

#[test]
fn shrinking_segments_never_restore_their_full_text_at_zero_width() {
    let mut row = Flex::row()
        .width("100%")
        .child(
            Tag::from_label(Label::new("/projects/a/very/long/path").truncate(Ellipsis::Start))
                .leading(Icon::new(Glyph::Folder))
                .segment_label_with(Label::new("zsh"), Icon::new(Glyph::Terminal))
                .segment_shrink(false)
                .segment_label_with(
                    Label::new("feature/a-very-long-branch"),
                    Icon::new(Glyph::GitBranch),
                ),
        )
        .child(Icon::new(Glyph::DotsThreeVertical));
    let theme = Theme::default();
    let mut layout = LayoutEngine::new();
    for width in (20..=500).rev() {
        layout.compute(&mut row, Size::new(f64::from(width), 60.0));
        let scene = common::paint_via_child(&row, &theme);
        for command in scene.iter() {
            if let DrawCommand::Text(text) = command {
                if text.font != heca_grid_ui::scene::FontRole::Text {
                    continue;
                }
                let needed = text.text.chars().count() as f64
                    * f64::from(text.size)
                    * f64::from(heca_grid_ui::font::MONO_ADVANCE_RATIO);
                assert!(
                    needed <= text.rect.size.w + 0.5,
                    "width {width}: {:?} needs {needed} in {:?}",
                    text.text,
                    text.rect
                );
            }
        }
    }
}

#[test]
fn segment_paint_is_confined_to_its_box_and_the_chip_without_clipping_tooltips() {
    let mut tag = Tag::new("short").segment_shrink(false).segment_label_with(
        Label::new("a long branch name"),
        Icon::new(Glyph::GitBranch),
    );
    let mut layout = LayoutEngine::new();
    let theme = Theme::default();
    for width in [300.0, 100.0, 40.0, 10.0, 300.0] {
        tag.base_mut().style.layout.width = Length::Px(width);
        layout.compute(&mut tag, Size::new(f64::from(width), 60.0));
        let scene = common::paint_via_child(&tag, &theme);
        let mut clips = Vec::new();
        for command in scene.iter() {
            match command {
                DrawCommand::PushClip(rect) => clips.push(*rect),
                DrawCommand::PopClip => {
                    clips.pop();
                }
                DrawCommand::Text(text) if !text.text.is_empty() => {
                    assert!(
                        clips.contains(&tag.base().bounds),
                        "the chip must contain its paint"
                    );
                    assert!(
                        tag.base()
                            .children
                            .iter()
                            .any(|segment| clips.contains(&segment.base().bounds)),
                        "each segment must contain its own paint, including leading icons"
                    );
                }
                _ => {}
            }
        }
        assert!(clips.is_empty());
        // What the branch segment offers on hover: its whole text while the chip cuts it, nothing
        // once it fits again.
        let tip = tip_text(tag.base().children[1].as_ref());
        match width {
            300.0 => assert_eq!(tip, None, "at full width nothing is cut, so nothing to add"),
            100.0 => assert_eq!(tip.as_deref(), Some("a long branch name"), "partly shown, it offers its whole text"),
            _ => {}
        }
    }
}

/// The tooltip text found on a widget or the first of its descendants that has one.
fn tip_text(w: &dyn heca_grid_ui::Component) -> Option<String> {
    w.base().tooltip.as_ref().map(|t| t.text.get_untracked())
        .or_else(|| w.base().children.iter().find_map(|c| tip_text(c.as_ref())))
}

#[test]
fn a_truncated_segments_tooltip_can_extend_beyond_the_chip() {
    let full = "/projects/a/very/long/path";
    heca_grid_ui::widgets::tooltip::set_default_delay(0.0);
    let mut tag = Tag::new(full).width(100.0);
    LayoutEngine::new().compute(&mut tag, Size::new(500.0, 200.0));
    let bounds = tag.base().children[0].base().bounds;
    heca_grid_ui::dispatch(
        &mut tag,
        &heca_grid_ui::Event::pointer_moved(heca_grid_ui::Point::new(
            bounds.loc.x + bounds.size.w / 2.0,
            bounds.loc.y + bounds.size.h / 2.0,
        )),
    );
    let scene = common::paint_via_child(&tag, &Theme::default());
    assert!(scene.iter().any(|command| matches!(command,
        DrawCommand::Text(text) if text.text == full && text.rect.size.w > tag.base().bounds.size.w)),
        "the full hover value must not be squeezed into the truncated segment");
}
