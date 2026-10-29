use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Flex {
    let theme = cx.theme;
    let sidebar_mode = cx.sidebar_mode;
    let rail_hints = cx.rail_hints;
    let rail_states = cx.rail_states.clone();
    let attention_req = cx.attention_req;

    // Single-select highlight shared across the explorer's file rows: the
    // clicked row goes active (accent bar), the rest clear. Immediate-mode.
    let selected = signal(0usize);
    let states: Rc<RefCell<Vec<Signal<bool>>>> = Rc::new(RefCell::new(Vec::new()));
    let file = move |name: &str, icon: Icon| -> Item {
        let item = Item::new(name).leading(icon).marker(ActiveMarker::Bar);
        let i = states.borrow().len();
        states.borrow_mut().push(item.state());
        let states = states.clone();
        item.on_activate(move || {
            selected.set(i);
            for (j, s) in states.borrow().iter().enumerate() {
                s.set(j == i);
            }
        })
    };

    // A pane "card": a `Tile` — status pip, icon, title, then a git line under it — inside
    // a `Row` that owns the selection, the click and the drag. **The same `Tile` the app's
    // sidebar mounts**, so this demo cannot drift from the real row. It
    // used to be a ninety-line hand-built copy that kept two defects the app had fixed:
    // two stacked title labels toggled by visibility, and the pip pinned in a 12px slot.
    //
    // Nothing here names a pixel or a colour for the text: the title takes the row's
    // selected colour through the content colour `Row` publishes, and the git line is the
    // `Small` size variant, which scales its font and icons together.
    let pane_sel = signal(0usize);
    let pane_states: Rc<RefCell<Vec<Signal<bool>>>> = Rc::new(RefCell::new(Vec::new()));
    let pane = move |icon: Glyph,
                     title: &str,
                     git_branch: Option<&str>,
                     git_added: Option<&str>,
                     git_modified: Option<&str>,
                     git_deleted: Option<&str>,
                     status: DotStatus|
          -> Row {
        // One count on the git line — an icon and a number in one tone — shown only when
        // there is a count to show.
        let count = |glyph: Glyph, tone: Color, text: Option<&str>| {
            Visibility::new(
                Flex::row()
                    .align("center")
                    .gap(Spacing::Xs)
                    .child(Icon::new(glyph).color(tone))
                    .child(Label::new(text.unwrap_or_default()).color(tone)),
                text.is_some(),
            )
        };
        // Attached always and hidden outside a repository, so it could appear later
        // without a rebuild (see `Tile::line`).
        let git_line = Visibility::new(
            Flex::row()
                .align("center")
                .gap(Spacing::Sm)
                .size(WidgetSize::Small)
                .child(Icon::new(Glyph::GitBranch).color(theme.colors.warning))
                .child(Label::new(git_branch.unwrap_or_default()))
                .child(count(Glyph::Plus, theme.colors.success, git_added))
                .child(count(Glyph::Warning, theme.colors.warning, git_modified))
                .child(count(Glyph::Minus, theme.colors.danger, git_deleted)),
            git_branch.is_some(),
        );
        let row = Row::new().padding(Spacing::Sm).child(
            Tile::new()
                .status(StatusDot::new(status))
                .icon(Icon::new(icon))
                .title(Label::new(title).bold(true))
                .line(git_line)
                .grow(1.0),
        );
        let i = pane_states.borrow().len();
        pane_states.borrow_mut().push(row.state());
        let pane_states = pane_states.clone();
        // DnD framework (universal `DragExt`): each card is a drag source, and what
        // it drags is the name it declares about itself. The app resolves a drop via
        // `drag::source_at`/`resolve_at` over the laid-out tree and paints
        // `PaintCx::drag_ghost`/`drop_indicator` — see docs/widgets.md §Drag.
        row.key(format!("pane:{i}")).draggable().on_activate(move || {
            pane_sel.set(i);
            for (j, s) in pane_states.borrow().iter().enumerate() {
                s.set(j == i);
            }
        })
    };

    // G4 DockFrame framing G3 ItemGroups; header slot carries a count
    // badge. `.rail(...)` makes it fold to a single icon when the sidebar
    // collapses to its rail (G5) — bound to the host-owned mode signal.
    let explorer = DockFrame::new("EXPLORER")
        .rail(sidebar_mode, Glyph::FolderOpen)
        // `.active(true)` paints the faint accent **wash** over the whole
        // frame (alpha = `theme.colors.active_wash_alpha`) — the cue the app uses
        // to mark the active workspace. Signal-backed, so a host flips it in
        // place via `.active_state()` without rebuilding.
        .active(true)
        // Inset the header count badge by the Item rows' horizontal
        // padding (~14px) so it lines up vertically with the rows'
        // trailing badges instead of sitting flush at the frame edge.
        .header(
            Flex::row()
                .align("center")
                .child(Badge::accent("3"))
                .child(Flex::row().width(14.0)),
        )
        .child(
            ItemGroup::new("src")
                .child(file(
                    "main.rs",
                    Icon::new(Glyph::FileCode).color(theme.colors.accent).size(18.0),
                ))
                .child(file(
                    "chrome_region.rs",
                    Icon::new(Glyph::FileCode)
                        .color(theme.colors.foreground)
                        .size(18.0),
                ))
                .child(file(
                    "dock_frame.rs",
                    Icon::new(Glyph::FileCode)
                        .color(theme.colors.foreground)
                        .size(18.0),
                )),
        )
        .child(
            // Collapsed group — verifies the paint fix: its rows must not
            // bleed to the top-left while hidden.
            ItemGroup::new("tests").expanded(false).child(file(
                "phase_a.rs",
                Icon::new(Glyph::FileCode).color(theme.colors.muted).size(18.0),
            )),
        );
    // Git status rows: a state-colored duotone icon + change-kind badge.
    let git_row =
        |icon: Icon, name: &str, tag: Badge| Item::new(name).leading(icon).trailing(tag);
    let source_control = DockFrame::new("SOURCE CONTROL")
        .rail(sidebar_mode, Glyph::GitBranch)
        // `.nav_selected(true)`: the sidebar-nav cursor on a workspace frame —
        // a hollow accent border, distinct from `.active(true)`'s filled wash
        // (see EXPLORER above).
        .nav_selected(true)
        .header(
            Flex::row()
                .align("center")
                .child(Badge::warning("3"))
                .child(Flex::row().width(14.0)),
        )
        .child(git_row(
            Icon::new(Glyph::GitBranch).color(theme.colors.warning).size(18.0),
            "chrome_region.rs",
            Badge::warning("M"),
        ))
        .child(git_row(
            Icon::new(Glyph::Plus).color(theme.colors.success).size(18.0),
            "showcase.rs",
            Badge::success("A"),
        ))
        .child(git_row(
            Icon::new(Glyph::Minus).color(theme.colors.danger).size(18.0),
            "old_sidebar.rs",
            Badge::danger("D"),
        ));

    // G5 ChromeRegion: a vertical sidebar shell hosting the docks. Bound
    // to the host-owned mode signal so `[` collapses it to the icon rail.
    let sidebar = ChromeRegion::vertical()
        .with_mode_signal(sidebar_mode)
        .expanded_size(340.0)
        .rail_size(64.0)
        .gap(14.0)
        .padding(14.0)
        .background(theme.colors.surface)
        .dock(explorer)
        .dock(source_control);

    // PANES: sidebar-style pane cards. Row 1 is a centered inline
    // `status dot + icon + name`; row 2 is a flat git metadata line that
    // hides outside repos.
    // The dock is a DnD drop target (universal `DragExt`) — its cards drop here.
    let panes = DockFrame::new("PANES")
        .key("dock:panes")
        .drop_target()
        .child(
            pane(
                Glyph::FileCode,
                "Neovim",
                Some("feature/pane-runtime"),
                Some("+2"),
                Some("~3"),
                None,
                DotStatus::Online,
            )
            // `n` fires a "needs attention" pulse on this pane.
            .attention(attention_req)
            .attention_color(theme.colors.warning),
        )
        .child(pane(
            Glyph::FolderOpen,
            "Yazi",
            Some("feature/pane-runtime"),
            None,
            None,
            None,
            DotStatus::Offline,
        ))
        .child(pane(
            Glyph::Terminal,
            "build",
            None,
            None,
            None,
            None,
            DotStatus::Error,
        ));
    // MarkerGroup: a column-style grouping fronted by a left marker bar
    // that lights to the accent when the group is active (here: the first).
    // Each group is also a DnD drag source + drop target (universal `DragExt`)
    // grabbed by its left grip gutter — this is how the app drags whole
    // *columns* (F4.5). A column drag accepts only column/workspace targets via
    // `drag::resolve_at_filtered`, so nested pane rows fall through to the group.
    let marker_demo = Flex::column()
        .gap(8.0)
        .child(
            Label::new("MARKER GROUP")
                .color(theme.colors.muted)
                .font_scale(0.8),
        )
        .child(
            // Wide rows wrapped in KeyHint with `CenterRight`: the pick keycap
            // pins to the right edge (vs the rail's `Center`) so the row label
            // stays readable, and the transparent wrapper lets each row fill the
            // column width. Driven by the same `p` pick as the rail.
            MarkerGroup::new()
                .active(true)
                .gap(4.0)
                .key("col:900")
                .draggable()
                .drop_target()
                .child(
                    KeyHint::new(Row::new().padding(6.0).child(Label::new("pane A")))
                        .hint(rail_hints[0])
                        .placement(HintPlacement::CenterRight),
                )
                .child(
                    // `.nav_selected(true)`: the nav cursor on a pane card — a
                    // hollow outline, distinct from `.active(true)`'s filled pill.
                    KeyHint::new(
                        Row::new()
                            .padding(6.0)
                            .nav_selected(true)
                            .child(Label::new("pane B")),
                    )
                        .hint(rail_hints[1])
                        .placement(HintPlacement::CenterRight),
                ),
        )
        .child(
            MarkerGroup::new()
                // `.nav_selected(true)`: the sidebar-nav cursor — a full-opacity
                // bar without the active glow, so it reads distinctly from the
                // active column's glowing bar.
                .nav_selected(true)
                .gap(4.0)
                .key("col:901")
                .draggable()
                .drop_target()
                .child(
                    // `.color(theme.colors.warning)` tints the keycap differently — the
                    // app uses this so a "move → workspace" pick reads distinctly
                    // from a pane pick.
                    KeyHint::new(Row::new().padding(6.0).child(Label::new("pane C")))
                        .hint(rail_hints[2])
                        .color(theme.colors.warning)
                        .placement(HintPlacement::CenterRight),
                ),
        )
        // The two host-painted drop visuals: a MOVE inserts (line on an edge);
        // a SWAP exchanges the whole item (double frame, no before/after).
        .child(
            Label::new("DROP INDICATORS")
                .color(theme.colors.muted)
                .font_scale(0.8),
        )
        .child(
            Flex::row()
                .gap(12.0)
                .child(
                    Flex::column()
                        .gap(4.0)
                        .child(
                            IndicatorSwatch::new(false)
                                .width(120.0)
                                .height(34.0),
                        )
                        .child(Label::new("move").color(theme.colors.muted).font_scale(0.8)),
                )
                .child(
                    Flex::column()
                        .gap(4.0)
                        .child(
                            IndicatorSwatch::new(true)
                                .width(120.0)
                                .height(34.0),
                        )
                        .child(
                            Label::new("swap (Shift)")
                                .color(theme.colors.muted)
                                .font_scale(0.8),
                        ),
                ),
        );
    // FocusScope: the generic **keyboard-focus outline** for a whole subtree, not a control.
    // A control draws its own ring because it owns its focus; an *area* the keyboard is aimed
    // at (a sidebar dock the scroll keys act on) has no such owner, so the host wraps it and
    // flips one signal. Transparent like KeyHint: it hugs the child and routes everything
    // through, adding only the theme's focus outline while its signal is on. heca uses it for
    // the focused dock (`focus_dock`).
    let focus_ring_demo = Flex::column()
        .gap(8.0)
        .child(
            Label::new("FOCUS RING — a focused AREA, not a control")
                .color(theme.colors.muted)
                .font_scale(0.82),
        )
        .child(
            Flex::row()
                .gap(24.0)
                .align("center")
                .child({
                    // The ring follows the keyboard: dock A holds it, so dock A is outlined.
                    let card = Card::new("dock A").child(Label::new("has keyboard focus"));
                    card.base().focus(true);
                    FocusScope::new(card)
                })
                .child(FocusScope::new(
                    Card::new("dock B").child(Label::new("does not")),
                )),
        );
    // Prefix-as-symbol: the keybinding "prefix" is *displayed* as λ (a plain
    // Geist Mono glyph — no icon/Nerd font needed). The config/parse token
    // stays "prefix+…"; only the rendered shortcut uses λ.
    let prefix_demo = Flex::column()
        .gap(8.0)
        .child(Label::new(format!("PREFIX AS SYMBOL — {PREFIX_SYMBOL}")).color(theme.colors.muted).font_scale(0.82))
        .child(
            Flex::row()
                .align("center")
                .gap(18.0)
                .child(Tag::new(display_shortcut("prefix+f")))
                .child(Tag::new(display_shortcut("prefix+q")))
                .child(Tag::new(display_shortcut("prefix+Shift+c"))),
        )
        .child(
            Label::new("(config token stays \"prefix+…\")")
                .color(theme.colors.muted)
                .font_scale(0.74),
        );
    let panes_col = Flex::column()
        .width(380.0)
        .gap(16.0)
        .child(panes)
        .child(marker_demo)
        .child(focus_ring_demo)
        .child(prefix_demo);

    // Workspaces rail (enumerate flavor): one icon cell PER pane, so every
    // pane stays visible + addressable when collapsed — unlike a tool dock
    // that folds to a single icon. Each cell is wrapped in the generic
    // KeyHint, whose keycap lights up during a move/swap/select pick (`p`).
    let rail_icons = [
        (Glyph::Terminal, theme.colors.success),
        (Glyph::FileCode, theme.colors.accent),
        (Glyph::GitBranch, theme.colors.warning),
        (Glyph::Gear, theme.colors.foreground),
        (Glyph::Warning, theme.colors.danger),
    ];
    let mut workspaces_rail = Flex::column().gap(8.0).align("center");
    for (i, (glyph, color)) in rail_icons.iter().enumerate() {
        let cell = RailCell::new(Icon::new(*glyph).color(*color).size(22.0))
            .cell_size(44.0)
            .active(i == 0);
        rail_states.borrow_mut().push(cell.state());
        let states = rail_states.clone();
        let cell = cell.on_activate(move || {
            for (j, s) in states.borrow().iter().enumerate() {
                s.set(j == i);
            }
        });
        workspaces_rail = workspaces_rail.child(
            KeyHint::new(cell)
                .hint(rail_hints[i])
                .placement(HintPlacement::Center),
        );
    }
    let rail_col = Flex::column()
        .gap(8.0)
        .align("center")
        .child(Label::new("WS").color(theme.colors.muted).font_scale(0.8))
        .child(workspaces_rail);

    Flex::row()
        .gap(28.0)
        .align("start")
        .child(rail_col)
        .child(sidebar)
        .child(panes_col)
}
