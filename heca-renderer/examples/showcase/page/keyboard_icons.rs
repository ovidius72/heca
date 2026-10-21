use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Box<dyn Component> {
    let theme = cx.theme;


    Box::new({
            const PER_ROW: usize = 7;
            let cell = |glyph: NfGlyph| {
                Flex::column()
                    .gap(6.0)
                    .align("center")
                    .width(124.0)
                    .child(NfIcon::new(glyph).color(theme.colors.foreground).size(26.0))
                    .child(
                        Label::new(format!("{}  U+{:05X}", glyph.name(), glyph.codepoint()))
                            .font_scale(0.58)
                            .color(theme.colors.muted),
                    )
            };
            let mut grid = Flex::column().gap(18.0);
            let mut row = Flex::row().gap(8.0).align("start");
            for (i, &glyph) in NfGlyph::ALL.iter().enumerate() {
                if i > 0 && i % PER_ROW == 0 {
                    grid = grid.child(row);
                    row = Flex::row().gap(8.0).align("start");
                }
                row = row.child(cell(glyph));
            }
            grid.child(row)
        })
}
