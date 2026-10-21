use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Box<dyn Component> {
    let theme = cx.theme;
    let ctl = cx.ctl;

    let click = |label: &str| {
        let name = label.to_string();
        move || println!("[showcase] {name} clicked")
    };
    Box::new(Flex::row()
                .gap(14.0)
                .align("center")
                .child(Button::primary("DEFAULT").on_click(click("DEFAULT")))
                .child(Button::secondary("SECONDARY"))
                .child(Button::outline("OUTLINE"))
                .child(Button::ghost("GHOST"))
                .child(Button::link("LINK"))
                .child(Button::destructive("DESTRUCTIVE").on_click(click("DESTRUCTIVE")))
                // Theme switcher: click to cycle grid_tron → mocha → latte → …
                // The label shows the current theme name; the showcase tree is
                // rebuilt on change so every widget picks up the new palette.
                .child(
                    Button::primary(format!("⇄ THEME: {}", theme.colors.name)).on_click(move || {
                        let next = (ctl.theme_idx.get_untracked() + 1) % THEME_NAMES.len();
                        ctl.theme_idx.set(next);
                    }),
                ),)
}
