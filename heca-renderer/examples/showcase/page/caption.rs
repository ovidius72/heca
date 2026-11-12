use super::*;

pub(super) fn build(_cx: &DemoCx<'_>) -> Flex {
    let caption = Caption::new("Build output");
    let text = caption.text_signal();
    Flex::column().gap(Spacing::Sm)
        .child(Caption::new("Release logs").size(WidgetSize::Normal).content_tone(Tone::Accent))
        .child(Input::new().value("Build output").on_change(move |action| {
            if let SignalData::String(value) = action.data { text.set(value); }
        }))
        .child(caption)
        .child(Caption::new(""))
}
