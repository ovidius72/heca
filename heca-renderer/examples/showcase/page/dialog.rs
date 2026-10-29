use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Flex {
    let open_it = cx.open_it;
    let dialog_open = cx.dialog_open;


    Flex::row()
        .gap(12.0)
        .align("center")
        .child(
            Button::destructive("DELETE PANE (DIALOG)…")
                .on_click(move || dialog_open.set(true)),
        )
        // The bare animated `Overlay` — one builder, `.animation(..)`, and nothing else
        // wired: no registry, no id, no host pass.
        .child(
            Button::secondary("ANIMATED SURFACE…")
                .on_click(move || open_it.set(true)),
        )
}
