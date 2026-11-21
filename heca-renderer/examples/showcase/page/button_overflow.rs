use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Surface {
    let theme = cx.theme;


    Surface::new()
        .background(theme.colors.surface)
        .padding(Spacing::Xs)
        .width(Length::HALF)
        .child(
            ButtonGroup::new()
                .size(WidgetSize::Small)
                .gap(Spacing::Xs)
                .child(
                    Button::new("Split")
                        .icon(Glyph::Plus)
                        .on_click(|_| println!("[showcase] split")),
                )
                .child(
                    Button::new("Zoom")
                        .icon(Glyph::FrameCorners)
                        .on_click(|_| println!("[showcase] zoom")),
                )
                .child(
                    Button::new("Float")
                        .icon(Glyph::Sidebar)
                        .on_click(|_| println!("[showcase] float")),
                )
                .child(
                    Button::new("Close")
                        .icon(Glyph::Minus)
                        .on_click(|_| println!("[showcase] close")),
                ),
        )
}
