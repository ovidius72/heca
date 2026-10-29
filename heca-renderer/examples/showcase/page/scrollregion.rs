use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Flex {
    let theme = cx.theme;


    Flex::column()
        .gap(8.0)
        .child({
            let mut list = ScrollRegion::new()
                .height(180.0)
                .width(300.0);
            for i in 1..=25 {
                list = list.child(
                    Item::new(format!("item {i:02}"))
                        .leading(Icon::new(Glyph::FileCode).color(theme.colors.accent).size(16.0)),
                );
            }
            list
        })
}
