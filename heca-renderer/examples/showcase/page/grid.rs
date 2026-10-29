use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Grid {
    let theme = cx.theme;


    Grid::new()
        .template_column("auto 1fr auto")
        .template_row("auto auto")
        .template_area(["icon title   status", "icon subtext ."])
        // Items sit at the TOP-LEFT of their cell by default (an explicit size has nothing
        // to stretch), so an Icon (h = font) and a Label (h = font × 1.4) would not share a
        // centre line. `align` is the vertical knob — the icon then centres across the two
        // rows it spans, and the status dot centres against the title.
        .align("center")
        // A leading icon spans both text rows, so it is sized to them (a body-size glyph
        // centred over two lines just floats in the gutter between them).
        .child(Icon::new(Glyph::Terminal).size(26.0).area("icon"))
        .child(Label::new("zsh").bold(true).area("title"))
        // …and `justify_self` is the horizontal one, per item: pin the dot to the right edge
        // of its cell instead of letting it stretch across the column.
        .child(StatusDot::online().justify_self("end").area("status"))
        .child(Label::new("~/projects/heca").color(theme.colors.muted).area("subtext"))
        .gap(8.0)
        .width(320.0)
}
