use super::*;

pub(super) fn build(_cx: &DemoCx<'_>) -> Box<dyn Component> {


    Box::new(Flex::row()
                .gap(20.0)
                .align("center")
                .child(Label::new("REGULAR"))
                .child(Label::new("BOLD").bold(true))
                .child(Label::new("ITALIC").italic(true))
                .child(Label::new("UNDERLINE").underline(true))
                .child(Label::new("STRIKETHROUGH").strikethrough(true))
                .child(
                    Label::new("ALL FOUR")
                        .bold(true)
                        .italic(true)
                        .underline(true)
                        .strikethrough(true),
                ),)
}
