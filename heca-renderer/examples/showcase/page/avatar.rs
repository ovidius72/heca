use super::*;

/// Avatars: a plain one, one with a fill of its own, one with a ring, and one with both — the shape
/// a member row uses (what it is on the fill, how it is doing on the ring). At the caption size and
/// the normal one.
pub(super) fn build(_cx: &DemoCx<'_>) -> Flex {
    let row = |size: WidgetSize| {
        Flex::row()
            .gap(12.0)
            .align("center")
            .child(Avatar::new("ada").size(size))
            .child(Avatar::new("pi").tone(Tone::Success).size(size))
            .child(Avatar::new("bob").ring(Tone::Warning).size(size))
            .child(
                Avatar::new("dee")
                    .tone(Tone::Muted)
                    .ring(Tone::Accent)
                    .size(size),
            )
    };
    Flex::column()
        .gap(8.0)
        .child(row(WidgetSize::Caption))
        .child(row(WidgetSize::Normal))
}
