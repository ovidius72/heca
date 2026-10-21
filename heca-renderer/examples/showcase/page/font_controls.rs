use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Box<dyn Component> {
    let theme = cx.theme;
    let ctl = cx.ctl;
    let font_opts = cx.font_opts;
    let font_idx = cx.font_idx;


    Box::new(Flex::row()
                .gap(16.0)
                .align("center")
                .child(Label::new("FONT").color(theme.colors.muted).font_scale(0.85))
                .child(
                    Select::new(["8", "10", "12", "15", "20", "28"])
                        .selected(font_idx)
                        .on_change(move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.font.set(font_opts[i.min(font_opts.len() - 1)]);
                            }
                        }),
                )
                .child(Input::new().value("SIZED"))
                .child(Select::new(["ALPHA", "BETA", "GAMMA"]))
                .child(Label::new("Aa").color(theme.colors.foreground)),)
}
