//! [`Input`] — a single- or multi-line text field: the first widget with an editable text
//! buffer and a blinking caret. It is a **change widget** like
//! [`Toggle`](super::Toggle)/[`Checkbox`](super::Checkbox): each edit emits
//! `Action::value("input-change", SignalData::String(new))` to an
//! [`on_change`](Input::on_change) handler.
//!
//! Editing keys (delivered to the focused field): printable
//! [`Char`](crate::component::GridKey::Char)/Space insert at the caret;
//! Backspace/Delete remove; Left/Right move the caret; Home/End jump to the line
//! ends (Ctrl/Cmd/Alt on these keys widen the granularity to line/word). A pointer
//! press places the caret by x. Monospace advance (`font_size * MONO_ADVANCE_RATIO`)
//! drives both caret placement and click hit-testing.
//!
//! The readline/select-all **shortcuts** are host-configured, not baked in
//! (`widget-keys-config`): Ctrl+h (delete back), Ctrl+u (delete to line start), and
//! Ctrl/Cmd+A (select all) and Cmd+Shift+Backspace / Ctrl+Shift+Backspace (empty the field) arrive as
//! the semantic [`Event::Widget`] intents `EditDeleteBack` / `EditDeleteToLineStart` /
//! `EditSelectAll` / `ClearInput`, which the host resolves from the configurable `[keys.widgets]`
//! `edit_delete_back` / `edit_delete_to_line_start` / `edit_select_all` / `clear_input` bindings.
//! [`clearable`](Input::clearable) adds the mouse twin of `ClearInput`: an × at the right end.

use crate::action::{Action, SignalData};
use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx, WidgetIntent};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::Border;
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};
use std::time::Instant;

mod builders;
mod editing;
mod navigation;
use navigation::{Granularity, next_word_boundary, prev_word_boundary, word_bounds};
mod component;
mod content;
mod suggest;

/// An editable field. Single-line by default; [`multiline(true)`](Self::multiline) wraps and grows
/// to [`max_lines`](Self::max_lines), then scrolls. The default limit is [`heca_theme::InputStyle::max_lines`].
/// [`LayoutExt::max_height`] can impose a smaller ceiling; overflow stays inside the field.
/// Its default minimum height keeps one complete font line and the configured padding readable.
/// An explicit height fills that height instead of applying the automatic line cap; use
/// [`crate::Resizable`] to let the user choose the height with a grip.
///
/// Both modes share selection, configurable editing shortcuts and committed text handling.
/// Enter inserts a newline only in multi-line mode. [`on_submit`](Self::on_submit) handles the
/// separate, unbound-by-default [`WidgetIntent::EditSubmit`] intent. A field owns no caller content:
/// its Label and ScrollRegion provide the same text and scrolling behavior wherever it is mounted.
///
/// ```
/// use heca_grid_ui::{Input, LayoutExt, SignalData};
/// let input = Input::new().multiline(true).max_lines(6).max_height(180.0)
///     .placeholder("Write a message").placeholder_on_focus(true)
///     .on_change(|action| { if let SignalData::String(text) = action.data { println!("{text}"); } });
/// let text = input.text_signal(); // writable signal; set_value also clears the selection
/// ```
///
/// Plugins and RPC describe the same field. `text` is the value; `change` and `submit` intents
/// carry the full value. The shared `max_height` layout prop accepts pixels or percentages.
///
/// ```ignore
/// use heca_view::{ViewNode, WidgetId, PropValue, Intent};
/// let input = ViewNode::new(WidgetId::Input)
///     .prop("multiline", PropValue::Bool(true))
///     .prop("max_lines", PropValue::Int(6))
///     .prop("max_height", PropValue::Float(180.0))
///     .on("change", Intent::new("draft.change"))
///     .on("submit", Intent::new("draft.submit"));
/// ```
pub struct Input {
    base: Base,
    /// The current text, exposed reactively via [`text`](Input::text).
    text: Signal<String>,
    /// Shown (muted) when empty and unfocused — a value that may change, which the field's own
    /// placeholder label follows (see [`placeholder`](Input::placeholder)).
    placeholder: Signal<String>,
    placeholder_on_focus: bool,
    seen_text: String,
    /// Caret position as a char index in `0..=text.chars().count()` (the moving
    /// head of a selection).
    cursor: usize,
    /// The fixed end of an active selection. The selection spans `anchor..cursor`
    /// (ordered); `None` (or `anchor == cursor`) means no selection.
    anchor: Option<usize>,
    /// When the current caret-blink cycle started. The caret is driven by real
    /// elapsed time (not the frame `dt`) so it blinks at a steady rate regardless of
    /// how sparsely the host redraws — see [`next_redraw`](Input::next_redraw).
    blink_origin: Instant,
    /// Caret visibility at the last paint, so `tick` damages the field only when the
    /// caret actually toggles (not every frame).
    last_caret: std::cell::Cell<bool>,
    multiline: bool,
    max_lines: Option<usize>,
    view: std::rc::Rc<std::cell::RefCell<content::View>>,
    on_change: Option<Box<dyn Fn(Action)>>,
    on_submit: Option<Box<dyn Fn(Action)>>,
    /// Set by a [`clearable`](Input::clearable) field's × when it is clicked; the click then
    /// bubbles to the field, which empties itself — the × owns no text and the field no hit-test.
    clear_requested: std::rc::Rc<std::cell::Cell<bool>>,
    /// Suggestions under the caret, when the field has them ([`suggest`](Input::suggest)).
    suggest: Option<suggest::Suggest>,
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}
impl LayoutExt for Input {}
