use super::*;

pub(super) fn build(_cx: &DemoCx<'_>) -> Flex {
    Flex::row()
        .gap(8.0)
        .align("center")
        .child(Tooltip::new(
            Button::secondary("TOP").on_click(|_| {}),
            "Above the target",
        ))
        .child(
            Tooltip::new(
                Button::secondary("BOTTOM").on_click(|_| {}),
                "Below the target",
            )
            .side(TooltipSide::Bottom),
        )
        .child(
            Tooltip::new(
                Button::secondary("LEFT").on_click(|_| {}),
                "Left of the target",
            )
            .side(TooltipSide::Left),
        )
        .child(
            Tooltip::new(
                Button::secondary("RIGHT").on_click(|_| {}),
                "Right of the target",
            )
            .side(TooltipSide::Right),
        )
}
