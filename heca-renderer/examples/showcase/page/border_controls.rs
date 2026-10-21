use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Box<dyn Component> {
    let theme = cx.theme;
    let ctl = cx.ctl;
    let radius_opts = cx.radius_opts;
    let border_opts = cx.border_opts;
    let glow_idx = cx.glow_idx;
    let radius_idx = cx.radius_idx;
    let border_idx = cx.border_idx;


    Box::new(Flex::row()
                .gap(16.0)
                .align("center")
                .child(Label::new("GLOW").color(theme.colors.muted).font_scale(0.85))
                .child(
                    Select::new(GlowLevel::ALL.map(|g| g.label()))
                        .selected(glow_idx)
                        .on_change(move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.glow
                                    .set(GlowLevel::ALL[i.min(GlowLevel::ALL.len() - 1)]);
                            }
                        }),
                )
                .child(Label::new("RADIUS").color(theme.colors.muted).font_scale(0.85))
                .child(
                    Select::new(["0", "4", "8", "16"])
                        .selected(radius_idx)
                        .on_change(move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.radius.set(radius_opts[i.min(3)]);
                            }
                        }),
                )
                .child(Label::new("BORDER").color(theme.colors.muted).font_scale(0.85))
                .child(
                    Select::new(["0", "1", "2", "3"])
                        .selected(border_idx)
                        .on_change(move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.border.set(border_opts[i.min(3)]);
                            }
                        }),
                ),)
}
