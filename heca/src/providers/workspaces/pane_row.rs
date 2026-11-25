//! One **pane row** — the smallest piece of the workspaces dock.
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
//! mistake produced three bugs in one day, none of which any test could see.
//!
//! So: the initial value goes to the widget, and the widget's signal goes to
//! [`ChromeSignals`](crate::chrome::ChromeSignals) in the same breath, a line apart. Adding a fifth
//! state means adding both lines, and this is the file where that is visible.

use super::seams::{DockRegistries, DockSeams};
use super::{MENU_PANE, PaneEntry, pane_key, pane_row_items, pane_row_press, row_hint};
use crate::chrome::panes::pane_items::LineCx;
use crate::chrome::{ChromeDragItem, PaneFacts, pane_info_view};
use heca_core::runtime::ProcessStatus;
use heca_grid_ui::RowVariant;
use heca_grid_ui::builders::{
    DragExt, EventExt, FlashExt, HintExt, IdentityExt, LayoutExt, MenuExt, Parent,
};
use heca_grid_ui::reactive::SignalGet;
use heca_grid_ui::style::Spacing;
use heca_grid_ui::widgets::{Flex, HintPlacement, Row, StatusDot, Tile};
use std::rc::Rc;

/// **How far a metadata line sits in from the name above it.** One token, read by every line under
/// the name, so a third line steps in with the others instead of picking its own number.
///
/// It is applied by each line to *itself*, inside its own visibility — never as a wrapper the card
/// puts around it. A wrapper does not disappear when the line hides, so a card with nothing to say
/// about its directory would keep an empty row and the gap above it.
pub(super) const META_INDENT: Spacing = Spacing::Xs;

/// A single pane **card**: a state-tinted background + radius, a leading program icon, the display
/// name, an optional exceptional-state indicator, and cwd/git metadata when present.
///
/// Written in THE SYNTAX: `PaneRow::new(pane).column(Some((ws, col))).build(seams, reg)`.
pub(crate) struct PaneRow<'a> {
    pane: &'a PaneEntry,
    column_of_pane: Option<(usize, usize)>,
}

impl<'a> PaneRow<'a> {
    /// The row for `pane`, already reduced to plain data by `model.rs`. Floating until told its
    /// column.
    pub(crate) fn new(pane: &'a PaneEntry) -> Self {
        Self {
            pane,
            column_of_pane: None,
        }
    }

    /// Which column this pane sits in, as `(ws_idx, col_idx)`, or `None` for a **floating** pane —
    /// which is in no column, so its menu simply lacks the entries that need one rather than
    /// aiming them at a guess.
    pub(crate) fn column(mut self, column: Option<(usize, usize)>) -> Self {
        self.column_of_pane = column;
        self
    }
    /// Build the row, returning the concrete widget so the **parent** can still size it
    /// (§ 0b-bis step 5).
    pub(crate) fn build(self, seams: &DockSeams<'_>, reg: &mut DockRegistries<'_>) -> Row {
        let PaneRow {
            pane,
            column_of_pane,
        } = self;
        let ws_state = seams.ws_state;
        let pane_id = pane.pane_id;
        // Read off this row's own projection rather than back through the store: the projection
        // was built from it, and the menu is about the row being drawn.
        let has_custom_name = pane.custom_name.is_some();
        // A constant theme-driven card; the selected outline and light wash
        // is drawn by `Row` from its `active` signal, not baked into the background. This
        // keeps styling fully signal-driven (active flips in place via `sync_chrome_signals`,
        // no tree rebuild) and theme-driven (no ad-hoc per-state alphas).
        // The card is both a drag source and a drop target, and what it drags is the name it
        // declares below (`pane:7`) — one identity for the cursor, the right-click, and this. The
        // registry records only what that name *means*, which is the component's own knowledge.
        reg.drag
            .register(pane_key(pane_id), ChromeDragItem::Pane(pane_id));
        // This row kind's declared click, by name — so a menu entry, a keybinding or RPC can fire the
        // same one. A **pick** is declared separately, on the wrapper below: it is a
        // different gesture and this row answers it differently.
        // No colour of its own: it inherits the readable foreground the row hands its content at
        // paint, so a theme change reaches it.
        // **The title line's parts, worked out from the pane** — the pip, the icon, the name (it cuts
        // rather than spilling, `components::PaneName`) and the dimmed `(process)` suffix of a
        // renamed pane (config-gated, `[settings] pane_renamed_add_process_name`). Every part reads
        // them through its own builder; `snapshot()` reads this pane's fields, so a neighbour's `cd`
        // does not wake this row.
        let head = {
            let runtime_signals = seams.panes.signals(pane_id);
            let programs = seams.programs.clone();
            let fallback_name = pane.name.clone();
            let add_process_name = ws_state.pane_renamed_add_process_name_signal();
            crate::chrome::panes::pane_items::RowHead::new(move || {
                let runtime = runtime_signals.presentation();
                let custom = runtime_signals.custom_name.get();
                head_view(&pane_info_view(
                    &programs,
                    &fallback_name,
                    custom.as_deref(),
                    Some(&runtime),
                    add_process_name.get(),
                ))
            })
        };
        // **What is true of this pane, as a function** — read inside each line's own subscription, so
        // reading it is what makes a line follow the pane. The card's lines (`title`, `cwd`, `git`, and
        // any a program built on heca adds) are named items built from it; the view's list decides
        // which show and in what order.
        let facts: Rc<dyn Fn() -> PaneFacts> = {
            let runtime_signals = seams.panes.signals(pane_id);
            let programs = seams.programs.clone();
            let fallback_name = pane.name.clone();
            Rc::new(move || {
                let runtime = runtime_signals.presentation();
                let custom = runtime_signals.custom_name.get();
                PaneFacts::of(
                    pane_id,
                    &programs,
                    &fallback_name,
                    custom.as_deref(),
                    Some(&runtime),
                )
            })
        };
        let line_cx = LineCx {
            head,
            facts,
            agent_statuses: ws_state.agent_statuses,
            agent_session_line: ws_state.agent_session_line,
        };
        let runtime_signals = seams.panes.signals(pane_id);
        let flash = runtime_signals.flash;
        // **The card's lines are the view's** — `[appearance.sidebar] pane_<view>_view`, or
        // `pane_current_view` while this is the current pane — each line by name, in order, the
        // title among them. The card follows the view and the focus and rebuilds its own lines;
        // nothing else is rebuilt. **Every line is attached while the list names it** — each is a
        // `Visibility` that decides for itself, so a directory the shell reports later still appears.
        //
        // **The pip sits on the title line**; in a view without one it goes in front of the first
        // line, so a card always says whether its program is alive.
        let lines = {
            let store = ws_state.clone();
            let line_cx = Rc::new(line_cx);
            heca_grid_ui::builders::BoundChildren::new(
                move || {
                    let current = store.active_pane() == Some(pane_id);
                    let lines = store.card_lines(current);
                    let titled = lines.names().any(|name| name == super::TITLE_LINE);
                    lines
                        .iter()
                        .enumerate()
                        .map(|(at, def)| CardLine {
                            def: def.clone(),
                            pip: at == 0 && !titled,
                        })
                        .collect::<Vec<_>>()
                },
                move |line: &CardLine| {
                    let built = (line.def.build)(&line_cx);
                    match line.pip {
                        true => heca_grid_ui::WidgetModel::new(
                            Tile::new()
                                .status(
                                    StatusDot::new(heca_grid_ui::DotStatus::default())
                                        .status(move || head.get().status),
                                )
                                .title(built),
                        ),
                        false => built,
                    }
                },
            )
        };
        let content = Flex::column().gap(Spacing::Xs).child(lines);
        let content = content.grow(1.0);
        let card = Row::new()
            .flash(flash)
            .variant(RowVariant::Tinted)
            // The selected panel identifies the current row; an extra marker would repeat it.
            // **The row follows focus itself.** Selected and last-visited are the store's signals
            // (`active_pane`, `previous_panes`): the row reads them through its own builders, so
            // nothing hands it a signal to write, the tree does not rebuild on a focus change, and
            // nobody else has to know this row exists to keep it right.
            .active({
                let store = ws_state.clone();
                move || store.active_pane() == Some(pane_id)
            })
            .previous({
                let store = ws_state.clone();
                move || store.is_previous_pane(pane_id) && store.active_pane() != Some(pane_id)
            })
            .nav_selected(false)
            // The row's ONE identity: the cursor and (later) drag read this single declaration.
            // **The right-click no longer does** — the menu is declared below, on
            // this widget, so a row that forgets `key` still opens its menu.
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
                let mount = seams.mount.to_string();
                let key = pane_key(pane_id);
                move |ev| {
                    use heca_view::PropValue::Text;
                    ev.dispatch(
                        crate::chrome::Intent::new("focus_dock").arg("dock", Text(mount.clone())),
                    );
                    ev.dispatch(
                        crate::chrome::Intent::new("cursor_to")
                            .arg("mount", Text(mount.clone()))
                            .arg("key", Text(key.clone())),
                    );
                }
            })
            // **This row's menu, built where this row's data is.** No `context_path`, no registered
            // builder, no menu-id string: the entries capture `pane_id` from the loop that is already
            // drawing it. Built at trigger time, so "Use process name" appears exactly when there is a
            // custom name to clear.
            .context_menu({
                let ctx_menu = crate::chrome::surfaces::context_menu::menu_from_items(
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
            // The card's own press, named — seated on this dock by the dock's listener. Not
            // `on_press`: the row's pick is its own declaration (`on_hint` below).
            .on_click(move |ev| ev.dispatch(pane_row_press(pane_id)))
            .child(content);
        // A move/swap/take pick stamps this pane's letter over the card. **The letter is offered
        // by key** (`chrome::hint`) and drawn by `paint_child` on the card itself — which is why
        // there is no wrapper: the card is a widget, so it can simply say so.
        // **The card says it about itself** — no `KeyHint` wrapper, which is for a region that is
        // not a widget you can put a builder on.
        //
        // **What `prefix+/` does to this row**, declared right where its letter is drawn: move the
        // cursor here and bring the pane to the front, staying in the sidebar. Nothing is
        // registered and no id leaves this line, which is what lets a plugin's row have the same
        // picker.
        card.on_hint(seams.picks(row_hint(pane_key(pane_id))))
            .hint_placement(HintPlacement::CenterRight)
    }
}

/// **One line of a pane card**: which line, and whether the pip goes in front of it (the first line
/// of a view without a title). Equal when it is the same line in the same place — what lets a card
/// keep a line it already built.
#[derive(Clone)]
struct CardLine {
    def: Rc<crate::chrome::panes::pane_items::PaneLineDef>,
    pip: bool,
}

impl PartialEq for CardLine {
    fn eq(&self, other: &Self) -> bool {
        self.def.name == other.def.name && self.pip == other.pip
    }
}

/// **What the title line shows, from what the pane is.**
fn head_view(
    view: &crate::chrome::panes::pane_header::PaneInfoView,
) -> crate::chrome::panes::pane_items::HeadView {
    crate::chrome::panes::pane_items::HeadView {
        title: view.title.clone(),
        icon: view.icon,
        status: dot_status(view.status.clone()),
        hint: view
            .process_hint
            .is_some()
            .then(|| view.process_hint_text()),
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
    use super::super::testing::{self, Fixture};
    use super::*;
    use heca_core::layout::PaneId;
    use heca_grid_ui::widgets::Flex;

    /// **The bug class this whole split exists for**.
    ///
    /// A row's states change with focus, and `chrome_signature` deliberately excludes focus — so
    /// the tree does **not** rebuild when focus moves. A state read as a plain bool at build time
    /// is therefore frozen for the session, and looks perfectly correct in the source.
    ///
    /// So the row follows the store's focus signals itself: change the active pane or the
    /// last-visited mark after the row is built, and the row follows — with nothing handed to the
    /// host to write back.
    #[test]
    fn a_pane_row_follows_every_state_that_moves_with_focus() {
        use heca_grid_ui::reactive::SignalGet;
        let mut fx = Fixture::default();
        let pane = testing::pane(PaneId(7), "seven");
        let row = {
            let (seams, mut reg) = fx.split("left");
            PaneRow::new(&pane)
                .column(Some((0, 0)))
                .build(&seams, &mut reg)
        };
        let (selected, previous) = (row.state(), row.previous_state());
        let store = fx.store.workspaces.clone();

        store.set_active_pane(Some(PaneId(7)));
        assert!(
            selected.get_untracked(),
            "selected follows focus without a rebuild"
        );
        store.set_active_pane(Some(PaneId(8)));
        assert!(!selected.get_untracked(), "and leaves when focus moves on");

        store.set_previous_panes(vec![Some(PaneId(7))]);
        assert!(
            previous.get_untracked(),
            "last-visited follows too — the one that was read at build time and froze",
        );
        store.set_active_pane(Some(PaneId(7)));
        assert!(
            !previous.get_untracked(),
            "the pane you are on is not \"where you were\""
        );

        assert_eq!(
            heca_grid_ui::Component::base(&row).key.as_deref(),
            Some(pane_key(PaneId(7)).as_str()),
            "the cursor outline and the pick letter both reach the row by its key, so the key is \
             all it owes — the dock's cursor lights it (`select_key_in_scope`)",
        );
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
        // The shell has not reported a directory yet — the state that decided, wrongly, whether
        // the line existed at all.
        fx.store
            .panes
            .set_runtime(PaneId(7), &heca_core::runtime::PaneRuntime::default(), None);
        let pane = testing::pane(PaneId(7), "seven");
        let row = {
            let (seams, mut reg) = fx.split("left");
            PaneRow::new(&pane)
                .column(Some((0, 0)))
                .build(&seams, &mut reg)
        };

        // The directory arrives. Nothing rebuilds the tree and no sync pass runs — the row is
        // subscribed to its own pane, so telling the store is the whole of it.
        pane_reports(
            &fx,
            PaneId(7),
            heca_core::runtime::PaneRuntime {
                cwd: Some(
                    dirs::home_dir()
                        .expect("a home folder")
                        .join("projects")
                        .join("heca"),
                ),
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
        fx.store.panes.set_runtime(pane_id, &runtime, None);
    }

    /// Lay the row out in a sidebar-sized box and return every run of text it actually drew.
    fn painted_text(row: Row) -> Vec<String> {
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

    /// A sidebar whose views are `compact = cwd`, `normal = title, cwd` and, when given, a current
    /// pane's list — a pane in a folder, its row built and held in a column for painting.
    fn viewed(current: Option<&[&str]>) -> (Fixture, Flex) {
        use heca_config::appearance::SidebarAppearance;
        let owned = |lines: &[&str]| lines.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut fx = Fixture::default();
        fx.store
            .workspaces
            .set_pane_views(
                crate::chrome::PaneRowLines::default().views(&SidebarAppearance {
                    pane_compact_view: owned(&["cwd"]),
                    pane_normal_view: owned(&["title", "cwd"]),
                    pane_current_view: current.map(owned),
                    ..SidebarAppearance::default()
                }),
            );
        fx.store
            .workspaces
            .set_pane_view(heca_config::appearance::PaneView::Normal);
        pane_reports(
            &fx,
            PaneId(7),
            heca_core::runtime::PaneRuntime {
                cwd: Some(dirs::home_dir().expect("a home folder").join("work")),
                ..Default::default()
            },
        );
        let pane = testing::pane(PaneId(7), "seven");
        let row = {
            let (seams, mut reg) = fx.split("left");
            PaneRow::new(&pane)
                .column(Some((0, 0)))
                .build(&seams, &mut reg)
        };
        (fx, Flex::column().child(row))
    }

    /// Lay `root` out and paint it.
    fn paint(root: &mut Flex) -> heca_grid_ui::Scene {
        use heca_grid_ui::Component as _;
        heca_grid_ui::LayoutEngine::new().compute(root, heca_core::layout::Size::new(300.0, 200.0));
        let mut scene = heca_grid_ui::Scene::new();
        let theme = heca_grid_ui::theme::Theme::default();
        root.paint(&mut heca_grid_ui::PaintCx::new(&mut scene, &theme));
        scene
    }

    fn says(scene: &heca_grid_ui::Scene, text: &str) -> bool {
        scene
            .iter()
            .any(|cmd| matches!(cmd, heca_grid_ui::DrawCommand::Text(t) if t.text == text))
    }

    /// **A card shows its view's lines, and follows a switch** — with no rebuild of the sidebar:
    /// the title is a line like the others, there in `normal` and gone in `compact`.
    #[test]
    fn a_card_shows_its_views_lines_and_follows_a_switch() {
        let (fx, mut root) = viewed(None);
        let scene = paint(&mut root);
        assert!(
            says(&scene, "seven") && says(&scene, "~/work"),
            "normal: title and folder"
        );
        fx.store
            .workspaces
            .set_pane_view(heca_config::appearance::PaneView::Compact);
        let scene = paint(&mut root);
        assert!(!says(&scene, "seven"), "compact: no title line");
        assert!(says(&scene, "~/work"), "compact: the folder");
    }

    /// **In a view without a title the pip goes in front of the first line**, so a card always says
    /// whether its program is alive.
    #[test]
    fn without_a_title_the_pip_leads_the_first_line() {
        let (fx, mut root) = viewed(None);
        fx.store
            .workspaces
            .set_pane_view(heca_config::appearance::PaneView::Compact);
        let scene = paint(&mut root);
        let path = scene
            .iter()
            .find_map(|cmd| match cmd {
                heca_grid_ui::DrawCommand::Text(t) if t.text == "~/work" => Some(t.rect),
                _ => None,
            })
            .expect("the folder line");
        let mid = |r: heca_core::layout::Rectangle| r.loc.y + r.size.h / 2.0;
        let pip = scene.iter().any(|cmd| {
            matches!(cmd, heca_grid_ui::DrawCommand::Rect(r)
                if r.rect.size.w < path.size.h
                    && r.rect.loc.x < path.loc.x
                    && (mid(r.rect) - mid(path)).abs() < 2.0)
        });
        assert!(pip, "a pip on the folder's line, before it");
    }

    /// **The current pane's card shows `pane_current_view`** — it moves with focus, and the other
    /// cards keep the view's lines.
    #[test]
    fn the_current_panes_card_shows_its_own_list() {
        let (fx, mut root) = viewed(Some(&["cwd"]));
        fx.store.workspaces.set_active_pane(Some(PaneId(7)));
        let scene = paint(&mut root);
        assert!(
            !says(&scene, "seven"),
            "current: its own list, without the title"
        );
        fx.store.workspaces.set_active_pane(Some(PaneId(8)));
        let scene = paint(&mut root);
        assert!(
            says(&scene, "seven"),
            "not current any more: the view's list again"
        );
    }

    /// The card's column — every line of the starting view, the title among them, stacked.
    fn metadata_column(node: &dyn heca_grid_ui::Component) -> Option<&dyn heca_grid_ui::Component> {
        use heca_config::appearance::{PaneView, default_view_lines};
        if node.base().children.len() == default_view_lines(PaneView::default()).len() {
            return Some(node);
        }
        node.base()
            .children
            .iter()
            .find_map(|c| metadata_column(c.as_ref()))
    }

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
            PaneRow::new(&pane)
                .column(Some((0, 0)))
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
            PaneRow::new(&pane)
                .column(Some((0, 0)))
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
            "the hidden lines, and the gaps above them, took height from a card with \
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
            PaneRow::new(&pane).build(&seams, &mut reg)
        };

        assert!(
            testing::declared_keys(&row).contains(&pane_key(PaneId(9))),
            "a float carries the same identity a tiled card does",
        );
    }
}
