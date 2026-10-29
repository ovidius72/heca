use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Flex {
    let theme = cx.theme;


    Flex::row()
        .gap(16.0)
        .align("start")
        .child(
            Panel::titled("Containers")
                .width(220.0)
                .background(theme.colors.foreground.with_alpha(6))
                .radius(theme.colors.control_radius())
                .child(Label::new("nginx").color(theme.colors.muted))
                .child(Label::new("postgres").color(theme.colors.muted)),
        )
        .child(
            Panel::new()
                .width(220.0)
                .background(theme.colors.foreground.with_alpha(6))
                .radius(theme.colors.control_radius())
                .child(Label::new("untitled — no header row").color(theme.colors.muted)),
        )
}
