//! The first built-in provider — [`WorkspacesContainerProvider`] — the workspace-tree
//! sidebar container, and the render code it owns.
//!
//! This is the whole point of plugin-03: the sidebar's workspace tree used to be a
//! bespoke façade that the chrome render path built and mounted directly, bypassing the
//! pluggable-chrome runtime entirely. It is now **a container like any other** —
//! registered in [`ChromeHost`](crate::chrome::ChromeHost) at startup, seated in a
//! region, and rendered because the region asked its provider for a contribution. The
//! host no longer knows that the left sidebar happens to hold workspaces; move the
//! container to the right region and its UI goes with it.
//!
//! The container body — [`build_workspaces_container`] and its
//! [`column_view`] / [`pane_card`] rows — lives here rather than in `chrome/mod.rs`,
//! so the provider owns its render code. What stays in `chrome` is the host-side
//! vocabulary a container *uses*: the shell it mounts into, the drag/hint registries it
//! registers ids in, and the shared projections (`pane_info_view`, `runtime_snapshot`)
//! the pane headers also read.

use crate::chrome::{
    alpha_u8, home_relative_path, pane_info_view, runtime_snapshot, truncate_sidebar_git_branch,
    BuildCx, ChromeDragItem, ChromeIntentEmitter, ChromeSignals, ContainerContribution,
    Contribution, DragItemRegistry, HintTargetRegistry, PaneInfoSignals, RegionId, RegionSet,
    RepaintWatch, WidgetModel, WorkspacesContainerState, CARD_META_FONT_SCALE,
};
use crate::providers::{ChromeCtx, Provider};
use crate::sidebar::{SidebarColEntry, SidebarPaneEntry, SidebarTree};
use heca_config::programs::ProgramsConfig;
use heca_core::layout::PaneId;
use heca_core::runtime::ProcessStatus;
use heca_grid_ui::builders::{DragExt, HintExt, LayoutExt, Parent, StyleExt};
use heca_grid_ui::reactive::signal;
use heca_grid_ui::style::{Align, Length};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{
    ActiveMarker, Badge, DockFrame, Flex, Glyph, HintPlacement, Icon, KeyHint, Label, MarkerGroup,
    Row, StatusDot, Tooltip, TooltipSide, Visibility,
};

/// The built-in workspace-tree sidebar container.
///
/// Mounts in the left sidebar by default and is movable to the right sidebar
/// (`supported_regions = sidebars`). It is the first real [`Provider`] registered
/// in [`ChromeHost`](crate::chrome::ChromeHost) (plugin-03), proving the
/// pluggable-chrome runtime hosts a domain container — not just the
/// `TestProvider` stand-in in `host.rs` tests.
pub struct WorkspacesContainerProvider;

impl WorkspacesContainerProvider {
    /// A default instance ready to register.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WorkspacesContainerProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for WorkspacesContainerProvider {
    fn id(&self) -> &str {
        "workspaces"
    }

    fn supported_regions(&self) -> RegionSet {
        RegionSet::sidebars()
    }

    fn default_region(&self) -> RegionId {
        RegionId::LeftSidebar
    }

    fn default_order(&self) -> i32 {
        0
    }

    fn title(&self) -> &str {
        "Workspaces"
    }

    fn movable(&self) -> bool {
        true
    }

    fn collapsible(&self) -> bool {
        true
    }

    fn build_contribution(&self, _ctx: &ChromeCtx<'_>) -> Contribution {
        Contribution::Container(ContainerContribution {
            id: self.id().to_string(),
            title: self.title().to_string(),
            supported_regions: self.supported_regions(),
            default_region: self.default_region(),
            default_order: self.default_order(),
            movable: self.movable(),
            collapsible: self.collapsible(),
            build: Box::new(build_body),
        })
    }
}

/// The render seam: project the frame's workspace tree into the container body.
///
/// Every input is read off the two contexts — nothing is captured — so one plain `fn`
/// serves as the build hook for every mount of this container.
///
/// Outside a render pass the context carries no tree/theme/emitter
/// ([`ChromeCtx::new`] vs [`ChromeCtx::for_build`]), so there is nothing to project:
/// the body is an empty column. That is a real state — a host may build a
/// contribution just to read its metadata — not an error, so it does not panic.
fn build_body(ctx: &ChromeCtx<'_>, bx: &mut BuildCx<'_>) -> WidgetModel {
    let (Some(tree), Some(programs), Some(theme), Some(emit)) =
        (ctx.tree(), ctx.programs(), ctx.theme(), ctx.emit_intent())
    else {
        return Box::new(Flex::column());
    };
    let state = ctx.state();
    // The three registries are borrowed as *disjoint* fields, which is exactly why they
    // are plain fields on `BuildCx` and not accessor methods: `bx.signals()` three times
    // in one call would be three overlapping `&mut *bx` borrows.
    Box::new(build_workspaces_container(
        tree,
        programs,
        theme,
        emit,
        state.workspaces(),
        bx.signals,
        bx.drag,
        bx.hints,
    ))
}

/// A single pane **card**, styled like the showcase PANES rows: a state-tinted
/// background + radius, an active accent bar, a leading program icon, the display
/// name, an optional exceptional-state indicator, and git metadata when present.
#[expect(
    clippy::too_many_arguments,
    reason = "pane-card projection still threads host/runtime context explicitly; phase-local fix before a larger ChromeCx refactor"
)]
fn pane_card(
    pane: &SidebarPaneEntry,
    programs: &ProgramsConfig,
    theme: &GuiTheme,
    emit_intent: &ChromeIntentEmitter,
    active_pane: Option<PaneId>,
    ws_state: &WorkspacesContainerState,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
    hints: &mut HintTargetRegistry,
) -> RepaintWatch {
    let active = active_pane == Some(pane.pane_id);
    let pane_id = pane.pane_id;
    let runtime = runtime_snapshot(ws_state, pane_id);
    let info = pane_info_view(
        programs,
        &pane.name,
        pane.custom_name.as_deref(),
        runtime.as_ref(),
        // A renamed pane shows a dimmed `(process)` suffix here when the setting is on,
        // so the sidebar keeps surfacing what's actually running under a custom name.
        ws_state.pane_renamed_add_process_name(),
    );
    // A constant theme-driven card; the *selected* look (accent pill + border + bar)
    // is drawn by `Row` from its `active` signal, not baked into the background. This
    // keeps styling fully signal-driven (active flips in place via `sync_chrome_signals`,
    // no tree rebuild) and theme-driven (no ad-hoc per-state alphas).
    // The card is both a drag source and a drop target (F4.5); its opaque DragItemId
    // is assigned by the registry (which records that it's this pane) so the kind
    // round-trips through `drag::source_at`/`resolve_at` without trusting raw ids.
    let drag_id = drag.register(ChromeDragItem::Pane(pane_id));
    // Hint target: the universal picker (`prefix+/`) focuses this pane by its letter.
    let hint_id = hints.register(crate::app::interaction::InteractionIntent::FocusPane { pane_id });
    let icon_widget = Icon::new(info.icon).size(14.0).color(theme.colors.foreground);
    let icon_signal = icon_widget.glyph_signal();
    let active_title_label = Label::new(info.title.clone())
        .color(theme.colors.accent)
        .bold(true);
    let active_title_signal = active_title_label.text_signal();
    let active_title = Visibility::new(active_title_label, active);
    let active_title_visible = active_title.visible_signal();
    let inactive_title_label = Label::new(info.title.clone())
        .color(theme.colors.foreground)
        .bold(true);
    let inactive_title_signal = inactive_title_label.text_signal();
    let inactive_title = Visibility::new(inactive_title_label, !active);
    let inactive_title_visible = inactive_title.visible_signal();
    let idle_dot = Visibility::new(StatusDot::offline(), info.status == ProcessStatus::Idle);
    let idle_dot_visible = idle_dot.visible_signal();
    let running_dot = Visibility::new(StatusDot::online(), info.status == ProcessStatus::Running);
    let running_dot_visible = running_dot.visible_signal();
    let success_dot = Visibility::new(StatusDot::online(), info.status == ProcessStatus::Success);
    let success_dot_visible = success_dot.visible_signal();
    let error_dot = Visibility::new(StatusDot::error(), info.status == ProcessStatus::Error);
    let error_dot_visible = error_dot.visible_signal();
    let branch_label_widget = Label::new(truncate_sidebar_git_branch(
        info.git_branch.as_deref().unwrap_or_default(),
    ))
    .color(theme.colors.foreground)
    .font_scale(0.8);
    let branch_display_signal = branch_label_widget.text_signal();
    let branch_signal = signal(info.git_branch.clone().unwrap_or_default());
    let add_label_widget = Label::new(info.git_added.clone().unwrap_or_default())
        .color(theme.colors.success)
        .font_scale(0.8);
    let add_label = add_label_widget.text_signal();
    let add_segment = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(4.0)
            .child(Icon::new(Glyph::Plus).size(12.0).color(theme.colors.success))
            .child(add_label_widget),
        info.git_added.is_some(),
    );
    let add_text_visible_signal = add_segment.visible_signal();
    let modified_label_widget = Label::new(info.git_modified.clone().unwrap_or_default())
        .color(theme.colors.warning)
        .font_scale(0.8);
    let modified_label = modified_label_widget.text_signal();
    let modified_segment = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(4.0)
            .child(Icon::new(Glyph::Warning).size(12.0).color(theme.colors.warning))
            .child(modified_label_widget),
        info.git_modified.is_some(),
    );
    let modified_text_visible_signal = modified_segment.visible_signal();
    let deleted_label_widget = Label::new(info.git_deleted.clone().unwrap_or_default())
        .color(theme.colors.danger)
        .font_scale(0.8);
    let deleted_label = deleted_label_widget.text_signal();
    let deleted_segment = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(4.0)
            .child(Icon::new(Glyph::Minus).size(12.0).color(theme.colors.danger))
            .child(deleted_label_widget),
        info.git_deleted.is_some(),
    );
    let deleted_text_visible_signal = deleted_segment.visible_signal();
    let git_row = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(6.0)
            .child(Icon::new(Glyph::GitBranch).size(12.0).color(theme.colors.warning))
            .child(
                Tooltip::new_signal(branch_label_widget, branch_signal)
                    .side(TooltipSide::Bottom)
                    .delay(0.25),
            )
            .child(add_segment)
            .child(modified_segment)
            .child(deleted_segment),
        info.git_branch.is_some(),
    );
    let git_visible_signal = git_row.visible_signal();
    // A renamed pane surfaces its running program as a dimmed `(process)` suffix after
    // the name (config-gated). Appended to the shared title area so it renders the same
    // in both the git and no-git card layouts. Like the title, it is **signal-driven**
    // (text + visibility updated in `sync_chrome_signals`) so renaming toggles it live —
    // a rename updates signals, it does not rebuild the sidebar card tree.
    let process_hint_text = info
        .process_hint
        .as_ref()
        .map(|program| format!("({program})"))
        .unwrap_or_default();
    // `foreground` (not `muted`) so it's readable on every theme; it still reads as
    // secondary next to the accent + bold name (regular weight, smaller scale).
    let process_hint_label = Label::new(process_hint_text)
        .color(theme.colors.foreground)
        .font_scale(CARD_META_FONT_SCALE);
    let process_hint_signal = process_hint_label.text_signal();
    let process_hint = Visibility::new(process_hint_label, info.process_hint.is_some());
    let process_hint_visible = process_hint.visible_signal();
    let title_area = Flex::row()
        .align(Align::Center)
        .gap(4.0)
        .child(Flex::column().child(active_title).child(inactive_title))
        .child(process_hint);
    // Optional cwd row (folder icon + home-relative path), stacked between the name and
    // git rows. Signal-driven like the git branch: the path updates live on `cd`, and the
    // row's visibility follows `[settings] pane_show_cwd` and whether the pane has a cwd.
    let cwd_path = runtime.as_ref().and_then(|rt| rt.cwd.clone());
    let show_cwd = ws_state.pane_show_cwd() && cwd_path.is_some();
    let cwd_text = cwd_path
        .as_deref()
        .map(home_relative_path)
        .unwrap_or_default();
    // Readable, matching the sibling git-branch row (which colors its label
    // `foreground`); `muted` was too dim for a primary info row.
    let cwd_label = Label::new(cwd_text)
        .color(theme.colors.foreground)
        .font_scale(CARD_META_FONT_SCALE);
    let cwd_signal = cwd_label.text_signal();
    let cwd_row = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(6.0)
            .child(Icon::new(Glyph::Folder).size(12.0).color(theme.colors.foreground))
            .child(cwd_label),
        show_cwd,
    );
    let cwd_visible_signal = cwd_row.visible_signal();
    // The pane's identity row (status dots + program icon + name) — shared by every card
    // layout so the cwd and git rows just stack beneath it in one column.
    let name_row = Flex::row()
        .align(Align::Center)
        .gap(8.0)
        .child(
            Flex::row()
                .align(Align::Center)
                .width(Length::Px(12.0))
                .child(idle_dot)
                .child(running_dot)
                .child(success_dot)
                .child(error_dot),
        )
        .child(Flex::row().align(Align::Center).child(icon_widget))
        .child(title_area);
    let emit = emit_intent.clone();
    // One column: the name row, then the optional cwd and git rows (each 2px-indented and
    // added only when shown, mirroring the git row). A card with only the name row lays
    // out exactly like the former single-row layout — a one-child column adds no gap.
    let mut content = Flex::column().gap(4.0).grow(1.0).child(name_row);
    if show_cwd {
        content = content.child(
            Flex::row()
                .child(Flex::row().width(Length::Px(2.0)))
                .child(cwd_row),
        );
    }
    if info.git_branch.is_some() {
        content = content.child(
            Flex::row()
                .child(Flex::row().width(Length::Px(2.0)))
                .child(git_row),
        );
    }
    let card = Row::new()
        .background(
            theme
                .colors
                .foreground
                .with_alpha(alpha_u8(theme.colors.card_background_alpha)),
        )
        .highlight(theme.colors.accent)
        .radius(theme.colors.control_radius())
        .padding(6.0)
        .marker(ActiveMarker::Bar)
        .active(active)
        .nav_selected(false)
        .draggable(drag_id)
        .drop_target(drag_id)
        .hint_target(hint_id)
        // On click/Enter the card records its pane id in the host sink; the app reads
        // it after dispatch and focuses that pane (read-via-signal / write-via-action).
        .on_activate(move || {
            emit(crate::app::interaction::InteractionIntent::FocusPane { pane_id });
        })
        .child(content);
    // Bind the card's active signal so focus changes update it without a rebuild.
    signals.pane_active.push((pane_id, card.state()));
    signals.pane_nav.push((pane_id, card.nav_state()));
    // Wrap the card in a universal `KeyHint` so a move/swap/take pick can stamp this
    // pane's letter over it. `KeyHint` is transparent — it hugs the child and routes
    // events/focus/drag straight through — so the card stays a drag source + target
    // and clickable. The hint signal is driven each frame in `sync_chrome_signals`
    // from the active `InputMode` candidates (keyboard logic stays the source of truth).
    let hint = signal(None);
    signals.pane_hint.push((pane_id, hint));
    signals.pane_info.push((
        pane_id,
        PaneInfoSignals {
            icon: icon_signal,
            title_active: active_title_signal,
            title_inactive: inactive_title_signal,
            title_active_visible: active_title_visible,
            title_inactive_visible: inactive_title_visible,
            process_hint: process_hint_signal,
            process_hint_visible,
            cwd: cwd_signal,
            cwd_visible: cwd_visible_signal,
            status_idle_visible: idle_dot_visible,
            status_running_visible: running_dot_visible,
            status_success_visible: success_dot_visible,
            status_error_visible: error_dot_visible,
            git_visible: git_visible_signal,
            git_branch: branch_signal,
            git_branch_display: branch_display_signal,
            git_added_visible: add_text_visible_signal,
            git_added: add_label,
            git_modified_visible: modified_text_visible_signal,
            git_modified: modified_label,
            git_deleted_visible: deleted_text_visible_signal,
            git_deleted: deleted_label,
        },
    ));
    let (watch, _repaint) = RepaintWatch::new(
        KeyHint::new(card)
            .hint(hint)
            .placement(HintPlacement::CenterRight),
    );
    watch
}

/// One **column**: a generic [`MarkerGroup`] (left marker bar + grip gutter) holding
/// the column's stacked pane cards — no per-column header row (columns are spatial
/// groupings whose only user-facing job is to be a move/swap target + drag handle).
/// The `MarkerGroup` bar brightens to the accent when the column holds the active
/// pane, and its grip gutter is the seam for the future move/swap [`KeyHint`] target
/// and DnD drag handle (F4.4/F4.5) — applied by the host via `KeyHint`/`DragExt`, not
/// baked into the widget.
#[expect(
    clippy::too_many_arguments,
    reason = "column projection still threads host/runtime context explicitly; phase-local fix before a larger ChromeCx refactor"
)]
fn column_view(
    c: &SidebarColEntry,
    ws_idx: usize,
    programs: &ProgramsConfig,
    theme: &GuiTheme,
    emit_intent: &ChromeIntentEmitter,
    active_pane: Option<PaneId>,
    ws_state: &WorkspacesContainerState,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
    hints: &mut HintTargetRegistry,
) -> RepaintWatch {
    let active = c.panes.iter().any(|p| active_pane == Some(p.pane_id));
    // The MarkerGroup is a column drag source + drop target (F4.5 step 2). Its grip
    // gutter is the only surface not covered by a child pane card, so innermost-first
    // hit-testing routes a grip press → column and a card press → pane, for free.
    let drag_id = drag.register(ChromeDragItem::Column {
        ws: ws_idx,
        col: c.col_idx,
    });
    let mut col = MarkerGroup::new()
        .active(active)
        .gap(3.0)
        .draggable(drag_id)
        .drop_target(drag_id);
    for pane in &c.panes {
        col = col.child(pane_card(
            pane,
            programs,
            theme,
            emit_intent,
            active_pane,
            ws_state,
            signals,
            drag,
            hints,
        ));
    }
    // Bind the column bar's active signal (lit iff it holds the active pane).
    let pane_ids = c.panes.iter().map(|p| p.pane_id).collect::<Vec<_>>();
    signals.col_active.push((pane_ids, col.state()));
    // Wrap the column in the universal `KeyHint` so a "move pane → column" pick can
    // stamp this column's letter over it (tinted `success`, distinct from pane/workspace
    // picks). Driven each frame in `sync_chrome_signals`.
    let col_hint = signal::<Option<String>>(None);
    signals.col_hint.push((ws_idx, c.col_idx, col_hint));
    let hinted = KeyHint::new(col)
        .hint(col_hint)
        .color(theme.colors.success)
        .placement(HintPlacement::CenterRight);
    let (watch, _repaint) = RepaintWatch::new(hinted);
    watch
}

/// Build the **WorkspacesContainer** content — the workspace tree mounted inside the
/// sidebar shell (see `heca-sidebar-design-spec`). Each workspace is a `.frameless()`
/// [`DockFrame`] (header count [`Badge`] = total panes); its columns are compact
/// [`column_view`]s (left marker bar + pane cards, no "Col N" header rows — those ate
/// the sidebar for no user value). Pure projection of the [`SidebarTree`].
///
/// This is the **body of the `workspaces` container** — what [`build_body`], the
/// provider's render seam, returns. It is reached only through that seam: the region
/// asks its mounted provider for a contribution and calls the contribution's `build`.
/// Nothing in `chrome` calls it any more.
#[expect(
    clippy::too_many_arguments,
    reason = "workspace-container projection threads host/runtime context + the drag and hint registries explicitly; phase-local before a larger ChromeCx refactor"
)]
fn build_workspaces_container(
    tree: &SidebarTree,
    programs: &ProgramsConfig,
    theme: &GuiTheme,
    emit_intent: &ChromeIntentEmitter,
    ws_state: &WorkspacesContainerState,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
    hints: &mut HintTargetRegistry,
) -> Flex {
    // Selection is sourced from the container's shared state (the Phase-2 boundary),
    // not from `Session`/`SidebarItemState`. A workspace is "active" iff it hosts the
    // active pane.
    let active_pane = ws_state.active_pane();
    let mut col = Flex::column().gap(6.0).grow(1.0);
    for ws in &tree.workspaces {
        let pane_count =
            ws.columns.iter().map(|c| c.panes.len()).sum::<usize>() + ws.floating_panes.len();
        let active_ws = active_pane.is_some_and(|pid| {
            ws.columns
                .iter()
                .flat_map(|c| &c.panes)
                .chain(&ws.floating_panes)
                .any(|p| p.pane_id == pid)
        });
        let badge = if active_ws {
            Badge::accent(pane_count.to_string())
        } else {
            Badge::neutral(pane_count.to_string())
        };
        // The header toggle records the workspace in the toggle sink; the app flips
        // its collapsed state (canonical, in `chrome_state.workspaces`) and the tree
        // rebuilds (F4.3). Collapse is read back from that same shared state.
        let ws_idx = ws.ws_idx;
        let emit = emit_intent.clone();
        let mut dock = DockFrame::new(ws.name.clone())
            .frameless()
            .gap(4.0) // tighten the workspace header → body spacing
            .expanded(!ws_state.is_ws_collapsed(ws_idx))
            .on_toggle(move |_| {
                emit(
                    crate::app::interaction::InteractionIntent::ToggleWorkspaceCollapsed { ws_idx },
                );
            })
            .header(
                Flex::row()
                    .align(Align::Center)
                    .child(badge)
                    .child(Flex::row().width(Length::Px(6.0))),
            );
        // Light accent wash over the whole active workspace area (+ the accent
        // count badge) makes the active workspace clearly prominent. Signal-driven
        // (like the pane/column highlights) so it flips in place via
        // `sync_chrome_signals` instead of forcing a tree rebuild; the alpha is the
        // theme's `active_wash_alpha` token, not a baked-in literal.
        dock = dock.active(active_ws);
        dock = dock.nav_selected(false);
        let ws_pane_ids = ws
            .columns
            .iter()
            .flat_map(|c| &c.panes)
            .chain(&ws.floating_panes)
            .map(|p| p.pane_id)
            .collect::<Vec<_>>();
        signals.ws_active.push((ws_pane_ids, dock.active_state()));
        signals.ws_nav.push((ws_idx, dock.nav_state()));
        // The whole workspace is a column drop target (F4.5 step 2 scope C): dropping a
        // column anywhere on it that isn't a deeper column/pane target moves the column
        // into this workspace. Innermost-first hit-testing lets columns/panes override.
        dock = dock.drop_target(drag.register(ChromeDragItem::Workspace { ws: ws_idx }));
        // Hint target: the universal picker (`prefix+/`) can focus this workspace by
        // its letter. The keycap is stamped over the dock's bounds by `paint_hint_targets`.
        dock = dock.hint_target(hints.register(
            crate::app::interaction::InteractionIntent::FocusWorkspace { ws_idx },
        ));
        // Columns stacked with a clear gap between them (the gap + bar mark each
        // column); panes inside a column are tight. Floating panes have no column.
        let mut cols = Flex::column().gap(8.0);
        for c in &ws.columns {
            cols = cols.child(column_view(
                c,
                ws_idx,
                programs,
                theme,
                emit_intent,
                active_pane,
                ws_state,
                signals,
                drag,
                hints,
            ));
        }
        for float in &ws.floating_panes {
            cols = cols.child(pane_card(
                float,
                programs,
                theme,
                emit_intent,
                active_pane,
                ws_state,
                signals,
                drag,
                hints,
            ));
        }
        dock = dock.child(cols);
        // Wrap the whole workspace dock in the universal `KeyHint` so a
        // "move column/pane → workspace" pick can stamp this workspace's letter over
        // it. The keycap is tinted `warning` (not accent) so a workspace target reads
        // distinctly from a pane target. The hint signal is driven each frame in
        // `sync_chrome_signals` from the active pick candidates.
        let ws_hint = signal::<Option<String>>(None);
        signals.ws_hint.push((ws_idx, ws_hint));
        col = col.child(
            KeyHint::new(dock)
                .hint(ws_hint)
                .color(theme.colors.warning)
                // Top-right (like the pane cards' right-aligned keycap), nudged down
                // onto the workspace title row so it lines up with the name.
                .placement(HintPlacement::TopRight)
                .offset_y((theme.font_size * 0.45) as f64),
        );
    }
    col
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::SidebarItemState;
    use crate::chrome::{
        ChromeEventBus, ChromeHost, ChromeIntentEmitter, ChromeSignals, DragItemRegistry,
        HintTargetRegistry, SharedChromeState,
    };
    use crate::sidebar::{SidebarColEntry, SidebarPaneEntry, SidebarTree, SidebarWsEntry};
    use heca_core::layout::PaneId;
    use heca_grid_ui::theme::Theme as GuiTheme;
    use std::rc::Rc;

    /// One workspace, one column, one (active) pane — the smallest tree that still
    /// exercises every level of the projection.
    fn tree() -> SidebarTree {
        let mut tree = SidebarTree::new();
        tree.workspaces.push(SidebarWsEntry {
            ws_idx: 0,
            name: "ws1".into(),
            collapsed: false,
            state: SidebarItemState::Active,
            columns: vec![SidebarColEntry {
                col_idx: 0,
                collapsed: false,
                panes: vec![SidebarPaneEntry {
                    pane_id: PaneId(1),
                    name: "pane1".into(),
                    custom_name: None,
                    state: SidebarItemState::Active,
                }],
            }],
            floating_panes: Vec::new(),
        });
        tree
    }

    fn store() -> SharedChromeState {
        let store = SharedChromeState::new(300.0, true, 300.0, false);
        store.workspaces.set_active_pane(Some(PaneId(1)));
        store
    }

    fn container(p: &WorkspacesContainerProvider, ctx: &ChromeCtx<'_>) -> ContainerContribution {
        match p.build_contribution(ctx) {
            Contribution::Container(c) => c,
            _ => panic!("the workspaces provider contributes a Container"),
        }
    }

    #[test]
    fn provider_metadata_is_self_consistent() {
        let p = WorkspacesContainerProvider::new();
        assert_eq!(p.id(), "workspaces");
        assert_eq!(p.title(), "Workspaces");
        assert_eq!(p.default_region(), RegionId::LeftSidebar);
        assert!(p.supported_regions().contains(RegionId::LeftSidebar));
        assert!(p.supported_regions().contains(RegionId::RightSidebar));
        assert_eq!(p.default_order(), 0);
        assert!(p.movable());
        assert!(p.collapsible());
    }

    #[test]
    fn register_seats_in_left_sidebar_at_order_zero() {
        let mut host = ChromeHost::new(ChromeEventBus::default());
        host.register(Box::new(WorkspacesContainerProvider::new()));
        // Seated in the left sidebar (its default_region), not the right.
        let left = host.contributions(RegionId::LeftSidebar);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id(), "workspaces");
        assert_eq!(left[0].title(), "Workspaces");
        assert!(host.contributions(RegionId::RightSidebar).is_empty());
        assert_eq!(host.placement("workspaces"), Some(RegionId::LeftSidebar));
    }

    #[test]
    fn contribution_carries_the_provider_metadata() {
        let p = WorkspacesContainerProvider::new();
        let ctx = ChromeCtx::new(crate::host::App::new(&store()));
        let c = container(&p, &ctx);
        assert_eq!(c.id, "workspaces");
        assert_eq!(c.title, "Workspaces");
        assert_eq!(c.default_region, RegionId::LeftSidebar);
        assert_eq!(c.default_order, 0);
        assert!(c.supported_regions.contains(RegionId::LeftSidebar));
        assert!(c.supported_regions.contains(RegionId::RightSidebar));
        assert!(c.movable);
        assert!(c.collapsible);
    }

    #[test]
    fn build_produces_the_real_workspace_tree_and_registers_host_ids() {
        // The seam builds the *same* body the bespoke path builds: a column with one
        // child per workspace, and — the part a placeholder could never fake — the
        // drag and hint ids the interactive rows need, allocated in the host's
        // registries.
        let p = WorkspacesContainerProvider::new();
        let tree = tree();
        let theme = GuiTheme::default();
        let programs = heca_config::programs::ProgramsConfig::default();
        let emit: ChromeIntentEmitter = Rc::new(|_| {});
        let store = store();

        let ctx = ChromeCtx::for_build(
            crate::host::App::new(&store),
            &tree,
            &programs,
            &theme,
            &emit,
        );
        let c = container(&p, &ctx);

        let mut signals = ChromeSignals::default();
        let mut drag = DragItemRegistry::default();
        let mut hints = HintTargetRegistry::default();
        let mut bx = BuildCx::new(&mut signals, &mut drag, &mut hints);
        let body = (c.build)(&ctx, &mut bx);

        assert_eq!(
            body.base().children.len(),
            1,
            "one workspace in the tree ⇒ one workspace dock in the body",
        );
        // Drag ids: the workspace (drop target), its column, its pane.
        assert_eq!(drag.items().len(), 3);
        // Hint targets: the pane card (focus pane) and the workspace dock (focus
        // workspace). A column has no target of its own — it only carries a pick-letter
        // signal, stamped when it is a *destination* for a move/swap.
        assert_eq!(hints.checkpoint(), 2);
        // The active pane's card bound its `active` signal for per-frame updates.
        assert_eq!(signals.pane_active.len(), 1);
    }

    #[test]
    fn build_outside_a_render_pass_yields_an_empty_body() {
        // An observe-only context has no tree/theme/emitter to project. Building is
        // still legal (a host may want the metadata) and must not panic.
        let p = WorkspacesContainerProvider::new();
        let store = store();
        let ctx = ChromeCtx::new(crate::host::App::new(&store));
        let c = container(&p, &ctx);

        let mut signals = ChromeSignals::default();
        let mut drag = DragItemRegistry::default();
        let mut hints = HintTargetRegistry::default();
        let mut bx = BuildCx::new(&mut signals, &mut drag, &mut hints);
        let body = (c.build)(&ctx, &mut bx);

        assert!(body.base().children.is_empty());
        assert!(drag.items().is_empty());
        assert_eq!(hints.checkpoint(), 0);
    }
}
