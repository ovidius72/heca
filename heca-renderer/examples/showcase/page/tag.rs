use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Box<dyn Component> {
    let theme = cx.theme;


    Box::new(Flex::row()
                .gap(12.0)
                .align("center")
                // Segmented status-bar chip: path · branch · diff-stat (colored).
                .child(
                    Tag::new("~/repos/do-things")
                        .leading(Icon::new(Glyph::Folder).size(13.0).color(theme.colors.muted))
                        .segment(
                            Flex::row()
                                .align("center")
                                .gap(6.0)
                                .child(Icon::new(Glyph::GitBranch).size(13.0).color(theme.colors.muted))
                                .child(Label::new("main").color(theme.colors.foreground).font_scale(0.8)),
                        )
                        .segment(
                            Flex::row()
                                .align("center")
                                .gap(6.0)
                                .child(Icon::new(Glyph::File).size(13.0).color(theme.colors.muted))
                                .child(Label::new("5").color(theme.colors.foreground).font_scale(0.8))
                                .child(Label::new("+152").color(theme.colors.success).font_scale(0.8))
                                .child(Label::new("-12").color(theme.colors.danger).font_scale(0.8)),
                        ),
                )
                .child(
                    Tag::new("feature/grid-ui")
                        .leading(Icon::new(Glyph::GitBranch).size(13.0).color(theme.colors.accent))
                        .color(theme.colors.accent),
                )
                .child(Tag::new("rust").color(theme.colors.success)),)
}
