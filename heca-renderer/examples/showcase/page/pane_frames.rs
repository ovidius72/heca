use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Flex {
    let theme = cx.theme;


    Flex::row()
        .gap(8.0)
        .align("center")
        .child(
            Pane::new()
                .frameless()
                .width(100.0)
                .height(80.0)
                .background(theme.colors.surface)
                .child(Label::new("None").font_size(12.0).color(theme.colors.muted)),
        )
        .child(
            Pane::new()
                .bordered()
                .width(100.0)
                .height(80.0)
                .background(theme.colors.surface)
                .border(theme.colors.border, theme.colors.border_width)
                .child(Label::new("Bordered").font_size(12.0).color(theme.colors.accent)),
        )
        .child(
            // Bracketed: the self-contained corner reticle (driven by
            // `theme.colors.border_width`); no `.border()` — that would compete.
            Pane::new()
                .bracketed()
                .width(100.0)
                .height(80.0)
                .background(theme.colors.surface)
                .child(Label::new("Bracketed").font_size(12.0).color(theme.colors.accent)),
        )
        .child(
            // Bordered with a per-pane width override: independent of the
            // global BORDER control (`theme.colors.border_width`). Used by the
            // self-themed sidebar shell (`[appearance] sidebar_border_width`).
            Pane::new()
                .bordered()
                .border_width(3.0)
                .radius(10.0)
                .width(100.0)
                .height(80.0)
                .background(theme.colors.surface)
                .border(theme.colors.accent, 0.0)
                .child(Label::new("Border 3px").font_size(12.0).color(theme.colors.accent)),
        )
        .child(
            // Bracketed in its own colour: `.border_color(c)` colours the frame in
            // every style (the sidebar's `[appearance.sidebar] border_color`).
            Pane::new()
                .bracketed()
                .border_color(theme.colors.danger)
                .width(100.0)
                .height(80.0)
                .background(theme.colors.surface)
                .child(Label::new("Own colour").font_size(12.0).color(theme.colors.danger)),
        )
}
