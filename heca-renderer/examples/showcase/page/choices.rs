use super::*;

pub(super) fn build(_cx: &DemoCx<'_>) -> Flex {

    let click = |label: &str| {
        let name = label.to_string();
        move || println!("[showcase] {name} clicked")
    };
    Flex::row()
        .gap(8.0)
        .align("center")
        .child(Choice::labeled("low", "LOW").on_activate(click("LOW")))
        .child(
            Choice::new("medium")
                .child(
                    Flex::row()
                        .gap(6.0)
                        .align("center")
                        .child(Icon::new(Glyph::Warning))
                        .child(Label::new("MEDIUM")),
                )
                .selected(true)
                .on_activate(click("MEDIUM")),
        )
        // Arbitrary content: two lines, a badge — still one option, one click target.
        .child(
            Choice::new("high")
                .child(
                    Flex::column()
                        .gap(2.0)
                        .child(
                            Flex::row()
                                .gap(6.0)
                                .align("center")
                                .child(Icon::new(Glyph::Lightning))
                                .child(Label::new("HIGH")),
                        )
                        .child(Label::new("uses more power").font_scale(0.75)),
                )
                .on_activate(click("HIGH")),
        )
}
