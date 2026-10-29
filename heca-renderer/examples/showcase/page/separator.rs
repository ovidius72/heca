use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Flex {
    let theme = cx.theme;


    Flex::row()
        .gap(12.0)
        .align("center")
        .height(40.0)
        .child(Label::new("full height").color(theme.colors.muted))
        .child(Separator::vertical())
        .child(Label::new("length(24)").color(theme.colors.muted))
        .child(Separator::vertical().length(24.0))
        .child(Label::new("end").color(theme.colors.muted))
}
