//! One result row, composed — and the handles the palette keeps to drive it.
//!
//! A row is `Row` (the selection shell) around a `Grid` of three cells: an **icon cell** (reserved
//! whether or not the command has an icon, so every title starts in one column), the **text column**
//! (title, description, detail — each a `Label`, so the cut, the reflow and the match marks are the
//! label's) and the **chord column** (one [`KeyChord`] per binding, stacked). Everything is sized by
//! the layout engine: the text track takes what the other two leave, a wrapped description makes the
//! row taller and the rows after it slide down.

use super::Command;
use super::chord::KeyChord;
use crate::builders::{EventExt, LayoutExt, Parent, ToneExt, TooltipExt};
use crate::reactive::{Signal, SignalUpdate, signal};
use crate::style::Length;
use crate::widgets::{Ellipsis, Flex, Grid, Icon, Label, Row};
use crate::{Component, Tone};

/// The three tracks of a row: icon, text, chords. The icon and chord cells carry their own widths.
const TRACKS: &str = "auto 1fr auto";
/// Where the chord column sits among the grid's cells — the one place that says so.
const CHORD_CELL: usize = 2;
/// The icon is a touch larger than the text, so the glyph reads as the row's subject.
const ICON_SCALE: f32 = 1.3;
/// The icon cell's width — a touch wider than the glyph, so the titles of every row start in one
/// column whether or not the row has an icon. In `em`, so it follows the font.
const ICON_CELL: &str = "1.3em";

/// What a row's own handlers report. A handler has no way to reach the palette, so it leaves a note
/// here and the palette reads it — hover selects a row, a press chooses it. **Command indices**, not
/// positions: a row is the same row however the list is ranked.
#[derive(Clone, Copy)]
pub(super) struct PointerInbox {
    pub(super) hover: Signal<Option<usize>>,
    pub(super) press: Signal<Option<usize>>,
}

impl PointerInbox {
    pub(super) fn new() -> Self {
        Self {
            hover: signal(None),
            press: signal(None),
        }
    }
}

/// The live signals of one row — everything the palette changes on it without rebuilding it.
pub(super) struct RowHandle {
    /// Whether the row is the selected one: its panel selection look, and what makes a scroll
    /// region keep it in view.
    active: Signal<bool>,
    icon_tone: Signal<Option<Tone>>,
    title_tone: Signal<Option<Tone>>,
    second_tone: Signal<Option<Tone>>,
    /// Is this the row you are already on — drawn in the accent.
    current: bool,
    pub(super) title_marks: Signal<Vec<usize>>,
    /// Only the selected row's description reflows; the rest are cut to one line.
    pub(super) desc_wrap: Option<Signal<bool>>,
    pub(super) desc_marks: Option<Signal<Vec<usize>>>,
}

impl RowHandle {
    /// Show this row selected or not: the cursor, and the ink of everything on it.
    ///
    /// Selected, the icon and chips take the accent and the title reads in full foreground, its
    /// second line at the hint alpha; at rest the icon and chips are muted, the second line is muted
    /// and the title is soft. A `current` row keeps the accent on its title and icon either way —
    /// where you already are, found at a glance.
    pub(super) fn look(&self, selected: bool) {
        self.active.set(selected);
        let lit = selected || self.current;
        self.icon_tone
            .set(Some(if lit { Tone::Accent } else { Tone::Muted }));
        self.title_tone.set(match (selected, self.current) {
            (true, _) => None,
            (false, true) => Some(Tone::Accent),
            (false, false) => Some(Tone::Soft),
        });
        self.second_tone.set(Some(if selected {
            Tone::Secondary
        } else {
            Tone::Muted
        }));
    }
}

/// Build one command's row. Returns the row, the handle that drives it, and the title's text signal
/// (kept by the palette so a host can rename a row in place).
pub(super) fn build(
    cmd: &Command,
    index: usize,
    keyboard_moved: Signal<bool>,
    inbox: PointerInbox,
) -> (Row, RowHandle, Signal<String>) {
    let icon_tone = signal(None);
    let title_tone = signal(None);
    let second_tone = signal(None);

    let title = Label::new(cmd.label.clone())
        .truncate(Ellipsis::End)
        .content_tone_signal(title_tone);
    let title_marks = title.marks_signal();
    let title_text = title.text_signal();
    let mut text = Flex::column().flex(1.0).child(title);

    let (mut desc_wrap, mut desc_marks) = (None, None);
    if let Some(desc) = &cmd.description {
        // Cut by default, reflowed while selected — the palette flips the signal. A path keeps its
        // tail and never reflows.
        let label = Label::new(desc.clone())
            .truncate(if cmd.description_path {
                Ellipsis::Start
            } else {
                Ellipsis::End
            })
            .wrap(false)
            .content_tone_signal(second_tone);
        if !cmd.description_path {
            desc_wrap = Some(label.wrap_signal());
        }
        desc_marks = Some(label.marks_signal());
        text = text.child(if cmd.description_path {
            label.tooltip(desc.clone())
        } else {
            label
        });
    }
    if let Some(detail) = &cmd.detail {
        text = text.child(
            Label::new(detail.clone())
                .truncate(Ellipsis::End)
                .wrap(false)
                .content_tone_signal(second_tone),
        );
    }

    // The cell exists for every row, so the track is the same whether or not there is a glyph.
    let icon = Flex::row().width(ICON_CELL).shrink(0.0);
    let icon = match cmd.icon {
        Some(glyph) => icon.child(
            Icon::new(glyph)
                .scale(ICON_SCALE)
                .content_tone_signal(icon_tone),
        ),
        None => icon,
    };

    let chords = cmd.chords.iter().fold(
        Flex::column()
            .align("end")
            .shrink(0.0)
            .content_tone_signal(icon_tone),
        |column, chord| column.child(KeyChord::new(chord.clone())),
    );

    let cells = Grid::new()
        .template_column(TRACKS)
        .gap("md")
        .align("start")
        .width("100%")
        .child(icon)
        .child(text)
        .child(chords);
    let row = Row::new()
        .padding_x("md")
        .padding_y("sm")
        .panel_selection(true)
        .reveal_when(keyboard_moved)
        .child(cells)
        .on_pointer_move(move |_| inbox.hover.set(Some(index)))
        .on_pointer_down(move |_| inbox.press.set(Some(index)));
    let active = row.state();
    let handle = RowHandle {
        active,
        icon_tone,
        title_tone,
        second_tone,
        current: cmd.current,
        title_marks,
        desc_wrap,
        desc_marks,
    };
    handle.look(false);
    (row, handle, title_text)
}

/// Reserve `width` for a row's chord column, so every row's text ends where the same column of
/// chips begins. The one measured term: chip widths come from the keycap primitive, and the
/// layout engine has no way to share a column's width between sibling rows.
pub(super) fn reserve_chords(row: &mut dyn Component, width: f32) {
    let cell = row
        .base_mut()
        .children
        .first_mut()
        .and_then(|cells| cells.base_mut().children.get_mut(CHORD_CELL));
    if let Some(cell) = cell {
        cell.base_mut().style.layout.width = Length::Px(width);
    }
}
