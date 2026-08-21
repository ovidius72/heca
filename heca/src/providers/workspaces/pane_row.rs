//! One **pane row** — the smallest piece of the workspaces dock (F006/P032/T429).
//!
//! It owns what a pane row *is*: its name and the program under it, its four states, its status
//! dots, its cwd and git lines, its menu, its drag identity and what `prefix+/` does to it. It owns
//! **nothing** about where it goes or how big it is — the parent gives it that (AGENTS.md § 0b-bis
//! step 1).
//!
//! # The four states, and the rule that keeps them right
//!
//! A row is simultaneously *selected*, *the cursor*, *last-visited* and *hovered*, and the four have
//! two different update paths. Everything that changes **with focus** must be a signal the host
//! flips in place, because `chrome_signature` deliberately excludes focus — the tree does not
//! rebuild when focus moves, so a bool read at build time freezes for the session. That single
//! mistake produced three bugs in one day (F003/P082/T426), none of which any test could see.
//!
//! So: the initial value goes to the widget, and the widget's signal goes to
//! [`ChromeSignals`](crate::chrome::ChromeSignals) in the same breath, a line apart. Adding a fifth
//! state means adding both lines, and this is the file where that is visible.

use super::seams::{DockRegistries, DockSeams};
use super::{pane_key, pane_row_items, pane_row_press, row_hint, PaneEntry, MENU_PANE};
use crate::chrome::{
    alpha_u8, home_relative_path, pane_info_view, runtime_snapshot, truncate_sidebar_git_branch,
    ChromeDragItem, PaneInfoSignals, RepaintWatch, CARD_META_FONT_SCALE,
};
use heca_core::runtime::ProcessStatus;
use heca_grid_ui::builders::{ComponentExt, LayoutExt, Parent, StyleExt};
use heca_grid_ui::reactive::signal;
use heca_grid_ui::style::{Align, Length};
use heca_grid_ui::widgets::{
    Flex, Glyph, HintPlacement, Icon, KeyHint, Label, Row, StatusDot, Tooltip, TooltipSide,
    Visibility,
};

/// A single pane **card**: a state-tinted background + radius, a leading program icon, the display
/// name, an optional exceptional-state indicator, and cwd/git metadata when present.
///
/// Properties are struct fields and the constructor is a struct literal — Flutter/SwiftUI named
/// parameters in Rust's spelling (AGENTS.md § 0b-bis step 2). This replaces an eleven-argument
/// positional `pane_card()` whose call sites could not be read without counting.
pub(crate) struct PaneRow<'a> {
    /// The pane this row draws, already reduced to plain data by `model.rs`.
    pub(crate) pane: &'a PaneEntry,
    /// Which column this pane sits in, or `None` for a **floating** pane — which is in no column,
    /// so its menu simply lacks the entries that need one rather than aiming them at a guess.
    pub(crate) column_of_pane: Option<(usize, usize)>,
}

impl PaneRow<'_> {
    /// Build the row, returning the concrete widget so the **parent** can still size it
    /// (§ 0b-bis step 5).
    pub(crate) fn build(self, seams: &DockSeams<'_>, reg: &mut DockRegistries<'_>) -> RepaintWatch {
        let PaneRow {
            pane,
            column_of_pane,
        } = self;
        let theme = seams.theme;
        let ws_state = seams.ws_state;
        let active = seams.active_pane == Some(pane.pane_id);
        let pane_id = pane.pane_id;
        // Read off this row's own projection rather than back through the store: the projection
        // was built from it, and the menu is about the row being drawn.
        let has_custom_name = pane.custom_name.is_some();
        let runtime = runtime_snapshot(ws_state, pane_id);
        let info = pane_info_view(
            seams.programs,
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
        let drag_id = reg.drag.register(ChromeDragItem::Pane(pane_id));
        // This row kind's declared click, by name — so a menu entry, a keybinding or RPC can fire the
        // same one (F003/P086/T365). A **pick** is declared separately, on the wrapper below: it is a
        // different gesture and this row answers it differently.
        let press = crate::chrome::fires(seams.mount, pane_row_press(pane_id), seams.emit);
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
        let running_dot =
            Visibility::new(StatusDot::online(), info.status == ProcessStatus::Running);
        let running_dot_visible = running_dot.visible_signal();
        let success_dot =
            Visibility::new(StatusDot::online(), info.status == ProcessStatus::Success);
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
                .child(
                    Icon::new(Glyph::Warning)
                        .size(12.0)
                        .color(theme.colors.warning),
                )
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
                .child(
                    Icon::new(Glyph::GitBranch)
                        .size(12.0)
                        .color(theme.colors.warning),
                )
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
        // **One shape, shared with the exposé's card** (`components::FolderLine`): the map and the
        // dock must not describe the same pane two different ways. The signals come back because a
        // cwd changes without a rebuild — a `cd` updates the path in place, and
        // `[settings] pane_show_cwd` turns the line on and off the same way.
        let cwd_path = runtime.as_ref().and_then(|rt| rt.cwd.clone());
        let cwd_text = cwd_path.as_deref().map(home_relative_path);
        let cwd = crate::components::FolderLine {
            path: cwd_text.as_deref(),
            show: ws_state.pane_show_cwd(),
            font_scale: CARD_META_FONT_SCALE,
            theme,
        }
        .build();
        let show_cwd = ws_state.pane_show_cwd() && cwd_text.is_some();
        let (cwd_row, cwd_signal, cwd_visible_signal) = (cwd.widget, cwd.text, cwd.visible);
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
            .radius(theme.colors.control_radius())
            .padding(6.0)
            // No bar: the column's `MarkerGroup` already draws one down the left of every row here,
            // and the selected panel says which row is current. Two lines said it twice.
            .active(active)
            .nav_selected(false)
            // The row's ONE identity: the cursor and (later) drag read this single declaration
            // (F003/P085/T354). **The right-click no longer does** — the menu is declared below, on
            // this widget, so a row that forgets `key` still opens its menu (F004/P084/T395).
            .key(pane_key(pane_id))
            // **This row's menu, built where this row's data is.** No `context_path`, no registered
            // builder, no menu-id string: the entries capture `pane_id` from the loop that is already
            // drawing it. Built at trigger time, so "Use process name" appears exactly when there is a
            // custom name to clear.
            .context_menu({
                let ctx_menu = crate::chrome::context_menu::menu_from_items(
                    "Pane",
                    "What you can do with this pane",
                    MENU_PANE,
                    pane_row_items(pane_id, column_of_pane, has_custom_name),
                    seams.catalog,
                    seams.emit,
                );
                heca_grid_ui::widgets::ContextMenu::new(MENU_PANE).child(ctx_menu)
            })
            .draggable(drag_id)
            .drop_target(drag_id)
            // Click / Enter / Space run the row's declared click.
            .on_activate(press)
            .child(content);
        // **Every state that moves with focus, published in the same breath as its initial value.**
        // See this module's note: a bool read at build time freezes, because the tree does not
        // rebuild on a focus change.
        reg.signals.pane_active.push((pane_id, card.state()));
        reg.signals
            .pane_previous
            .push((pane_id, card.previous_state()));
        reg.signals.row_nav.push((
            seams.mount.to_string(),
            pane_key(pane_id),
            card.nav_state(),
        ));
        // Wrap the card in a universal `KeyHint` so a move/swap/take pick can stamp this pane's
        // letter over it. `KeyHint` is transparent — it hugs the child and routes events, focus and
        // drag straight through — so the card stays a drag source and target, and clickable. **The
        // letter is offered by key** (`chrome::hint`) and drawn by the widget itself.
        reg.signals.pane_info.push((
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
                // **What `prefix+/` does to this row**, declared right where its letter is drawn: move
                // the cursor here and bring the pane to the front, staying in the sidebar. Nothing is
                // registered and no id leaves this line — which is the only reason a plugin's row could
                // ever have the same picker (RULE ZERO, F004/P084/T399).
                .on_hint(crate::chrome::fires(
                    seams.mount,
                    row_hint(pane_key(pane_id)),
                    seams.emit,
                ))
                .placement(HintPlacement::CenterRight),
        );
        watch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::testing::{self, Fixture};
    use heca_core::layout::PaneId;

    /// **The bug class this whole split exists for** (F003/P082/T426, three rounds).
    ///
    /// A row's states change with focus, and `chrome_signature` deliberately excludes focus — so
    /// the tree does **not** rebuild when focus moves. A state published as a plain bool at build
    /// time is therefore frozen for the session, and looks perfectly correct in the source.
    ///
    /// Every state a pane row owns must appear in `ChromeSignals` for that pane. Adding a fifth
    /// state without publishing it fails here, which is the only place it can fail.
    #[test]
    fn a_pane_row_publishes_every_state_that_moves_with_focus() {
        let mut fx = Fixture::default();
        let pane = testing::pane(PaneId(7), "seven");
        {
            let (seams, mut reg) = fx.split("left");
            let _ = PaneRow {
                pane: &pane,
                column_of_pane: Some((0, 0)),
            }
            .build(&seams, &mut reg);
        }

        assert!(
            fx.signals.pane_active.iter().any(|(p, _)| *p == PaneId(7)),
            "selected is a signal, so focus can flip it without a rebuild",
        );
        assert!(
            fx.signals.pane_previous.iter().any(|(p, _)| *p == PaneId(7)),
            "last-visited is a signal too — this is the one that was read at build time and froze",
        );
        assert!(
            fx.signals
                .row_nav
                .iter()
                .any(|(m, k, _)| m == "left" && k == &pane_key(PaneId(7))),
            "the cursor outline is per placement, so it is keyed by mount",
        );
        // The pick letter is **not** a signal this component registers any more: it is offered by
        // the row's own `key` (`chrome::hint`) and drawn by the widget, so what this component
        // owes is the identity — asserted above through `row_nav` — and nothing else.
    }

    /// A row declares the one identity everything else reads — the cursor, the drag, the
    /// right-click. A floating pane declares it exactly like a tiled one.
    #[test]
    fn a_floating_pane_is_a_row_with_no_column_not_a_second_row_shape() {
        let mut fx = Fixture::default();
        let pane = testing::pane(PaneId(9), "float");
        let row = {
            let (seams, mut reg) = fx.split("left");
            PaneRow {
                pane: &pane,
                column_of_pane: None,
            }
            .build(&seams, &mut reg)
        };

        assert!(
            testing::declared_keys(&row).contains(&pane_key(PaneId(9))),
            "a float carries the same identity a tiled card does",
        );
    }
}
