use super::*;

pub(super) fn build(_cx: &DemoCx<'_>) -> Box<dyn Component> {


    Box::new(Flex::column()
                .gap(8.0)
                .child({
                    let bar = ScrollBar::new().height(180.0);
                    bar.content_extent_signal().set(240.0);
                    bar.viewport_extent_signal().set(48.0);
                    bar.offset_signal().set(96.0);
                    Flex::row()
                        .align("center")
                        .gap(14.0)
                        .child(bar)
                        .child(BadgeButton::accent("144 lines above"))
                }),)
}
