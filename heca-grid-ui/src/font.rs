//! Default embedded monospace font.
//!
//! The Grid look ships with a default mono family so it renders identically
//! everywhere without depending on installed system fonts.
//!
//! Chosen face: **Geist Mono** (SIL Open Font License 1.1 — free to embed and
//! redistribute). <https://fonts.google.com/specimen/Geist+Mono>
//!
//! The renderer loads the embedded faces into its `cosmic-text` font system so
//! `Geist Mono` resolves without a system install. These are *defaults*, not
//! hardcoded overrides: `Theme.font_family` / `Theme.font_size` (and a future
//! weight/style token) stay configurable.
//!
//! ## Weights & italic
//! Both [`DEFAULT_MONO_BYTES`] (Regular) and [`DEFAULT_MONO_BOLD_BYTES`] (Bold)
//! register under the same family `"Geist Mono"`; `cosmic-text` selects between
//! them via `Attrs::weight(...)`. Geist Mono ships **no italic face**, so italic
//! is rendered as a **synthesized oblique** (skew) — request it with
//! `Attrs::style(cosmic_text::Style::Italic)` and let the shaper fake-slant the
//! upright glyphs, or apply [`OBLIQUE_SKEW`] manually.

/// Family name of the default embedded monospace font (shared by all weights).
pub const DEFAULT_MONO_FAMILY: &str = "Geist Mono";

/// Embedded bytes of the default monospace font (Geist Mono **Regular**, OFL 1.1).
/// Load once into the renderer's font database:
/// ```ignore
/// font_system.db_mut().load_font_data(DEFAULT_MONO_BYTES.to_vec());
/// font_system.db_mut().load_font_data(DEFAULT_MONO_BOLD_BYTES.to_vec());
/// ```
pub const DEFAULT_MONO_BYTES: &[u8] = include_bytes!("../assets/GeistMono-Regular.ttf");

/// Embedded bytes of the **Bold** weight (Geist Mono Bold, OFL 1.1). Registers
/// under the same family as [`DEFAULT_MONO_BYTES`]; selected via `Attrs::weight`.
pub const DEFAULT_MONO_BOLD_BYTES: &[u8] = include_bytes!("../assets/GeistMono-Bold.ttf");

/// Horizontal skew (tangent of the slant angle, ~12°) for synthesizing an
/// oblique/italic from the upright faces, since Geist Mono has no italic face.
pub const OBLIQUE_SKEW: f32 = 0.21;

/// Family name of the embedded icon font — **Phosphor Duotone** (MIT licensed;
/// see `assets/PHOSPHOR-LICENSE.txt`). <https://phosphoricons.com>. The host
/// registers it as a second family alongside [`DEFAULT_MONO_FAMILY`]; the
/// renderer selects it for [`FontRole::Icon`](crate::scene::FontRole) text runs.
pub const ICON_FONT_FAMILY: &str = "Phosphor-Duotone";

/// Embedded bytes of the icon font (Phosphor Duotone, MIT). Load alongside the
/// mono faces:
/// ```ignore
/// font_system.db_mut().load_font_data(ICON_FONT_BYTES.to_vec());
/// ```
/// Duotone glyphs come in consecutive codepoint pairs: the **secondary** layer
/// (the `:before` codepoint, drawn at ~0.2 alpha) and the **primary** layer
/// (secondary + 1, full alpha) stacked at the same spot — see
/// [`Icon`](crate::widgets::Icon).
pub const ICON_FONT_BYTES: &[u8] = include_bytes!("../assets/Phosphor-Duotone.ttf");

/// Approximate advance width of a monospace glyph as a fraction of font size.
/// Used for the Phase-A naive text measure until `cosmic-text` shaping lands.
pub const MONO_ADVANCE_RATIO: f32 = 0.6;

/// Approximate line height as a fraction of font size (Phase-A measure only).
pub const MONO_LINE_RATIO: f32 = 1.4;

/// Where a run's **baseline** sits inside its line box, as a fraction of the line height.
///
/// The renderer centres a run's line box in the rect it is given and puts the baseline an ascent
/// below the top of that box, so this is the one number the layout side needs in order to line two
/// runs of *different sizes* up on a shared baseline — see [`Flex`](crate::widgets::Flex), which is
/// the only thing that reads it. Both the line height and the ascent scale with the font, so
/// centring two boxes can never make their baselines meet; only shifting by the difference can.
pub const BASELINE_RATIO: f32 = 0.78;

/// How many monospace cells fit in `width`.
///
/// **The slack is not cosmetic, and half a pixel is not enough of it.** Two different roundings bite
/// here. An exact multiple of the cell divides to `n - 0.0000001` in f64 and floors one short. Worse,
/// taffy rounds every computed box to whole pixels, so a label measured at its own natural width of
/// 105.3px is handed back 105.0 — and a label that cannot fit its own text cuts it, putting an
/// ellipsis on a string that was never too long. That was a real, visible defect: `Reload Config`
/// drew as `Reload Conf…` in a panel with 450px to spare.
///
/// So the slack is **half a pixel**, which is exactly what taffy's rounding can take away. It cannot
/// gain a cell that does not fit: half a pixel is a fraction of any legible cell.
pub fn mono_cells(width: f64, cell: f64) -> usize {
    if cell <= 0.0 {
        return 0;
    }
    ((width + PIXEL_ROUNDING_SLACK) / cell).floor().max(0.0) as usize
}

/// What a whole-pixel rounding of a layout box can take away from a measured width.
const PIXEL_ROUNDING_SLACK: f64 = 0.5;

/// Break `text` into the lines that fit `cells` monospace cells, on **word** boundaries.
///
/// The one implementation of wrapping: the layout measure calls it to learn how tall a label will
/// be, and the paint calls it to draw. Two spellings of this would put the glyphs on a different
/// number of lines than the box was sized for — text drawn outside its own bounds, with nothing to
/// catch it.
///
/// A word longer than the line is **broken** rather than allowed to overflow: a URL or a path
/// with no spaces is common enough that refusing to break it means refusing to fit at all. It is
/// broken **after a path separator** (`/` or `\`) when one lies inside the line, so a path folds
/// at its directories; with none, it is hard-broken at the line's width. Runs of whitespace collapse
/// at a break, as they do in every wrapper, and an empty text is one empty line so a label never
/// measures zero-height.
///
/// `max_lines` caps the result. Text that needs more ends its last line with `…` — the only mark a
/// reader needs to know there is more — instead of growing without bound.
pub fn wrap_lines(text: &str, cells: usize, max_lines: Option<usize>) -> Vec<String> {
    wrap_indexed(text, cells, max_lines)
        .into_iter()
        .map(|line| line.into_iter().map(|(c, _)| c).collect())
        .collect()
}

/// [`wrap_lines`], keeping each character's **index in the source string**.
///
/// The mapping exists because wrapping otherwise destroys it: runs of whitespace collapse and a
/// joining space is re-inserted, so the nth character of a wrapped line is not the nth character of
/// the text. Anything the caller attached to a character — a match highlight, most obviously — would
/// land on the wrong letter after a reflow if it counted its way through the output.
///
/// The inserted joining space carries the index of the whitespace it stands for, so a query that
/// matched a space still marks one. The `…` that closes a capped text carries an index past the end
/// of the text, which is how a caller tells a character it added from one it was given.
pub fn wrap_indexed(text: &str, cells: usize, max_lines: Option<usize>) -> Vec<Vec<(char, usize)>> {
    if cells == 0 {
        return vec![Vec::new()];
    }
    let mut lines = wrap_all(text, cells);
    if let Some(max) = max_lines.map(|m| m.max(1))
        && lines.len() > max
    {
        lines.truncate(max);
        // Close the last line with an ellipsis, making room for it if the line is full.
        let last = lines.last_mut().expect("max is at least one line");
        last.truncate(cells.saturating_sub(1));
        last.push(('…', text.chars().count()));
    }
    lines
}

/// Characters a long word may be broken after — path separators, and nothing else.
const PATH_SEPARATORS: [char; 2] = ['/', '\\'];

/// Every line the text needs, uncapped.
fn wrap_all(text: &str, cells: usize) -> Vec<Vec<(char, usize)>> {
    let chars: Vec<char> = text.chars().collect();
    // Word boundaries as index ranges over `chars` — `split_whitespace` would give the substrings
    // but not where they came from, which is the whole point here.
    let mut words: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        words.push((start, i));
    }

    let take = |range: std::ops::Range<usize>| -> Vec<(char, usize)> {
        range.map(|k| (chars[k], k)).collect()
    };
    let mut lines: Vec<Vec<(char, usize)>> = Vec::new();
    let mut line: Vec<(char, usize)> = Vec::new();
    for (start, end) in words {
        let word_len = end - start;
        let current = line.len();
        // Does it fit after what is already on this line (plus the joining space)?
        let needed = if current == 0 {
            word_len
        } else {
            current + 1 + word_len
        };
        if needed <= cells {
            if current > 0 {
                line.push((' ', start.saturating_sub(1)));
            }
            line.extend(take(start..end));
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        // The word alone still may not fit: hard-break it across as many lines as it needs.
        if word_len <= cells {
            line.extend(take(start..end));
            continue;
        }
        let mut k = start;
        while end - k > cells {
            // Break after the last path separator that lies inside this line; with none, at the
            // line's width. `j` is where the next piece starts, so the separator stays on this line.
            let cut = (k + 1..=k + cells)
                .rev()
                .find(|&j| PATH_SEPARATORS.contains(&chars[j - 1]))
                .unwrap_or(k + cells);
            lines.push(take(k..cut));
            k = cut;
        }
        line = take(k..end);
    }
    lines.push(line);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str, cells: usize, max: Option<usize>) -> Vec<String> {
        wrap_lines(text, cells, max)
    }

    #[test]
    fn a_long_word_breaks_after_a_slash_when_one_is_in_the_line() {
        assert_eq!(
            lines("/Users/antonio/projects", 12, None),
            ["/Users/", "antonio/", "projects"]
        );
    }

    #[test]
    fn a_windows_path_folds_after_a_backslash_the_same_way() {
        assert_eq!(
            lines("C:\\Users\\antonio\\src", 11, None),
            ["C:\\Users\\", "antonio\\src"]
        );
    }

    #[test]
    fn a_long_word_with_no_separator_is_still_hard_broken() {
        assert_eq!(lines("abcdefghij", 4, None), ["abcd", "efgh", "ij"]);
    }

    #[test]
    fn text_over_the_cap_ends_its_last_line_with_an_ellipsis() {
        let capped = lines("one two three four five six", 8, Some(2));
        assert_eq!(capped.len(), 2, "{capped:?}");
        assert!(capped[1].ends_with('…') && capped[1].chars().count() <= 8);
        // At or under the cap, nothing is added.
        assert_eq!(lines("one two", 8, Some(2)), ["one two"]);
        // A cap of zero is one line, never none.
        assert_eq!(lines("one two three", 5, Some(0)).len(), 1);
    }
}
