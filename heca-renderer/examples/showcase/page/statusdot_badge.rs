use super::*;

pub(super) fn build(_cx: &DemoCx<'_>) -> Box<dyn Component> {


    Box::new(Flex::row()
                .gap(12.0)
                .align("center")
                .child(StatusDot::online())
                .child(Badge::success("ONLINE"))
                .child(Badge::warning("DEGRADED"))
                .child(Badge::danger("OFFLINE"))
                .child(Badge::accent("v2.0"))
                .child(Badge::outline("BETA")),)
}
