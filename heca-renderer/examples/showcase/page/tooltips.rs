use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Box<dyn Component> {
    let theme = cx.theme;


    Box::new(Flex::row()
                .gap(8.0)
                .align("center")
                // **The tooltip is a property of the button**, not a box around it — one builder,
                // on every widget. This row used to wrap each button in a `Tooltip`, which is the
                // noise the property removed.
                .child(
                    IconButton::new(Icon::new(Glyph::Search).color(theme.colors.foreground).size(20.0))
                        .on_click(|| println!("[showcase] search"))
                        .tooltip("Search"),
                )
                .child(
                    IconButton::new(Icon::new(Glyph::Gear).color(theme.colors.foreground).size(20.0))
                        .on_click(|| println!("[showcase] settings"))
                        .tooltip("Settings"),
                )
                .child(
                    IconButton::new(Icon::new(Glyph::Plus).color(theme.colors.foreground).size(20.0))
                        .on_click(|| println!("[showcase] add"))
                        .tooltip("New pane"),
                )
                // `.active(true)`: held-on (toggled) status — a persistent tone-tinted
                // frame, like the in-pane zoom/float buttons when engaged.
                .child(Tooltip::new(
                    IconButton::new(
                        Icon::new(Glyph::FrameCorners)
                            .color(theme.colors.foreground)
                            .size(20.0),
                    )
                    .active(true)
                    .on_click(|| println!("[showcase] unzoom")),
                    "Zoom (active)",
                ))
                // `WidgetSize::Header`: the emphasized variant for pane/info-bar action
                // buttons — the glyph out-sizes the body text and the button self-sizes
                // from the variant (no explicit icon px), so the whole cluster scales
                // with the bar font. This is exactly what the in-pane header buttons use.
                .child(Tooltip::new(
                    IconButton::new(Icon::new(Glyph::Plus).color(theme.colors.foreground))
                        .size(WidgetSize::Header)
                        .on_click(|| println!("[showcase] header add")),
                    "Header size",
                ))
                .child(
                    Tooltip::new(
                        IconButton::new(Icon::new(Glyph::Close).color(theme.colors.danger).size(20.0))
                            .accent(theme.colors.danger)
                            .on_click(|| println!("[showcase] close")),
                        "Close",
                    )
                    .side(TooltipSide::Bottom),
                ),)
}
