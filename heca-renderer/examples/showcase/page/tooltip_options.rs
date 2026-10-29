use super::*;

pub(super) fn build(_cx: &DemoCx<'_>) -> Flex {


    Flex::row()
        .gap(8.0)
        .align("center")
        .child(Tooltip::new(
            Button::secondary("TOP").on_click(|| {}),
            "Above the target",
        ))
        .child(
            Tooltip::new(Button::secondary("BOTTOM").on_click(|| {}), "Below the target")
                .side(TooltipSide::Bottom),
        )
        .child(
            Tooltip::new(Button::secondary("LEFT").on_click(|| {}), "Left of the target")
                .side(TooltipSide::Left),
        )
        .child(
            Tooltip::new(Button::secondary("RIGHT").on_click(|| {}), "Right of the target")
                .side(TooltipSide::Right),
        )
}
