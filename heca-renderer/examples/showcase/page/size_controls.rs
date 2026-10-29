use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Flex {
    let theme = cx.theme;
    let ctl = cx.ctl;
    let size_idx = cx.size_idx;
    let zoom_idx = cx.zoom_idx;


    Flex::row()
        .gap(16.0)
        .align("center")
        .child(Label::new("SIZE").color(theme.colors.muted).font_scale(0.85))
        .child(
            Select::new(["NORMAL", "SMALL", "LARGE", "HEADER"])
                .selected(size_idx)
                .on_change(move |a| {
                    if let SignalData::Usize(i) = a.data {
                        ctl.size.set(SIZE_OPTS[i.min(SIZE_OPTS.len() - 1)]);
                    }
                }),
        )
        .child(Label::new("ZOOM").color(theme.colors.muted).font_scale(0.85))
        .child(
            Select::new(["0", "1", "2", "3", "4", "5"])
                .selected(zoom_idx)
                .on_change(move |a| {
                    if let SignalData::Usize(i) = a.data {
                        ctl.zoom.set((i as f32).min(ZOOM_MAX));
                    }
                }),
        )
}
