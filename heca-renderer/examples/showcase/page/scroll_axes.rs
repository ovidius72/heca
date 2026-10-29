use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> ScrollRegion {
    let theme = cx.theme;


    let mut grid = ScrollRegion::new()
        .both()
        .width(240.0)
        .height(140.0)
        .background(theme.colors.surface)
        .border(theme.colors.accent, 1.0);
    for r in 1..=14 {
        grid = grid.child(
            Flex::row()
                .gap(6.0)
                .width(440.0) // wider than the viewport → overflow
                .child(
                    Icon::new(Glyph::FileCode)
                        .color(theme.colors.accent)
                        .size(14.0),
                )
                .child(Label::new(format!(
                    "row {r:02} — a wide line that overflows the viewport horizontally"
                ))),
        );
    }
    grid
}
