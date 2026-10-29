use super::*;

pub(super) fn build(_cx: &DemoCx<'_>) -> Flex {

    let click = |label: &str| {
        let name = label.to_string();
        move || println!("[showcase] {name} clicked")
    };
    Flex::column()
        .gap(10.0)
        .child(Toast::success("Build succeeded").body_text("12 crates compiled in 4.2s"))
        .child(
            Toast::danger("Connection lost")
                .body_text("Reconnecting to the grid…")
                // Two actions, each a real Button: the card places them and publishes its
                // severity, so they tone themselves — and `prefix+/` letters them with
                // nothing declared.
                .action(Button::outline("Retry").on_click(click("toast-retry")))
                .action(Button::ghost("Details").on_click(click("toast-details")))
                .on_dismiss(click("toast-dismiss")),
        )
        // A COMPOSED body: the slot takes any component, not just a line of text — so a
        // notification can carry structure. Neither Label names a colour; the body slot
        // is painted under the theme's muted token and the card publishes its severity.
        .child(
            Toast::warning("Build finished with warnings")
                .body(
                    Flex::column()
                        .gap(2.0)
                        .child(Label::new("3 warnings in heca-grid-ui"))
                        .child(Label::new("cargo check exited 0").font_scale(0.85)),
                )
                .action(Button::new("Open log").on_click(click("toast-log")))
                .action(Button::ghost("Ignore").on_click(click("toast-ignore"))),
        )
        // A clickable card with no body — the inline "notification row" case.
        .child(Toast::info("New message from GRID-7").on_click(click("toast-open")))
}
