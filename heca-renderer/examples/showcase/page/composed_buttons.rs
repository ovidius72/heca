use super::*;

pub(super) fn build(_cx: &DemoCx<'_>) -> Flex {

    let click = |label: &str| {
        let name = label.to_string();
        move || println!("[showcase] {name} clicked")
    };
    Flex::row()
        .gap(14.0)
        .align("center")
        // Sugar: `.icon(..)` prepends an Icon child → [Icon, Label].
        .child(Button::primary("SAVE").icon(Glyph::Check).on_click(click("SAVE")))
        .child(Button::destructive("DELETE").icon(Glyph::Trash).on_click(click("DELETE")))
        // Disabled: the whole cluster (icon + label) fades with the chrome.
        .child(Button::primary("SAVE").icon(Glyph::Check).disabled(true))
        // An arbitrary tree: a column of a row + an accelerator hint. The button hugs it.
        .child(
            Button::empty()
                .variant(ButtonVariant::Outline)
                .child(
                    Flex::column()
                        .gap(2.0)
                        .align("center")
                        .child(
                            Flex::row()
                                .gap(6.0)
                                .align("center")
                                .child(Icon::new(Glyph::Lightning))
                                .child(Label::new("COMPOSED").bold(true)),
                        )
                        // Not styled → inherits the button's state color, like the rest.
                        .child(Label::new("any tree, any depth").font_scale(0.75)),
                )
                .on_click(click("COMPOSED")),
        )
        // The size variant reaches the composed content (icon + label shrink too).
        .child(
            Button::secondary("SMALL")
                .icon(Glyph::Gear)
                .size(WidgetSize::Small),
        )
}
