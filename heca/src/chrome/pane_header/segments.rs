//! The info bar's chips: which show, what they say, and how they give way to a narrow bar.

use super::*;

/// **What each chip has to say right now** — the one derivation, read three times.
///
/// The builder turns these into a tag, the header's identity reads only *which chips came back*
/// (that is its shape), and the per-frame update writes the texts into the tree that is already on
/// screen. Three readers, one answer, so a chip cannot be built from one thing and rewritten from
/// another. A chip with nothing to say for this pane is skipped (no empty pill).
///
/// Returns `(chip name, icon, text)`.
pub(crate) fn segment_items(
    chips: &super::pane_items::PaneChips,
    defs: &[&super::pane_items::PaneChipDef],
    facts: &super::pane_items::PaneFacts,
    max_width: f32,
    font: f32,
) -> Vec<(String, Glyph, String)> {
    use super::pane_items::Fit;

    // Collect the produced (icon, text) chips, noting the one that can shorten itself (a path), so
    // we can fit the bar to `max_width` before building it.
    let mut items: Vec<(String, Glyph, String)> = Vec::new();
    let mut shrinkable: Option<usize> = None;
    for def in defs {
        let Some(chip) = chips.chip(def, facts) else {
            continue;
        };
        if def.fit == Fit::PathLeft {
            shrinkable = Some(items.len());
        }
        items.push((def.name.clone(), chip.icon, chip.text));
    }

    // Fit to width: if the bar would overflow the pane, shorten the path chip (left-ellipsised) by
    // the overflow. The per-pane render clip is the hard backstop; this keeps it readable instead of
    // a hard cut.
    let char_w = (font * 0.6).max(1.0);
    let per_segment_overhead = font * 2.5; // icon + gaps + segment padding + divider
    let text_chars: usize = items.iter().map(|(_, _, text)| text.chars().count()).sum();
    let estimated = text_chars as f32 * char_w + items.len() as f32 * per_segment_overhead;
    if estimated > max_width
        && let Some(idx) = shrinkable
    {
        let overflow_chars = ((estimated - max_width) / char_w).ceil() as usize;
        let loc_chars = items[idx].2.chars().count();
        let keep = loc_chars.saturating_sub(overflow_chars).max(1);
        items[idx].2 = truncate_path_left(&items[idx].2, keep);
    }

    items
}

/// Build the pane info bar's segmented [`Tag`] from what the chips said (`segment_items`). `None`
/// when nothing was produced. Used by the terminal pane shell.
pub(crate) fn build_pane_info_bar(
    items: Vec<(String, Glyph, String)>,
    theme: &GuiTheme,
) -> Option<Tag> {
    if items.is_empty() {
        return None;
    }

    let mut tag: Option<Tag> = None;
    for (name, glyph, text) in items {
        // No explicit size → the icon inherits the bar's base font, so glyph and
        // label stay balanced when the bar font changes.
        let leading = Icon::new(glyph).color(theme.colors.foreground);
        // **Each chip is named by what it is**, so its words can be rewritten without the bar
        // being rebuilt (F003/P097/T500). The first chip is the tag's own label; the rest are
        // child labels. Both answer `set_text`.
        let key = segment_text_key(&name);
        tag = Some(match tag.take() {
            None => {
                use heca_grid_ui::builders::ComponentExt as _;
                Tag::new(text).leading(leading).key(key)
            }
            Some(existing) => {
                existing.segment_text_keyed(text, Some(Box::new(leading)), Some(&key))
            }
        });
    }
    tag
}

/// **What a chip's words are called**, so they can be rewritten in place.
///
/// One derivation, read by the builder and by the update pass, so the two cannot address different
/// nodes. Keyed by the chip's *name* rather than its position: which chips have data changes (a pane
/// outside a repository has no branch), and a positional name would then follow whichever chip
/// happened to take that slot.
pub(super) fn segment_text_key(name: &str) -> String {
    format!("hdr.seg:{name}")
}
