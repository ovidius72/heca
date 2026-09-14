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
    ChromeDragItem, RepaintWatch, CARD_META_FONT_SCALE,
};
use heca_core::runtime::ProcessStatus;
use heca_grid_ui::builders::{ComponentExt, LayoutExt, Parent, StyleExt};
use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate, create_effect, signal};
use heca_grid_ui::style::{Align, Spacing};
use heca_grid_ui::widgets::{
    Flex, Glyph, HintPlacement, Icon, Label, Row, StatusDot, Tooltip, TooltipSide, Visibility,
};

/// **How far a metadata line sits in from the name above it.** One token, read by every line under
/// the name, so a third line steps in with the others instead of picking its own number.
///
/// It is applied by each line to *itself*, inside its own visibility — never as a wrapper the card
/// puts around it. A wrapper does not disappear when the line hides, so a card with nothing to say
/// about its directory would keep an empty row and the gap above it.
const META_INDENT: Spacing = Spacing::Xs;

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
        // The card is both a drag source and a drop target, and what it drags is the name it
        // declares below (`pane:7`) — one identity for the cursor, the right-click, and this. The
        // registry records only what that name *means*, which is the component's own knowledge.
        reg.drag
            .register(pane_key(pane_id), ChromeDragItem::Pane(pane_id));
        // This row kind's declared click, by name — so a menu entry, a keybinding or RPC can fire the
        // same one (F003/P086/T365). A **pick** is declared separately, on the wrapper below: it is a
        // different gesture and this row answers it differently.
        let press = crate::chrome::fires(seams.mount, pane_row_press(pane_id), seams.emit);
        let icon_widget = Icon::new(info.icon).size(14.0).color(theme.colors.foreground);
        let icon_signal = icon_widget.glyph_signal();
        // The pane's name — ONE label, not one per colour. Its colour follows the row's selected
        // state through the content colour the `Row` publishes each paint, which unstyled labels
        // inherit; a caller that wants its own says so with `.color(..)`.
        //
        // It used to be built twice, once accent and once foreground, stacked in a column with one
        // hidden — so the visible name sat at the top of that stack when selected and the bottom
        // when not, and never shared a line with the `(program)` suffix beside it (F003/P082/T480).
        // And it cuts rather than spilling when the sidebar is narrow (`components::PaneName`).
        let name = crate::components::PaneName {
            text: &info.title,
            color: None,
            bold: true,
            font_scale: 1.0,
            theme,
        }
        .build();
        let title_label = name.widget;
        let title_signal = name.text;
        // **One pip that changes what it says**, not one pip per state hidden behind the others.
        // The row is retained, so the host rewrites the status signal when a process changes
        // (F003/P096/T483).
        let status_dot = StatusDot::new(dot_status(info.status));
        let status_signal = status_dot.status_signal();
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
                .pad_x(META_INDENT)
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
        // Smaller than the name it follows, and `foreground` — NOT `muted`, which is unreadable
        // against the row's selected fill on this theme. The size difference and the parentheses
        // are what make it read as secondary. The two runs sit on one shared baseline because
        // `Flex` puts them there; a row of text at two sizes is the framework's problem, not this
        // call site's.
        let process_hint_label = Label::new(process_hint_text)
            .color(theme.colors.foreground)
            .font_scale(CARD_META_FONT_SCALE);
        let process_hint_signal = process_hint_label.text_signal();
        let process_hint = Visibility::new(process_hint_label, info.process_hint.is_some());
        let process_hint_visible = process_hint.visible_signal();
        // The name and the dimmed `(program)` suffix are two runs on one line, centred on each
        // other — CSS's `align-items: center`. Measured: the name's box is 18 tall and the
        // suffix's 14, and centring puts both mid-lines on the same pixel.
        let title_area = Flex::row()
            .align(Align::Center)
            .gap_spacing(heca_grid_ui::style::Spacing::Sm)
            .child(title_label)
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
            indent: META_INDENT,
            theme,
        }
        .build();
        let (cwd_row, cwd_signal, cwd_visible_signal) = (cwd.widget, cwd.text, cwd.visible);
        // The pane's identity row (status dots + program icon + name) — shared by every card
        // layout so the cwd and git rows just stack beneath it in one column.
        let name_row = Flex::row()
            .align(Align::Center)
            .gap(8.0)
            .child(status_dot)
            .child(Flex::row().align(Align::Center).child(icon_widget))
            .child(title_area);
        // One column: the name row, then the metadata lines under it.
        //
        // **Every line is attached, always** — each one is a `Visibility` that decides for itself,
        // and hidden is `display: none`, so a card with nothing to say lays out exactly like the
        // one-row card and spends no gap on what it is not showing.
        //
        // Attaching a line only when it *already* had something to say is the bug this shape
        // exists to stop: a directory arrives from the shell after the row is on screen and the
        // sidebar tree is not rebuilt when it does, so a line left out at build time could never
        // be revealed by its own signal. Whether a pane showed its path came down to whether the
        // shell had answered by the instant that row was built. The git line was
        // written the same way and only looked right because `chrome_signature` carried a term for
        // it — a second copy of the rule, in a file nobody adding a line would think to open.
        let content = Flex::column()
            .gap(4.0)
            .grow(1.0)
            .child(name_row)
            .child(cwd_row)
            .child(git_row);
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
            // **What a CLICK does to this row: put the container's cursor on it.** Declared
            // here, where the row already knows which row it is, instead of being recovered
            // afterwards by hit-testing a rectangle (AGENTS.md § 0c — reach for a handler).
            //
            // Two intents, in order, because they are two acts: focus the dock, then move its
            // cursor. Events are queued and processed in order, so the dock is
            // `Domain::Container` by the time `cursor_to` runs, which is what its
            // `ActionPolicy::ContainerFocused` requires. Same shape as a pane-header button
            // that emits `FocusPane` before an action aimed at the focused pane.
            //
            // It does NOT activate the row — a click on the card already does that through the
            // card's own `on_activate`. A pick is a third, separate declaration — the `on_hint`
            // below — because a click, an activation and a pick are three different acts.
            .on_click({
                let emit = seams.emit.clone();
                let mount = seams.mount.to_string();
                let key = pane_key(pane_id);
                move |_| {
                    emit.fire(crate::app::interaction::InteractionIntent::ActivateAction(
                        crate::input::WmAction::FocusDock {
                            dock: Some(mount.clone()),
                        },
                    ));
                    emit.fire(crate::app::interaction::InteractionIntent::ActivateAction(
                        crate::input::WmAction::CursorTo {
                            mount: mount.clone(),
                            key: key.clone(),
                        },
                    ));
                }
            })
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
            // A pane card is a pane, and it takes panes. A column dragged over it is refused
            // before anything is drawn, so no line appears where a release would do nothing.
            .draggable_as("pane")
            .accepts(["pane"])
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
        // A move/swap/take pick stamps this pane's letter over the card. **The letter is offered
        // by key** (`chrome::hint`) and drawn by `paint_child` on the card itself — which is why
        // there is no wrapper: the card is a widget, so it can simply say so.
        // **The row follows its pane. Nothing writes to it.**
        //
        // These sixteen signals used to be handed to the host, which walked every pane every frame,
        // recomputed this same view and wrote them back — a scan pretending to be reactivity. That
        // cost every new piece of pane state four edits in four files, two of which fail silently
        // when forgotten, and left a plugin's row with nowhere to join in: the write was a
        // host-private arm, so there was no line a plugin author could write.
        //
        // Now the row subscribes, here, where it knows what it draws. `snapshot()` reads this
        // pane's fields, so the subscription depends on this pane and not on the map holding every
        // pane's — a neighbour's `cd` does not wake this row.
        let face = PaneFace {
            icon: icon_signal,
            title: title_signal,
            process_hint: process_hint_signal,
            process_hint_visible,
            cwd: cwd_signal,
            cwd_visible: cwd_visible_signal,
            status: status_signal,
            git_visible: git_visible_signal,
            git_branch: branch_signal,
            git_branch_display: branch_display_signal,
            git_added_visible: add_text_visible_signal,
            git_added: add_label,
            git_modified_visible: modified_text_visible_signal,
            git_modified: modified_label,
            git_deleted_visible: deleted_text_visible_signal,
            git_deleted: deleted_label,
        };
        let runtime_signals = ws_state.pane_runtime_signals(pane_id);
        let programs = seams.programs.clone();
        let fallback_name = pane.name.clone();
        let show_cwd = ws_state.pane_show_cwd_signal();
        let add_process_name = ws_state.pane_renamed_add_process_name_signal();
        create_effect(move |_| {
            let runtime = runtime_signals.snapshot();
            let custom = runtime_signals.custom_name.get();
            face.show(
                &pane_info_view(
                    &programs,
                    &fallback_name,
                    custom.as_deref(),
                    Some(&runtime),
                    add_process_name.get(),
                ),
                runtime.cwd.as_deref(),
                show_cwd.get(),
            );
        });
        let (watch, _repaint) = RepaintWatch::new(
            // **The card says it about itself** — no `KeyHint` wrapper, which is for a region that
            // is not a widget you can put a builder on.
            //
            // **What `prefix+/` does to this row**, declared right where its letter is drawn: move
            // the cursor here and bring the pane to the front, staying in the sidebar. Nothing is
            // registered and no id leaves this line — which is the only reason a plugin's row could
            // ever have the same picker (RULE ZERO).
            card.on_hint(seams.picks(row_hint(pane_key(pane_id))))
                .hint_placement(HintPlacement::CenterRight),
        );
        watch
    }
}

/// **Everything a pane row shows that changes without the tree being rebuilt.**
///
/// One signal per thing on the card, held together so the row's subscription writes them in one
/// place — and so adding a seventeenth is one field and one line in [`show`](PaneFace::show),
/// beside the widget it belongs to, instead of an edit in a host file the author never opens.
#[derive(Clone, Copy)]
pub(crate) struct PaneFace {
    pub(crate) icon: Signal<Glyph>,
    /// The pane's name. **One signal, because there is one name** — its colour follows the row's
    /// selected state through the inherited content colour, so no second copy exists to keep in
    /// step.
    pub(crate) title: Signal<String>,
    /// The dimmed `(process)` suffix beside a renamed pane's name, or empty when hidden.
    pub(crate) process_hint: Signal<String>,
    pub(crate) process_hint_visible: Signal<bool>,
    /// The working-directory line: the home-relative path, and whether it shows at all
    /// (`[settings] pane_show_cwd` and the pane having a directory).
    pub(crate) cwd: Signal<String>,
    pub(crate) cwd_visible: Signal<bool>,
    /// What the status pip shows. **One pip that changes what it says**, not one pip per state
    /// with four booleans revealing one.
    pub(crate) status: Signal<heca_grid_ui::DotStatus>,
    pub(crate) git_visible: Signal<bool>,
    pub(crate) git_branch: Signal<String>,
    pub(crate) git_branch_display: Signal<String>,
    pub(crate) git_added_visible: Signal<bool>,
    pub(crate) git_added: Signal<String>,
    pub(crate) git_modified_visible: Signal<bool>,
    pub(crate) git_modified: Signal<String>,
    pub(crate) git_deleted_visible: Signal<bool>,
    pub(crate) git_deleted: Signal<String>,
}

impl PaneFace {
    /// Write what the pane currently is onto the card.
    ///
    /// Every write is change-guarded: a signal set to what it already holds would mark the tree
    /// dirty and cost a repaint for nothing, every time anything about any pane moved.
    fn show(
        &self,
        view: &crate::chrome::pane_header::PaneInfoView,
        cwd: Option<&std::path::Path>,
        show_cwd: bool,
    ) {
        set_if_changed(self.icon, view.icon);
        set_if_changed(self.title, view.title.clone());
        set_if_changed(
            self.process_hint,
            view.process_hint
                .as_deref()
                .map(|program| format!("({program})"))
                .unwrap_or_default(),
        );
        set_if_changed(self.process_hint_visible, view.process_hint.is_some());
        // The path is cut to a home-relative form here, at the binding, rather than by whoever
        // happens to write the signal — so there is one spelling of "where this pane is".
        set_if_changed(
            self.cwd,
            cwd.map(crate::chrome::home_relative_path)
                .unwrap_or_default(),
        );
        set_if_changed(self.cwd_visible, show_cwd && cwd.is_some());
        set_if_changed(self.status, dot_status(view.status.clone()));
        set_if_changed(self.git_visible, view.git_branch.is_some());
        let branch = view.git_branch.clone().unwrap_or_default();
        set_if_changed(
            self.git_branch_display,
            crate::chrome::truncate_sidebar_git_branch(&branch),
        );
        set_if_changed(self.git_branch, branch);
        for (visible, label, value) in [
            (self.git_added_visible, self.git_added, &view.git_added),
            (
                self.git_modified_visible,
                self.git_modified,
                &view.git_modified,
            ),
            (
                self.git_deleted_visible,
                self.git_deleted,
                &view.git_deleted,
            ),
        ] {
            set_if_changed(visible, value.is_some());
            set_if_changed(label, value.clone().unwrap_or_default());
        }
    }
}

/// Write a signal only when the value actually differs.
///
/// Stated once rather than at each of the sixteen writes above: setting a signal to what it already
/// holds marks the tree dirty, and a card that rewrote itself unchanged would cost a repaint every
/// time anything about any pane moved.
fn set_if_changed<T: Clone + PartialEq + 'static>(signal: Signal<T>, value: T) {
    if signal.get_untracked() != value {
        signal.set(value);
    }
}

/// **What a running process looks like as a pip.** The app's process states and the library's dot
/// states are two vocabularies — one about a shell, one about a colour — and this is the single
/// place they meet, so a new process state is mapped once rather than wherever a dot is built.
pub(crate) fn dot_status(status: ProcessStatus) -> heca_grid_ui::DotStatus {
    match status {
        ProcessStatus::Running => heca_grid_ui::DotStatus::Online,
        ProcessStatus::Success => heca_grid_ui::DotStatus::Online,
        ProcessStatus::Error => heca_grid_ui::DotStatus::Error,
        ProcessStatus::Idle => heca_grid_ui::DotStatus::Offline,
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

    /// **A metadata line that had nothing to say when the row was built must still be able to
    /// speak later.**
    ///
    /// A pane's directory arrives from its shell after the row is on screen, and the sidebar tree
    /// is not rebuilt when it does — the path and its visibility are signals for exactly that
    /// reason. A line attached only when it already had something to show can never be revealed by
    /// its own signal, so whether a pane showed its path came down to whether the shell had
    /// answered by the instant that row was built: neighbouring rows, same program, one with a
    /// path line and one without.
    #[test]
    fn a_directory_that_arrives_after_the_row_was_built_still_shows() {
        let mut fx = Fixture::default();
        fx.store.workspaces.set_pane_show_cwd(true);
        // The shell has not reported a directory yet — the state that decided, wrongly, whether
        // the line existed at all.
        fx.store.workspaces.set_pane_runtime(
            PaneId(7),
            &heca_core::runtime::PaneRuntime::default(),
            None,
        );
        let pane = testing::pane(PaneId(7), "seven");
        let row = {
            let (seams, mut reg) = fx.split("left");
            PaneRow {
                pane: &pane,
                column_of_pane: Some((0, 0)),
            }
            .build(&seams, &mut reg)
        };

        // The directory arrives. Nothing rebuilds the tree and no sync pass runs — the row is
        // subscribed to its own pane, so telling the store is the whole of it.
        pane_reports(
            &fx,
            PaneId(7),
            heca_core::runtime::PaneRuntime {
                cwd: Some("/Users/antonio/projects/heca".into()),
                ..Default::default()
            },
        );

        assert!(
            painted_text(row).iter().any(|t| t == "~/projects/heca"),
            "the path arrived and the row is still not drawing it",
        );
    }

    /// Tell the store what the pane's shell just reported — the one write the app makes, and
    /// the only thing a row should need in order to change what it shows.
    fn pane_reports(fx: &Fixture, pane_id: PaneId, runtime: heca_core::runtime::PaneRuntime) {
        fx.store
            .workspaces
            .set_pane_runtime(pane_id, &runtime, None);
    }

    /// Lay the row out in a sidebar-sized box and return every run of text it actually drew.
    fn painted_text(row: RepaintWatch) -> Vec<String> {
        use heca_grid_ui::Component as _;
        let mut root = Flex::column().child(row);
        heca_grid_ui::LayoutEngine::new()
            .compute(&mut root, heca_core::layout::Size::new(300.0, 200.0));
        let mut scene = heca_grid_ui::Scene::new();
        let theme = heca_grid_ui::theme::Theme::default();
        root.paint(&mut heca_grid_ui::PaintCx::new(&mut scene, &theme));
        scene
            .iter()
            .filter_map(|cmd| match cmd {
                heca_grid_ui::DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    }

    /// The card's inner column — the name row plus every metadata line stacked under it.
    fn metadata_column(node: &dyn heca_grid_ui::Component) -> Option<&dyn heca_grid_ui::Component> {
        if node.base().children.len() == METADATA_COLUMN_CHILDREN {
            return Some(node);
        }
        node.base()
            .children
            .iter()
            .find_map(|c| metadata_column(c.as_ref()))
    }

    /// The name row, the directory line and the git line.
    const METADATA_COLUMN_CHILDREN: usize = 3;

    /// **The git line is the same line, and it was written the same way.** It only ever looked
    /// right because `chrome_signature` carried a term saying "this pane has a branch", forcing the
    /// rebuild that attached it — the rule written twice, once in the card and once in a file
    /// nobody adding a metadata line would open. With the line attached always, the term is gone
    /// and a branch appearing reveals it the way the directory does.
    #[test]
    fn a_branch_that_arrives_after_the_row_was_built_still_shows() {
        let mut fx = Fixture::default();
        let pane = testing::pane(PaneId(7), "seven");
        let row = {
            let (seams, mut reg) = fx.split("left");
            PaneRow {
                pane: &pane,
                column_of_pane: Some((0, 0)),
            }
            .build(&seams, &mut reg)
        };

        pane_reports(
            &fx,
            PaneId(7),
            heca_core::runtime::PaneRuntime {
                git: Some(heca_core::runtime::GitInfo {
                    branch: Some("main".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );

        assert!(
            painted_text(row).iter().any(|t| t == "main"),
            "the branch arrived and the row is still not drawing it",
        );
    }

    /// **A line with nothing to say costs nothing** — not its own height, and not the gap above it.
    ///
    /// This is the risk in attaching every line always, and it is the one worth guarding: an empty
    /// strip under half the rows would be a worse defect than the one being fixed. `Visibility`
    /// hides with `display: none`, so the layout skips the line *and* the gap that would have
    /// preceded it — which is why the inset each line applies has to live inside that visibility
    /// rather than in a wrapper around it.
    #[test]
    fn a_card_with_nothing_to_show_is_exactly_as_tall_as_its_name() {
        let mut fx = Fixture::default();
        let pane = testing::pane(PaneId(7), "seven");
        let row = {
            let (seams, mut reg) = fx.split("left");
            PaneRow {
                pane: &pane,
                column_of_pane: Some((0, 0)),
            }
            .build(&seams, &mut reg)
        };

        let mut root = Flex::column().child(row);
        heca_grid_ui::LayoutEngine::new()
            .compute(&mut root, heca_core::layout::Size::new(300.0, 200.0));

        let content = metadata_column(&root).expect("the card stacks a name and its metadata");
        let name_row = &content.base().children[0];
        assert_eq!(
            content.base().bounds.size.h,
            name_row.base().bounds.size.h,
            "the two hidden lines, and the gaps above them, took height from a card with \
             nothing to say",
        );
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
