use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Flex {
    let theme = cx.theme;


    Flex::row()
        .gap(12.0)
        .align("start")
        // Idle terminal pane: location · shell name.
        .child(
            Pane::new()
                .bordered()
                .width(220.0)
                .height(90.0)
                .padding(8.0)
                .gap(8.0)
                .background(theme.colors.surface)
                .border(theme.colors.border, theme.colors.border_width)
                .child(
                    Tag::new("~/projects/heca")
                        .leading(Icon::new(Glyph::Folder).size(13.0).color(theme.colors.muted))
                        .segment(
                            Flex::row()
                                .align("center")
                                .gap(6.0)
                                .child(
                                    Icon::new(Glyph::Terminal)
                                        .size(13.0)
                                        .color(theme.colors.muted),
                                )
                                .child(
                                    Label::new("zsh")
                                        .italic(true)
                                        .color(theme.colors.drag_ghost_fg)
                                        .font_scale(0.8),
                                ),
                        ),
                )
                .child(Label::new("idle").font_size(12.0).color(theme.colors.muted)),
        )
        // Running pane: full header = segments (left) + action buttons
        // (right). The app's pane info bar is exactly this — a segment `Tag`
        // and an `IconButton` cluster (default split + close, each a tooltip'd
        // action with its keybind) laid out space-between inside the pane top.
        .child(
            Pane::new()
                .bordered()
                .width(360.0)
                .height(90.0)
                .padding(8.0)
                .gap(8.0)
                .background(theme.colors.surface)
                .border(theme.colors.border, theme.colors.border_width)
                .child(
                    Flex::row()
                        .width(344.0)
                        .align("center")
                        .justify("space-between")
                        .child(
                            Tag::new("Neovim")
                                .leading(
                                    Icon::new(Glyph::FileCode)
                                        .size(13.0)
                                        .color(theme.colors.accent),
                                )
                                .segment(
                                    Flex::row()
                                        .align("center")
                                        .gap(6.0)
                                        .child(
                                            Icon::new(Glyph::GitBranch)
                                                .size(13.0)
                                                .color(theme.colors.muted),
                                        )
                                        .child(
                                            Label::new("…phase-7")
                                                .color(theme.colors.foreground)
                                                .font_scale(0.8),
                                        ),
                                )
                                .color(theme.colors.accent),
                        )
                        .child(
                            Flex::row()
                                .align("center")
                                .gap(2.0)
                                .child(
                                    Tooltip::new(
                                        IconButton::new(
                                            Icon::new(Glyph::SquareSplitVertical)
                                                .color(theme.colors.foreground)
                                                .size(15.0),
                                        )
                                        .cell(24.0),
                                        "Add pane  ⌃B V",
                                    )
                                    .side(TooltipSide::Bottom),
                                )
                                .child(
                                    Tooltip::new(
                                        IconButton::new(
                                            Icon::new(Glyph::XSquare)
                                                .color(theme.colors.danger)
                                                .size(15.0),
                                        )
                                        .cell(24.0)
                                        .accent(theme.colors.danger),
                                        "Close  ⌃B X",
                                    )
                                    .side(TooltipSide::Bottom),
                                ),
                        ),
                )
                .child(Label::new("running").font_size(12.0).color(theme.colors.muted)),
        )
}
