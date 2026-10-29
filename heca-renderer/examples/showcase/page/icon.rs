use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Flex {
    let theme = cx.theme;


    const PER_ROW: usize = 8;
    // One labelled cell per glyph: the icon over its `Glyph` enum name (via Debug,
    // so no extra method). Fixed-width cells make the rows line up as a grid.
    let cell = |glyph: Glyph| {
        Flex::column()
            .gap(6.0)
            .align("center")
            .width(124.0)
            .child(Icon::new(glyph).color(theme.colors.foreground).size(26.0))
            .child(
                Label::new(format!("{glyph:?}"))
                    .font_scale(0.62)
                    .color(theme.colors.muted),
            )
    };
    let mut grid = Flex::column().gap(18.0);
    let mut row = Flex::row().gap(8.0).align("start");
    for (i, &glyph) in Glyph::ALL.iter().enumerate() {
        if i > 0 && i % PER_ROW == 0 {
            grid = grid.child(row);
            row = Flex::row().gap(8.0).align("start");
        }
        row = row.child(cell(glyph));
    }
    grid.child(row)
}
