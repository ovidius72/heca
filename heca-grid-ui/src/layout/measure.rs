//! Taffy's measure callback: how big is a piece of text in the room it is offered?

use super::TextMeasure;
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO, mono_cells, wrap_lines};
use taffy::prelude::*;

/// Taffy's measure callback: how tall is this text in the width being offered?
///
/// Called only for nodes carrying a [`TextMeasure`], and only while taffy is resolving them — which
/// is the whole point: the width is known here and nowhere earlier.
///
/// The three width questions taffy asks are answered separately, because a wrapping label has three
/// honest answers. Collapsing them onto the definite case makes a label in an `auto`-sized parent
/// measure one line and then paint three.
pub(super) fn measure_text_node(
    known: taffy::Size<Option<f32>>,
    available: taffy::Size<AvailableSpace>,
    _node: taffy::NodeId,
    ctx: Option<&mut TextMeasure>,
    _style: &taffy::Style,
) -> taffy::Size<f32> {
    let Some(ctx) = ctx else {
        return taffy::Size::ZERO;
    };
    let cell = (ctx.font * MONO_ADVANCE_RATIO) as f64;
    let line = ctx.font * MONO_LINE_RATIO;
    // One line, uncut — what the label would ask for if nothing constrained it.
    let natural = ctx.text.chars().count() as f64 * cell;
    let width = match (known.width, available.width) {
        // The engine already resolved a width: wrap into exactly that.
        (Some(w), _) => w as f64,
        (None, AvailableSpace::Definite(w)) => w as f64,
        // "How wide would you like to be?" — one line.
        (None, AvailableSpace::MaxContent) => natural,
        // "How narrow can you get without overflowing?"
        //
        // **A cutting label can get down to one character — that is what cutting is, and where it
        // stops.** Answering with its longest word made it its own container's floor: a card whose
        // folder line reads `~/projects/heca` could not be laid out narrower than that path, so the
        // card overflowed its box and every card in a narrow window drew across its neighbours.
        //
        // One character rather than **zero**: `min_width: auto` means "my floor is whatever I
        // answered here", so answering zero is saying *my floor is nothing* — and a box resolved to
        // nothing holds no characters and draws no text, which is how a `Card` lost its title
        // outright. Text with something to say is never silent; at its narrowest it is `…`
        // (F003/P082/T438).
        (None, AvailableSpace::MinContent) if !ctx.wrap => cell,
        // A **wrapping** label is the case the longest word belongs to: wrapping cannot break a word
        // down further, so that word is a real floor (a longer-than-a-line word is hard-broken, so
        // it never sets one).
        (None, AvailableSpace::MinContent) => {
            ctx.text
                .split_whitespace()
                .map(|w| w.chars().count())
                .max()
                .unwrap_or(0) as f64
                * cell
        }
    };
    // A cutting label is one line whatever happens to it — it is the *width* it accepts, not the
    // height. Only a wrapping one turns width into height.
    let lines = if ctx.wrap {
        wrap_lines(&ctx.text, mono_cells(width, cell), ctx.max_lines)
            .len()
            .max(1)
    } else {
        1
    };
    taffy::Size {
        // Never wider than the text actually is: a short label in a wide box keeps its own width,
        // so `align` still has room to place it — the same measure a non-wrapping label reports.
        //
        // **Rounded UP, because half a character is not a character.** Six cells of 8.1px want
        // 48.6px; reporting that gets a box floored to 48, and a cutting label then finds itself
        // one cell short of its own text and draws `edit…` where `editor` fits. `mono_cells` keeps
        // half a pixel of slack for exactly this, and half a pixel is not enough — the loss is up
        // to a whole one. The measure is the place to fix it: a box that cannot hold the text it
        // was measured for is wrong before anyone looks at it (F003/P082/T438).
        width: width.min(natural).ceil() as f32,
        height: line * lines as f32,
    }
}
