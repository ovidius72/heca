use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Box<dyn Component> {
    let theme = cx.theme;


    Box::new(Surface::new()
                .background(theme.colors.surface)
                .padding(Spacing::Xs)
                .child(
                    ButtonGroup::new()
                        .size(WidgetSize::Small)
                        .display(heca_grid_ui::widgets::Display::IconOnly)
                        .gap(Spacing::Xs)
                        .child(
                            Button::new("Split")
                                .icon(Glyph::Plus)
                                .on_click(|| println!("[showcase] split")),
                        )
                        .child(
                            Button::new("Zoom")
                                .icon(Glyph::FrameCorners)
                                .on_click(|| println!("[showcase] zoom")),
                        )
                        .child(
                            Button::new("Close")
                                .icon(Glyph::Minus)
                                .on_click(|| println!("[showcase] close")),
                        ),
                ),)
}
