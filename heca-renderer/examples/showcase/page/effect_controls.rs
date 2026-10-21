use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Box<dyn Component> {
    let theme = cx.theme;
    let ctl = cx.ctl;
    let intensity_idx = cx.intensity_idx;
    let overlay_frame_idx = cx.overlay_frame_idx;


    Box::new(Flex::row()
                .gap(16.0)
                .align("center")
                .child(Label::new("INTENSITY").color(theme.colors.muted).font_scale(0.85))
                .child(
                    Select::new(["OFF", "LOW", "MEDIUM", "HEAVY"])
                        .selected(intensity_idx)
                        .on_change(move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.intensity.set(INTENSITY_OPTS[i.min(3)]);
                            }
                        }),
                )
                // The frame every OVERLAY panel draws (this Select's own dropdown
                // included, so the change is visible the moment you reopen it).
                // Mirrors `[appearance] overlay_border_style` in the real app.
                .child(Label::new("OVERLAY FRAME").color(theme.colors.muted).font_scale(0.85))
                .child(
                    Select::new(["BRACKETED", "BORDERED", "NONE"])
                        .selected(overlay_frame_idx)
                        .on_change(move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.overlay_frame.set(
                                    OVERLAY_FRAME_OPTS[i.min(OVERLAY_FRAME_OPTS.len() - 1)],
                                );
                            }
                        }),
                ),)
}
