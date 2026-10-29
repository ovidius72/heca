use super::*;

pub(super) fn build(_cx: &DemoCx<'_>) -> Flex {

    let report = |a: Action| println!("[showcase] {} -> {:?}", a.name, a.data);
    Flex::row()
        .gap(24.0)
        .align("center")
        .child(
            Checkbox::new()
                .checked(true)
                .label("ENCRYPT")
                .on_change(report),
        )
        .child(Checkbox::new().label("VERBOSE").on_change(report))
        .child(
            Checkbox::new()
                .checked(true)
                .label("READ-ONLY")
                .disabled(true),
        )
        .child(
            Checkbox::new()
                .label("LABEL LEFT")
                .label_side(LabelSide::Left)
                .on_change(report),
        )
}
