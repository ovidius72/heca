use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Flex {
    let theme = cx.theme;


    Flex::row()
        .gap(28.0)
        .align("center")
        .child(
            Flex::column()
                .gap(6.0)
                .align("center")
                .child(Icon::new(Glyph::Lightning).color(theme.colors.accent).size(34.0))
                .child(Label::new("FLAT").font_scale(0.62).color(theme.colors.muted)),
        )
        .child(
            Flex::column()
                .gap(6.0)
                .align("center")
                .child(
                    Icon::new(Glyph::Lightning)
                        .color(theme.colors.accent)
                        .size(34.0)
                        .glow(true),
                )
                .child(Label::new("GLOW").font_scale(0.62).color(theme.colors.muted)),
        )
        .child(
            Flex::column()
                .gap(6.0)
                .align("center")
                .child(
                    Icon::new(Glyph::Terminal)
                        .color(theme.colors.foreground)
                        .size(34.0)
                        .glow(true),
                )
                .child(Label::new("GLOW").font_scale(0.62).color(theme.colors.muted)),
        )
}
