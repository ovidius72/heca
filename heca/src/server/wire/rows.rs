//! **A terminal on the socket travels as the rows that changed.**
//!
//! The writer of a connection remembers the last screen it carried for each terminal and sends only
//! the rows that differ from it; the reader keeps the same screens and puts the rows back, so what
//! comes out of the socket is the whole [`TerminalFrame`] the server made. Nothing above the socket
//! knows: the server builds whole frames, the queue still lets a newer one take an older one's place
//! (the writer compares against what it actually wrote), and the window receives whole frames.
//!
//! Each row is also written compactly: cells of one style are a run, a style is written once per
//! frame and named by its index, and a run of single-character cells is one string.

use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use heca_core::backend::{TerminalCell, TerminalLine, TerminalUnderlineStyle};
use serde::{Deserialize, Serialize};

use super::{ServerMsg, terminal::TerminalFrame};
use crate::app::backend_store::TerminalOwner;

/// The screens one end of a connection last carried, per terminal. The writer and the reader of
/// one stream each hold one, and they agree because the stream keeps order. Each is the shared
/// frame itself — the one the server sent, the one handed to the window — never a copy of it.
#[derive(Default)]
pub(crate) struct Screens(HashMap<TerminalOwner, Arc<TerminalFrame>>);

/// What one server message is on the socket.
#[derive(Serialize)]
pub(super) enum Outgoing<'a> {
    #[serde(rename = "m")]
    Message(&'a ServerMsg),
    #[serde(rename = "t")]
    Terminal(&'a Packed),
}

#[derive(Deserialize)]
pub(super) enum Incoming {
    #[serde(rename = "m")]
    Message(ServerMsg),
    #[serde(rename = "t")]
    Terminal(Box<Packed>),
}

/// A terminal frame with its screen's lines taken out and replaced by the rows that changed.
#[derive(Serialize, Deserialize)]
pub(super) struct Packed {
    #[serde(rename = "f")]
    frame: Box<TerminalFrame>,
    /// Every row is here: the reader starts from nothing.
    #[serde(rename = "a")]
    full: bool,
    #[serde(rename = "s")]
    styles: Vec<Style>,
    #[serde(rename = "r")]
    rows: Vec<Row>,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Style {
    #[serde(rename = "f")]
    fg: [f32; 4],
    #[serde(rename = "b")]
    bg: [f32; 4],
    #[serde(rename = "o", default, skip_serializing_if = "is_false")]
    bold: bool,
    #[serde(rename = "i", default, skip_serializing_if = "is_false")]
    italic: bool,
    #[serde(rename = "u")]
    underline: TerminalUnderlineStyle,
}

#[derive(Serialize, Deserialize)]
struct Row {
    #[serde(rename = "y")]
    at: usize,
    #[serde(rename = "r")]
    runs: Vec<Run>,
}

/// Cells side by side that share one style.
#[derive(Serialize, Deserialize)]
struct Run {
    #[serde(rename = "s")]
    style: usize,
    /// Each character is one cell one column wide — the usual case, written as one string.
    #[serde(rename = "c", default, skip_serializing_if = "Option::is_none")]
    chars: Option<String>,
    /// Otherwise each cell's text and width.
    #[serde(rename = "t", default, skip_serializing_if = "Vec::is_empty")]
    cells: Vec<(String, usize)>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl Screens {
    /// Pack a terminal frame against the screen last written for its terminal, and remember this one.
    pub(super) fn pack(&mut self, frame: Arc<TerminalFrame>) -> Packed {
        let lines = &frame.screen.lines;
        let previous = self
            .0
            .get(&frame.owner)
            .map(|previous| &previous.screen.lines)
            .filter(|previous| previous.len() == lines.len());
        let full = previous.is_none();
        let mut styles = Vec::new();
        let rows = lines
            .iter()
            .enumerate()
            .filter(|(at, line)| previous.is_none_or(|previous| previous[*at] != **line))
            .map(|(at, line)| Row {
                at,
                runs: runs(line, &mut styles),
            })
            .collect();
        let head = Box::new(without_lines(&frame));
        self.0.insert(frame.owner.clone(), frame);
        Packed {
            frame: head,
            full,
            styles,
            rows,
        }
    }

    /// Put a packed frame's rows back onto the screen last read for its terminal.
    pub(super) fn unpack(&mut self, packed: Packed) -> io::Result<Arc<TerminalFrame>> {
        let Packed {
            mut frame,
            full,
            styles,
            rows,
        } = packed;
        let height = frame.screen.rows;
        let mut lines = match self.0.get(&frame.owner) {
            _ if full => vec![TerminalLine { cells: Vec::new() }; height],
            Some(previous) if previous.screen.lines.len() == height => {
                previous.screen.lines.clone()
            }
            _ => {
                return Err(invalid(
                    "terminal rows arrived for a screen this connection never carried",
                ));
            }
        };
        for row in rows {
            let line = lines
                .get_mut(row.at)
                .ok_or_else(|| invalid("terminal row outside the screen"))?;
            *line = cells(row.runs, &styles)?;
        }
        frame.screen.lines = lines;
        let frame: Arc<TerminalFrame> = Arc::from(frame);
        self.0.insert(frame.owner.clone(), frame.clone());
        Ok(frame)
    }

    /// A terminal is gone: forget its screen.
    pub(super) fn forget(&mut self, owner: &TerminalOwner) {
        self.0.remove(owner);
    }
}

/// Everything of a frame but its screen's lines, which travel as rows.
fn without_lines(frame: &TerminalFrame) -> TerminalFrame {
    let screen = &frame.screen;
    TerminalFrame {
        id: frame.id,
        owner: frame.owner.clone(),
        spec: frame.spec.clone(),
        title: frame.title.clone(),
        display_title: frame.display_title.clone(),
        damage: frame.damage.clone(),
        runtime: frame.runtime.clone(),
        mouse_grabbed: frame.mouse_grabbed,
        screen: heca_core::backend::WireSnapshot {
            cols: screen.cols,
            rows: screen.rows,
            cell_w: screen.cell_w,
            cell_h: screen.cell_h,
            default_fg: screen.default_fg,
            default_bg: screen.default_bg,
            cursor_color: screen.cursor_color,
            cursor: screen.cursor,
            lines: Vec::new(),
            viewport_offset: screen.viewport_offset,
            at_bottom: screen.at_bottom,
            scrollback_rows: screen.scrollback_rows,
            viewport_top_stable_row: screen.viewport_top_stable_row,
            hyperlinks: screen.hyperlinks.clone(),
            graphics: screen.graphics.clone(),
        },
    }
}

fn runs(line: &TerminalLine, styles: &mut Vec<Style>) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for cell in &line.cells {
        let style = Style {
            fg: cell.fg,
            bg: cell.bg,
            bold: cell.bold,
            italic: cell.italic,
            underline: cell.underline,
        };
        let index = match styles.iter().position(|known| *known == style) {
            Some(index) => index,
            None => {
                styles.push(style);
                styles.len() - 1
            }
        };
        if runs.last().is_none_or(|run| run.style != index) {
            runs.push(Run {
                style: index,
                chars: Some(String::new()),
                cells: Vec::new(),
            });
        }
        let run = runs.last_mut().expect("a run was just pushed");
        let single = cell.width == 1 && cell.text.chars().count() == 1;
        match (&mut run.chars, single) {
            (Some(chars), true) => chars.push_str(&cell.text),
            (Some(chars), false) => {
                run.cells = chars.chars().map(|ch| (ch.to_string(), 1)).collect();
                run.chars = None;
                run.cells.push((cell.text.clone(), cell.width));
            }
            (None, _) => run.cells.push((cell.text.clone(), cell.width)),
        }
    }
    runs
}

fn cells(runs: Vec<Run>, styles: &[Style]) -> io::Result<TerminalLine> {
    let mut cells = Vec::new();
    for run in runs {
        let style = *styles
            .get(run.style)
            .ok_or_else(|| invalid("terminal run names a style that was not sent"))?;
        let cell = |text: String, width: usize| TerminalCell {
            text,
            fg: style.fg,
            bg: style.bg,
            bold: style.bold,
            italic: style.italic,
            underline: style.underline,
            width,
        };
        match run.chars {
            Some(chars) => cells.extend(chars.chars().map(|ch| cell(ch.to_string(), 1))),
            None => cells.extend(run.cells.into_iter().map(|(text, width)| cell(text, width))),
        }
    }
    Ok(TerminalLine { cells })
}

fn invalid(reason: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason.to_owned())
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
