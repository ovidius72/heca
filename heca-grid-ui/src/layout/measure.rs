//! Shared intrinsic-content measurement: text reflow and natural image proportions.

use super::Measure;
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO, mono_cells, wrap_lines};
use taffy::prelude::*;

/// Measure intrinsic content against the width/height constraints taffy resolved.
///
/// Text carries its reflow data; replaced content carries its natural size and preferred ratio.
/// Measurement runs inside taffy, after constraints have supplied the available space.
///
/// The three width questions taffy asks are answered separately, because a wrapping label has three
/// honest answers. Collapsing them onto the definite case makes a label in an `auto`-sized parent
/// measure one line and then paint three.
pub(super) fn measure_content(
    known: taffy::Size<Option<f32>>,
    available: taffy::Size<AvailableSpace>,
    _node: taffy::NodeId,
    ctx: Option<&mut Measure>,
    style: &taffy::Style,
) -> taffy::Size<f32> {
    let Some(ctx) = ctx else {
        return taffy::Size::ZERO;
    };
    let ctx = match ctx {
        Measure::Intrinsic { size, ratio } => {
            let definite = |space| match space {
                AvailableSpace::Definite(value) => Some(value),
                _ => None,
            };
            let authored = |dimension, space| match dimension {
                Dimension::Auto => None,
                _ => definite(space),
            };
            let width = known.width.or_else(|| authored(style.size.width, available.width));
            let height = known.height.or_else(|| authored(style.size.height, available.height));
            let width = width.or_else(|| height.map(|h| h * *ratio)).unwrap_or_else(|| {
                definite(available.width).map_or(size.w as f32, |available| (size.w as f32).min(available))
            });
            return taffy::Size { width, height: height.unwrap_or(width / *ratio) };
        }
        Measure::Text(text) => text,
    };
    let cell = (ctx.font * MONO_ADVANCE_RATIO) as f64;
    let line = ctx.font * MONO_LINE_RATIO;
    // One line, uncut — what the label would ask for if nothing constrained it.
    let natural_chars = if ctx.preserve_whitespace {
        ctx.text.split('\n').map(|line| line.chars().count()).max().unwrap_or(0)
    } else { ctx.text.chars().count() };
    let natural = natural_chars as f64 * cell;
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
        // outright. Text with something to say is never silent; at its narrowest it is `…`.
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
        if ctx.preserve_whitespace {
            crate::font::wrap_preserved(&ctx.text, mono_cells(width, cell), ctx.max_lines).len()
        } else { wrap_lines(&ctx.text, mono_cells(width, cell), ctx.max_lines).len().max(1) }
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
        // was measured for is wrong before anyone looks at it.
        width: width.min(natural).ceil() as f32,
        height: line * lines as f32,
    }
}
