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

/// How many monospace cells fit in `width`.
///
/// The epsilon is not cosmetic: a box sized to an exact number of cells divides to `n - 0.0000001`
/// in f64 and floors one short, dropping a character that fits. Every text fit in this crate goes
/// through it, so the cut, the wrap and the measure all agree on where the box ends.
pub fn mono_cells(width: f64, cell: f64) -> usize {
    if cell <= 0.0 {
        return 0;
    }
    (width / cell + 1e-6).floor().max(0.0) as usize
}

/// Break `text` into the lines that fit `cells` monospace cells, on **word** boundaries.
///
/// The one implementation of wrapping: the layout measure calls it to learn how tall a label will
/// be, and the paint calls it to draw. Two spellings of this would put the glyphs on a different
/// number of lines than the box was sized for — text drawn outside its own bounds, with nothing to
/// catch it.
///
/// A word longer than the line is **hard-broken** rather than allowed to overflow: a URL or a path
/// with no spaces is common enough that refusing to break it means refusing to fit at all. Runs of
/// whitespace collapse at a break, as they do in every wrapper, and an empty text is one empty line
/// so a label never measures zero-height.
pub fn wrap_lines(text: &str, cells: usize) -> Vec<String> {
    wrap_indexed(text, cells)
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
/// matched a space still marks one.
pub fn wrap_indexed(text: &str, cells: usize) -> Vec<Vec<(char, usize)>> {
    if cells == 0 {
        return vec![Vec::new()];
    }
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
        let needed = if current == 0 { word_len } else { current + 1 + word_len };
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
            lines.push(take(k..k + cells));
            k += cells;
        }
        line = take(k..end);
    }
    lines.push(line);
    lines
}
