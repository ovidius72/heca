use super::*;

/// Every way a label's text is set: weight, slant and decorations; letter spacing and case; line
/// height on a wrapped paragraph.
pub(super) fn build(_cx: &DemoCx<'_>) -> Flex {
    Flex::column()
        .gap(Spacing::Md)
        .child(
            Flex::row()
                .gap(Spacing::Lg)
                .align("center")
                .child(Label::new("Regular"))
                .child(Label::new("Medium").font_weight(FontWeight::Medium))
                .child(Label::new("SemiBold").font_weight(FontWeight::SemiBold))
                .child(Label::new("Bold").font_weight(FontWeight::Bold))
                .child(Label::new("Italic").italic(true))
                .child(Label::new("Underline").underline(true))
                .child(Label::new("Strikethrough").strikethrough(true)),
        )
        .child(
            Flex::row()
                .gap(Spacing::Lg)
                .align("center")
                .child(Label::new("tight spacing").letter_spacing(LetterSpacing::Tight))
                .child(Label::new("wide spacing").letter_spacing(LetterSpacing::Wide))
                .child(Label::new("wider spacing").letter_spacing(LetterSpacing::Wider))
                .child(
                    // Written "members · 4", shown as a small spaced heading.
                    Label::new("members · 4")
                        .font_weight(FontWeight::SemiBold)
                        .letter_spacing(LetterSpacing::Wide)
                        .transform(TextTransform::Uppercase)
                        .muted(true),
                )
                .child(Label::new("capitalize every word").transform(TextTransform::Capitalize)),
        )
        .child(
            Grid::new()
                .template_column("1fr 1fr")
                .gap(Spacing::Lg)
                .child([
                    Label::new("Normal line height: the workspace keeps every pane you opened.")
                        .wrap(true),
                    Label::new("Relaxed line height: the workspace keeps every pane you opened.")
                        .wrap(true)
                        .line_height(LineHeight::Relaxed),
                ]),
        )
}
