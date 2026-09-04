//! **One pane, as a card** — the leaf of the map and the only thing in it with a surface.
//!
//! It owns what a card *is*: its name, whether it is its workspace's active pane, the cursor
//! signal that makes it the keyboard's target, what a click does, and what the delete letters do to
//! it. It owns nothing about **where** it goes or **how big** it is — a tiled card takes its share
//! of its [`ColumnCard`](super::column_card::ColumnCard), a floating one is placed at its own rect
//! by its [`WorkspaceRow`](super::workspace_row::WorkspaceRow). That split is why the same card
//! serves both.

use heca_core::layout::PaneId;
use heca_grid_ui::builders::{ComponentExt, LayoutExt, Parent, StyleExt};
use heca_grid_ui::style::{Align, Justify, Length, Spacing};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{Flex, GridCell, HintPlacement, KeyHint, Row};
use heca_grid_ui::Component;

/// The `key` of a pane's box — the row's one identity, so the cursor, the right-click target
/// and later a drag are three readers of a single declaration (F003/P085/T354).
pub(crate) fn pane_key(pane_id: PaneId) -> String {
    format!("expose.pane.{}", pane_id.0)
}

/// Dispatch a registry action by name with integer arguments — the one door the map's delete keys
/// go through, so `x`, `X` and `d` are ordinary actions rather than a private path.
pub(crate) type DispatchAction = std::rc::Rc<dyn Fn(&str, &[(&str, i64)])>;

/// **Which typed characters the map's cards answer to**, one list per thing that can be deleted.
///
/// Plain data, so the map stays a composition that takes plain data and needs no `AppState`
/// (§ 0b) — the lists are read from the built keymaps by [`register`](super::register) and handed
/// in. Empty lists are a working state, not a bug: a user who unbinds them gets a map with no
/// delete keys.
#[derive(Clone, Default, Debug, PartialEq)]
pub(crate) struct ExposeDeleteKeys {
    /// `delete_pane` — the card the cursor is on.
    pub(crate) pane: Vec<String>,
    /// `delete_column` — the column that card sits in.
    pub(crate) column: Vec<String>,
    /// `delete_workspace` — the workspace that column sits in.
    pub(crate) workspace: Vec<String>,
}

/// **The seams the map binds**, gathered once and passed down the tree.
///
/// Each is an app concept — an action name, an intent — which is exactly what makes these
/// components rather than widgets (§ 0b). Passing the group rather than seven arguments per level
/// keeps a component's own properties readable as the properties they are.
pub(crate) struct ExposeCallbacks {
    /// Activate a card: focus the pane, hand the keyboard back, put the map away.
    pub(crate) choose: std::rc::Rc<dyn Fn(PaneId)>,
    /// Dispatch a registry action by name — how the delete letters act.
    pub(crate) delete: DispatchAction,
    /// Move the map's own highlight, the same intent `CardGrid::on_move` sends.
    pub(crate) cursor_to: std::rc::Rc<dyn Fn(PaneId)>,
    /// Put the map away — the `close_overlay` action, so a dismissal is the same act however it
    /// was asked for.
    pub(crate) dismiss: std::rc::Rc<dyn Fn()>,
    /// The letters, resolved from `[[keys.surface]] heca.expose`.
    pub(crate) keys: ExposeDeleteKeys,
}

/// One pane in the map. Properties in, a card and its cursor cell out.
pub(crate) struct PaneCard<'a> {
    /// Which pane this is — its identity in every intent the card sends.
    pub(crate) pane_id: PaneId,
    /// The display name, already composed by the one name composer (`pane_info_view`).
    pub(crate) name: &'a str,
    /// Is this its workspace's focused pane?
    pub(crate) active: bool,
    /// **Where the pane is** — its working directory, home-relative, or `None` when there is none
    /// to show. Shown under the name by the same [`FolderLine`](crate::components::FolderLine) the
    /// sidebar's row uses, so the map and the dock describe a pane the same way.
    pub(crate) folder: Option<&'a str>,
    /// **Is this where back-and-forth would take you?** (`prefix+i`.) The faintest of the marks —
    /// the map is where "where would I land" is most worth knowing.
    pub(crate) previous: bool,
    /// Where it sits, for the letters that delete the column and the workspace around it.
    pub(crate) ws_idx: usize,
    pub(crate) col_idx: usize,
    /// **Where the cursor goes if this card is deleted** — resolved by the row while this card and
    /// its neighbour both still exist, because afterwards there is nothing left to ask.
    pub(crate) next: Option<PaneId>,
    pub(crate) theme: &'a GuiTheme,
    pub(crate) cb: &'a ExposeCallbacks,
}

impl PaneCard<'_> {
    /// Build the card and the cell the grid navigates it by.
    ///
    /// Returns the concrete [`KeyHint`] rather than a boxed component **on purpose**: the parent
    /// still has to give it its size — a share of a column, or a rect of a row — and a box has no
    /// builders left to do it with. `KeyHint` is transparent (it hugs its child and routes events,
    /// focus and drag straight through), so wrapping costs the parent nothing.
    pub(crate) fn build(self) -> (KeyHint, GridCell) {
        let theme = self.theme;
        // The card's own cursor signal, created before anything reads it: the `GridCell` lights it,
        // the card is focused by it, and both are the same value rather than two kept in step.
        let card = Row::new();
        let cell = GridCell::new(self.pane_id.0.to_string(), card.nav_state()).hovered(card.hovered());
        let mut card = card
            // **A card in the map is a SURFACE, not a wash.**
            //
            // It used to be `foreground` at `card_background_alpha` — 2% — which is a tint for a
            // card sitting on a background you control. This one sits on a full-screen overlay
            // above a *blurred photograph of the session*, whose brightness nothing here decides.
            // At 2% the card contributed nothing, so what made it visible at all was the theme's
            // glow: grid_tron has one and its cards read; mocha sets `glow_size = "none"` and its
            // cards vanished into the scrim, pale `foreground` text and all (Antonio, driving,
            // 2026-08-13). An opaque `surface` is a box in every theme, and `foreground` on
            // `surface` is legible by the palette's own construction.
            .background(theme.colors.surface)
                        .radius(theme.colors.control_radius())
            .pad_all(Spacing::Xs)
            // **The name sits in the middle of the card.** A card in the map is a picture of a
            // pane, not a row in a list: there is no column of names to align down, and a label
            // against the left edge reads as the start of a list item (Antonio, driving,
            // 2026-08-13). `Row` already centres on its cross axis; this is the main one.
            .justify(Justify::Center)
            // **The card fills the wrapper the parent sized.** `KeyHint` is transparent and hugs
            // its child, so the share or the rect the parent handed the wrapper has to be passed on
            // deliberately — a card left to hug its own label would collapse to the width of the
            // word in it, whatever the column was given.
            .width(Length::Pct(1.0))
            .height(Length::Pct(1.0))
            .active(self.active)
            .previous(self.previous)
            // **A card has a scale of its own.** The theme's default previous mark is tuned for a
            // row in a list and shouts on a surface the size of a pane; the workspace *frame*'s is
            // tuned for a whole container and disappears on one. Both were tried here and both were
            // wrong, in opposite directions — so the theme carries the third scale, and this asks
            // for it by name rather than picking a number.
            .previous_tint(theme.colors.effective_card_previous_background())
            .key(pane_key(self.pane_id));
        // **The pane you are on wears the frame it wears in the app.**
        //
        // The two fills alone could not carry it: "selected" and "last visited" are one accent ramp
        // eight points apart, tuned for a sidebar ROW — and the same two lifts on a card the size of
        // a pane read as one colour (Antonio, driving, 2026-08-21). Area changes how a lift reads,
        // which the theme already knows: a workspace *frame* lifts 0.13 where a row lifts 0.42.
        //
        // So the two states differ in **kind** rather than in strength — the same move `Row` makes
        // between its selected panel and its cursor ring. The current pane takes the accent edge and
        // the halo a focused pane frame carries (`chrome::pane::ACTIVE_GLOW_*`, one definition for
        // both, so the map cannot drift from the app it pictures); last-visited keeps the faint fill
        // it had, and now has it to itself.
        if self.active {
            card = card
                .border(theme.colors.accent, theme.focus_border_width)
                .glow_with(
                    theme.colors.accent,
                    crate::chrome::pane::ACTIVE_GLOW_RADIUS,
                    crate::chrome::pane::ACTIVE_GLOW_STRENGTH,
                );
        }
        // **The card the cursor is on holds the keyboard**, so its own handlers are what a key
        // reaches — and what it does not take bubbles up to the `CardGrid` for the nav keys,
        // exactly as a browser's listbox option does (AGENTS § 0c). The cursor signal *is* the
        // focus signal, so there is nothing to keep in step.
        card.base_mut().focused = card.nav_state();
        let card = card
            .on_text_input(delete_keys(
                self.cb.keys.clone(),
                self.cb.delete.clone(),
                self.cb.cursor_to.clone(),
                self.pane_id,
                self.next,
                self.ws_idx,
                self.col_idx,
            ))
            // Click to go there. `Row` already provides the hover tint and the press handling, so
            // the mouse costs one line rather than a second input path.
            .on_activate({
                let choose = self.cb.choose.clone();
                let id = self.pane_id;
                move || choose(id)
            })
            .child({
                // The name, and under it where the pane is — one column so a card with no folder
                // to show lays out exactly as it did before (a one-child column adds no gap).
                let folder = crate::components::FolderLine {
                    path: self.folder,
                    // Nothing to show *is* the setting being off — `model` resolved both into the
                    // same absence, so a card has one question to answer rather than two.
                    show: true,
                    font_scale: crate::chrome::CARD_META_FONT_SCALE,
                    theme,
                }
                .build();
                Flex::column()
                    .align(Align::Center)
                    .gap(2.0)
                    // **A card is a share of the strip, so its content absorbs the squeeze** —
                    // which is how a widget asks for it here (`Style::flex_shrink`: nothing shrinks
                    // unless it says so). Without it the block keeps its natural width, the card
                    // cannot be narrower than the longest path inside it, and a narrow window drew
                    // every card's text across its neighbours.
                    .shrink(1.0)
                    .max_width(Length::Pct(1.0))
                    // The name as every surface shows it — and it stays inside the card, however
                    // narrow the window makes it (`components::PaneName`).
                    .child(
                        crate::components::PaneName {
                            text: self.name,
                            color: Some(theme.colors.foreground),
                            bold: false,
                            font_scale: 1.0,
                            theme,
                        }
                        .build()
                        .widget,
                    )
                    .child(folder.widget)
            });
        // **Type a letter to jump to any card** (F003/P082/T427). One line on the widget, which is
        // the whole of it: the framework collects the declaration out of the laid-out tree
        // (`collect_hint_targets` already walks every visible layer) and paints the letters itself.
        // Nothing is registered, so nothing has to be un-registered when the map rebuilds — and a
        // plugin's own surface gets the picker by writing this same line (⭐⭐ RULE ZERO).
        //
        // **A pick and a click point at the same closure here, and that is worth saying out loud**
        // because the default assumption is the opposite: the two are different gestures and a
        // surface may answer them differently. In the sidebar a pick means *look at that one* and
        // keeps the keyboard, while a click means *go there and leave*. In the map both mean choose
        // that pane — the map exists to be left.
        let card = KeyHint::new(card)
            .on_hint({
                let choose = self.cb.choose.clone();
                let id = self.pane_id;
                move || choose(id)
            })
            // **Top-left, inside the card.** `CenterRight` put the letter on the card's right
            // border, where it reads as falling out of the box (Antonio, driving, 2026-08-14) —
            // and it was invisible as a choice until now, because the deleted host pass drew every
            // large target's cap in a top band and ignored what the widget declared.
            .placement(HintPlacement::TopLeft);
        (card, cell)
    }
}

/// What `x` / `X` / `d` do to the card they are pressed on: delete the pane, its column, or the
/// whole workspace.
///
/// One handler for both the tiled cards and the floating ones, because a float answers `x` the same
/// way — it is a pane. `X` and `d` name the column and workspace the card sits in, which the card
/// knows because it was built inside them; nothing is looked up and no cursor is consulted.
///
/// **The letters come from `[[keys.surface]] heca.expose`**, not from this file (F003/P082/T416).
/// They used to be literals here, which made them the one part of the map a user could not rebind
/// while the workspaces dock's identical `x` sat in `keybindings.default.toml`. The *handler* stays
/// on the card, because the card is what knows which pane, column and workspace it is — the host
/// resolves which letters, the widget resolves what they act on.
fn delete_keys(
    keys: ExposeDeleteKeys,
    delete: DispatchAction,
    cursor_to: std::rc::Rc<dyn Fn(PaneId)>,
    pane_id: PaneId,
    next: Option<PaneId>,
    ws_idx: usize,
    col_idx: usize,
) -> impl FnMut(&mut heca_grid_ui::event::EventCx<'_>) + 'static {
    use heca_grid_ui::event::Event;
    move |cx| {
        // **The typed character, not the key** — because `x` and `X` are the same *key*.
        // `Event::Key` deliberately carries no modifiers (a chord is resolved in the keymap, so a
        // widget sees `Char('x')` with or without Shift); the case lives in the text the platform
        // committed, which is exactly what `TextInput` is for — `Shift+2` is `Char('2')` as a key
        // and `"@"` as text. Reading the key made `X` do what `x` does (Antonio, driving,
        // 2026-08-11).
        let Event::TextInput(typed) = cx.event() else {
            return;
        };
        let typed = typed.as_str();
        let matches = |bound: &[String]| bound.iter().any(|k| k == typed);
        let dispatched = if matches(&keys.pane) {
            // **Hand the cursor on before the card goes.** The neighbour was resolved while this
            // row still had both of them; after the delete there is nothing left to ask. Sent
            // first so the rebuild the delete triggers already finds the cursor moved — the events
            // are queued and processed in order.
            if let Some(next) = next {
                cursor_to(next);
            }
            delete("close_pane_by_id", &[("pane_id", pane_id.0 as i64)]);
            true
        } else if matches(&keys.column) {
            // A column is named by the workspace holding it, so both indices go in one intent.
            //
            // The shipped default is `r`, not `X`: `x` and `X` are the same *key* and only differ
            // as typed text, so the pair read as one gesture with a modifier that the key layer
            // cannot see. Three plain letters, one per thing — pane, column, workspace — is what a
            // user can actually keep in their head (Antonio, 2026-08-11).
            delete(
                "delete_column",
                &[("ws_idx", ws_idx as i64), ("col_idx", col_idx as i64)],
            );
            true
        } else if matches(&keys.workspace) {
            delete("delete_workspace", &[("ws_idx", ws_idx as i64)]);
            true
        } else {
            false
        };
        // Claim only what was acted on, so every other key still bubbles to the grid for the nav.
        if dispatched {
            cx.stop_propagation();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::expose::model::ExposePane;
    use crate::chrome::expose::testing::{actions, callbacks, lay_out, theme};
    use heca_grid_ui::event::Event;
    use heca_grid_ui::reactive::{SignalGet, SignalUpdate};

    /// **A card keeps everything it draws inside itself**, at every width the map can squeeze it to
    /// (F003/P082/T438).
    ///
    /// This is the test the components rules ask of anything sized by its container — *lay it out
    /// in a box and assert it never exceeds it* — and its absence is why narrowing the window made
    /// every card's name paint across its neighbours until three of them were one smear.
    #[test]
    fn a_card_draws_nothing_outside_itself_however_narrow_it_gets() {
        for box_w in [400.0, 160.0, 80.0, 40.0, 16.0] {
            let (cb, _sink) = callbacks();
            let theme = theme();
            let (row, _cell) = PaneCard {
                pane_id: PaneId(7),
                folder: Some("~/projects/heca"),
                name: "editor",
                active: false,
                previous: false,
                ws_idx: 0,
                col_idx: 0,
                next: None,
                theme: &theme,
                cb: &cb,
            }
            .build();
            // The card is a **share** of the strip it sits in (`Pct`), so it is laid out inside a
            // parent that gives it one — as the map does. At the root of a layout a percentage has
            // nothing to be a percentage of.
            use heca_grid_ui::builders::{LayoutExt as _, Parent as _};
            let strip = heca_grid_ui::widgets::Flex::row()
                .width(heca_grid_ui::Length::Px(box_w as f32))
                .child(row);
            let root = lay_out(strip, box_w, 200.0);
            if box_w == 80.0 {
                fn dump(n: &dyn heca_grid_ui::Component, d: usize) {
                    eprintln!("{:i$}{:?}", "", n.base().bounds, i = d * 2);
                    for c in &n.base().children { dump(c.as_ref(), d + 1); }
                }
                dump(root.as_ref(), 0);
            }
            let mut scene = heca_grid_ui::Scene::new();
            root.paint(&mut heca_grid_ui::PaintCx::new(&mut scene, &theme));

            for cmd in scene.iter() {
                if let heca_grid_ui::DrawCommand::Text(t) = cmd
                    // An empty run paints nothing; a card too small for even one character has
                    // cut its text away entirely, which is the right answer at that size.
                    && !t.text.is_empty()
                {
                    assert!(
                        t.rect.loc.x + t.rect.size.w <= box_w + 0.5,
                        "at {box_w}px the card drew {:?} out to {}",
                        t.text,
                        t.rect.loc.x + t.rect.size.w,
                    );
                }
            }
        }
    }

    /// **A card says where its pane is**, with the same line the sidebar's row uses — so the map
    /// and the dock describe a pane the same way. Nothing to show is nothing drawn: `model` folds
    /// "no cwd" and "`pane_show_cwd` is off" into one absence before a card ever sees it.
    #[test]
    fn a_card_shows_the_folder_its_pane_is_in() {
        let text_of = |folder: Option<&str>| {
            let (cb, _sink) = callbacks();
            let theme = theme();
            let (row, _cell) = PaneCard {
                pane_id: PaneId(7),
                folder,
                name: "editor",
                active: false,
                previous: false,
                ws_idx: 0,
                col_idx: 0,
                next: None,
                theme: &theme,
                cb: &cb,
            }
            .build();
            let root = lay_out(row, 400.0, 200.0);
            let mut scene = heca_grid_ui::Scene::new();
            root.paint(&mut heca_grid_ui::PaintCx::new(&mut scene, &theme));
            scene
                .iter()
                .filter_map(|c| match c {
                    heca_grid_ui::DrawCommand::Text(t) => Some(t.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };

        let shown = text_of(Some("~/projects/heca"));
        assert!(shown.iter().any(|t| t == "editor"), "the name: {shown:?}");
        assert!(shown.iter().any(|t| t == "~/projects/heca"), "and the folder: {shown:?}");

        let bare = text_of(None);
        assert!(bare.iter().any(|t| t == "editor"), "the name is always there: {bare:?}");
        assert!(
            !bare.iter().any(|t| t.contains('/')),
            "and nothing is drawn where there is no folder: {bare:?}",
        );
    }

    /// **The pane you are on is told apart by KIND, not by strength.** Two fills eight points
    /// apart on one accent ramp read as one colour at card size — so the current card takes the
    /// accent edge and halo a focused pane frame wears, and last-visited keeps the fill to itself.
    #[test]
    fn the_current_card_wears_the_frame_a_focused_pane_wears() {
        let visual = |active: bool| {
            let (cb, _sink) = callbacks();
            let theme = theme();
            let (row, _cell) = PaneCard {
                pane_id: PaneId(7),
                folder: None,
                name: "editor",
                active,
                previous: !active,
                ws_idx: 0,
                col_idx: 0,
                next: None,
                theme: &theme,
                cb: &cb,
            }
            .build();
            let card = &row.base().children[0];
            let v = card.base().style.visual;
            (v.border.is_some(), v.glow.is_some())
        };

        assert_eq!(visual(true), (true, true), "the current card: an accent edge and a halo");
        assert_eq!(
            visual(false),
            (false, false),
            "and last-visited keeps only its fill, so the two cannot read as one",
        );
    }

    /// Build one card on its own — the whole point of the component being one.
    type Built = (
        Box<dyn heca_grid_ui::Component>,
        heca_grid_ui::reactive::Signal<bool>,
        crate::chrome::expose::testing::Sink,
    );

    fn card(active: bool, next: Option<PaneId>) -> Built {
        let (cb, sink) = callbacks();
        let theme = theme();
        let pane = ExposePane {
            pane_id: PaneId(7),
            folder: None,
            name: "editor".into(),
            active,
            height: 300.0,
        };
        let (row, _cell) = PaneCard {
            pane_id: pane.pane_id,
            folder: None,
            name: &pane.name,
            active: pane.active,
            previous: false,
            ws_idx: 2,
            col_idx: 3,
            next,
            theme: &theme,
            cb: &cb,
        }
        .build();
        // The card's own cursor signal — the one the `GridCell` was handed, so lighting it here is
        // exactly what the grid does when the cursor arrives. Read off the wrapped card rather than
        // the `KeyHint` around it: the wrapper is transparent, and the signal belongs to the card.
        let cursor = row.base().children[0].base().focused;
        (lay_out(row, 200.0, 100.0), cursor, sink)
    }

    /// **It renders its name and reports its cursor state.** The cell the grid navigates by and the
    /// focus the keyboard follows are the *same* signal, so there is nothing to keep in step.
    #[test]
    fn a_card_shows_its_name_and_its_cell_shares_the_cards_cursor_signal() {
        let (root, cursor, _) = card(true, None);
        let theme = theme();
        let mut scene = heca_grid_ui::Scene::new();
        root.paint(
            &mut heca_grid_ui::PaintCx::new(&mut scene, &theme)
                .with_viewport(heca_grid_ui::Size::new(200.0, 100.0)),
        );
        let drawn: Vec<String> = scene
            .iter()
            .filter_map(|c| match c {
                heca_grid_ui::DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();
        assert!(drawn.iter().any(|t| t == "editor"), "the card shows its name: {drawn:?}");
        let inner = root.base().children[0].base();
        assert_eq!(
            inner.key.as_deref(),
            Some(pane_key(PaneId(7)).as_str()),
            "and answers to its pane's one identity",
        );
        // **Centred, not against the left edge** — a card is a picture of a pane, not a list row.
        let card = inner.bounds;
        let label = inner.children[0].base().bounds;
        let slack = (label.loc.x - card.loc.x) - ((card.loc.x + card.size.w) - (label.loc.x + label.size.w));
        // One pixel of asymmetry is centred: a 49px label in a 200px card has 75.5px either side,
        // and boxes are whole pixels.
        assert!(
            slack.abs() <= 1.0,
            "the name sits in the middle: label {label:?} in card {card:?}",
        );
        // The signal the cell was handed is the card's own: lighting it focuses the card.
        assert!(!cursor.get_untracked());
        cursor.set(true);
        assert!(
            root.base().children[0].base().focused.get_untracked(),
            "the card the cursor is on is the card that holds the keyboard",
        );
    }

    /// **The delete letters act on the pane, the column and the workspace this card sits in** —
    /// which it knows because it was built inside them. Nothing is looked up and no cursor is
    /// consulted, and an unbound letter is nobody's.
    #[test]
    fn the_delete_letters_act_on_what_this_card_sits_in() {
        let (mut root, cursor, sink) = card(true, Some(PaneId(8)));
        // **The card the cursor is on is the one that gets the keys** — nothing focused, nothing
        // delivered (AGENTS § 0c). In the map the grid lights this signal; here the test does.
        cursor.set(true);
        let mut press = |c: &str| {
            heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput(c.to_string()));
        };

        press("x");
        assert_eq!(
            actions(&sink),
            vec![("close_pane_by_id".to_string(), vec![("pane_id".to_string(), 7)])],
        );
        // …and the cursor was handed to the neighbour FIRST, while it could still be resolved.
        assert!(
            matches!(
                sink.borrow().first(),
                Some(crate::app::interaction::InteractionIntent::ExposeCursor { pane_id })
                    if *pane_id == PaneId(8)
            ),
            "the neighbour takes the cursor before the card goes: {:?}",
            sink.borrow(),
        );

        sink.borrow_mut().clear();
        press("r");
        assert_eq!(
            actions(&sink),
            vec![(
                "delete_column".to_string(),
                vec![("col_idx".to_string(), 3), ("ws_idx".to_string(), 2)],
            )],
        );

        sink.borrow_mut().clear();
        press("d");
        assert_eq!(
            actions(&sink),
            vec![("delete_workspace".to_string(), vec![("ws_idx".to_string(), 2)])],
        );

        sink.borrow_mut().clear();
        press("z");
        assert!(actions(&sink).is_empty(), "an unclaimed letter dispatches nothing");
    }

    /// **A click activates it** — the same act the cursor performs, so the mouse and the keyboard
    /// cannot drift apart. Driven as a real pointer press and release, because a click is both: a
    /// host that delivers only presses produces no clicks at all.
    #[test]
    fn clicking_a_card_chooses_its_pane() {
        use heca_grid_ui::event::{PointerButton, RawPointer, RawPointerKind};
        let (mut root, _cursor, sink) = card(false, None);
        // The card's own middle, read off the laid-out tree — a card hugs its label, so a guess
        // at the box's centre can land outside it.
        let b = root.base().bounds;
        let at = heca_grid_ui::Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
        for kind in [RawPointerKind::Pressed, RawPointerKind::Released] {
            heca_grid_ui::dispatch(
                root.as_mut(),
                &Event::Raw(RawPointer {
                    kind,
                    pos: at,
                    button: PointerButton::Left,
                    modifiers: Default::default(),
                    delta_x: 0.0,
                    delta_y: 0.0,
                }),
            );
        }
        let got = format!("{:?}", sink.borrow());
        assert!(
            got.contains("FocusPaneThenAction") && got.contains("PaneId(7)"),
            "it focuses its own pane: {got}",
        );
        assert!(got.contains("CloseOverlay"), "and puts the map away: {got}");
    }

    /// **Every card declares a pick, and it chooses that card's pane** (F003/P082/T427).
    ///
    /// The declaration is the whole feature: `collect_hints` walks the laid-out tree of every
    /// visible layer, so a card that declares one is in the picker and a card that does not is
    /// invisible to it — there is no registry to forget to update. This asserts the declaration is
    /// there and that firing it is the same act as choosing the card, which is what lets `s` and a
    /// click agree.
    #[test]
    fn a_card_declares_a_pick_that_chooses_its_pane() {
        let (mut root, _cursor, sink) = card(false, None);
        let hints = heca_grid_ui::collect_hints(root.as_ref());
        assert_eq!(hints.len(), 1, "one card, one pick: {hints:?}");

        assert!(
            heca_grid_ui::fire_hint(root.as_mut(), &hints[0].0),
            "and the declaration runs",
        );
        let got = format!("{:?}", sink.borrow());
        assert!(
            got.contains("FocusPaneThenAction") && got.contains("PaneId(7)"),
            "the pick chooses this card's pane, exactly as a click does: {got}",
        );
    }
}
