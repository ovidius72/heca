use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Flex {
    let theme = cx.theme;

    let toggle_row = |toggle: Toggle, label: &str, color: Color| {
        Flex::row()
            .gap(16.0)
            .align("center")
            .child(toggle)
            .child(Label::new(label).color(color))
    };
    let report = |a: Action| println!("[showcase] {} -> {:?}", a.name, a.data);
    Flex::column()
        .gap(16.0)
        .child(toggle_row(
            Toggle::new().on(true).on_change(report),
            "GRID UPLINK",
            theme.colors.accent,
        ))
        .child(toggle_row(
            Toggle::new().on(true).on_change(report),
            "AUTO-SCAN",
            theme.colors.accent,
        ))
        // Disabled + off: stealth look, muted ("opaque") label.
        .child(toggle_row(
            Toggle::new().disabled(true),
            "STEALTH MODE",
            theme.colors.muted,
        ))
        // Disabled + on: active-but-locked, colored label.
        .child(toggle_row(
            Toggle::new().on(true).disabled(true),
            "LOCKED OUT",
            theme.colors.accent,
        ))
}
