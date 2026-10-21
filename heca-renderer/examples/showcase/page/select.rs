use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Box<dyn Component> {
    let theme = cx.theme;
    let workspaces = cx.workspaces.clone();

    let report = |a: Action| println!("[showcase] {} -> {:?}", a.name, a.data);
    Box::new(Flex::row()
                .gap(16.0)
                .align("center")
                .child(Label::new("MODE").color(theme.colors.muted).font_scale(0.85))
                .child(Select::new(["NORMAL", "PREFIX", "PASSTHROUGH"]).on_change(report))
                .child(Label::new("WORKSPACE").color(theme.colors.muted).font_scale(0.85))
                .child(Select::new(workspaces).selected(3).on_change(report))
                // Composed options: an option is a value plus any content — here an icon and a
                // label, which tint together as the row is chosen.
                .child(Label::new("LEVEL").color(theme.colors.muted).font_scale(0.85))
                .child(
                    Select::empty()
                        .option(level_option("low", Glyph::Circle, "LOW"))
                        .option(level_option("medium", Glyph::Warning, "MEDIUM"))
                        .option(level_option("high", Glyph::Lightning, "HIGH"))
                        .selected(1)
                        .on_change(report),
                ),)
}
